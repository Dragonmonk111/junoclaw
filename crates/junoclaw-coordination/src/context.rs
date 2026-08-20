//! Context fetcher — retrieves moultbook provenance during coordination.
//!
//! The `ContextFetcher` trait abstracts the retrieval of historical context
//! (moultbook heartbeat chains, prior decisions, topic entries) so the
//! consensus engine can attach provenance to each finalized batch.
//!
//! `MoultbookContextFetcher` is the production implementation — it calls the
//! DAO-mandated context agent's HTTP API (same API as the runtime
//! `context_query` tool). `MockContextFetcher` is for testing.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::warn;

/// A summary of moultbook context fetched for a batch.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ContextSummary {
    /// Number of heartbeat entries found in the citation chain
    pub heartbeat_count: usize,
    /// Latest entry ID (moult:...)
    pub latest_entry_id: Option<String>,
    /// SHA-256 of the latest entry's commitment
    pub latest_commitment_hash: Option<String>,
    /// Human-readable digest for logging / batch attachment
    pub digest: String,
}

/// Trait for fetching moultbook context during coordination.
#[async_trait]
pub trait ContextFetcher: Send + Sync {
    /// Fetch context summary for a given robot ID and batch height.
    async fn fetch_context(&self, robot_id: &str, batch_height: u64) -> Result<ContextSummary>;
}

/// Mock context fetcher for testing — returns deterministic summaries.
pub struct MockContextFetcher;

#[async_trait]
impl ContextFetcher for MockContextFetcher {
    async fn fetch_context(&self, robot_id: &str, batch_height: u64) -> Result<ContextSummary> {
        Ok(ContextSummary {
            heartbeat_count: (batch_height % 10) as usize,
            latest_entry_id: Some(format!("moult:{}:{}", robot_id, batch_height)),
            latest_commitment_hash: Some(format!("sha256:mock:{}:{}", robot_id, batch_height)),
            digest: format!(
                "mock-context: robot={}, height={}, heartbeats={}",
                robot_id,
                batch_height,
                batch_height % 10
            ),
        })
    }
}

/// Production context fetcher — calls the DAO-mandated context agent HTTP API.
///
/// This is the same API surface as `junoclaw-runtime::context_agent::ContextAgentClient`
/// but reduced to the minimal interface needed by the coordination layer.
pub struct MoultbookContextFetcher {
    base_url: String,
    client: reqwest::Client,
}

impl MoultbookContextFetcher {
    /// Create from an explicit base URL (e.g. "http://localhost:3000").
    pub fn new(base_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    /// Create from the `JUNO_CONTEXT_AGENT_URL` env var, falling back to
    /// `http://localhost:3000`.
    pub fn from_env() -> Self {
        let url = std::env::var("JUNO_CONTEXT_AGENT_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());
        Self::new(&url)
    }
}

#[derive(Debug, Deserialize)]
struct EntriesResponse {
    entries: Vec<MoultbookEntry>,
}

#[derive(Debug, Deserialize)]
struct MoultbookEntry {
    id: String,
    commitment: String,
}

#[async_trait]
impl ContextFetcher for MoultbookContextFetcher {
    async fn fetch_context(&self, robot_id: &str, batch_height: u64) -> Result<ContextSummary> {
        // Fetch the latest entries for this robot author
        let url = format!(
            "{}/entries?author={}&limit=10",
            self.base_url,
            urlencoding::encode(robot_id)
        );

        let resp = self.client.get(&url).send().await;
        match resp {
            Ok(r) if r.status().is_success() => {
                let body: EntriesResponse = r.json().await.map_err(|e| {
                    anyhow::anyhow!("context-agent entries parse error: {}", e)
                })?;

                let heartbeat_count = body.entries.len();
                let latest = body.entries.first();

                let digest = format!(
                    "moultbook-context: robot={}, height={}, heartbeats={}, latest={}",
                    robot_id,
                    batch_height,
                    heartbeat_count,
                    latest.map(|e| e.id.as_str()).unwrap_or("none")
                );

                Ok(ContextSummary {
                    heartbeat_count,
                    latest_entry_id: latest.map(|e| e.id.clone()),
                    latest_commitment_hash: latest.map(|e| e.commitment.clone()),
                    digest,
                })
            }
            Ok(r) => {
                warn!(
                    "context-agent returned {} for robot {} — proceeding without context",
                    r.status(),
                    robot_id
                );
                Ok(ContextSummary {
                    digest: format!("context-unavailable: robot={}, status={}", robot_id, r.status()),
                    ..Default::default()
                })
            }
            Err(e) => {
                warn!(
                    "context-agent unreachable for robot {}: {} — proceeding without context",
                    robot_id, e
                );
                Ok(ContextSummary {
                    digest: format!("context-error: robot={}, err={}", robot_id, e),
                    ..Default::default()
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_context_fetcher() {
        let fetcher = MockContextFetcher;
        let ctx = fetcher.fetch_context("robot-01", 5).await.unwrap();
        assert_eq!(ctx.heartbeat_count, 5);
        assert!(ctx.latest_entry_id.is_some());
        assert!(ctx.digest.contains("robot-01"));
    }

    #[tokio::test]
    async fn test_moultbook_context_fetcher_parses_entries_response() {
        // Spawn a mock HTTP server that returns the /entries format
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = socket.read(&mut buf).await.unwrap();
            let body = serde_json::json!({
                "entries": [
                    {"id": "moult:abc123", "commitment": "sha256:deadbeef"},
                    {"id": "moult:def456", "commitment": "sha256:cafef00d"}
                ],
                "next_after": "moult:def456"
            });
            let body_str = serde_json::to_string(&body).unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body_str.len(),
                body_str
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let fetcher = MoultbookContextFetcher::new(&format!("http://{}", addr));
        let ctx = fetcher.fetch_context("robot-01", 1).await.unwrap();
        assert_eq!(ctx.heartbeat_count, 2);
        assert_eq!(ctx.latest_entry_id.as_deref(), Some("moult:abc123"));
        assert_eq!(ctx.latest_commitment_hash.as_deref(), Some("sha256:deadbeef"));
        assert!(ctx.digest.contains("heartbeats=2"));
    }

    #[tokio::test]
    async fn test_moultbook_context_fetcher_handles_empty_entries() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = socket.read(&mut buf).await.unwrap();
            let body = serde_json::json!({"entries": [], "next_after": null});
            let body_str = serde_json::to_string(&body).unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body_str.len(),
                body_str
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let fetcher = MoultbookContextFetcher::new(&format!("http://{}", addr));
        let ctx = fetcher.fetch_context("robot-empty", 1).await.unwrap();
        assert_eq!(ctx.heartbeat_count, 0);
        assert!(ctx.latest_entry_id.is_none());
    }

    #[tokio::test]
    async fn test_moultbook_context_fetcher_handles_connection_refused() {
        let fetcher = MoultbookContextFetcher::new("http://127.0.0.1:1");
        let ctx = fetcher.fetch_context("robot-01", 1).await.unwrap();
        // Should return a default context with error digest, not panic
        assert!(ctx.digest.contains("context-error"));
    }
}
