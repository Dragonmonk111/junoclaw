//! junoclaw-rest-gateway — read-only Cosmos LCD-style REST shim.
//!
//! Maps a bounded subset of standard Cosmos REST (LCD) paths onto the
//! node's SDK gRPC surface (:9090). Read-only: no broadcast endpoint —
//! a public unauthenticated tx submit is a spam vector; add it behind
//! auth if ever needed.
//!
//! Covered paths return spec-shaped JSON (u64s as strings, bytes as
//! base64). JunoClaw-specific block internals (proposal/cert/payload)
//! ride under a `layer` key so standard clients can ignore them.
//! Anything outside the subset 404s — we claim a subset, not "Cosmos
//! REST support".

use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use clap::Parser;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use junoclaw_chain_grpc::{ClientConfig, GrpcClient};

#[derive(Parser)]
#[command(name = "junoclaw-rest-gateway", version, about)]
struct CliArgs {
    #[arg(long, env = "REST_BIND", default_value = "0.0.0.0:1317")]
    bind: SocketAddr,
    #[arg(long, env = "REST_CHAIN_ID", default_value = "junoclaw-g1")]
    chain_id: String,
    #[arg(long, env = "REST_GRPC", default_value = "51.195.201.74:9090")]
    grpc_endpoint: String,
}

#[derive(Clone)]
struct AppState {
    chain: Arc<Mutex<GrpcClient>>,
    chain_id: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("junoclaw_rest_gateway=info,tower_http=info"));
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).compact())
        .with(filter)
        .init();
    let _ = dotenvy::dotenv();

    let args = CliArgs::parse();
    let chain = GrpcClient::connect(&ClientConfig {
        grpc_endpoint: args.grpc_endpoint.clone(),
        chain_id: args.chain_id.clone(),
        gas_price: "0.001ujclaw".into(),
        gas_limit: 0,
        mnemonic_env: None, // read-only — never loads a key
        account_prefix: "junoclaw".into(),
        hd_path: "m/44'/118'/0'/0/0".into(),
    })
    .await
    .context("grpc connect")?;

    let state = AppState {
        chain: Arc::new(Mutex::new(chain)),
        chain_id: args.chain_id.clone(),
    };

    let app = Router::new()
        .route("/healthz", get(|| async { Json(json!({"status":"ok","service":"junoclaw-rest-gateway"})) }))
        // node status
        .route("/cosmos/base/node/v1beta1/status", get(node_status))
        // blocks (tendermint-compat shape + `layer` extras)
        .route(
            "/cosmos/base/tendermint/v1beta1/blocks/latest",
            get(block_latest),
        )
        .route(
            "/cosmos/base/tendermint/v1beta1/blocks/{height}",
            get(block_by_height),
        )
        // auth
        .route(
            "/cosmos/auth/v1beta1/accounts/{address}",
            get(get_account),
        )
        // bank
        .route(
            "/cosmos/bank/v1beta1/balances/{address}",
            get(get_balances),
        )
        .route(
            "/cosmos/bank/v1beta1/balances/{address}/by_denom",
            get(get_balance_by_denom),
        )
        // tx
        .route("/cosmos/tx/v1beta1/txs/{hash}", get(get_tx))
        // cosmwasm smart query
        .route(
            "/cosmwasm/wasm/v1/contract/{address}/smart/{query}",
            get(wasm_smart),
        )
        // raw layer surface (own namespace — the node's native API)
        .route("/layer/lightclient/v1/latest_height", get(layer_latest))
        .route("/layer/lightclient/v1/blocks/{height}", get(layer_block))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    tracing::info!(bind = %args.bind, grpc = %args.grpc_endpoint, chain_id = %args.chain_id, "rest gateway ready");
    let listener = tokio::net::TcpListener::bind(args.bind)
        .await
        .with_context(|| format!("bind {}", args.bind))?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────

fn err(status: StatusCode, msg: impl std::fmt::Display) -> Response {
    (
        status,
        Json(json!({"code": status.as_u16(), "message": msg.to_string()})),
    )
        .into_response()
}

fn chain_err(e: anyhow::Error) -> Response {
    let msg = e.to_string();
    if msg.contains("not found") || msg.contains("NotFound") {
        err(StatusCode::NOT_FOUND, msg)
    } else {
        err(StatusCode::BAD_GATEWAY, msg)
    }
}

fn nanos_to_rfc3339(nanos: u64) -> String {
    if nanos == 0 {
        return "1970-01-01T00:00:00Z".to_string();
    }
    let secs = (nanos / 1_000_000_000) as i64;
    let ns = (nanos % 1_000_000_000) as u32;
    match time::OffsetDateTime::from_unix_timestamp(secs) {
        Ok(t) => t
            .replace_nanosecond(ns)
            .map(|t| {
                t.format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default()
            })
            .unwrap_or_else(|_| secs.to_string()),
        Err(_) => secs.to_string(),
    }
}

fn block_json(state: &AppState, b: junoclaw_chain_grpc::LayerBlock) -> Value {
    json!({
        "block_id": {
            "hash": B64.encode(sha2_256(&b.payload_bytes)),
            "part_set_header": {"total": 1, "hash": ""},
        },
        "block": {
            "header": {
                "version": {"block": "11"},
                "chain_id": state.chain_id,
                "height": b.height.to_string(),
                "time": nanos_to_rfc3339(b.timestamp_nanos),
            },
            "data": {"txs": null},
            "evidence": {"evidence": null},
            "last_commit": {"signatures": []},
        },
        // JunoClaw-native payload — the PQ-hybrid cert + consensus proposal
        // a verifier actually needs. Standard clients can ignore this.
        "layer": {
            "scheme": "hybrid-bls-mayo2",
            "proposal_bytes": B64.encode(&b.proposal_bytes),
            "certificate_bytes": B64.encode(&b.certificate_bytes),
            "payload_bytes": B64.encode(&b.payload_bytes),
        },
    })
}

