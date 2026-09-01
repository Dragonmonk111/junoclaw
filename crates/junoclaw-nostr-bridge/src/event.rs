//! Builds Nostr kind 38402 events from JunoClaw task data.
//!
//! ## Tag schema (proposed NIP-XX: Verifiable Compute Task Discovery)
//!
//! | Tag | Value | Purpose |
//! |-----|-------|---------|
//! | `d`        | `{chain}:{contract}:{task_id}` | Replaceable event identifier |
//! | `chain`    | `juno-1`                       | Cosmos chain ID |
//! | `contract` | `juno1...`                     | task-ledger contract |
//! | `task`     | `42`                           | task_id integer |
//! | `reward`   | `1000000ujuno`                 | Cosmos sdk Coin format |
//! | `deadline` | `1750000000`                   | Unix epoch seconds |
//! | `verifier` | `juno1...`                     | zk-verifier contract |
//! | `vk_hash`  | `sha256:abc...`                | Verification key hash |
//! | `caps`     | `compute,storage`              | Required capabilities |
//! | `status`   | `open`                         | Task status |
//! | `height`   | `12345678`                     | Block height of post |

use anyhow::Result;
use nostr_sdk::{
    EventBuilder, JsonUtil, Keys, Kind, Tag,
};

use crate::types::{TaskInfo, TaskStatus};

/// Nostr event kind for JunoClaw task discovery.
pub const KIND_TASK_DISCOVERY: u16 = 38402;

/// A built Nostr event ready to publish.
pub struct TaskEvent {
    pub event_json: String,
    pub task_id: u64,
    pub status: TaskStatus,
}

/// Build a kind 38402 Nostr event from a [`TaskInfo`].
pub fn build_task_event(task: &TaskInfo, keys: &Keys) -> Result<TaskEvent> {
    let d_tag = format!("{}:{}:{}", task.chain_id, task.contract, task.task_id);
    let task_id_str = task.task_id.to_string();
    let deadline_str = task.deadline.to_string();
    let height_str = task.block_height.to_string();
    let caps_str = task.caps.join(",");

    // Content: JSON-encoded task summary (up to 64 KB; relays may reject larger)
    let content = serde_json::to_string_pretty(&serde_json::json!({
        "task_id": task.task_id,
        "chain_id": task.chain_id,
        "contract": task.contract,
        "reward": task.reward,
        "deadline": task.deadline,
        "verifier": task.verifier,
        "vk_hash": task.vk_hash,
        "caps": task.caps,
        "description": task.description,
        "status": task.status.as_str(),
        "block_height": task.block_height,
    }))?;

    let tags: Vec<Tag> = vec![
        Tag::custom(nostr_sdk::TagKind::Custom("d".into()), vec![d_tag]),
        Tag::custom(nostr_sdk::TagKind::Custom("chain".into()), vec![task.chain_id.clone()]),
        Tag::custom(nostr_sdk::TagKind::Custom("contract".into()), vec![task.contract.clone()]),
        Tag::custom(nostr_sdk::TagKind::Custom("task".into()), vec![task_id_str]),
        Tag::custom(nostr_sdk::TagKind::Custom("reward".into()), vec![task.reward.clone()]),
        Tag::custom(nostr_sdk::TagKind::Custom("deadline".into()), vec![deadline_str]),
        Tag::custom(nostr_sdk::TagKind::Custom("verifier".into()), vec![task.verifier.clone()]),
        Tag::custom(nostr_sdk::TagKind::Custom("vk_hash".into()), vec![task.vk_hash.clone()]),
        Tag::custom(nostr_sdk::TagKind::Custom("caps".into()), vec![caps_str]),
        Tag::custom(nostr_sdk::TagKind::Custom("status".into()), vec![task.status.as_str().to_string()]),
        Tag::custom(nostr_sdk::TagKind::Custom("height".into()), vec![height_str]),
    ];

    let builder = EventBuilder::new(Kind::Custom(KIND_TASK_DISCOVERY), content, tags);
    let event = builder.to_event(keys)?;
    let event_json = event.as_json();

    Ok(TaskEvent {
        event_json,
        task_id: task.task_id,
        status: task.status.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr_sdk::Keys;

    #[test]
    fn test_build_task_event_roundtrip() {
        let keys = Keys::generate();
        let task = TaskInfo {
            task_id: 42,
            contract: "juno1task...".into(),
            chain_id: "uni-7".into(),
            reward: "1000000ujunox".into(),
            deadline: 1750000000,
            verifier: "juno1verifier...".into(),
            vk_hash: "sha256:abc123".into(),
            caps: vec!["compute".into(), "llm".into()],
            description: "Test task".into(),
            block_height: 12345678,
            status: TaskStatus::Open,
        };

        let event = build_task_event(&task, &keys).unwrap();
        assert!(!event.event_json.is_empty());
        assert_eq!(event.task_id, 42);
        assert_eq!(event.status, TaskStatus::Open);

        let parsed: serde_json::Value = serde_json::from_str(&event.event_json).unwrap();
        assert_eq!(parsed["kind"], serde_json::json!(38402));

        let tags = parsed["tags"].as_array().unwrap();
        let task_tag = tags.iter().find(|t| t[0] == "task").unwrap();
        assert_eq!(task_tag[1], "42");
    }
}
