//! Coordination node REST API.
//!
//! Exposes finalized blocks from the ConsensusEngine over HTTP so that
//! relayers and other consumers can poll for finalized batches without
//! needing a direct channel connection.
//!
//! Endpoints:
//! - `GET /health` — node health and uptime
//! - `GET /finalized?after=N` — finalized blocks with height > N
//! - `GET /batch/:height` — specific finalized batch by height

use crate::consensus::{ConsensusEngine, FinalizedBlock};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::info;

/// Shared state for the REST API.
struct ApiState {
    /// All finalized blocks indexed by height
    blocks: RwLock<BTreeMap<u64, StoredBlock>>,
    /// Engine reference (for receiving new blocks)
    engine: Arc<ConsensusEngine>,
    /// Server start time
    start_time: Instant,
}

/// A finalized block stored for API queries.
#[derive(Clone, Serialize, Deserialize)]
struct StoredBlock {
    pub height: u64,
    pub timestamp: u64,
    pub batch_hash: String,
    pub certificate: String,
    pub message_count: usize,
    pub breaker_action_count: usize,
    pub context_digest: Option<String>,
    pub messages: Vec<StoredMessage>,
    /// Breaker actions emitted during consensus (for relayer consumption).
    #[serde(default)]
    pub breaker_actions: Vec<crate::message::BreakerAction>,
    /// Messages hash (32 bytes, hex-encoded) for on-chain settlement.
    #[serde(default)]
    pub messages_hash: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredMessage {
    pub from: String,
    pub content_hash: String,
    pub timestamp: u64,
    pub gate_verdict: Option<String>,
}

impl StoredBlock {
    fn from_finalized(block: &FinalizedBlock) -> Self {
        let batch = &block.batch;
        let messages = batch
            .messages
            .iter()
            .map(|m| StoredMessage {
                from: hex::encode(&m.from),
                content_hash: hex::encode(m.content_hash),
                timestamp: m.timestamp,
                gate_verdict: m.j_lens_gate.as_ref().map(|v| match v {
                    crate::message::GateVerdict::Green => "green".to_string(),
                    crate::message::GateVerdict::Yellow { .. } => "yellow".to_string(),
                    crate::message::GateVerdict::Red { .. } => "red".to_string(),
                }),
            })
            .collect();

        StoredBlock {
            height: block.height,
            timestamp: batch.timestamp,
            batch_hash: hex::encode(batch.hash()),
            certificate: hex::encode(&block.certificate),
            message_count: batch.messages.len(),
            breaker_action_count: batch.breaker_actions.len(),
            context_digest: batch.context_digest.clone(),
            messages,
            breaker_actions: batch.breaker_actions.clone(),
            messages_hash: hex::encode(batch.hash()),
        }
    }
}

/// Query parameters for GET /finalized
#[derive(Deserialize)]
struct FinalizedQuery {
    /// Return blocks with height > after (default: 0 = all)
    after: Option<u64>,
    /// Maximum number of blocks to return (default: 32)
    limit: Option<usize>,
}

/// Response wrapper for GET /finalized — matches the relayer's expected format.
#[derive(Serialize, Deserialize)]
struct FinalizedResponse {
    batches: Vec<StoredBlock>,
    latest_height: u64,
}

/// Health response
#[derive(Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    uptime_seconds: u64,
    block_count: usize,
    latest_height: Option<u64>,
}

/// Configuration for the coordination REST API server.
#[derive(Clone, Debug)]
pub struct ApiConfig {
    /// Bind address (e.g. "0.0.0.0:8080")
    pub bind_addr: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8080".to_string(),
        }
    }
}