fn sha2_256(data: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).to_vec()
}

// ── handlers ─────────────────────────────────────────────────────────

async fn node_status(State(s): State<AppState>) -> Response {
    let mut c = s.chain.lock().await;
    match c.latest().await {
        Ok(l) => Json(json!({
            "node_info": {
                "default_node_info": {},
                "application_version": {},
            },
            "sync_info": {
                "latest_block_hash": "",
                "latest_app_hash": "",
                "latest_block_height": l.height.to_string(),
                "latest_block_time": nanos_to_rfc3339(l.timestamp_nanos),
                "catching_up": false,
            },
            "validator_info": {"address": "", "voting_power": "0"},
        }))
        .into_response(),
        Err(e) => chain_err(e),
    }
}

async fn block_latest(State(s): State<AppState>) -> Response {
    let mut c = s.chain.lock().await;
    match c.latest().await {
        Ok(l) => match c.block(l.height).await {
            Ok(b) => Json(block_json(&s, b)).into_response(),
            Err(e) => chain_err(e),
        },
        Err(e) => chain_err(e),
    }
}

async fn block_by_height(
    State(s): State<AppState>,
    Path(height): Path<u64>,
) -> Response {
    let mut c = s.chain.lock().await;
    match c.block(height).await {
        Ok(b) => Json(block_json(&s, b)).into_response(),
        Err(e) => chain_err(e),
    }
}

async fn get_account(
    State(s): State<AppState>,
    Path(address): Path<String>,
) -> Response {
    let mut c = s.chain.lock().await;
    match c.account(&address).await {
        Ok(Some((num, seq))) => Json(json!({
            "account": {
                "@type": "/cosmos.auth.v1beta1.BaseAccount",
                "address": address,
                "pub_key": null,
                "account_number": num.to_string(),
                "sequence": seq.to_string(),
            }
        }))
        .into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, format!("account {address} not found")),
        Err(e) => chain_err(e),
    }
}

async fn get_balances(
    State(s): State<AppState>,
    Path(address): Path<String>,
) -> Response {
    let mut c = s.chain.lock().await;
    match c.all_balances(&address).await {
        Ok(coins) => Json(json!({
            "balances": coins.into_iter()
                .map(|(denom, amount)| json!({"denom": denom, "amount": amount}))
                .collect::<Vec<_>>(),
            "pagination": {"next_key": null, "total": "0"},
        }))
        .into_response(),
        Err(e) => chain_err(e),
    }
}

#[derive(serde::Deserialize)]
struct DenomQ {
    denom: String,
}

async fn get_balance_by_denom(
    State(s): State<AppState>,
    Path(address): Path<String>,
    Query(q): Query<DenomQ>,
) -> Response {
    let mut c = s.chain.lock().await;
    match c.balance(&address, &q.denom).await {
        Ok(amount) => Json(json!({
            "balance": {"denom": q.denom, "amount": amount}
        }))
        .into_response(),
        Err(e) => chain_err(e),
    }
}

async fn get_tx(State(s): State<AppState>, Path(hash): Path<String>) -> Response {
    let mut c = s.chain.lock().await;
    match c.get_tx(&hash).await {
        Ok(resp) => {
            let tr = resp.tx_response.map(|t| {
                json!({
                    "height": t.height.to_string(),
                    "txhash": t.txhash,
                    "codespace": t.codespace,
                    "code": t.code,
                    "data": t.data,
                    "raw_log": t.raw_log,
                    "gas_wanted": t.gas_wanted.to_string(),
                    "gas_used": t.gas_used.to_string(),
                    "timestamp": t.timestamp,
                })
            });
            Json(json!({ "tx_response": tr })).into_response()
        }
        Err(e) => chain_err(e),
    }
}

async fn wasm_smart(
    State(s): State<AppState>,
    Path((address, query_b64)): Path<(String, String)>,
) -> Response {
    let query_raw = match B64.decode(query_b64.trim_end_matches('/')) {
        Ok(b) => b,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("bad base64 query: {e}")),
    };
    let query: Value = match serde_json::from_slice(&query_raw) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, format!("query not json: {e}")),
    };
    let mut c = s.chain.lock().await;
    match c.query_smart(&address, &query).await {
        Ok(v) => Json(json!({ "data": v })).into_response(),
        Err(e) => chain_err(e),
    }
}

// ── layer (native) surface ──────────────────────────────────────────

async fn layer_latest(State(s): State<AppState>) -> Response {
    let mut c = s.chain.lock().await;
    match c.latest().await {
        Ok(l) => Json(json!({
            "height": l.height.to_string(),
            "timestamp_nanos": l.timestamp_nanos.to_string(),
        }))
        .into_response(),
        Err(e) => chain_err(e),
    }
}

async fn layer_block(State(s): State<AppState>, Path(h): Path<u64>) -> Response {
    let mut c = s.chain.lock().await;
    match c.block(h).await {
        Ok(b) => Json(json!({
            "height": b.height.to_string(),
            "timestamp_nanos": b.timestamp_nanos.to_string(),
            "proposal_bytes": B64.encode(&b.proposal_bytes),
            "certificate_bytes": B64.encode(&b.certificate_bytes),
            "payload_bytes": B64.encode(&b.payload_bytes),
        }))
        .into_response(),
        Err(e) => chain_err(e),
    }
}
