//! Bridge — submits batches to the coordination-settler contract on Juno.
//!
//! This module handles the actual on-chain transaction submission.
//! It builds a CosmWasm ExecuteMsg::SubmitBatch, signs it with the
//! relayer's key, and broadcasts it to the Juno RPC endpoint.

use anyhow::Result;
use tracing::info;

use crate::watcher::FinalizedBatch;

/// Submit a finalized batch to the coordination-settler contract.
///
/// TODO: Implement using cosmrs or by shelling out to the junoclaw MCP
/// wallet store. The current implementation is a scaffold that logs
/// the intended submission.
pub async fn submit_batch(
    rpc_endpoint: &str,
    contract_addr: &str,
    relayer_key: &str,
    batch: &FinalizedBatch,
) -> Result<()> {
    let cert_hex = hex::encode(&batch.certificate);
    let msg_hash_hex = hex::encode(&batch.messages_hash);

    info!(
        "Built SubmitBatch tx for contract {} (height={}, cert={}..., msg_hash={})",
        contract_addr, batch.commonware_height, &cert_hex[..16], msg_hash_hex
    );

    // TODO: Sign and broadcast using:
    // Option A: cosmrs (Rust native Cosmos SDK client)
    //   - Parse mnemonic → signing key
    //   - Build MsgExecuteContract
    //   - Sign + broadcast to rpc_endpoint
    //
    // Option B: Shell out to junoclaw MCP wallet store
    //   - Use the encrypted WalletStore from junoclaw/mcp/dist/wallet
    //   - Sign via the MCP server's tx-builder tool
    //
    // Option C: Use junoclaw CLI if it supports contract execution
    //
    // For now, we just log and return Ok to keep the watcher loop running.
    // The actual submission will be wired once we decide on the signing approach.

    let _ = (rpc_endpoint, relayer_key);

    Ok(())
}
