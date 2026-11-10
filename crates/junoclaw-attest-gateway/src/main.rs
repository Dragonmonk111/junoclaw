//! junoclaw-attest-gateway — HTTP /attest front door for JunoClaw g1.

mod batcher;
mod canon;
mod config;
mod error;
mod merkle;
mod routes;
mod store;
mod types;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use axum::Router;
use clap::Parser;
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::{
    config::{CliArgs, Config},
    routes::{build_router, AppState},
    store::Store,
};
use junoclaw_chain_grpc::{ClientConfig, GrpcClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let _ = dotenvy::dotenv();

    let args = CliArgs::parse();
    let cfg = Config::from_cli(args).context("build config")?;
    let cfg = Arc::new(cfg);

    tracing::info!(
        chain_id = %cfg.chain_id,
        grpc = %cfg.grpc_endpoint,
        bind = %cfg.bind,
        merkle_contract = %cfg.merkle_contract,
        window_secs = cfg.window_secs,
        customers = cfg.api_keys.len(),
        "attest gateway starting"
    );

    let store = Arc::new(Store::open(&cfg.spool_path).context("open spool")?);
    let requeued = store.requeue_anchoring();
    if requeued > 0 {
        tracing::warn!(requeued, "re-queued anchoring items after restart");
    }

    let chain = GrpcClient::connect(&ClientConfig {
        grpc_endpoint: cfg.grpc_endpoint.clone(),
        chain_id: cfg.chain_id.clone(),
        gas_price: cfg.gas_price.clone(),
        gas_limit: cfg.gas_limit,
        mnemonic_env: Some(cfg.mnemonic_env.clone()),
        account_prefix: cfg.account_prefix.clone(),
        hd_path: cfg.hd_path.clone(),
    })
    .await
    .context("grpc connect")?;
    if let Some(addr) = chain.signer_address() {
        tracing::info!(signer = %addr, "anchor signer loaded");
    }
    let chain = Arc::new(Mutex::new(chain));

    // Anchor loop
    tokio::spawn(batcher::run(
        store.clone(),
        chain.clone(),
        (*cfg).clone(),
    ));

    let keys: HashMap<String, String> = cfg
        .api_keys
        .iter()
        .map(|(cust, key)| (key.clone(), cust.clone()))
        .collect();
    if keys.is_empty() {
        tracing::warn!("no api keys configured — dev mode, all requests accepted as 'dev'");
    }

    let state = AppState {
        store,
        cfg: cfg.clone(),
        keys: Arc::new(keys),
        window_secs: cfg.window_secs,
        max_payload_bytes: cfg.max_payload_bytes,
    };

    let app: Router = build_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind(cfg.bind)
        .await
        .with_context(|| format!("bind {}", cfg.bind))?;
    tracing::info!(addr = %cfg.bind, "ready");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum serve")?;
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("junoclaw_attest_gateway=info,tower_http=info"));
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).compact())
        .with(filter)
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
