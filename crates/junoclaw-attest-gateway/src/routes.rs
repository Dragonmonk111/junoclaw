//! HTTP routes for the /attest API.

use std::{collections::HashMap, sync::Arc, time::{SystemTime, UNIX_EPOCH}};

use axum::{
    extract::{Path, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{
    canon, config::Config,
    error::{GatewayError, GatewayResult},
    store::Store,
    types::{
        AttStatus, AttestRequest, AttestResponse, Attestation, BatchRequest, UsageResponse,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub cfg: Arc<Config>,
    /// sha256(api_key) hex -> customer
    pub keys: Arc<HashMap<String, String>>,
    pub rate: Arc<crate::rate::RateLimiter>,
    pub window_secs: u64,
    pub max_payload_bytes: usize,
}

#[derive(Clone)]
pub struct Customer(pub String);

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/attest", post(post_attest))
        .route("/attest/batch", post(post_batch))
        .route("/attest/{id}", get(get_attestation))
        .route("/attest/{id}/receipt", get(get_receipt))
        .route("/attest/{id}/payload", delete(delete_payload))
        .route("/usage", get(get_usage))
        .route("/healthz", get(healthz))
        .route("/metrics", get(metrics))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state)
}

async fn healthz() -> Response {
    Json(json!({ "status": "ok", "service": "junoclaw-attest-gateway" })).into_response()
}

async fn metrics(State(state): State<AppState>) -> Response {
    let (total, queued, anchoring) = state.store.counts();
    Json(json!({
        "attestations_total": total,
        "queued": queued,
        "anchoring_in_flight": anchoring,
    }))
    .into_response()
}

// ── auth ─────────────────────────────────────────────────────────────

async fn auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, GatewayError> {
    // healthz/metrics are open
    if req.uri().path() == "/healthz" || req.uri().path() == "/metrics" {
        return Ok(next.run(req).await);
    }
    if state.keys.is_empty() {
        if !state.rate.allow("dev") {
            return Err(GatewayError::RateLimited);
        }
        req.extensions_mut().insert(Customer("dev".to_string()));
        return Ok(next.run(req).await);
    }
    let header_val = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let key_hash = header_val.map(|k| hex::encode(Sha256::digest(k.as_bytes())));
    match key_hash.and_then(|h| state.keys.get(&h)) {
        Some(customer) => {
            if !state.rate.allow(customer) {
                return Err(GatewayError::RateLimited);
            }
            req.extensions_mut().insert(Customer(customer.clone()));
            Ok(next.run(req).await)
        }
        None => Err(GatewayError::Unauthorized),
    }
}

// ── handlers ─────────────────────────────────────────────────────────

async fn post_attest(
    State(state): State<AppState>,
    axum::Extension(Customer(customer)): axum::Extension<Customer>,
    Json(req): Json<AttestRequest>,
) -> GatewayResult<Response> {
    let (resp, status) = submit_one(&state, &customer, req)?;
    Ok((status, Json(resp)).into_response())
}

async fn post_batch(
    State(state): State<AppState>,
    axum::Extension(Customer(customer)): axum::Extension<Customer>,
    Json(req): Json<BatchRequest>,
) -> GatewayResult<Response> {
    if req.items.len() > 256 {
        return Err(GatewayError::BadRequest("batch max 256 items".into()));
    }
    let mut out = Vec::with_capacity(req.items.len());
    for item in req.items {
        let (resp, _) = submit_one(&state, &customer, item)?;
        out.push(resp);
    }
    Ok(Json(json!({ "results": out })).into_response())
}

fn submit_one(
    state: &AppState,
    customer: &str,
    req: AttestRequest,
) -> GatewayResult<(AttestResponse, StatusCode)> {
    if req.idempotency_key.is_empty() || req.idempotency_key.len() > 255 {
        return Err(GatewayError::BadRequest(
            "idempotency_key required (1..255 chars)".into(),
        ));
    }
    let (_, queued, _) = state.store.counts();
    if queued >= state.cfg.max_queue {
        return Err(GatewayError::QueueFull);
    }
    let ctx = req.context.clone().unwrap_or(crate::types::Context {
        agent_id: None,
        session_id: None,
        occurred_at: None,
        policy_ref: None,
        jlens: None,
    });

    // Leaf material: raw payload (size-capped, discarded) or asserted hash.
    let _payload_bytes = match (&req.action.payload, &req.action.payload_hash) {
        (Some(p), _) => {
            let canon = canon::canonicalize(p);
            if canon.len() > state.max_payload_bytes {
                return Err(GatewayError::PayloadTooLarge {
                    max: state.max_payload_bytes,
                });
            }
            canon
        }
        (None, Some(h)) => {
            let h = h.strip_prefix("sha256:").unwrap_or(h);
            if hex::decode(h).map(|b| b.len()) != Ok(32) {
                return Err(GatewayError::BadRequest(
                    "payload_hash must be 32-byte hex (optionally sha256:-prefixed)".into(),
                ));
            }
            h.as_bytes().to_vec()
        }
        (None, None) => {
            return Err(GatewayError::BadRequest(
                "action.payload or action.payload_hash required".into(),
            ))
        }
    };

    // Leaf = sha256 of the canonical leaf document.
    let leaf_doc = json!({
        "kind": req.action.kind,
        "agent_id": ctx.agent_id.clone().unwrap_or_default(),
        "occurred_at": ctx.occurred_at,
        "payload": req.action.payload.clone()
            .unwrap_or_else(|| json!({"hash": req.action.payload_hash})),
        "v": 1,
    });
    let leaf_hash: [u8; 32] = Sha256::digest(canon::canonicalize(&leaf_doc)).into();

    // Idempotency: body fingerprint excludes the key itself.
    let body_doc = json!({
        "action": {"kind": req.action.kind, "payload": req.action.payload,
                   "payload_hash": req.action.payload_hash},
        "context": {"agent_id": ctx.agent_id, "occurred_at": ctx.occurred_at},
        "tier": req.tier,
    });
    let body_hash = hex::encode(Sha256::digest(canon::canonicalize(&body_doc)));

    if let Some((existing_id, stored_hash)) =
        state.store.idem_lookup(customer, &req.idempotency_key)
    {
        if stored_hash == body_hash {
            return Ok((
                AttestResponse {
                    attestation_id: existing_id,
                    status: "queued",
                    est_anchor_secs: state.window_secs,
                    deduplicated: true,
                },
                StatusCode::OK,
            ));
        }
        return Err(GatewayError::IdempotencyConflict {
            key: req.idempotency_key,
        });
    }

    let att = Attestation {
        attestation_id: new_id(),
        customer: customer.to_string(),
        idempotency_key: req.idempotency_key,
        body_hash,
        leaf_hash: hex::encode(leaf_hash),
        kind: req.action.kind,
        agent_id: ctx.agent_id,
        occurred_at: ctx.occurred_at,
        jlens: ctx.jlens,
        tier: req.tier.unwrap_or_else(|| "anchor".into()),
        callback_url: req.callback_url,
        status: AttStatus::Queued,
        submitted_at: now_secs(),
        batch: None,
        retained_payload: None, // never retain by default
    };
    let id = att.attestation_id.clone();
    state
        .store
        .submit(att)
        .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok((
        AttestResponse {
            attestation_id: id,
            status: "queued",
            est_anchor_secs: state.window_secs,
            deduplicated: false,
        },
        StatusCode::ACCEPTED,
    ))
}

async fn get_attestation(
    State(state): State<AppState>,
    axum::Extension(Customer(customer)): axum::Extension<Customer>,
    Path(id): Path<String>,
) -> GatewayResult<Response> {
    let att = lookup(&state, &customer, &id)?;
    Ok(Json(json!({
        "attestation_id": att.attestation_id,
        "status": status_str(att.status),
        "leaf_hash": att.leaf_hash,
        "kind": att.kind,
        "agent_id": att.agent_id,
        "occurred_at": att.occurred_at,
        "submitted_at": att.submitted_at,
        "batch": att.batch,
    }))
    .into_response())
}

async fn get_receipt(
    State(state): State<AppState>,
    axum::Extension(Customer(customer)): axum::Extension<Customer>,
    Path(id): Path<String>,
) -> GatewayResult<Response> {
    let att = lookup(&state, &customer, &id)?;
    Ok(Json(json!({
        "attestation_id": att.attestation_id,
        "status": status_str(att.status),
        "leaf_hash": att.leaf_hash,
        "batch": att.batch,
        "certificate": att.batch.as_ref().map(|b| json!({
            "height": b.anchored_at_height,
            "scheme": "hybrid-bls-mayo2",
            "note": "verify via node gRPC cert at height or merkle-verifier get_root",
        })),
        "evidence": {"jlens": att.jlens, "tier": att.tier},
    }))
    .into_response())
}

async fn delete_payload(
    State(state): State<AppState>,
    axum::Extension(Customer(customer)): axum::Extension<Customer>,
    Path(id): Path<String>,
) -> GatewayResult<Response> {
    let att = lookup(&state, &customer, &id)?;
    state
        .store
        .purge_payload(&att.attestation_id)
        .map_err(GatewayError::Internal)?;
    Ok(Json(json!({ "purged": true })).into_response())
}

async fn get_usage(
    State(state): State<AppState>,
    axum::Extension(Customer(customer)): axum::Extension<Customer>,
) -> GatewayResult<Response> {
    let (c, queued) = state.store.usage(&customer);
    Ok(Json(UsageResponse {
        customer,
        submitted: c.submitted,
        anchored: c.anchored,
        failed: c.failed,
        queued,
    })
    .into_response())
}

fn lookup(state: &AppState, customer: &str, id: &str) -> GatewayResult<Attestation> {
    let att = state
        .store
        .get(id)
        .ok_or_else(|| GatewayError::NotFound(id.to_string()))?;
    if att.customer != customer && customer != "dev" {
        // Customers only see their own attestations.
        return Err(GatewayError::NotFound(id.to_string()));
    }
    Ok(att)
}

fn status_str(s: AttStatus) -> &'static str {
    match s {
        AttStatus::Queued => "queued",
        AttStatus::Anchoring => "anchoring",
        AttStatus::Anchored => "anchored",
        AttStatus::AnchorFailed => "anchor_failed",
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn new_id() -> String {
    use rand::Rng;
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let mut rng = rand::thread_rng();
    format!("att_{:x}{:016x}", ms, rng.gen::<u64>())
}
