//! Moultbook addendum — publishes each settled coordination batch as a
//! moultbook-v0 entry, giving the batch a semantic on-chain index alongside
//! the minimal settler anchor.
//!
//! The settler stores ~120 bytes for machines (certificate + messages_hash).
//! The moult is the human/agent-facing surface: same commitment, plus topic
//! namespacing, refs to coordination heights, visibility control, and later
//! anonymous incident reporting via PublishAnon.
//!
//! Entry shape (ExecuteMsg::Post):
//!   commitment   = batch.messages_hash (same 32 bytes the settler anchors)
//!   content_type = "application/x-junoclaw-batch"
//!   size_bytes   = payload size reported by the coordination node (0 = unknown)
//!   refs         = ["commonware:<height>", "topic:<namespace>"] — links the
//!                  moult to the BFT sequence and makes it discoverable via
//!                  ListByRef under the namespace. (On-chain `topic_hash` is
//!                  only populated by PublishAnon; regular posts index topics
//!                  through refs instead.)
//!   visibility   = Public (default)

use anyhow::Result;
use cosmwasm_std::Binary;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tracing::info;

use crate::watcher::FinalizedBatch;

pub const BATCH_CONTENT_TYPE: &str = "application/x-junoclaw-batch";

/// Options for the moultbook addendum. When `moultbook_addr` is set on the
/// CLI, every successfully settled batch also gets a moult.
#[derive(Clone, Debug)]
pub struct MoultConfig {
    pub moultbook_addr: String,
    /// Human namespace for the topic, e.g. "pipeline-A12". Hashed to
    /// "sha256:<hex>" to match moultbook's topic_hash convention.
    pub topic_namespace: String,
}

/// moultbook-v0 ExecuteMsg::Post (visibility fixed to Public for now).
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum MoultExecuteMsg {
    Post {
        commitment: Binary,
        content_type: String,
        size_bytes: u64,
        attestation_ref: Option<serde_json::Value>,
        visibility: MoultVisibility,
        refs: Vec<String>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum MoultVisibility {
    Public,
}

/// Derive a moultbook topic_hash from a human-readable namespace.
pub fn topic_hash(namespace: &str) -> String {
    let digest = Sha256::digest(namespace.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

/// Build the Post message for a settled batch.
pub fn build_batch_post(config: &MoultConfig, batch: &FinalizedBatch) -> serde_json::Value {
    let msg = MoultExecuteMsg::Post {
        commitment: Binary::from(batch.messages_hash),
        content_type: BATCH_CONTENT_TYPE.to_string(),
        size_bytes: batch.payload_size_bytes,
        attestation_ref: None,
        visibility: MoultVisibility::Public,
        refs: vec![
            format!("commonware:{}", batch.commonware_height),
            format!("topic:{}", config.topic_namespace),
        ],
    };
    serde_json::to_value(msg).expect("moult post serialization is infallible")
}

/// Post a settled batch to moultbook as a semantic index entry.
///
/// Same signing scaffold as `bridge::submit_batch` — builds and logs the
/// message; broadcast wiring lands with the chosen signer (cosmrs or the
/// junoclaw MCP wallet store).
pub async fn post_batch_moult(
    rpc_endpoint: &str,
    relayer_key: &str,
    config: &MoultConfig,
    batch: &FinalizedBatch,
) -> Result<()> {
    let msg = build_batch_post(config, batch);

    info!(
        "Built moultbook Post for contract {} (height={}, topic={}, commitment={})",
        config.moultbook_addr,
        batch.commonware_height,
        topic_hash(&config.topic_namespace),
        hex::encode(batch.messages_hash),
    );

    // TODO: sign and broadcast, same signer decision as bridge::submit_batch.
    // The tx is a MsgExecuteContract on config.moultbook_addr with `msg` above
    // and no funds.
    let _ = (rpc_endpoint, relayer_key, msg);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_batch() -> FinalizedBatch {
        FinalizedBatch {
            commonware_height: 4041,
            messages_hash: [0xAB; 32],
            certificate: vec![0xCD; 32],
            timestamp: 1_755_000_000,
            payload_size_bytes: 12_400,
        }
    }

    #[test]
    fn topic_hash_is_sha256_namespaced() {
        let h = topic_hash("pipeline-A12");
        assert!(h.starts_with("sha256:"));
        assert_eq!(h.len(), "sha256:".len() + 64);
        // deterministic
        assert_eq!(h, topic_hash("pipeline-A12"));
        assert_ne!(h, topic_hash("pipeline-A13"));
    }

    #[test]
    fn post_carries_same_commitment_as_settler() {
        let config = MoultConfig {
            moultbook_addr: "juno1moultbook".to_string(),
            topic_namespace: "pipeline-A12".to_string(),
        };
        let msg = build_batch_post(&config, &test_batch());
        let post = &msg["post"];

        // commitment must be base64 of the exact 32 bytes the settler anchors
        let expected = cosmwasm_std::Binary::from([0xAB; 32]).to_base64();
        assert_eq!(post["commitment"].as_str().unwrap(), expected);
        assert_eq!(
            post["content_type"].as_str().unwrap(),
            BATCH_CONTENT_TYPE
        );
        assert_eq!(post["size_bytes"].as_u64().unwrap(), 12_400);
        assert_eq!(post["refs"][0].as_str().unwrap(), "commonware:4041");
        assert_eq!(post["refs"][1].as_str().unwrap(), "topic:pipeline-A12");
        assert_eq!(post["visibility"].as_str().unwrap(), "public");
    }

    #[test]
    fn zero_payload_size_means_unknown_not_empty() {
        let config = MoultConfig {
            moultbook_addr: "juno1moultbook".to_string(),
            topic_namespace: "soak-test".to_string(),
        };
        let mut batch = test_batch();
        batch.payload_size_bytes = 0;
        let msg = build_batch_post(&config, &batch);
        assert_eq!(msg["post"]["size_bytes"].as_u64().unwrap(), 0);
    }
}
