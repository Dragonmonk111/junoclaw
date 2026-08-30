//! Nostr publisher — sends kind 38402 events to configured relays.
//!
//! Uses raw WebSocket with manual NIP-42 authentication because the Buzz relay
//! is behind an Nginx proxy that forwards `Host: localhost:3000`. The NIP-42
//! auth event's `relay` tag must match `ws://localhost:3000`, not the public
//! URL. The `nostr_sdk` high-level Client doesn't allow customizing this, so
//! we handle the WebSocket connection and auth handshake manually.

use anyhow::{bail, Result};
use nostr_sdk::{JsonUtil, Keys, SecretKey};
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    MaybeTlsStream, WebSocketStream,
};
use futures_util::{SinkExt, StreamExt};
use tracing::{info, warn};
use tokio::sync::Mutex;
use std::sync::Arc;

use crate::config::BridgeConfig;
use crate::event::{build_task_event, TaskEvent};
use crate::types::TaskInfo;

/// The relay URL the Buzz relay expects in NIP-42 auth events (due to Nginx proxy).
const AUTH_RELAY_URL: &str = "ws://localhost:3000";

pub struct NostrPublisher {
    keys: Keys,
    relay_urls: Vec<String>,
    /// Per-relay WebSocket connections (lazily connected on first publish).
    conns: Arc<Mutex<Vec<Option<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>>>>,
}

impl NostrPublisher {
    pub async fn new(config: &BridgeConfig) -> Result<Self> {
        if config.nostr_privkey_hex.is_empty() {
            bail!(
                "JUNOCLAW_NOSTR_PRIVKEY not set — required to sign Nostr events. \
                 Use --dry-run to build + log events without publishing."
            );
        }
        let privkey_bytes = hex::decode(&config.nostr_privkey_hex)?;
        let secret_key = SecretKey::from_slice(&privkey_bytes)?;
        let keys = Keys::new(secret_key);

        let relay_count = config.relays.len();
        for relay_url in &config.relays {
            info!("Added Nostr relay: {relay_url}");
        }

        info!(
            "Nostr publisher ready. Bridge pubkey: {}",
            keys.public_key()
        );

        Ok(Self {
            keys,
            relay_urls: config.relays.clone(),
            conns: Arc::new(Mutex::new((0..relay_count).map(|_| None).collect())),
        })
    }

    /// Connect to a relay and perform NIP-42 authentication.
    async fn connect_and_auth(&self, idx: usize) -> Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
        let url = &self.relay_urls[idx];
        info!("Connecting to relay: {url}");

        let (ws_stream, _) = connect_async(url).await?;
        let mut ws = ws_stream;

        // Wait for AUTH challenge
        let mut challenge: Option<String> = None;
        let auth_timeout = tokio::time::Duration::from_secs(10);
        let deadline = tokio::time::sleep(auth_timeout);

        tokio::pin!(deadline);

