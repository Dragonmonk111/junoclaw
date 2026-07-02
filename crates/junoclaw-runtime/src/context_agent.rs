use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

const DEFAULT_TIMEOUT_SECONDS: u64 = 10;
const DEFAULT_BASE_URL: &str = "http://localhost:3000";

/// Read-only client for the A17 DAO-mandated context agent.
#[derive(Clone, Debug)]
pub struct ContextAgentClient {
    base_url: String,
    client: Client,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub indexed_at: String,
    pub entry_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MoultbookEntry {
    pub id: String,
    pub author: String,
    #[serde(default)]
    pub author_alias: Option<String>,
    pub commitment: String,
    pub content_type: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub refs: Vec<String>,
    pub posted_at: String,
    #[serde(default)]
    pub topic_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainResponse {
    pub chain: Vec<MoultbookEntry>,
    #[serde(default)]
    pub next_after: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntriesResponse {
    pub entries: Vec<MoultbookEntry>,
    #[serde(default)]
    pub next_after: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DigestResponse {
    #[serde(default)]
    pub markdown: Option<String>,
    #[serde(default)]
    pub json: Option<serde_json::Value>,
}

impl ContextAgentClient {
    /// Create a client from the `JUNO_CONTEXT_AGENT_URL` environment variable,
    /// falling back to `http://localhost:3000`.
    pub fn from_env() -> Self {
        let base_url = std::env::var("JUNO_CONTEXT_AGENT_URL")
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        Self::new(&base_url)
    }

    pub fn new(base_url: &str) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
            .build()
            .unwrap_or_default();
        Self { base_url, client }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Lightweight alive check. Returns health metadata if the agent is up.
    pub async fn health(&self) -> anyhow::Result<HealthResponse> {
        let res = self.client.get(self.url("/health")).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("context-agent health returned HTTP {}", res.status());
        }
        Ok(res.json().await?)
    }

    /// Fetch the latest heartbeat digest (markdown + JSON) from the GitHub mirror.
    pub async fn latest_digest(&self) -> anyhow::Result<DigestResponse> {
        let res = self.client.get(self.url("/digest/latest")).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("context-agent digest returned HTTP {}", res.status());
        }
        Ok(res.json().await?)
    }

    /// Reconstruct the heartbeat citation chain from `from_id` backwards.
    /// If `from_id` is `None`, starts from the latest entry.
    pub async fn chain(
        &self,
        from_id: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<ChainResponse> {
        let mut url = format!("{}/chain?limit={}", self.base_url, limit);
        if let Some(id) = from_id {
            url.push_str(&format!("&from_id={}", urlencoding::encode(id)));
        }
        let res = self.client.get(url).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("context-agent chain returned HTTP {}", res.status());
        }
        Ok(res.json().await?)
    }

    /// Fetch a single Moultbook entry by id (e.g. `moult:...`).
    pub async fn entry(&self, id: &str) -> anyhow::Result<MoultbookEntry> {
        let url = format!(
            "{}/entry?id={}",
            self.base_url,
            urlencoding::encode(id)
        );
        let res = self.client.get(url).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("context-agent entry returned HTTP {}", res.status());
        }
        Ok(res.json().await?)
    }

    /// Paginated list of entries. All filters are optional.
    pub async fn entries(
        &self,
        author: Option<&str>,
        topic: Option<&str>,
        content_type: Option<&str>,
        start_after: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<EntriesResponse> {
        let mut url = format!("{}/entries?limit={}", self.base_url, limit);
        if let Some(a) = author {
            url.push_str(&format!("&author={}", urlencoding::encode(a)));
        }
        if let Some(t) = topic {
            url.push_str(&format!("&topic={}", urlencoding::encode(t)));
        }
        if let Some(ct) = content_type {
            url.push_str(&format!("&content_type={}", urlencoding::encode(ct)));
        }
        if let Some(sa) = start_after {
            url.push_str(&format!("&start_after={}", urlencoding::encode(sa)));
        }
        let res = self.client.get(url).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("context-agent entries returned HTTP {}", res.status());
        }
        Ok(res.json().await?)
    }

    /// Trigger an on-demand re-index. Returns the new entry count.
    pub async fn refresh(&self) -> anyhow::Result<usize> {
        let res = self
            .client
            .post(self.url("/refresh"))
            .send()
            .await?;
        if !res.status().is_success() {
            anyhow::bail!("context-agent refresh returned HTTP {}", res.status());
        }
        let health: HealthResponse = res.json().await?;
        Ok(health.entry_count)
    }

    /// Try to reach the agent. Logs the result and returns whether it is available.
    pub async fn probe(&self) -> bool {
        match self.health().await {
            Ok(h) => {
                info!(
                    "context-agent ready at {} — {} entries indexed at {}",
                    self.base_url, h.entry_count, h.indexed_at
                );
                true
            }
            Err(e) => {
                warn!("context-agent not reachable at {}: {}", self.base_url, e);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url_trimmed() {
        let c = ContextAgentClient::new("http://localhost:3000/");
        assert_eq!(c.base_url, "http://localhost:3000");
    }

    #[test]
    fn url_building() {
        let c = ContextAgentClient::new("http://localhost:3000");
        assert_eq!(c.url("/health"), "http://localhost:3000/health");
    }

    /// Live integration test against the local A17 context agent.
    /// Run with: `cargo test -p junoclaw-runtime live_context_agent -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn live_context_agent() {
        let client = ContextAgentClient::from_env();
        let health = client.health().await.expect("context agent should be reachable");
        assert_eq!(health.status, "ok");
        assert!(health.entry_count > 0, "should have indexed heartbeat entries");

        let chain = client.chain(None, 10).await.expect("chain query should work");
        assert!(!chain.chain.is_empty(), "citation chain should not be empty");

        let digest = client.latest_digest().await.expect("digest query should work");
        assert!(digest.markdown.is_some() || digest.json.is_some(), "digest should have content");
    }
}
