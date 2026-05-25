//! Error types for the JunoClaw x402 gateway.
//!
//! All public APIs return `GatewayResult<T>` = `Result<T, GatewayError>`.
//! `GatewayError` implements `axum::response::IntoResponse` so handlers can `?` errors directly.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

pub type GatewayResult<T> = Result<T, GatewayError>;

#[derive(Debug, Error)]
pub enum GatewayError {
    // --- Client-side (4xx) ---
    #[error("payment required for resource")]
    PaymentRequired { envelope: serde_json::Value },

    #[error("invalid payment envelope: {0}")]
    InvalidEnvelope(String),

    #[error("payment signature does not match envelope")]
    SignatureMismatch,

    #[error("payment envelope has expired (exp={exp}, now={now})")]
    EnvelopeExpired { exp: i64, now: i64 },

    #[error("nonce has already been used")]
    NonceReplayed,

    #[error("rate limit exceeded for this caller")]
    RateLimited,

    #[error("task value {requested} exceeds gateway limit {max} ujuno")]
    TaskValueTooHigh { requested: u128, max: u128 },

    #[error("requested chain {got} does not match gateway chain {want}")]
    ChainIdMismatch { want: String, got: String },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("malformed request: {0}")]
    BadRequest(String),

    // --- Server-side (5xx) ---
    #[error("chain RPC error: {0}")]
    ChainRpc(String),

    #[error("chain simulation failed: {0}")]
    Simulation(String),

    #[error("broadcast failed: {0}")]
    Broadcast(String),

    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    detail: String,
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        // PaymentRequired is a special case — it returns 402 with the envelope as the body.
        if let GatewayError::PaymentRequired { envelope } = &self {
            return (StatusCode::PAYMENT_REQUIRED, Json(envelope.clone())).into_response();
        }

        let (status, label) = match &self {
            GatewayError::InvalidEnvelope(_)
            | GatewayError::SignatureMismatch
            | GatewayError::EnvelopeExpired { .. }
            | GatewayError::NonceReplayed
            | GatewayError::ChainIdMismatch { .. }
            | GatewayError::TaskValueTooHigh { .. }
            | GatewayError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),

            GatewayError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            GatewayError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),

            GatewayError::ChainRpc(_) => (StatusCode::BAD_GATEWAY, "chain_rpc_error"),
            GatewayError::Simulation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "simulation_failed"),
            GatewayError::Broadcast(_) => (StatusCode::BAD_GATEWAY, "broadcast_failed"),
            GatewayError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),

            // Already handled above
            GatewayError::PaymentRequired { .. } => unreachable!(),
        };

        let body = ErrorBody {
            error: label.to_string(),
            detail: self.to_string(),
        };

        tracing::warn!(error = %self, status = status.as_u16(), "request failed");
        (status, Json(body)).into_response()
    }
}
