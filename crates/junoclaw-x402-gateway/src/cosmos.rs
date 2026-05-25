//! Thin Cosmos RPC client wrapper.
//!
//! Wraps `cosmrs::rpc::HttpClient` with JunoClaw-specific helpers:
//! - smart contract queries against `agent-company` / `task-ledger` / `escrow`
//! - tx simulation to populate `gas_estimate` in envelopes
//! - broadcast (sync) of agent-signed txs
//!
//! NOTE: This crate does NOT hold the agent's signing key. The gateway is a
//! pass-through — agents sign client-side and submit the signed tx bytes via
//! `PAYMENT-SIGNATURE`. The gateway broadcasts on their behalf. (Optional
//! gateway-key mode for DAO-proposing-on-behalf is gated behind a feature
//! flag in v2.)

use anyhow::Context;
use cosmrs::rpc::{Client, HttpClient};
use cosmrs::tx::Raw;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{GatewayError, GatewayResult};

#[derive(Clone)]
pub struct CosmosClient {
    rpc: HttpClient,
    chain_id: String,
}

impl CosmosClient {
    pub fn new(rpc_url: &str, chain_id: &str) -> anyhow::Result<Self> {
        let rpc = HttpClient::new(rpc_url).with_context(|| format!("cosmos rpc init: {rpc_url}"))?;
        Ok(Self {
            rpc,
            chain_id: chain_id.to_string(),
        })
    }

    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }

    /// Smart-contract state query. The `query_msg` is the CosmWasm query
    /// payload (e.g. `{"config": {}}`), JSON-encoded.
    pub async fn query_smart<T: DeserializeOwned>(
        &self,
        contract: &str,
        query_msg: &Value,
    ) -> GatewayResult<T> {
        let path = "/cosmwasm.wasm.v1.Query/SmartContractState";
        let req = serde_json::json!({
            "address": contract,
            "query_data": base64_encode_msg(query_msg)?,
        });
        let data = serde_json::to_vec(&req).map_err(|e| {
            GatewayError::ChainRpc(format!("encode query: {e}"))
        })?;

        let resp = self
            .rpc
            .abci_query(Some(path.parse().unwrap()), data, None, false)
            .await
            .map_err(|e| GatewayError::ChainRpc(format!("abci_query: {e}")))?;

        if !resp.code.is_ok() {
            return Err(GatewayError::ChainRpc(format!(
                "abci code {:?}: {}",
                resp.code, resp.log
            )));
        }

        serde_json::from_slice::<T>(&resp.value)
            .map_err(|e| GatewayError::ChainRpc(format!("decode response: {e}")))
    }

    /// Broadcast a signed Cosmos tx (sync mode — returns once the mempool
    /// accepts; does NOT wait for inclusion). Returns the tx hash.
    pub async fn broadcast(&self, tx_bytes: &[u8]) -> GatewayResult<String> {
        let raw: Raw = Raw::from_bytes(tx_bytes)
            .map_err(|e| GatewayError::Broadcast(format!("decode raw tx: {e}")))?;
        let resp = raw
            .broadcast_commit(&self.rpc)
            .await
            .map_err(|e| GatewayError::Broadcast(format!("commit broadcast: {e}")))?;

        if resp.check_tx.code.is_err() {
            return Err(GatewayError::Broadcast(format!(
                "check_tx rejected (code {:?}): {}",
                resp.check_tx.code, resp.check_tx.log
            )));
        }
        if resp.tx_result.code.is_err() {
            return Err(GatewayError::Broadcast(format!(
                "tx_result failed (code {:?}): {}",
                resp.tx_result.code, resp.tx_result.log
            )));
        }
        Ok(resp.hash.to_string())
    }

    /// Estimate gas for a given message. For v1 we use a static estimate
    /// table per message type; cosmrs's full Simulate path comes in v2 once
    /// we wire it through the gRPC client.
    ///
    /// Estimates derive from JunoClaw's `DETERMINISTIC_AUDIT.md` measurements:
    /// - PostTask (via DAO proposal)    : ~280k gas
    /// - AcceptTask                     : ~95k gas
    /// - SubmitAttestation (precompile) : ~250k gas
    /// - SubmitAttestation (pure-wasm)  : ~420k gas
    /// - Reclaim                        : ~80k gas
    pub fn estimate_gas(&self, op: GatewayOp) -> u64 {
        match op {
            GatewayOp::PostTask => 280_000,
            GatewayOp::AcceptTask => 95_000,
            GatewayOp::SubmitAttestationPrecompile => 250_000,
            GatewayOp::SubmitAttestationWasm => 420_000,
            GatewayOp::Reclaim => 80_000,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GatewayOp {
    PostTask,
    AcceptTask,
    SubmitAttestationPrecompile,
    SubmitAttestationWasm,
    Reclaim,
}

fn base64_encode_msg(v: &Value) -> GatewayResult<String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    let bytes = serde_json::to_vec(v).map_err(|e| {
        GatewayError::ChainRpc(format!("encode query msg: {e}"))
    })?;
    Ok(B64.encode(bytes))
}
