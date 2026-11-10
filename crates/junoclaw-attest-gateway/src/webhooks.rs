//! Webhook delivery worker.
//!
//! On `commit_batch` the store enqueues a pending webhook per attestation
//! that carried a `callback_url`; this worker drains the pending set and
//! POSTs the anchored receipt subset with an HMAC-SHA256 signature using
//! the customer's API key as the shared secret. Pending entries are
//! spooled, so delivery survives restarts (at-least-once; receivers must
//! tolerate duplicates — attestation_id is the dedup key).

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{config::Config, store::Store};

const MAX_ATTEMPTS: u32 = 8;

/// `secrets`: customer -> HMAC secret (the API key when known, else its
/// sha256 hex — either way the customer can recompute the signature).
pub async fn run(
    store: Arc<Store>,
    cfg: Arc<Config>,
    secrets: Arc<std::collections::HashMap<String, String>>,
) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(cfg.webhook_timeout_secs))
        .build()
        .unwrap_or_default();
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    loop {
        interval.tick().await;
        for (att_id, url, customer, attempts) in store.pending_webhooks() {
            if attempts >= MAX_ATTEMPTS {
                store.webhook_dead(&att_id).ok();
                continue;
            }
            let att = match store.get(&att_id) {
                Some(a) => a,
                None => {
                    store.webhook_dead(&att_id).ok();
                    continue;
                }
            };
            let body = json!({
                "attestation_id": att.attestation_id,
                "status": "anchored",
                "leaf_hash": att.leaf_hash,
                "batch": att.batch,
                "court": att.court,
            });
            let secret = secrets.get(&customer).cloned().unwrap_or_default();
            let sig = hmac_sha256_hex(secret.as_bytes(), body.to_string().as_bytes());
            let res = client
                .post(&url)
                .header("content-type", "application/json")
                .header("x-junoclaw-signature", sig)
                .json(&body)
                .send()
                .await;
            match res {
                Ok(r) if r.status().is_success() => {
                    store.webhook_sent(&att_id).ok();
                }
                Ok(r) => {
                    tracing::warn!(att_id, status = %r.status(), "webhook rejected");
                    store.webhook_retry(&att_id).ok();
                }
                Err(e) => {
                    tracing::warn!(att_id, error = %e, "webhook delivery failed");
                    store.webhook_retry(&att_id).ok();
                }
            }
        }
    }
}

/// RFC 2104 HMAC-SHA256 — avoids an hmac dependency for one call site.
fn hmac_sha256_hex(key: &[u8], msg: &[u8]) -> String {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    hex::encode(outer.finalize())
}
