//! Tendermint websocket subscriber — watches task-ledger events.
//!
//! Subscribes to `tm.event='Tx' AND wasm._contract_address='{contract}'`
//! and parses `wasm` events into [`TaskInfo`] structs.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

use crate::types::{PostTaskAttributes, TaskInfo, TaskStatus};

/// Subscribe to task-ledger events and call `on_task` for each new task.
/// This function runs until the websocket connection drops.
pub async fn subscribe_task_ledger(
    ws_url: &str,
    contract: &str,
    chain_id: &str,
    zk_verifier: &str,
    on_task: impl Fn(TaskInfo) + Send + Sync + 'static,
) -> Result<()> {
    info!("Connecting to Tendermint websocket: {ws_url}");
    let (mut ws, _) = connect_async(ws_url)
        .await
        .context("Failed to connect to Tendermint websocket")?;

    // Subscribe to wasm events from our contract
    let sub_msg = json!({
        "jsonrpc": "2.0",
        "method": "subscribe",
        "id": 1,
        "params": {
            "query": format!(
                "tm.event='Tx' AND wasm._contract_address='{contract}'"
            )
        }
    });
    ws.send(Message::Text(sub_msg.to_string().into())).await?;
    info!("Subscribed to task-ledger events for {contract} on {chain_id}");

    while let Some(msg) = ws.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(e) = handle_message(
                    &text,
                    contract,
                    chain_id,
                    zk_verifier,
                    &on_task,
                ) {
                    warn!("Error handling message: {e}");
                }
            }
            Ok(Message::Ping(p)) => {
                ws.send(Message::Pong(p)).await.ok();
            }
            Ok(Message::Close(_)) => {
                info!("Websocket closed by server");
                break;
            }
            Err(e) => {
                warn!("Websocket error: {e}");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

fn handle_message(
    text: &str,
    contract: &str,
    chain_id: &str,
    zk_verifier: &str,
    on_task: &impl Fn(TaskInfo),
) -> Result<()> {
    let v: Value = serde_json::from_str(text)?;
    let result = &v["result"];

    // Tendermint subscription results look like: result.data.value.TxResult.result.events
    let events = result
        .pointer("/data/value/TxResult/result/events")
        .and_then(|e| e.as_array());

    let Some(events) = events else {
        debug!("No events in message");
        return Ok(());
    };

    // Extract block height from TxResult
    let block_height = result
        .pointer("/data/value/TxResult/height")
        .and_then(|h| h.as_str())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    for event in events {
        let event_type = event["type"].as_str().unwrap_or_default();
        if event_type != "wasm" {
            continue;
        }

        let attrs = event["attributes"].as_array().map(|a| {
            a.iter()
                .filter_map(|attr| {
                    let key = attr["key"].as_str().unwrap_or_default();
                    let val = attr["value"].as_str().map(String::from);
                    if key.is_empty() { None } else { Some((key.to_string(), val)) }
                })
                .collect::<Vec<_>>()
        }).unwrap_or_default();

        // Check if this event is an "action": "post_task" from our contract
        let action = attrs.iter()
            .find(|(k, _)| k == "action")
            .and_then(|(_, v)| v.as_deref());

        if action != Some("post_task") {
            continue;
        }

        let mut parsed = PostTaskAttributes::default();
        parsed.block_height = Some(block_height);

        for (key, val) in &attrs {
            match key.as_str() {
                "task_id" => parsed.task_id = val.as_deref().and_then(|s| s.parse().ok()),
                "reward" => parsed.reward = val.clone(),
                "deadline" => parsed.deadline = val.as_deref().and_then(|s| s.parse().ok()),
                "caps" => parsed.caps = val.as_deref().map(|s| {
                    s.split(',').map(|c| c.trim().to_string()).collect()
                }),
                "description" => parsed.description = val.clone(),
                _ => {}
            }
        }

        if let (Some(task_id), Some(reward)) = (parsed.task_id, parsed.reward.clone()) {
            let task = TaskInfo {
                task_id,
                contract: contract.to_string(),
                chain_id: chain_id.to_string(),
                reward,
                deadline: parsed.deadline.unwrap_or(0),
                verifier: zk_verifier.to_string(),
                vk_hash: String::new(), // fetched separately if needed
                caps: parsed.caps.unwrap_or_default(),
                description: parsed.description.unwrap_or_default(),
                block_height: parsed.block_height.unwrap_or(0),
                status: TaskStatus::Open,
            };
            info!("New task detected: id={task_id} reward={} height={block_height}", task.reward);
            on_task(task);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn mock_tm_message(action: &str, task_id: &str, reward: &str, height: &str) -> String {
        serde_json::json!({
            "result": {
                "data": {
                    "value": {
                        "TxResult": {
                            "height": height,
                            "result": {
                                "events": [
                                    {
                                        "type": "wasm",
                                        "attributes": [
                                            {"key": "action", "value": action},
                                            {"key": "_contract_address", "value": "juno1contract"},
                                            {"key": "task_id", "value": task_id},
                                            {"key": "reward", "value": reward},
                                            {"key": "deadline", "value": "1750000000"},
                                            {"key": "caps", "value": "compute,llm"},
                                            {"key": "description", "value": "Test task"}
                                        ]
                                    }
                                ]
                            }
                        }
                    }
                }
            }
        }).to_string()
    }

    #[test]
    fn test_handle_post_task_event() {
        let tasks: Arc<Mutex<Vec<TaskInfo>>> = Arc::new(Mutex::new(vec![]));
        let tasks_clone = tasks.clone();
        let on_task = move |t: TaskInfo| { tasks_clone.lock().unwrap().push(t); };

        let msg = mock_tm_message("post_task", "42", "1000000ujuno", "12345");
        handle_message(&msg, "juno1contract", "juno-1", "juno1verifier", &on_task).unwrap();

        let received = tasks.lock().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].task_id, 42);
        assert_eq!(received[0].reward, "1000000ujuno");
        assert_eq!(received[0].deadline, 1750000000);
        assert_eq!(received[0].caps, vec!["compute", "llm"]);
        assert_eq!(received[0].block_height, 12345);
        assert_eq!(received[0].chain_id, "juno-1");
        assert_eq!(received[0].status, TaskStatus::Open);
    }

    #[test]
    fn test_handle_non_post_task_action_ignored() {
        let tasks: Arc<Mutex<Vec<TaskInfo>>> = Arc::new(Mutex::new(vec![]));
        let tasks_clone = tasks.clone();
        let on_task = move |t: TaskInfo| { tasks_clone.lock().unwrap().push(t); };

        let msg = mock_tm_message("accept_task", "42", "1000000ujuno", "12345");
        handle_message(&msg, "juno1contract", "juno-1", "juno1verifier", &on_task).unwrap();

        assert!(tasks.lock().unwrap().is_empty());
    }

    #[test]
    fn test_handle_no_events_in_message() {
        let tasks: Arc<Mutex<Vec<TaskInfo>>> = Arc::new(Mutex::new(vec![]));
        let tasks_clone = tasks.clone();
        let on_task = move |t: TaskInfo| { tasks_clone.lock().unwrap().push(t); };

        let msg = r#"{"result":{}}"#;
        handle_message(msg, "juno1contract", "juno-1", "juno1verifier", &on_task).unwrap();

        assert!(tasks.lock().unwrap().is_empty());
    }

    #[test]
    fn test_handle_missing_task_id_ignored() {
        let tasks: Arc<Mutex<Vec<TaskInfo>>> = Arc::new(Mutex::new(vec![]));
        let tasks_clone = tasks.clone();
        let on_task = move |t: TaskInfo| { tasks_clone.lock().unwrap().push(t); };

        let msg = serde_json::json!({
            "result": {
                "data": { "value": { "TxResult": { "height": "100", "result": {
                    "events": [{"type": "wasm", "attributes": [
                        {"key": "action", "value": "post_task"},
                        {"key": "reward", "value": "500ujuno"}
                    ]}]
                }}}}
            }
        }).to_string();

        handle_message(&msg, "juno1contract", "juno-1", "juno1verifier", &on_task).unwrap();
        assert!(tasks.lock().unwrap().is_empty());
    }

    #[test]
    fn test_handle_non_wasm_event_ignored() {
        let tasks: Arc<Mutex<Vec<TaskInfo>>> = Arc::new(Mutex::new(vec![]));
        let tasks_clone = tasks.clone();
        let on_task = move |t: TaskInfo| { tasks_clone.lock().unwrap().push(t); };

        let msg = serde_json::json!({
            "result": {
                "data": { "value": { "TxResult": { "height": "100", "result": {
                    "events": [{"type": "transfer", "attributes": [
                        {"key": "recipient", "value": "juno1abc"}
                    ]}]
                }}}}
            }
        }).to_string();

        handle_message(&msg, "juno1contract", "juno-1", "juno1verifier", &on_task).unwrap();
        assert!(tasks.lock().unwrap().is_empty());
    }

    /// End-to-end pipeline (offline, deterministic): a raw Tendermint `post_task`
    /// message is parsed into a [`TaskInfo`] and then built + signed into the exact
    /// kind-38402 Nostr event the daemon would publish. This exercises the full
    /// subscriber -> publisher path the live bridge runs, without needing a chain
    /// websocket or live relays — so the only thing the real e2e run adds is secrets.
    #[test]
    fn test_end_to_end_tm_message_to_signed_kind_38402() {
        use crate::event::{build_task_event, KIND_TASK_DISCOVERY};

        // 1. Parse a realistic uni-7 post_task event through the subscriber.
        let captured: Arc<Mutex<Option<TaskInfo>>> = Arc::new(Mutex::new(None));
        let captured_clone = captured.clone();
        let on_task = move |t: TaskInfo| { *captured_clone.lock().unwrap() = Some(t); };

        let msg = mock_tm_message("post_task", "7", "2500000ujunox", "14254800");
        handle_message(&msg, "juno1taskledger", "uni-7", "juno1verifier", &on_task).unwrap();

        let task = captured.lock().unwrap().clone().expect("post_task parsed into TaskInfo");
        assert_eq!(task.task_id, 7);
        assert_eq!(task.chain_id, "uni-7");

        // 2. Build + sign the Nostr event the publisher would broadcast.
        let keys = nostr_sdk::Keys::generate();
        let built = build_task_event(&task, &keys).unwrap();
        let event: serde_json::Value = serde_json::from_str(&built.event_json).unwrap();

        // 3. Assert the on-wire shape agents subscribe to.
        assert_eq!(event["kind"], serde_json::json!(KIND_TASK_DISCOVERY));

        let tags = event["tags"].as_array().unwrap();
        let d_tag = tags.iter().find(|t| t[0] == "d").expect("replaceable d tag present");
        assert_eq!(d_tag[1], "uni-7:juno1taskledger:7");

        // Signed: non-empty signature + pubkey populated.
        assert!(event["sig"].as_str().map(|s| !s.is_empty()).unwrap_or(false));
        assert!(event["pubkey"].as_str().map(|s| !s.is_empty()).unwrap_or(false));

        // Content round-trips the task the DAO posted.
        let content: serde_json::Value =
            serde_json::from_str(event["content"].as_str().unwrap()).unwrap();
        assert_eq!(content["task_id"], serde_json::json!(7));
        assert_eq!(content["status"], "open");
        assert_eq!(content["reward"], "2500000ujunox");
    }
}
