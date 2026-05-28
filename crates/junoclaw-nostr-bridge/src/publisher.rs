//! Nostr publisher — sends kind 38402 events to configured relays.

use anyhow::Result;
use nostr_sdk::{Client, JsonUtil, Keys};
use tracing::{info, warn};

use crate::config::BridgeConfig;
use crate::event::{build_task_event, TaskEvent};
use crate::types::TaskInfo;

pub struct NostrPublisher {
    keys: Keys,
    client: Client,
}

impl NostrPublisher {
    pub async fn new(config: &BridgeConfig) -> Result<Self> {
        let privkey_bytes = hex::decode(&config.nostr_privkey_hex)?;
        let secret_key = nostr_sdk::SecretKey::from_slice(&privkey_bytes)?;
        let keys = Keys::new(secret_key);

        let client = Client::new(keys.clone());

        for relay_url in &config.relays {
            client.add_relay(relay_url).await?;
            info!("Added Nostr relay: {relay_url}");
        }

        client.connect().await;
        info!(
            "Nostr publisher ready. Bridge pubkey: {}",
            keys.public_key()
        );

        Ok(Self { keys, client })
    }

    /// Build and publish a kind 38402 event for the given task.
    pub async fn publish_task(&self, task: &TaskInfo) -> Result<TaskEvent> {
        let task_event = build_task_event(task, &self.keys)?;

        // Parse back the signed event and send it
        let event: nostr_sdk::Event = nostr_sdk::Event::from_json(&task_event.event_json)?;
        match self.client.send_event(event).await {
            Ok(output) => {
                info!(
                    "Task {} published to Nostr. Success: {}/{} relays",
                    task.task_id,
                    output.success.len(),
                    output.success.len() + output.failed.len()
                );
                if !output.failed.is_empty() {
                    warn!("Failed relays: {:?}", output.failed);
                }
            }
            Err(e) => warn!("Failed to publish task {}: {e}", task.task_id),
        }

        Ok(task_event)
    }

    pub fn pubkey_hex(&self) -> String {
        self.keys.public_key().to_string()
    }
}
