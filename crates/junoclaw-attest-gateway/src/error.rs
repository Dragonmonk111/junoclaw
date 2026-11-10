//! Error types for the attest gateway.

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
    #[error("unauthorized")]
    Unauthorized,

    #[error("malformed request: {0}")]
    BadRequest(String),

    #[error("payload exceeds {max} bytes")]
    PayloadTooLarge { max: usize },

    #[error("idempotency key {key} already used with a different body")]
    IdempotencyConflict { key: String },

    #[error("attestation not found: {0}")]
    NotFound(String),

    #[error("rate limit exceeded")]
    RateLimited,

    #[error("queue is full")]
    QueueFull,

    #[error("chain error: {0}")]
    Chain(String),

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
        let (status, label) = match &self {
            GatewayError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            GatewayError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            GatewayError::PayloadTooLarge { .. } => {
                (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large")
            }
            GatewayError::IdempotencyConflict { .. } => {
                (StatusCode::CONFLICT, "idempotency_conflict")
            }
            GatewayError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            GatewayError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
            GatewayError::QueueFull => (StatusCode::SERVICE_UNAVAILABLE, "queue_full"),
            GatewayError::Chain(_) => (StatusCode::BAD_GATEWAY, "chain_error"),
            GatewayError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        let body = ErrorBody {
            error: label.to_string(),
            detail: self.to_string(),
        };
        tracing::warn!(error = %self, status = status.as_u16(), "request failed");
        (status, Json(body)).into_response()
    }
}
