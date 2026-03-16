//! Trigger decoding for JunoClaw WAVS component.
//!
//! Handles incoming trigger data from WAVS runtime, supporting:
//! - Cosmos contract events (production: from agent-company on uni-7)
//! - Raw data (local testing via `wasi-exec`)

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::bindings::wavs::operator::output::WasmResponse;
use crate::bindings::wavs::types::events::TriggerData;

// ──────────────────────────────────────────────
// Task types matching our on-chain contract events
// ──────────────────────────────────────────────

/// The type of verification work the component should perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationTask {
    /// Fetch external data, hash it, return attestation.
    /// Used by WavsPush proposals for off-chain verification.
    DataVerify {
        task_id: u64,
        data_sources: Vec<String>,
        verification_criteria: String,
    },
    /// Fetch drand beacon randomness for sortition.
    /// Used by SortitionRequest proposals when NOIS is unavailable.
    DrandRandomness {
        job_id: String,
        drand_round: Option<u64>,
    },
    /// Verify outcome market resolution data.
    /// Used by OutcomeResolve proposals.
    OutcomeVerify {
        market_id: u64,
        question: String,
        resolution_criteria: String,
    },
}

/// Result produced by the component after verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// The original task type
    pub task_type: String,
    /// Hash of the fetched/computed data (SHA-256 hex)
    pub data_hash: String,
    /// The attestation identifier (component digest + data hash)
    pub attestation_hash: String,
    /// Raw output data (JSON-encoded verification details)
    pub output: serde_json::Value,
    /// Unix timestamp of when verification was performed
    pub timestamp: u64,
}

// ──────────────────────────────────────────────
// Destination routing
// ──────────────────────────────────────────────

/// Where the component output should be routed.
pub enum Destination {
    /// Submit result to Cosmos chain (production)
    Cosmos,
    /// Return raw output for CLI testing
    CliOutput,
}

// ──────────────────────────────────────────────
// Trigger decoding
// ──────────────────────────────────────────────

/// Decode incoming trigger data into a VerificationTask.
///
/// In production, the trigger comes from a Cosmos contract event
/// emitted by agent-company when a WavsPush/SortitionRequest/OutcomeResolve
/// proposal is executed.
///
/// For local testing, raw JSON bytes are passed directly.
pub fn decode_trigger(trigger_data: TriggerData) -> Result<(VerificationTask, Destination)> {
    match trigger_data {
        // Cosmos contract event trigger (production)
        // The event has typed attributes: list<(key, value)>
        TriggerData::CosmosContractEvent(event) => {
            let attrs = &event.event.attributes;
            let event_type = &event.event.ty;

            // Parse task from event attributes based on event type
            let task = parse_cosmos_event(event_type, attrs)?;
            Ok((task, Destination::Cosmos))
        }
        // Raw data for local testing (JSON bytes)
        TriggerData::Raw(data) => {
            let task: VerificationTask = serde_json::from_slice(&data)
                .map_err(|e| anyhow!("Failed to decode raw trigger data: {}", e))?;
            Ok((task, Destination::CliOutput))
        }
        _ => Err(anyhow!("Unsupported trigger data type")),
    }
}

/// Parse a Cosmos wasm event's attributes into a VerificationTask.
///
/// CosmWasm events emit attributes as key-value string pairs.
/// Our agent-company contract emits these event types:
/// - `wasm-wavs_push`: task_description, execution_tier, escrow_amount
/// - `wasm-sortition_request`: job_id, count, purpose
/// - `wasm-outcome_create`: market_id, question, resolution_criteria
fn parse_cosmos_event(
    event_type: &str,
    attrs: &[(String, String)],
) -> Result<VerificationTask> {
    let get_attr = |key: &str| -> Result<String> {
        attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| anyhow!("Missing attribute '{}' in event '{}'", key, event_type))
    };

    match event_type {
        // WavsPush proposal executed — verify off-chain data
        t if t.contains("wavs_push") => {
            let task_description = get_attr("task_description")
                .unwrap_or_else(|_| "unknown".to_string());
            // data_sources are comma-separated URLs in the attribute
            let sources_str = get_attr("data_sources").unwrap_or_default();
            let data_sources: Vec<String> = if sources_str.is_empty() {
                vec![]
            } else {
                sources_str.split(',').map(|s| s.trim().to_string()).collect()
            };

            Ok(VerificationTask::DataVerify {
                task_id: get_attr("task_id")
                    .and_then(|v| v.parse().map_err(|e| anyhow!("{}", e)))
                    .unwrap_or(0),
                data_sources,
                verification_criteria: task_description,
            })
        }
        // SortitionRequest — fetch drand randomness
        t if t.contains("sortition_request") => {
            let job_id = get_attr("job_id")
                .unwrap_or_else(|_| "unknown".to_string());
            let drand_round = get_attr("drand_round")
                .ok()
                .and_then(|v| v.parse().ok());

            Ok(VerificationTask::DrandRandomness {
                job_id,
                drand_round,
            })
        }
        // OutcomeCreate — verify resolution criteria
        t if t.contains("outcome") => {
            Ok(VerificationTask::OutcomeVerify {
                market_id: get_attr("market_id")
                    .and_then(|v| v.parse().map_err(|e| anyhow!("{}", e)))
                    .unwrap_or(0),
                question: get_attr("question").unwrap_or_default(),
                resolution_criteria: get_attr("resolution_criteria").unwrap_or_default(),
            })
        }
        _ => Err(anyhow!("Unknown Cosmos event type: {}", event_type)),
    }
}

/// Encode verification result into a WasmResponse for submission.
pub fn encode_response(result: &VerificationResult, _dest: &Destination) -> Option<WasmResponse> {
    let payload = serde_json::to_vec(result).ok()?;
    Some(WasmResponse {
        payload,
        ordering: None,
    })
}
