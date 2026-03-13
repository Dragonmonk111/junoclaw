use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use junoclaw_core::error::{JunoClawError, Result};
use junoclaw_core::plugin::{LlmProvider, StreamEvent};
use junoclaw_core::types::{
    CompletionRequest, CompletionResponse, ModelInfo, TokenCost, TokenUsage,
};

pub struct OllamaProvider {
    client: Client,
    endpoint: String,
    default_model: String,
}

impl OllamaProvider {
    pub fn new(endpoint: &str, default_model: &str) -> Self {
        Self {
            client: Client::new(),
            endpoint: endpoint.trim_end_matches('/').to_string(),
            default_model: default_model.to_string(),
        }
    }

    /// Returns the model name to use — falls back to default_model if requested model
    /// is not in the local Ollama library (avoids silent hang on missing model).
    async fn resolve_model(&self, requested: &str) -> String {
        let url = format!("{}/api/tags", self.endpoint);
        if let Ok(resp) = self.client.get(&url).send().await {
            if let Ok(tags) = resp.json::<OllamaTagsResponse>().await {
                let available: Vec<&str> = tags.models.iter().map(|m| m.name.as_str()).collect();
                if available.iter().any(|n| *n == requested || n.starts_with(&format!("{}:", requested.split(':').next().unwrap_or(requested)))) {
                    return requested.to_string();
                }
                warn!(
                    "Model '{}' not in local Ollama library {:?} — falling back to '{}'",
                    requested, available, self.default_model
                );
                return self.default_model.clone();
            }
        }
        // If we can't reach Ollama tags endpoint, just try the requested model
        requested.to_string()
    }
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaMessage>,
    done: bool,
    #[serde(default)]
    eval_count: u64,
    #[serde(default)]
    prompt_eval_count: u64,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelInfo>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelInfo {
    name: String,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn models(&self) -> Vec<ModelInfo> {
        // Dynamically fetched via /api/tags, but return default for now
        vec![ModelInfo {
            id: self.default_model.clone(),
            name: self.default_model.clone(),
            provider: "ollama".to_string(),
            context_window: 128_000,
            supports_tools: false,
            supports_vision: false,
        }]
    }

    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse> {
        let resolved = self.resolve_model(if req.model.is_empty() { &self.default_model } else { &req.model }).await;
        let model = &resolved;

        let messages: Vec<OllamaMessage> = req
            .messages
            .iter()
            .map(|m| OllamaMessage {
                role: serde_json::to_string(&m.role)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
                content: m.content.clone(),
            })
            .collect();

        let ollama_req = OllamaChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
            options: Some(OllamaOptions {
                temperature: req.temperature,
                num_predict: req.max_tokens,
            }),
        };

        let url = format!("{}/api/chat", self.endpoint);
        debug!("Ollama request to {}: model={}", url, model);

        let response = self
            .client
            .post(&url)
            .json(&ollama_req)
            .send()
            .await
            .map_err(|e| JunoClawError::LlmProvider {
                provider: "ollama".to_string(),
                message: format!("HTTP error: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(JunoClawError::LlmProvider {
                provider: "ollama".to_string(),
                message: format!("HTTP {}: {}", status, body),
            });
        }

        let ollama_resp: OllamaChatResponse =
            response.json().await.map_err(|e| JunoClawError::LlmProvider {
                provider: "ollama".to_string(),
                message: format!("JSON parse error: {}", e),
            })?;

        let content = ollama_resp
            .message
            .map(|m| m.content)
            .unwrap_or_default();

        Ok(CompletionResponse {
            content,
            tool_calls: Vec::new(),
            usage: TokenUsage {
                prompt_tokens: ollama_resp.prompt_eval_count,
                completion_tokens: ollama_resp.eval_count,
                total_tokens: ollama_resp.prompt_eval_count + ollama_resp.eval_count,
            },
            model: model.to_string(),
            finish_reason: "stop".to_string(),
        })
    }

    async fn stream(
        &self,
        req: &CompletionRequest,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let (tx, rx) = mpsc::channel(256);

        let model = self.resolve_model(if req.model.is_empty() { &self.default_model } else { &req.model }).await;

        let messages: Vec<OllamaMessage> = req
            .messages
            .iter()
            .map(|m| OllamaMessage {
                role: serde_json::to_string(&m.role)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
                content: m.content.clone(),
            })
            .collect();

        let ollama_req = OllamaChatRequest {
            model: model.clone(),
            messages,
            stream: true,
            options: Some(OllamaOptions {
                temperature: req.temperature,
                num_predict: req.max_tokens,
            }),
        };

        let url = format!("{}/api/chat", self.endpoint);
        let client = self.client.clone();

        tokio::spawn(async move {
            let response = match client.post(&url).json(&ollama_req).send().await {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(format!("HTTP error: {}", e))).await;
                    return;
                }
            };

            let mut stream = response.bytes_stream();
            let mut full_content = String::new();
            let mut total_prompt: u64;
            let mut total_eval: u64;

            use futures::StreamExt;
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        for line in text.lines() {
                            if line.trim().is_empty() {
                                continue;
                            }
                            if let Ok(resp) = serde_json::from_str::<OllamaChatResponse>(line) {
                                if let Some(ref msg) = resp.message {
                                    if !msg.content.is_empty() {
                                        full_content.push_str(&msg.content);
                                        let _ = tx
                                            .send(StreamEvent::Token(msg.content.clone()))
                                            .await;
                                    }
                                }
                                total_prompt = resp.prompt_eval_count;
                                total_eval = resp.eval_count;

                                if resp.done {
                                    let _ = tx
                                        .send(StreamEvent::Done(CompletionResponse {
                                            content: full_content.clone(),
                                            tool_calls: Vec::new(),
                                            usage: TokenUsage {
                                                prompt_tokens: total_prompt,
                                                completion_tokens: total_eval,
                                                total_tokens: total_prompt + total_eval,
                                            },
                                            model: model.clone(),
                                            finish_reason: "stop".to_string(),
                                        }))
                                        .await;
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(StreamEvent::Error(format!("Stream error: {}", e))).await;
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }

    fn supports_tools(&self) -> bool {
        false
    }

    fn supports_vision(&self) -> bool {
        false
    }

    fn cost_per_token(&self, _model: &str) -> Option<TokenCost> {
        None // Ollama is free/local
    }
}