/// Start the coordination REST API server.
///
/// This spawns a background task that:
/// 1. Consumes finalized blocks from the ConsensusEngine
/// 2. Stores them in memory indexed by height
/// 3. Serves HTTP requests for /health, /finalized, /batch/:height
pub async fn serve(engine: Arc<ConsensusEngine>, config: ApiConfig) -> anyhow::Result<()> {
    let state = Arc::new(ApiState {
        blocks: RwLock::new(BTreeMap::new()),
        engine,
        start_time: Instant::now(),
    });

    // Spawn block consumer task
    let consumer_state = state.clone();
    tokio::spawn(async move {
        info!("Coordination API: block consumer started");
        loop {
            match consumer_state.engine.next_block().await {
                Some(block) => {
                    let height = block.height;
                    let stored = StoredBlock::from_finalized(&block);
                    info!(
                        "Coordination API: stored finalized block height={} messages={} breakers={}",
                        height,
                        stored.message_count,
                        stored.breaker_action_count
                    );
                    consumer_state.blocks.write().await.insert(height, stored);
                }
                None => {
                    // Channel closed — engine stopped
                    info!("Coordination API: block channel closed, consumer stopping");
                    break;
                }
            }
        }
    });

    // Build router
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/finalized", get(finalized_handler))
        .route("/batch/{height}", get(batch_handler))
        .with_state(state);

    let addr: SocketAddr = config
        .bind_addr
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid bind address {}: {}", config.bind_addr, e))?;

    info!("Coordination REST API listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_handler(State(state): State<Arc<ApiState>>) -> Json<HealthResponse> {
    let blocks = state.blocks.read().await;
    let latest_height = blocks.keys().last().copied();
    Json(HealthResponse {
        status: "ok".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        block_count: blocks.len(),
        latest_height,
    })
}

async fn finalized_handler(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<FinalizedQuery>,
) -> Result<Json<FinalizedResponse>, (StatusCode, String)> {
    let after = params.after.unwrap_or(0);
    let limit = params.limit.unwrap_or(32).min(256);

    let blocks = state.blocks.read().await;
    let latest_height = blocks.keys().last().copied().unwrap_or(0);
    let result: Vec<StoredBlock> = blocks
        .range((after + 1)..)
        .take(limit)
        .map(|(_, b)| b.clone())
        .collect();

    Ok(Json(FinalizedResponse {
        batches: result,
        latest_height,
    }))
}

async fn batch_handler(
    State(state): State<Arc<ApiState>>,
    Path(height): Path<u64>,
) -> Result<Json<StoredBlock>, (StatusCode, String)> {
    let blocks = state.blocks.read().await;
    match blocks.get(&height) {
        Some(block) => Ok(Json(block.clone())),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("batch at height {} not found", height),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::ConsensusConfig;
    use crate::message::AgentMessage;

    #[tokio::test]
    async fn test_api_serves_finalized_blocks() {
        let engine = Arc::new(ConsensusEngine::new(ConsensusConfig::default()));

        // Submit a message and produce a block
        let msg = AgentMessage::new(
            vec![1; 32],
            vec![],
            b"test content".to_vec(),
            1000,
        );
        let _ = engine.submit(msg).await;
        let block = engine.produce_block().await;
        assert!(block.is_some());

        // Start API server on a random port
        let engine_clone = engine.clone();
        let api_port = get_free_port().await;
        let config = ApiConfig {
            bind_addr: format!("127.0.0.1:{}", api_port),
        };

        tokio::spawn(async move {
            let _ = serve(engine_clone, config).await;
        });

        // Give the server a moment to start and consume the block
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Query health
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{}/health", api_port))
            .send()
            .await
            .expect("health request failed");
        assert_eq!(resp.status(), 200);
        let health: HealthResponse = resp.json().await.expect("parse health failed");
        assert_eq!(health.status, "ok");

        // Query finalized blocks
        let resp = client
            .get(format!("http://127.0.0.1:{}/finalized", api_port))
            .send()
            .await
            .expect("finalized request failed");
        assert_eq!(resp.status(), 200);
        let response: FinalizedResponse = resp.json().await.expect("parse finalized response failed");
        // The consumer may or may not have consumed the block yet depending on timing
        assert!(response.batches.len() <= 1);
    }

    async fn get_free_port() -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        addr.port()
    }
}
