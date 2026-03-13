use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ──────────────────────────────────────────────
// Execution Tiers
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionTier {
    /// Fast, free, no verification. Runs on user's machine.
    Local,
    /// GPU compute via Akash Network. Result hash on-chain.
    Akash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Personality {
    Professional,
    Creative,
    Analytical,
    Conversational,
    Custom,
}

impl Default for Personality {
    fn default() -> Self {
        Self::Professional
    }
}

impl Default for ExecutionTier {
    fn default() -> Self {
        Self::Local
    }
}

// ──────────────────────────────────────────────
// Agent
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub model: String,
    pub capabilities: Vec<Capability>,
    pub default_tier: ExecutionTier,
    pub wavs_verified: bool,
    pub personality: Personality,
    pub system_prompt: Option<String>,
    pub created_at: DateTime<Utc>,
    pub is_active: bool,
    /// On-chain agent ID (if registered on Juno)
    pub chain_id: Option<String>,
    /// Parent agent ID for hierarchy (main -> sub -> sub)
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    WebBrowsing,
    FileReadWrite,
    ShellExecution,
    CodeExecution,
    ImageGeneration,
    DataAnalysis,
}

// ──────────────────────────────────────────────
// Task
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub agent_id: String,
    pub input: String,
    pub tier: ExecutionTier,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<TaskResult>,
    pub cost: Option<TaskCost>,
    /// On-chain tx hash (if logged to TaskLedger)
    pub chain_tx: Option<String>,
}

impl Task {
    pub fn new(agent_id: &str, input: &str, tier: ExecutionTier) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            input: input.to_string(),
            tier,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            completed_at: None,
            result: None,
            cost: None,
            chain_tx: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    BudgetExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub output: String,
    pub output_hash: String,
    pub tool_calls: Vec<ToolCallRecord>,
    pub tokens_used: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub duration_ms: u64,
    pub approved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCost {
    pub amount: f64,
    pub denom: String,
    pub usd_equivalent: Option<f64>,
}

// ──────────────────────────────────────────────
// Session
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub messages: Vec<ChatMessage>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Session {
    pub fn new(agent_id: &str) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCallRecord>>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

// ──────────────────────────────────────────────
// LLM
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<LlmMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: TokenUsage,
    pub model: String,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub context_window: u32,
    pub supports_tools: bool,
    pub supports_vision: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCost {
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub currency: String,
}

// ──────────────────────────────────────────────
// WebSocket Messages (daemon <-> frontend)
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum WsClientMessage {
    SendMessage { agent_id: String, content: String },
    CreateAgent(AgentInfo),
    ListAgents,
    ListTasks { agent_id: Option<String> },
    CancelTask { task_id: String },
    ApproveToolCall { task_id: String, tool_call_id: String },
    DenyToolCall { task_id: String, tool_call_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum WsServerMessage {
    StreamToken { agent_id: String, token: String },
    StreamComplete { agent_id: String, message: ChatMessage },
    ToolCallRequest { task_id: String, tool_call: ToolCall },
    ToolCallResult { task_id: String, record: ToolCallRecord },
    TaskStatusUpdate { task: Task },
    AgentList(Vec<AgentInfo>),
    TaskList(Vec<Task>),
    Error { message: String },
    Connected { version: String },
}
