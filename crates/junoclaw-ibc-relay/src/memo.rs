//! ICS-20 memo wire format for JunoClaw cross-chain task operations.
//!
//! The memo is placed in the ICS-20 `MsgTransfer.memo` field.
//! PFM reads the `wasm` key and forwards execution to the `ibc-task-host`
//! contract on Juno.

use serde::{Deserialize, Serialize};

/// Top-level memo structure that PFM/wasm middleware reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JunoClawMemo {
    pub wasm: WasmMemo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmMemo {
    /// `ibc-task-host` contract address on Juno
    pub contract: String,
    pub msg: WasmMsg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmMsg {
    pub junoclaw_v1: JunoClawOp,
}

/// The four operations supported in v2.1.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JunoClawOp {
    AcceptTask(AcceptTask),
    SubmitProof(SubmitProof),
    ReclaimExpired(ReclaimExpired),
    /// Cross-chain autonomous Junoswap swap via ICS-20 + PFM.
    /// The agent sends tokens from the origin chain; PFM routes to
    /// `junoswap-pair` on Juno for atomic swap execution.
    Swap(SwapOp),
}

/// Agent registers as worker for an open task.
///
/// The agent's origin chain + address are recorded so the settlement
/// ICS-20 reverse transfer can reach them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptTask {
    pub task_id: u64,
    /// Juno address derived from the agent's key (for on-chain identity binding)
    pub agent_addr: String,
    /// Source chain ID (e.g. "osmosis-1")
    pub agent_origin_chain: String,
    /// Agent's native address on the origin chain (e.g. "osmo1...")
    pub agent_origin_addr: String,
}

/// Agent submits work proof. Triggers `zk-verifier::VerifyProof` on Juno.
///
/// If verification fails, the IBC packet acknowledgment is `Err` and
/// the relayer returns the funds to the agent's origin chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitProof {
    pub task_id: u64,
    /// Base64-encoded Groth16 proof bytes
    pub proof_b64: String,
    /// Base64-encoded serialized public inputs
    pub public_inputs_b64: String,
    /// Agent's origin chain + address for settlement routing
    pub agent_origin_chain: String,
    pub agent_origin_addr: String,
}

/// DAO reclaims escrow on a task that has passed its deadline without a proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReclaimExpired {
    pub task_id: u64,
    /// DAO's origin chain + address
    pub dao_origin_chain: String,
    pub dao_origin_addr: String,
}

/// Cross-chain swap operation — an agent on another Cosmos chain sends
/// an ICS-20 transfer with swap instructions in the memo. PFM routes
/// the tokens to the `junoswap-pair` contract for atomic execution.
///
/// The swap return is routed back via ICS-20 reverse transfer to the
/// agent's origin address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapOp {
    /// The junoswap-pair contract address on Juno
    pub pair_contract: String,
    /// Offer denom (IBC denom on Juno, e.g. "ibc/27394FB092D2ECCD56123C74F36E4C1F926001CE...")
    pub offer_denom: String,
    /// Minimum return amount (slippage protection)
    pub min_return: String,
    /// Agent's origin chain ID for settlement routing
    pub agent_origin_chain: String,
    /// Agent's address on the origin chain for return transfer
    pub agent_origin_addr: String,
    /// Optional: maximum price impact percentage allowed (basis points)
    pub max_price_impact_bps: Option<u32>,
}

impl JunoClawMemo {
    /// Build an AcceptTask memo for a given `ibc-task-host` address.
    pub fn accept_task(host_contract: impl Into<String>, op: AcceptTask) -> Self {
        Self {
            wasm: WasmMemo {
                contract: host_contract.into(),
                msg: WasmMsg {
                    junoclaw_v1: JunoClawOp::AcceptTask(op),
                },
            },
        }
    }

    /// Build a SubmitProof memo.
    pub fn submit_proof(host_contract: impl Into<String>, op: SubmitProof) -> Self {
        Self {
            wasm: WasmMemo {
                contract: host_contract.into(),
                msg: WasmMsg {
                    junoclaw_v1: JunoClawOp::SubmitProof(op),
                },
            },
        }
    }

    /// Build a ReclaimExpired memo.
    pub fn reclaim_expired(host_contract: impl Into<String>, op: ReclaimExpired) -> Self {
        Self {
            wasm: WasmMemo {
                contract: host_contract.into(),
                msg: WasmMsg {
                    junoclaw_v1: JunoClawOp::ReclaimExpired(op),
                },
            },
        }
    }

