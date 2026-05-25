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