        loop {
            tokio::select! {
                _ = &mut deadline => {
                    bail!("Auth timeout — no AUTH challenge received from {url}");
                }
                msg = ws.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            let v: serde_json::Value = serde_json::from_str(&text)?;
                            if v[0].as_str() == Some("AUTH") {
                                challenge = v[1].as_str().map(String::from);
                                break;
                            }
                            // Ignore NOTICE or other messages
                        }
                        _ => continue,
                    }
                }
            }
        }

        let challenge = challenge.ok_or_else(|| anyhow::anyhow!("No challenge in AUTH message"))?;

        // Build and sign the NIP-42 auth event (kind 22242)
        let auth_event = self.build_auth_event(&challenge)?;
        info!("Auth event signed for relay {url}");

        // Send ["AUTH", authEvent]
        let auth_msg = serde_json::json!(["AUTH", serde_json::from_str::<serde_json::Value>(&auth_event)?]);
        ws.send(Message::Text(auth_msg.to_string())).await?;

        // Wait for OK or just proceed after a short delay
        let confirm_timeout = tokio::time::Duration::from_secs(5);
        let deadline = tokio::time::sleep(confirm_timeout);
        tokio::pin!(deadline);

        loop {
            tokio::select! {
                _ = &mut deadline => {
                    info!("No auth confirmation, proceeding anyway");
                    break;
                }
                msg = ws.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            let v: serde_json::Value = serde_json::from_str(&text)?;
                            if v[0].as_str() == Some("OK") {
                                let accepted = v[2].as_bool().unwrap_or(false);
                                if accepted {
                                    info!("Auth accepted by relay {url}");
                                } else {
                                    let reason = v[3].as_str().unwrap_or("unknown");
                                    warn!("Auth rejected by relay {url}: {reason}");
                                }
                                break;
                            }
                        }
                        _ => continue,
                    }
                }
            }
        }

        Ok(ws)
    }

    /// Build a signed NIP-42 auth event (kind 22242).
    fn build_auth_event(&self, challenge: &str) -> Result<String> {
        use nostr_sdk::{EventBuilder, Kind, Tag};

        let tags = vec![
            Tag::custom(nostr_sdk::TagKind::Custom("relay".into()), vec![AUTH_RELAY_URL.to_string()]),
            Tag::custom(nostr_sdk::TagKind::Custom("challenge".into()), vec![challenge.to_string()]),
        ];

        let builder = EventBuilder::new(Kind::Custom(22242), "", tags);
        let event = builder.to_event(&self.keys)?;
        Ok(event.as_json())
    }

    /// Build and publish a kind 38402 event for the given task.
    pub async fn publish_task(&self, task: &TaskInfo) -> Result<TaskEvent> {
        let task_event = build_task_event(task, &self.keys)?;

        let mut conns = self.conns.lock().await;
        let mut success_count = 0;
        let mut fail_count = 0;

        for (idx, url) in self.relay_urls.iter().enumerate() {
            // Connect if not already connected
            if conns[idx].is_none() {
                match self.connect_and_auth(idx).await {
                    Ok(ws) => conns[idx] = Some(ws),
                    Err(e) => {
                        warn!("Failed to connect to relay {url}: {e}");
                        fail_count += 1;
                        continue;
                    }
                }
            }

            let ws = conns[idx].as_mut().unwrap();
            let event_value: serde_json::Value = serde_json::from_str(&task_event.event_json)?;
            let msg = serde_json::json!(["EVENT", event_value]);

            match ws.send(Message::Text(msg.to_string())).await {
                Ok(_) => {
                    // Wait for OK response
                    let ok_timeout = tokio::time::Duration::from_secs(10);
                    let deadline = tokio::time::sleep(ok_timeout);
                    tokio::pin!(deadline);

                    let mut accepted = false;
                    loop {
                        tokio::select! {
                            _ = &mut deadline => {
                                warn!("Timeout waiting for OK from relay {url}");
                                break;
                            }
                            msg = ws.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                                        if v[0].as_str() == Some("OK") {
                                            accepted = v[2].as_bool().unwrap_or(false);
                                            if accepted {
                                                info!("Task {} published to relay {url}", task.task_id);
                                            } else {
                                                let reason = v[3].as_str().unwrap_or("unknown");
                                                warn!("Relay {url} rejected task {}: {reason}", task.task_id);
                                            }
                                            break;
                                        }
                                    }
                                    _ => continue,
                                }
                            }
                        }
                    }

                    if accepted {
                        success_count += 1;
                    } else {
                        fail_count += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to send to relay {url}: {e}");
                    conns[idx] = None; // Reset connection
                    fail_count += 1;
                }
            }
        }

        if success_count > 0 {
            info!(
                "Task {} published. Success: {}/{} relays",
                task.task_id,
                success_count,
                success_count + fail_count
            );
        } else {
            warn!("Failed to publish task {} to any relay", task.task_id);
        }

        Ok(task_event)
    }

    pub fn pubkey_hex(&self) -> String {
        self.keys.public_key().to_string()
    }
}
