use serde::{Deserialize, Serialize};

/// A JunoClaw task as seen from the bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub task_id: u64,
    pub contract: String,
    pub chain_id: String,
    pub reward: String,        // Cosmos sdk Coin format e.g. "1000000ujuno"
    pub deadline: u64,         // Unix epoch seconds
    pub verifier: String,      // zk-verifier contract address
    pub vk_hash: String,       // sha256: prefixed hex of the active VK
    pub caps: Vec<String>,     // required agent capabilities e.g. ["compute","llm"]
    pub description: String,   // free-form task description (goes in content field)
    pub block_height: u64,     // block at which the task was posted
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Claimed,
    Completed,
    Expired,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Open => "open",
            TaskStatus::Claimed => "claimed",
            TaskStatus::Completed => "completed",
            TaskStatus::Expired => "expired",
        }
    }
}

/// A raw Tendermint websocket event from the chain.
#[derive(Debug, Deserialize)]
pub struct TmEvent {
    pub r#type: String,
    pub attributes: Vec<TmAttribute>,
}

#[derive(Debug, Deserialize)]
pub struct TmAttribute {
    pub key: String,
    pub value: Option<String>,
}

/// Parsed from a `wasm` event emitted by task-ledger::PostTask.
#[derive(Debug, Default)]
pub struct PostTaskAttributes {
    pub task_id: Option<u64>,
    pub reward: Option<String>,
    pub deadline: Option<u64>,
    pub caps: Option<Vec<String>>,
    pub description: Option<String>,
    pub block_height: Option<u64>,
}
