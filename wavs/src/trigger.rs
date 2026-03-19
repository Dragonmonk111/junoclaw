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
    /// Verify a swap event from Junoswap v2 pair contract.
    /// Triggered by wasm-swap events — checks price correctness and manipulation.
    SwapVerify {
        pair: String,
        offer_asset: String,
        offer_amount: String,
        return_asset: String,
        return_amount: String,
        spread_amount: String,
        fee_amount: String,
        reserve_a: String,
        reserve_b: String,
    },
    /// Check pool health for a Junoswap v2 pair.
    /// Triggered by wasm-provide_liquidity or wasm-withdraw_liquidity events.
    PoolHealthCheck {
        pair: String,
        reserve_a: String,
        reserve_b: String,
        total_lp_shares: String,
        action: String,
    },
    /// Produce a TEE-attested price snapshot for a Junoswap v2 pair.
    /// Can be triggered on schedule or by large swaps.
    PriceAttestation {
        pair: String,
        token_a: String,
        token_b: String,
        reserve_a: String,
        reserve_b: String,
    },

    // ── Chain Intelligence Module (7-10) ──

    /// 7. Governance surveillance: analyze voting/proposal patterns for anomalies.
    /// Triggered by wasm events from agent-company (create_proposal, cast_vote, execute_proposal).
    GovernanceWatch {
        proposal_id: String,
        action_type: String,
        actor: String,
        proposal_kind: String,
        yes_weight: String,
        no_weight: String,
        total_voted_weight: String,
        total_weight: String,
        status: String,
        voting_deadline_block: String,
        block_height: String,
    },
    /// 8. Migration watchdog: verify contract migrations against DAO approval.
    /// Triggered by wasm-code_upgrade_migrate events from agent-company.
    MigrationWatch {
        proposal_id: String,
        contract_addr: String,
        new_code_id: String,
        action_index: String,
        title: String,
    },
    /// 9. Whale alert: flag large swap activity that could impact pool stability.
    /// Triggered by wasm-swap events where offer_amount exceeds threshold % of reserves.
    WhaleAlert {
        pair: String,
        sender: String,
        offer_asset: String,
        offer_amount: String,
        return_asset: String,
        return_amount: String,
        spread_amount: String,
        fee_amount: String,
        reserve_a: String,
        reserve_b: String,
        block_height: String,
        timestamp: String,
    },
    /// 10. IBC channel health: monitor channel state and packet relay quality.
    /// Triggered via WavsPush or periodic RPC query.
    IbcHealthCheck {
        channel_id: String,
        port_id: String,
        counterparty_channel: String,
        connection_id: String,
        state: String,
        packets_sent: String,
        packets_recv: String,
        packets_timeout: String,
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
        // Junoswap v2: swap event — verify price correctness
        t if t.contains("swap") => {
            Ok(VerificationTask::SwapVerify {
                pair: get_attr("pair").unwrap_or_default(),
                offer_asset: get_attr("offer_asset").unwrap_or_default(),
                offer_amount: get_attr("offer_amount").unwrap_or_default(),
                return_asset: get_attr("return_asset").unwrap_or_default(),
                return_amount: get_attr("return_amount").unwrap_or_default(),
                spread_amount: get_attr("spread_amount").unwrap_or_default(),
                fee_amount: get_attr("fee_amount").unwrap_or_default(),
                reserve_a: get_attr("reserve_a").unwrap_or_default(),
                reserve_b: get_attr("reserve_b").unwrap_or_default(),
            })
        }
        // Junoswap v2: liquidity event — check pool health
        t if t.contains("provide_liquidity") || t.contains("withdraw_liquidity") => {
            Ok(VerificationTask::PoolHealthCheck {
                pair: get_attr("pair").unwrap_or_default(),
                reserve_a: get_attr("reserve_a").unwrap_or_default(),
                reserve_b: get_attr("reserve_b").unwrap_or_default(),
                total_lp_shares: get_attr("lp_shares").unwrap_or_default(),
                action: event_type.to_string(),
            })
        }

        // ── Chain Intelligence Module (7-10) ──

        // 8. Migration watchdog — triggered by code_upgrade_migrate events
        t if t.contains("code_upgrade_migrate") => {
            Ok(VerificationTask::MigrationWatch {
                proposal_id: get_attr("proposal_id").unwrap_or_default(),
                contract_addr: get_attr("contract_addr").unwrap_or_default(),
                new_code_id: get_attr("new_code_id").unwrap_or_default(),
                action_index: get_attr("action_index").unwrap_or_default(),
                title: get_attr("title").unwrap_or_default(),
            })
        }

        // 10. IBC health — triggered by IBC-related events or manual push
        t if t.contains("channel_open") || t.contains("recv_packet") || t.contains("ibc_health") => {
            Ok(VerificationTask::IbcHealthCheck {
                channel_id: get_attr("channel_id").or_else(|_| get_attr("packet_channel")).unwrap_or_default(),
                port_id: get_attr("port_id").or_else(|_| get_attr("packet_port")).unwrap_or_default(),
                counterparty_channel: get_attr("counterparty_channel_id").unwrap_or_default(),
                connection_id: get_attr("connection_id").unwrap_or_default(),
                state: get_attr("state").unwrap_or_else(|_| "unknown".to_string()),
                packets_sent: get_attr("packets_sent").unwrap_or_else(|_| "0".to_string()),
                packets_recv: get_attr("packets_recv").unwrap_or_else(|_| "0".to_string()),
                packets_timeout: get_attr("packets_timeout").unwrap_or_else(|_| "0".to_string()),
            })
        }

        // 7. Governance watch — triggered by standard wasm events with governance actions
        // This MUST come after all specific event_type matches since "wasm" is generic.
        t if t == "wasm" || t == "execute" => {
            let action = get_attr("action").unwrap_or_default();
            match action.as_str() {
                "create_proposal" | "cast_vote" | "execute_proposal" | "expire_proposal" => {
                    Ok(VerificationTask::GovernanceWatch {
                        proposal_id: get_attr("proposal_id").unwrap_or_default(),
                        action_type: action,
                        actor: get_attr("voter")
                            .or_else(|_| get_attr("_contract_address"))
                            .unwrap_or_default(),
                        proposal_kind: get_attr("kind").unwrap_or_default(),
                        yes_weight: get_attr("yes_weight").unwrap_or_else(|_| "0".to_string()),
                        no_weight: get_attr("no_weight").unwrap_or_else(|_| "0".to_string()),
                        total_voted_weight: get_attr("total_voted_weight").unwrap_or_else(|_| "0".to_string()),
                        total_weight: get_attr("total_weight").unwrap_or_else(|_| "10000".to_string()),
                        status: get_attr("status").unwrap_or_default(),
                        voting_deadline_block: get_attr("voting_deadline_block").unwrap_or_default(),
                        block_height: get_attr("block_height").unwrap_or_default(),
                    })
                }
                _ => Err(anyhow!("Non-governance wasm event action: {}", action)),
            }
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
