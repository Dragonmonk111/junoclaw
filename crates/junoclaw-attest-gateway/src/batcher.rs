//! Batch anchor loop.
//!
//! Every `window_secs`, drains queued attestations, builds a Merkle tree,
//! anchors the root on the merkle-verifier contract, then commits the
//! batch (per-leaf proofs + anchor tx) to the store. Failures retry with
//! bounded attempts before parking items as `anchor_failed`.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::{merkle, store::Store};
use junoclaw_chain_grpc::GrpcClient;

const MAX_ANCHOR_ATTEMPTS: u32 = 5;

pub async fn run(store: Arc<Store>, chain: Arc<Mutex<GrpcClient>>, cfg: crate::config::Config) {
    let mut interval = tokio::time::interval(Duration::from_secs(cfg.window_secs));
    loop {
        interval.tick().await;
        if let Err(e) = anchor_once(&store, &chain, &cfg).await {
            tracing::error!(error = %e, "anchor cycle failed");
        }
    }
}

async fn anchor_once(
    store: &Arc<Store>,
    chain: &Arc<Mutex<GrpcClient>>,
    cfg: &crate::config::Config,
) -> anyhow::Result<()> {
    let items = store.drain_queued(cfg.max_leaves);
    if items.is_empty() {
        return Ok(());
    }

    let batch_height = store.next_batch_height();
    let robot_id = "attest:default".to_string(); // multi-customer ns: v2

    // Build the tree.
    let mut leaves = Vec::with_capacity(items.len());
    for att in &items {
        let leaf: merkle::Hash = hex::decode(&att.leaf_hash)
            .map_err(|e| anyhow::anyhow!("bad leaf_hash {}: {e}", att.leaf_hash))?
            .try_into()
            .map_err(|_| anyhow::anyhow!("leaf_hash wrong length"))?;
        leaves.push(leaf);
    }
    let root = merkle::root_hex(&leaves);

    // Anchor on-chain with retries.
    let msg = serde_json::json!({
        "anchor_root": {
            "robot_id": robot_id,
            "batch_height": batch_height,
            "merkle_root": root,
            "cycle_count": items.len() as u32,
        }
    });

    let mut attempt = 0u32;
    let outcome = loop {
        attempt += 1;
        let res = {
            let mut c = chain.lock().await;
            c.execute_contract(&cfg.merkle_contract, &msg).await
        };
        match res {
            Ok(o) => break Some(o),
            Err(e) => {
                tracing::warn!(attempt, error = %e, batch_height, "anchor attempt failed");
                if attempt >= MAX_ANCHOR_ATTEMPTS {
                    break None;
                }
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            }
        }
    };

    let ids: Vec<String> = items.iter().map(|a| a.attestation_id.clone()).collect();
    match outcome {
        Some(o) => {
            let anchored_height = if o.height > 0 {
                o.height
            } else {
                let mut c = chain.lock().await;
                c.latest_height().await.unwrap_or(0)
            };
            let packed: Vec<(String, u32, Vec<String>)> = items
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let proof_hex: Vec<String> = merkle::proof(&leaves, i)
                        .iter()
                        .map(hex::encode)
                        .collect();
                    (a.attestation_id.clone(), i as u32, proof_hex)
                })
                .collect();
            store.commit_batch(
                batch_height,
                &robot_id,
                &root,
                &o.tx_hash,
                anchored_height,
                &packed,
            )?;
            tracing::info!(
                batch_height,
                leaves = items.len(),
                tx = %o.tx_hash,
                "batch anchored"
            );
        }
        None => {
            store.fail_batch(
                batch_height,
                &ids,
                "anchor tx failed after max attempts",
            )?;
            tracing::error!(batch_height, "batch parked as anchor_failed");
        }
    }
    Ok(())
}
