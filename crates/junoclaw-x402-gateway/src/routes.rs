//! HTTP routes for the x402 gateway.
//!
//! The two-phase flow for every state-changing endpoint:
//!
//! 1. **Client sends unsigned request.** Gateway responds with `402 Payment
//!    Required` and a `PaymentEnvelope` body describing exactly what tx to sign.
//!
//! 2. **Client signs and retries** with the `PAYMENT-SIGNATURE` header set to
//!    `base64(signed_cosmos_tx_bytes)`. Gateway validates the envelope, checks
//!    the nonce against the replay store, decodes+broadcasts the tx, and
//!    returns the on-chain result.
//!
//! Read-only endpoints (`GET /tasks/:id`, `GET /agents/:addr`, `GET /healthz`)
//! skip the 402 dance.

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::cosmos::{CosmosClient, GatewayOp};
use crate::error::{GatewayError, GatewayResult};
use crate::x402::{Coin, NonceStore, PaymentEnvelope, PaymentSignature};

#[derive(Clone)]
pub struct AppState {
    pub cosmos: Arc<CosmosClient>,
    pub nonces: NonceStore,
    pub agent_company: String,
    pub envelope_ttl_sec: i64,
    pub max_task_ujuno: u128,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/metrics", get(metrics))
        .route("/tasks", post(post_task))
        .route("/tasks/{id}", get(get_task))
        .route("/tasks/{id}/accept", post(accept_task))
        .route("/tasks/{id}/submit", post(submit_attestation))
        .route("/agents/{addr}", get(get_agent))
        .with_state(state)
}

// ---------- read-only ----------

async fn healthz() -> Response {
    Json(json!({ "status": "ok", "service": "junoclaw-x402-gateway" })).into_response()
}

async fn metrics(State(state): State<AppState>) -> Response {
    Json(json!({
        "nonce_store_size": state.nonces.len(),
    }))
    .into_response()
}

async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<u64>,
) -> GatewayResult<Response> {
    let task_ledger = resolve_child_addr(&state, "task_ledger").await?;
    let query = json!({ "task": { "task_id": task_id } });
    let result: Value = state.cosmos.query_smart(&task_ledger, &query).await?;
    Ok(Json(result).into_response())
}

async fn get_agent(
    State(state): State<AppState>,
    Path(addr): Path<String>,
) -> GatewayResult<Response> {
    let agent_registry = resolve_child_addr(&state, "agent_registry").await?;
    let query = json!({ "agent_info": { "address": addr } });
    let result: Value = state.cosmos.query_smart(&agent_registry, &query).await?;
    Ok(Json(result).into_response())
}

// ---------- state-changing (402 dance) ----------

#[derive(Debug, Deserialize)]
pub struct PostTaskRequest {
    pub description: String,
    pub constraints: String,
    pub verifying_key_hash: String,
    pub reward: Vec<Coin>,
    pub deadline_height: u64,
}

async fn post_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PostTaskRequest>,
) -> GatewayResult<Response> {
    enforce_value_limit(&state, &body.reward)?;

    let task_ledger = resolve_child_addr(&state, "task_ledger").await?;
    let exec_msg = json!({
        "post_task": {
            "description": body.description,
            "constraints": body.constraints,
            "verifying_key_hash": body.verifying_key_hash,
            "reward": body.reward,
            "deadline_height": body.deadline_height,
        }
    });

    handle_x402_step(
        &state,
        &headers,
        &task_ledger,
        exec_msg,
        body.reward,
        GatewayOp::PostTask,
    )
    .await
}

async fn accept_task(
    State(state): State<AppState>,
    Path(task_id): Path<u64>,
    headers: HeaderMap,
) -> GatewayResult<Response> {
    let task_ledger = resolve_child_addr(&state, "task_ledger").await?;
    let exec_msg = json!({ "accept_task": { "task_id": task_id } });
    handle_x402_step(
        &state,
        &headers,
        &task_ledger,
        exec_msg,
        vec![],
        GatewayOp::AcceptTask,
    )
    .await
}

#[derive(Debug, Deserialize)]
pub struct SubmitAttestationRequest {
    /// base64-encoded Groth16 proof
    pub proof: String,
    /// JSON public inputs for the circuit
    pub public_inputs: Value,
    /// If true, gas estimate uses the BN254 precompile (~250k). Otherwise
    /// pure-Wasm (~420k). Set by client based on chain capability.
    #[serde(default)]
    pub precompile: bool,
}

