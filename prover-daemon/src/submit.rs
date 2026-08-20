// On-chain submission module.
//
// Submits ZK proofs to the zk-verifier CosmWasm contract via the chain's
// RPC endpoint. Also queries the circuit-breaker contract for robot lock state.

use anyhow::{Context, Result};
use ark_serialize::CanonicalSerialize;
use std::path::Path;

/// Submit a proof to the zk-verifier contract on-chain.
pub async fn submit_proof_onchain(
    chain_rpc: &str,
    verifier_addr: &str,
    proof_path: &Path,
    vk_path: &Path,
    public_inputs: &str,
) -> Result<()> {
    let proof_bytes = std::fs::read(proof_path)?;
    let vk_bytes = std::fs::read(vk_path)?;

    let proof_hex = hex::encode(&proof_bytes);
    let vk_hex = hex::encode(&vk_bytes);

    let inputs: Vec<String> = serde_json::from_str(public_inputs)?;

    let msg = serde_json::json!({
        "verify_proof": {
            "proof": proof_hex,
            "verifying_key": vk_hex,
            "public_inputs": inputs,
        }
    });

    let url = format!(
        "{}/cosmwasm/wasm/v1/contract/{}/smart",
        chain_rpc.trim_end_matches('/'),
        verifier_addr,
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&msg)
        .send()
        .await
        .context("failed to submit proof on-chain")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("on-chain submit failed: status={}, body={}", status, body);
    }

    let result: serde_json::Value = resp.json().await?;
    tracing::info!("On-chain verify result: {}", result);

    Ok(())
}

/// Submit a raw proof (bytes) to the zk-verifier contract.
pub async fn submit_proof_raw(
    chain_rpc: &str,
    verifier_addr: &str,
    proof_bytes: &[u8],
    keys_dir: &Path,
) -> Result<String> {
    let vk_path = keys_dir.join("sensor_verifying_key.bin");
    let vk_bytes = std::fs::read(&vk_path)
        .with_context(|| format!("failed to read VK from {}", vk_path.display()))?;

    let proof_hex = hex::encode(proof_bytes);
    let vk_hex = hex::encode(&vk_bytes);

    // Public inputs would be extracted from the proof/circuit context
    // For now, use placeholder values matching the circuit's public inputs
    let public_inputs = vec![
        "0".to_string(), // envelope_commitment
        "0".to_string(), // merkle_root
        "0".to_string(), // cycle_index
    ];

    let msg = serde_json::json!({
        "verify_proof": {
            "proof": proof_hex,
            "verifying_key": vk_hex,
            "public_inputs": public_inputs,
        }
    });

    let query_url = format!(
        "{}/cosmwasm/wasm/v1/contract/{}/smart",
        chain_rpc.trim_end_matches('/'),
        verifier_addr,
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(&query_url)
        .json(&msg)
        .send()
        .await
        .context("failed to submit proof on-chain")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("on-chain submit failed: status={}, body={}", status, body);
    }

    let result: serde_json::Value = resp.json().await?;
    let tx_hash = result
        .get("txhash")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(tx_hash)
}

/// Check the circuit breaker state for a robot.
pub async fn check_circuit_breaker(
    chain_rpc: &str,
    breaker_addr: &str,
    robot_id: &str,
) -> Result<bool> {
    let query_msg = serde_json::json!({
        "is_locked": { "robot_id": robot_id }
    });

    let query_b64 = base64_url_encode(&serde_json::to_vec(&query_msg)?);

    let url = format!(
        "{}/abci_query?path=\"/cosmwasm.wasm.v1.Query/SmartContractState/{}%2F{}\"",
        chain_rpc.trim_end_matches('/'),
        breaker_addr,
        query_b64,
    );

    let resp = reqwest::get(&url).await
        .context("failed to query circuit breaker")?;

    if !resp.status().is_success() {
        anyhow::bail!("circuit breaker query failed: status={}", resp.status());
    }

    let body: serde_json::Value = resp.json().await?;
    let value = body
        .get("result")
        .and_then(|r| r.get("response"))
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("malformed ABCI query response"))?;

    let decoded = base64_std_decode(value)?;
    let result: serde_json::Value = serde_json::from_slice(&decoded)?;

    let is_locked = result
        .get("is_locked")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(is_locked)
}

fn base64_url_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        }
    }
    result
}

fn base64_std_decode(s: &str) -> Result<Vec<u8>> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let s = s.trim_end_matches('=');
    let mut result = Vec::new();
    let bytes = s.as_bytes();

    for chunk in bytes.chunks(4) {
        let mut vals = [0u32; 4];
        for (i, &b) in chunk.iter().enumerate() {
            vals[i] = CHARS.iter().position(|&c| c == b)
                .ok_or_else(|| anyhow::anyhow!("invalid base64 character: {}", b as char))?
                as u32;
        }
        let quad = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        result.push((quad >> 16) as u8);
        if chunk.len() > 2 {
            result.push((quad >> 8) as u8);
        }
        if chunk.len() > 3 {
            result.push(quad as u8);
        }
    }
    Ok(result)
}
