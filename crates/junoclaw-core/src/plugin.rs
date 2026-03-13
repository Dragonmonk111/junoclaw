use async_trait::async_trait;
use serde_json::Value;

use crate::error::Result;
use crate::types::{Task, TaskResult};

// ──────────────────────────────────────────────
// Plugin Trait
// ──────────────────────────────────────────────

/// Core trait that all JunoClaw plugins must implement.
/// Plugins provide capabilities like LLM inference, compute,
/// storage, shell execution, browser automation, etc.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Unique identifier for this plugin (e.g., "plugin-llm", "plugin-compute-akash")
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// Plugin version
    fn version(&self) -> &str;

    /// List of capabilities this plugin provides
    fn capabilities(&self) -> Vec<PluginCapability>;

    /// Whether this plugin is currently available and configured
    fn is_available(&self) -> bool;

    /// JSON Schema for the plugin's configuration (rendered in Settings UI)
    fn config_schema(&self) -> Value;

    /// Initialize the plugin with its configuration
    async fn initialize(&mut self, config: Value) -> Result<()>;

    /// Execute a task using this plugin
    async fn execute(&self, task: &Task, context: &PluginContext) -> Result<TaskResult>;

    /// Gracefully shut down the plugin
    async fn shutdown(&self) -> Result<()>;
}

// ──────────────────────────────────────────────
// LLM Provider Trait
// ──────────────────────────────────────────────

/// Specialized trait for LLM providers (Ollama, Anthropic, OpenAI, Akash-hosted).
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;

    fn models(&self) -> Vec<crate::types::ModelInfo>;

    async fn complete(
        &self,
        req: &crate::types::CompletionRequest,
    ) -> Result<crate::types::CompletionResponse>;

    async fn stream(
        &self,
        req: &crate::types::CompletionRequest,
    ) -> Result<tokio::sync::mpsc::Receiver<StreamEvent>>;

    fn supports_tools(&self) -> bool;

    fn supports_vision(&self) -> bool;

    fn cost_per_token(&self, model: &str) -> Option<crate::types::TokenCost>;
}

// ──────────────────────────────────────────────
// Supporting Types
// ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PluginCapability {
    LlmInference,
    ComputeLocal,
    ComputeAkash,
    WavsVerification,
    StorageLocal,
    ShellExecution,
    BrowserAutomation,
    IbcMessaging,
}

#[derive(Debug, Clone)]
pub struct PluginContext {
    pub agent_id: String,
    pub session_id: String,
    pub workspace_dir: std::path::PathBuf,
    pub budget_remaining_usd: f64,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Token(String),
    ToolCall(crate::types::ToolCall),
    Done(crate::types::CompletionResponse),
    Error(String),
}
