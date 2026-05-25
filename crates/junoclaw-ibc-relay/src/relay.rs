//! Core relay logic — builds and validates ICS-20 + PFM memo transactions.

use anyhow::Result;
use tracing::{info, warn};

use crate::{
    config::RelayConfig,
    error::RelayError,
    memo::{AcceptTask, JunoClawMemo, ReclaimExpired, SubmitProof, SwapOp},
};

const MAX_MEMO_BYTES: usize = 32 * 1024;  // 32 KB (conservative ICS-20 limit)
const MAX_PROOF_B64_BYTES: usize = 700;   // Groth16 BN254 proof is ~500 bytes

/// Relay client. Stateless — call methods per-operation.
pub struct IbcTaskRelay {
    config: RelayConfig,
}

impl IbcTaskRelay {
    pub fn new(config: RelayConfig) -> Self {
        Self { config }
    }

    /// Build the ICS-20 memo for an AcceptTask operation.
    ///
    /// Returns the memo JSON string ready to embed in a `MsgTransfer`.
    pub fn build_accept_task_memo(&self, op: AcceptTask) -> Result<String, RelayError> {
        let memo = JunoClawMemo::accept_task(&self.config.host_contract, op);
        let json = memo.to_json()?;
        self.validate_memo_size(&json)?;
        info!("AcceptTask memo built: {} bytes", json.len());
        Ok(json)
    }

    /// Build the ICS-20 memo for a SubmitProof operation.
    ///
    /// Validates that the proof fits within the memo size limit.
    pub fn build_submit_proof_memo(&self, op: SubmitProof) -> Result<String, RelayError> {
        if op.proof_b64.len() > MAX_PROOF_B64_BYTES {
            return Err(RelayError::ProofTooLarge { size: op.proof_b64.len() });
        }
        let memo = JunoClawMemo::submit_proof(&self.config.host_contract, op);
        let json = memo.to_json()?;
        self.validate_memo_size(&json)?;
        info!("SubmitProof memo built: {} bytes", json.len());
        Ok(json)
    }

    /// Build the ICS-20 memo for a ReclaimExpired operation.
    pub fn build_reclaim_expired_memo(&self, op: ReclaimExpired) -> Result<String, RelayError> {
        let memo = JunoClawMemo::reclaim_expired(&self.config.host_contract, op);
        let json = memo.to_json()?;
        self.validate_memo_size(&json)?;
        info!("ReclaimExpired memo built: {} bytes", json.len());
        Ok(json)
    }

    /// Check whether a task deadline is reachable given IBC latency.
    ///
    /// IBC relay takes ~30-120 seconds in practice. If the task deadline is
    /// within `grace_blocks` of the current height, warn the caller.
    pub fn check_deadline_reachable(
        &self,
        task_id: u64,
        task_deadline_block: u64,
        current_block: u64,
    ) -> Result<(), RelayError> {
        let blocks_remaining = task_deadline_block.saturating_sub(current_block);
        if blocks_remaining < self.config.grace_blocks {
            warn!(
                "Task {task_id}: only {blocks_remaining} blocks until deadline \
                (grace window = {}). IBC relay may miss the deadline.",
                self.config.grace_blocks
            );
            return Err(RelayError::DeadlineTooClose { task_id });
        }
        Ok(())
    }

    /// Build the ICS-20 memo for a cross-chain Junoswap swap operation.
    ///
    /// The agent's ICS-20 transfer carries the offer tokens; the memo instructs
    /// `ibc-task-host` to forward execution to the `junoswap-pair` contract.
    /// The swap return is routed back via ICS-20 reverse transfer.
    pub fn build_swap_memo(&self, op: SwapOp) -> Result<String, RelayError> {
        // Validate min_return is parseable as u128
        op.min_return
            .parse::<u128>()
            .map_err(|_| RelayError::InvalidSwapAmount {
                field: "min_return".into(),
                value: op.min_return.clone(),
            })?;

        let memo = JunoClawMemo::swap(&self.config.host_contract, op);
        let json = memo.to_json()?;
        self.validate_memo_size(&json)?;
        info!("Swap memo built: {} bytes", json.len());
        Ok(json)
    }

