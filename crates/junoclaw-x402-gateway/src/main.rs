//! JunoClaw x402 gateway — HTTP/x402 façade in front of the JunoClaw
//! task-ledger / escrow / agent-registry / zk-verifier stack.
//!
//! See `docs/ADR-002-X402-COMPOSITION.md` and `docs/X402_RISK_ANALYSIS.md`
//! for the design and threat model. Operationally this is a stateless HTTP
//! service except for the in-memory nonce store; for multi-replica deployments
//! the store must move to a shared backend.

mod config;
mod cosmos;
mod error;
mod routes;
mod x402;

use std::sync::Arc;

use anyhow::Context;
use axum::Router;
use clap::Parser;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::config::{CliArgs, Config};
use crate::cosmos::CosmosClient;
use crate::routes::{build_router, AppState};
use crate::x402::NonceStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    // Load .env if present (developer convenience). In production, env vars
    // come from the orchestrator (Akash / k8s); the .env path is benign.
    let _ = dotenvy::dotenv();

    let args = CliArgs::parse();
    let cfg = Config::from_cli(args).context("build config")?;

    tracing::info!(
        chain_id = %cfg.chain_id,
        rpc = %cfg.rpc_url,
        bind = %cfg.bind,
        agent_company = %cfg.agent_company,
        rate_limit_rpm = cfg.rate_limit_rpm,
        max_task_ujuno = cfg.max_task_ujuno,
        envelope_ttl_sec = cfg.envelope_ttl_sec,
        "x402 gateway starting"
    );
    // Operator key path is NOT logged.

    let cosmos = Arc::new(
        CosmosClient::new(&cfg.rpc_url, &cfg.chain_id).context("cosmos client init")?,
    );

    let state = AppState {
        cosmos,
        nonces: NonceStore::new(),
        agent_company: cfg.agent_company.clone(),
        envelope_ttl_sec: cfg.envelope_ttl_sec,
        max_task_ujuno: cfg.max_task_ujuno,
    };

    let app: Router = build_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive()); // tighten for prod via config

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
        .unwrap_or_else(|_| EnvFilter::new("junoclaw_x402_gateway=info,tower_http=info"));
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
