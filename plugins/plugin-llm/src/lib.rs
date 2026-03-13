pub mod ollama;

use junoclaw_core::plugin::{LlmProvider, StreamEvent};
use junoclaw_core::types::{CompletionRequest, CompletionResponse, ModelInfo};

/// Registry of available LLM providers with failover support.
pub struct LlmProviderRegistry {
    providers: Vec<Box<dyn LlmProvider>>,
}

impl LlmProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn LlmProvider>) {
        tracing::info!("Registered LLM provider: {}", provider.name());
        self.providers.push(provider);
    }

    pub fn list_providers(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }

    pub fn list_models(&self) -> Vec<ModelInfo> {
        self.providers.iter().flat_map(|p| p.models()).collect()
    }

    /// Try providers in order until one succeeds (failover chain).
    pub async fn complete(
        &self,
        req: &CompletionRequest,
    ) -> junoclaw_core::error::Result<CompletionResponse> {
        let mut last_err = None;

        for provider in &self.providers {
            match provider.complete(req).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    tracing::warn!(
                        "LLM provider '{}' failed: {}, trying next...",
                        provider.name(),
                        e
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            junoclaw_core::error::JunoClawError::LlmProvider {
                provider: "none".to_string(),
                message: "No LLM providers configured".to_string(),
            }
        }))
    }

    /// Stream from the first available provider.
    pub async fn stream(
        &self,
        req: &CompletionRequest,
    ) -> junoclaw_core::error::Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        for provider in &self.providers {
            match provider.stream(req).await {
                Ok(rx) => return Ok(rx),
                Err(e) => {
                    tracing::warn!(
                        "LLM provider '{}' stream failed: {}, trying next...",
                        provider.name(),
                        e
                    );
                }
            }
        }

        Err(junoclaw_core::error::JunoClawError::LlmProvider {
            provider: "none".to_string(),
            message: "No LLM providers available for streaming".to_string(),
        })
    }
}