    /// Log the memo that would be sent — useful for dry-run / debugging.
    pub fn preview_memo(&self, memo_json: &str) {
        info!("IBC memo preview ({} bytes):\n{memo_json}", memo_json.len());
    }

    fn validate_memo_size(&self, json: &str) -> Result<(), RelayError> {
        if json.len() > MAX_MEMO_BYTES {
            return Err(RelayError::MemoTooLarge { size: json.len() });
        }
        Ok(())
    }

    pub fn config(&self) -> &RelayConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memo::AcceptTask;

    fn test_relay() -> IbcTaskRelay {
        IbcTaskRelay::new(RelayConfig {
            juno_grpc: "https://test".into(),
            host_contract: "juno1host...".into(),
            channel_to_juno: "channel-0".into(),
            source_grpc: "https://osmosis-test".into(),
            source_chain_id: "osmosis-1".into(),
            transfer_denom: "uosmo".into(),
            grace_blocks: 14400,
        })
    }

    #[test]
    fn test_accept_task_memo_valid() {
        let relay = test_relay();
        let memo = relay.build_accept_task_memo(AcceptTask {
            task_id: 42,
            agent_addr: "juno1agent...".into(),
            agent_origin_chain: "osmosis-1".into(),
            agent_origin_addr: "osmo1agent...".into(),
        }).unwrap();
        assert!(memo.contains("accept_task"));
        assert!(memo.len() < 32 * 1024);
    }

    #[test]
    fn test_submit_proof_too_large_rejected() {
        let relay = test_relay();
        let result = relay.build_submit_proof_memo(SubmitProof {
            task_id: 42,
            proof_b64: "A".repeat(800), // exceeds MAX_PROOF_B64_BYTES
            public_inputs_b64: "B".repeat(100),
            agent_origin_chain: "osmosis-1".into(),
            agent_origin_addr: "osmo1agent...".into(),
        });
        assert!(matches!(result, Err(RelayError::ProofTooLarge { .. })));
    }

    #[test]
    fn test_deadline_check_too_close() {
        let relay = test_relay();
        let result = relay.check_deadline_reachable(42, 100, 1); // only 99 blocks remaining
        assert!(matches!(result, Err(RelayError::DeadlineTooClose { .. })));
    }

    #[test]
    fn test_deadline_check_ok() {
        let relay = test_relay();
        let result = relay.check_deadline_reachable(42, 100_000, 1); // 99999 blocks remaining
        assert!(result.is_ok());
    }

    #[test]
    fn test_swap_memo_valid() {
        use crate::memo::SwapOp;
        let relay = test_relay();
        let memo = relay.build_swap_memo(SwapOp {
            pair_contract: "juno1pair...".into(),
            offer_denom: "ibc/27394F...".into(),
            min_return: "1000000".into(),
            agent_origin_chain: "osmosis-1".into(),
            agent_origin_addr: "osmo1agent...".into(),
            max_price_impact_bps: Some(50),
        }).unwrap();
        assert!(memo.contains("swap"));
        assert!(memo.contains("1000000"));
    }

    #[test]
    fn test_swap_memo_invalid_amount_rejected() {
        use crate::memo::SwapOp;
        let relay = test_relay();
        let result = relay.build_swap_memo(SwapOp {
            pair_contract: "juno1pair...".into(),
            offer_denom: "ibc/27394F...".into(),
            min_return: "not_a_number".into(),
            agent_origin_chain: "osmosis-1".into(),
            agent_origin_addr: "osmo1agent...".into(),
            max_price_impact_bps: None,
        });
        assert!(matches!(result, Err(RelayError::InvalidSwapAmount { .. })));
    }
}
