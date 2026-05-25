use thiserror::Error;

#[derive(Debug, Error)]
pub enum RelayError {
    #[error("IBC channel not found: {channel}")]
    ChannelNotFound { channel: String },
    #[error("PFM not available on chain {chain_id}")]
    PfmNotAvailable { chain_id: String },
    #[error("Task {task_id} deadline would expire before IBC timeout")]
    DeadlineTooClose { task_id: u64 },
    #[error("Memo exceeds ICS-20 limit: {size} bytes (max 32768)")]
    MemoTooLarge { size: usize },
    #[error("Proof too large for memo: {size} bytes (max ~700 bytes base64)")]
    ProofTooLarge { size: usize },
    #[error("Invalid swap amount in field '{field}': '{value}' (must be u128)")]
    InvalidSwapAmount { field: String, value: String },
    #[error("gRPC error: {0}")]
    Grpc(String),
    #[error("JSON serialization: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