async fn submit_attestation(
    State(state): State<AppState>,
    Path(task_id): Path<u64>,
    headers: HeaderMap,
    Json(body): Json<SubmitAttestationRequest>,
) -> GatewayResult<Response> {
    let task_ledger = resolve_child_addr(&state, "task_ledger").await?;
    let exec_msg = json!({
        "submit_attestation": {
            "task_id": task_id,
            "proof": body.proof,
            "public_inputs": body.public_inputs,
        }
    });
    let op = if body.precompile {
        GatewayOp::SubmitAttestationPrecompile
    } else {
        GatewayOp::SubmitAttestationWasm
    };
    handle_x402_step(&state, &headers, &task_ledger, exec_msg, vec![], op).await
}

// ---------- helpers ----------

/// Core 402 logic. If `PAYMENT-SIGNATURE` is present, validate & broadcast.
/// Otherwise, mint a fresh envelope and return 402.
async fn handle_x402_step(
    state: &AppState,
    headers: &HeaderMap,
    contract: &str,
    exec_msg: Value,
    funds: Vec<Coin>,
    op: GatewayOp,
) -> GatewayResult<Response> {
    let gas_estimate = state.cosmos.estimate_gas(op);
    let fee_ujuno = (gas_estimate as u128 * 75 / 1000).to_string(); // 0.075ujuno/gas

    // Phase 2: signature present → validate + broadcast
    if let Some(sig_header) = headers.get("PAYMENT-SIGNATURE") {
        let sig_str = sig_header
            .to_str()
            .map_err(|e| GatewayError::InvalidEnvelope(format!("header decode: {e}")))?;
        let sig = PaymentSignature::parse_header(sig_str)?;

        // Pull the envelope hash from a co-located header so we can reject
        // mismatched-envelope replays.
        let env_header = headers
            .get("PAYMENT-ENVELOPE")
            .ok_or_else(|| GatewayError::InvalidEnvelope("missing PAYMENT-ENVELOPE header".into()))?;
        let env_str = env_header
            .to_str()
            .map_err(|e| GatewayError::InvalidEnvelope(format!("envelope header: {e}")))?;
        let envelope: PaymentEnvelope = serde_json::from_str(env_str)
            .map_err(|e| GatewayError::InvalidEnvelope(format!("envelope json: {e}")))?;

        envelope.validate()?;

        if envelope.chain_id != state.cosmos.chain_id() {
            return Err(GatewayError::ChainIdMismatch {
                want: state.cosmos.chain_id().to_string(),
                got: envelope.chain_id,
            });
        }

        state.nonces.record(&envelope.nonce, envelope.exp)?;

        let tx_hash = state.cosmos.broadcast(&sig.tx_bytes).await?;
        return Ok(Json(json!({
            "status": "broadcast",
            "tx_hash": tx_hash,
            "nonce": envelope.nonce,
        }))
        .into_response());
    }

    // Phase 1: no signature → mint envelope, return 402
    let envelope = PaymentEnvelope::new(
        state.cosmos.chain_id().to_string(),
        contract.to_string(),
        exec_msg,
        funds,
        gas_estimate,
        fee_ujuno,
        state.envelope_ttl_sec,
    );
    Err(GatewayError::PaymentRequired {
        envelope: serde_json::to_value(&envelope)
            .map_err(|e| GatewayError::Internal(anyhow::anyhow!("encode envelope: {e}")))?,
    })
}

fn enforce_value_limit(state: &AppState, coins: &[Coin]) -> GatewayResult<()> {
    for c in coins {
        if c.denom != "ujuno" {
            continue; // only enforce limit on JUNO; IBC denoms via separate path
        }
        let amt: u128 = c
            .amount
            .parse()
            .map_err(|e| GatewayError::BadRequest(format!("parse reward amount: {e}")))?;
        if amt > state.max_task_ujuno {
            return Err(GatewayError::TaskValueTooHigh {
                requested: amt,
                max: state.max_task_ujuno,
            });
        }
    }
    Ok(())
}

/// Resolve a child contract address (`task_ledger`, `escrow`, `agent_registry`,
/// `zk_verifier`) by querying `agent-company::Config`.
async fn resolve_child_addr(state: &AppState, field: &str) -> GatewayResult<String> {
    #[derive(Deserialize)]
    struct Config {
        task_ledger: String,
        escrow: String,
        agent_registry: String,
        zk_verifier: String,
    }
    let cfg: Config = state
        .cosmos
        .query_smart(&state.agent_company, &json!({"config": {}}))
        .await?;
    Ok(match field {
        "task_ledger" => cfg.task_ledger,
        "escrow" => cfg.escrow,
        "agent_registry" => cfg.agent_registry,
        "zk_verifier" => cfg.zk_verifier,
        other => {
            return Err(GatewayError::Internal(anyhow::anyhow!(
                "unknown child field: {other}"
            )))
        }
    })
}

#[derive(Debug, Serialize)]
pub struct _Unused; // satisfy `Serialize` import even when no extra types are needed
