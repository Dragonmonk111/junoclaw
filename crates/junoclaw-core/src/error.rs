use thiserror::Error;

#[derive(Error, Debug)]
pub enum JunoClawError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Plugin error: {plugin} — {message}")]
    Plugin { plugin: String, message: String },

    #[error("Agent error: {agent_id} — {message}")]
    Agent { agent_id: String, message: String },

    #[error("LLM provider error: {provider} — {message}")]
    LlmProvider { provider: String, message: String },

    #[error("Task execution error: {0}")]
    TaskExecution(String),

    #[error("Budget exceeded: spent {spent}, limit {limit}")]
    BudgetExceeded { spent: f64, limit: f64 },

    #[error("Chain error: {0}")]
    Chain(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serialization(#[from] serde_json::Error),

    #[error(transparent)]
    TomlParse(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, JunoClawError>;
