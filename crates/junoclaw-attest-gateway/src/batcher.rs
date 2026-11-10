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

    // Bonded items take the thought-court lane — one anchor tx each,
    // operator bond attached, challengeable during the court window.
    let (bonded, items): (Vec<_>, Vec<_>) = items
        .into_iter()
        .partition(|a| a.tier == "bonded" && !cfg.court_contract.is_empty());
    if !bonded.is_empty() {
        anchor_court(store, chain, cfg, bonded).await;
    }
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

/// Bonded lane: each attestation gets its own `anchor_attestation` tx
/// with the configured operator bond. Bond economics don't batch —
/// every court anchor is an independently challengeable commitment.
/// The bond is locked until the attestation finalizes/disputes resolve;
/// a finalize sweep that returns matured bonds is a later ops step.
async fn anchor_court(
    store: &Arc<Store>,
    chain: &Arc<Mutex<GrpcClient>>,
    cfg: &crate::config::Config,
    items: Vec<crate::types::Attestation>,
) {
    let (amount, denom) = cfg.operator_bond.clone();
    for att in items {
        let msg = serde_json::json!({
            "anchor_attestation": {
                "id": att.attestation_id,
                "activation_root": att.leaf_hash,
            }
        });
        let mut attempt = 0u32;
        let outcome = loop {
            attempt += 1;
            let res = {
                let mut c = chain.lock().await;
                c.execute_contract_funded(
                    &cfg.court_contract,
                    &msg,
                    vec![(denom.clone(), amount)],
                )
                .await
            };
            match res {
                Ok(o) => break Some(o),
                Err(e) => {
                    // g1 quirk: a tx can execute on-chain AND report a
                    // failure (sequence semantics). If the court already
                    // holds our attestation the anchor landed — commit it
                    // rather than retrying into `already exists` walls.
                    if e.to_string().contains("already exists") {
                        if let Some(recovered) =
                            verify_court_anchor(store, chain, cfg, &att).await
                        {
                            break Some(recovered);
                        }
                    }
                    tracing::warn!(
                        attempt,
                        attestation = %att.attestation_id,
                        error = %e,
                        "court anchor attempt failed"
                    );
                    if attempt >= MAX_ANCHOR_ATTEMPTS {
                        break None;
                    }
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                }
            }
        };
        match outcome {
            Some(o) => {
                let height = if o.height > 0 {
                    o.height
                } else {
                    chain.lock().await.latest_height().await.unwrap_or(0)
                };
                if let Err(e) = store.commit_court(
                    &att.attestation_id,
                    crate::types::CourtInfo {
                        contract: cfg.court_contract.clone(),
                        court_id: att.attestation_id.clone(),
                        anchor_tx: o.tx_hash.clone(),
                        anchored_at_height: height,
                    },
                ) {
                    tracing::error!(attestation = %att.attestation_id, error = %e, "commit_court failed");
                }
                tracing::info!(
                    attestation = %att.attestation_id,
                    tx = %o.tx_hash,
                    "bonded attestation anchored on court"
                );
            }
            None => {
                if let Err(e) =
                    store.fail_court(&att.attestation_id, "court anchor failed after max attempts")
                {
                    tracing::error!(attestation = %att.attestation_id, error = %e, "fail_court failed");
                }
            }
        }
    }
}

/// Recovery path for g1's execute-then-fail sequence semantics: query the
/// court's `get_attestation` for our id — if it exists with our
/// activation_root, the anchor genuinely landed. Returns a synthetic
/// outcome (tx hash recovered from the error string when present).
async fn verify_court_anchor(
    _store: &Arc<Store>,
    chain: &Arc<Mutex<GrpcClient>>,
    cfg: &crate::config::Config,
    att: &crate::types::Attestation,
) -> Option<junoclaw_chain_grpc::TxOutcome> {
    #[derive(serde::Deserialize)]
    struct CourtAtt {
        activation_root: String,
        #[serde(default)]
        anchored_height: Option<u64>,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        attestation: Option<CourtAtt>,
    }
    let q = serde_json::json!({
        "get_attestation": { "attestation_id": att.attestation_id }
    });
    let mut c = chain.lock().await;
    let resp: Resp = c
        .query_smart_typed(&cfg.court_contract, &q)
        .await
        .ok()?;
    let rec = resp.attestation?;
    if rec.activation_root != att.leaf_hash {
        tracing::error!(
            attestation = %att.attestation_id,
            "court holds attestation id with DIFFERENT activation_root — refusing to commit"
        );
        return None;
    }
    let height = rec
        .anchored_height
        .unwrap_or(0);
    tracing::warn!(
        attestation = %att.attestation_id,
        "recovered: court anchor exists on-chain despite reported tx failure"
    );
    Some(junoclaw_chain_grpc::TxOutcome {
        tx_hash: String::new(), // landed but hash unknown — see chain events
        height,
    })
}