    /// Build a cross-chain Junoswap swap memo.
    ///
    /// The `host_contract` here is the `ibc-task-host` which forwards the swap
    /// instruction to the `junoswap-pair` contract. The ICS-20 transfer carries
    /// the offer tokens; the swap return is routed back via ICS-20 reverse transfer.
    pub fn swap(host_contract: impl Into<String>, op: SwapOp) -> Self {
        Self {
            wasm: WasmMemo {
                contract: host_contract.into(),
                msg: WasmMsg {
                    junoclaw_v1: JunoClawOp::Swap(op),
                },
            },
        }
    }

    /// Serialize to JSON string for embedding in ICS-20 transfer memo.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accept_task_memo_roundtrip() {
        let memo = JunoClawMemo::accept_task(
            "juno1hosttask...",
            AcceptTask {
                task_id: 42,
                agent_addr: "juno1agent...".into(),
                agent_origin_chain: "osmosis-1".into(),
                agent_origin_addr: "osmo1agent...".into(),
            },
        );

        let json = memo.to_json().unwrap();
        assert!(json.contains("\"accept_task\""));
        assert!(json.contains("\"task_id\":42"));
        assert!(json.contains("osmosis-1"));

        let parsed: JunoClawMemo = serde_json::from_str(&json).unwrap();
        match &parsed.wasm.msg.junoclaw_v1 {
            JunoClawOp::AcceptTask(op) => {
                assert_eq!(op.task_id, 42);
                assert_eq!(op.agent_origin_chain, "osmosis-1");
            }
            _ => panic!("Expected AcceptTask"),
        }
    }

    #[test]
    fn test_submit_proof_memo_contains_proof_b64() {
        let memo = JunoClawMemo::submit_proof(
            "juno1hosttask...",
            SubmitProof {
                task_id: 42,
                proof_b64: "dGVzdA==".into(),
                public_inputs_b64: "dGVzdA==".into(),
                agent_origin_chain: "osmosis-1".into(),
                agent_origin_addr: "osmo1agent...".into(),
            },
        );

        let json = memo.to_json().unwrap();
        assert!(json.contains("submit_proof"));
        assert!(json.contains("dGVzdA=="));
    }

    #[test]
    fn test_swap_memo_roundtrip() {
        let memo = JunoClawMemo::swap(
            "juno1hosttask...",
            SwapOp {
                pair_contract: "juno1pair...".into(),
                offer_denom: "ibc/27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2".into(),
                min_return: "1000000".into(),
                agent_origin_chain: "osmosis-1".into(),
                agent_origin_addr: "osmo1agent...".into(),
                max_price_impact_bps: Some(100), // 1%
            },
        );

        let json = memo.to_json().unwrap();
        assert!(json.contains("\"swap\""));
        assert!(json.contains("\"pair_contract\""));
        assert!(json.contains("\"min_return\":\"1000000\""));
        assert!(json.contains("osmosis-1"));

        let parsed: JunoClawMemo = serde_json::from_str(&json).unwrap();
        match &parsed.wasm.msg.junoclaw_v1 {
            JunoClawOp::Swap(op) => {
                assert_eq!(op.min_return, "1000000");
                assert_eq!(op.max_price_impact_bps, Some(100));
                assert_eq!(op.agent_origin_chain, "osmosis-1");
            }
            _ => panic!("Expected Swap"),
        }
    }

    #[test]
    fn test_memo_fits_in_ics20_limit() {
        // ICS-20 memo is typically limited to 32KB by relayers.
        // A typical AcceptTask memo is ~300 bytes; SubmitProof with a
        // ~500-byte proof is ~800 bytes. Both well within limits.
        let memo = JunoClawMemo::submit_proof(
            "juno1hosttask...",
            SubmitProof {
                task_id: 999,
                proof_b64: "A".repeat(700), // ~500 bytes of base64
                public_inputs_b64: "B".repeat(100),
                agent_origin_chain: "osmosis-1".into(),
                agent_origin_addr: "osmo1agent...".into(),
            },
        );

        let json = memo.to_json().unwrap();
        assert!(json.len() < 32 * 1024, "Memo too large: {} bytes", json.len());
    }
}
