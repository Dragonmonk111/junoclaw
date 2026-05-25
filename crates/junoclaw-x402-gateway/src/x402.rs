//! x402 envelope construction, parsing, and anti-replay state.
//!
//! Wire format (Cosmos-shaped, JSON-encoded in `PAYMENT-REQUIRED` header
//! or 402 response body):
//!
//! ```json
//! {
//!   "version": "1",
//!   "scheme": "cosmos-direct",
//!   "chain_id": "juno-1",
//!   "contract": "juno1agentcompany...",
//!   "msg": { "...wasm execute msg JSON..." },
//!   "funds": [{ "denom": "ujuno", "amount": "100000000" }],
//!   "gas_estimate": 250000,
//!   "fee_ujuno": "18750",
//!   "nonce": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
//!   "exp": 1716000000,
//!   "binding": "sha256:..."     // hash over the above fields, anti-tamper
//! }
//! ```
//!
//! The client retries with header `PAYMENT-SIGNATURE: <base64-signed-tx>` whose
//! Cosmos tx must:
//! - sign over the exact `contract`/`msg`/`funds` from the envelope
//! - reference the same `chain_id`
//! - be within the gateway's `envelope_ttl_sec` window (anti-replay)
//! - carry a `memo` of `"x402:<nonce>"` so we can correlate
//!
//! Nonce reuse is rejected. The in-memory replay cache lives in [`NonceStore`].

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{GatewayError, GatewayResult};

pub const ENVELOPE_VERSION: &str = "1";
pub const ENVELOPE_SCHEME: &str = "cosmos-direct";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coin {
    pub denom: String,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentEnvelope {
    pub version: String,
    pub scheme: String,
    pub chain_id: String,
    pub contract: String,
    pub msg: serde_json::Value,
    pub funds: Vec<Coin>,
    pub gas_estimate: u64,
    pub fee_ujuno: String,
    pub nonce: String,
    pub exp: i64,
    pub binding: String,
}

impl PaymentEnvelope {
    /// Construct + bind a new envelope. The binding hash covers every field
    /// except `binding` itself, so any tampering invalidates it.
    pub fn new(
        chain_id: String,
        contract: String,
        msg: serde_json::Value,
        funds: Vec<Coin>,
        gas_estimate: u64,
        fee_ujuno: String,
        ttl_sec: i64,
    ) -> Self {
        let nonce = Uuid::new_v4().to_string();
        let exp = now_unix() + ttl_sec;
        let mut env = Self {
            version: ENVELOPE_VERSION.to_string(),
            scheme: ENVELOPE_SCHEME.to_string(),
            chain_id,
            contract,
            msg,
            funds,
            gas_estimate,
            fee_ujuno,
            nonce,
            exp,
            binding: String::new(),
        };
        env.binding = env.compute_binding();
        env
    }

    /// Re-compute the binding hash. Used both on construction and on verification.
    fn compute_binding(&self) -> String {
        let payload = serde_json::json!({
            "v": self.version,
            "s": self.scheme,
            "c": self.chain_id,
            "k": self.contract,
            "m": self.msg,
            "f": self.funds,
            "g": self.gas_estimate,
            "u": self.fee_ujuno,
            "n": self.nonce,
            "e": self.exp,
        });
        let bytes = serde_json::to_vec(&payload).expect("serialize envelope payload");
        let digest = Sha256::digest(&bytes);
        format!("sha256:{}", hex::encode(digest))
    }

    /// Validate the envelope's self-consistency and expiry. Does NOT check the
    /// signature — that's [`PaymentSignature::verify`]'s job.
    pub fn validate(&self) -> GatewayResult<()> {
        if self.version != ENVELOPE_VERSION {
            return Err(GatewayError::InvalidEnvelope(format!(
                "unsupported envelope version: {}",
                self.version
            )));
        }
        if self.scheme != ENVELOPE_SCHEME {
            return Err(GatewayError::InvalidEnvelope(format!(
                "unsupported scheme: {}",
                self.scheme
            )));
        }
        if self.binding != self.compute_binding() {
            return Err(GatewayError::InvalidEnvelope(
                "binding hash mismatch (envelope tampered)".into(),
            ));
        }
        let now = now_unix();
        if self.exp < now {
            return Err(GatewayError::EnvelopeExpired {
                exp: self.exp,
                now,
            });
        }
        Ok(())
    }
}

/// Decoded payment signature carrying the signed Cosmos tx bytes and the
/// nonce we expect to find in the tx memo.
#[derive(Debug, Clone)]
pub struct PaymentSignature {
    pub tx_bytes: Vec<u8>,
    pub claimed_nonce: String,
}

impl PaymentSignature {
    /// Parse the `PAYMENT-SIGNATURE` header value (base64 of the signed tx
    /// bytes; nonce is carried in tx memo).
    pub fn parse_header(header: &str) -> GatewayResult<Self> {
        let tx_bytes = B64
            .decode(header)
            .map_err(|e| GatewayError::InvalidEnvelope(format!("base64 decode: {e}")))?;
        // Nonce extraction from memo is done by the cosmrs decoder downstream;
        // for now we leave claimed_nonce empty and let cosmos.rs fill it in.
        Ok(Self {
            tx_bytes,
            claimed_nonce: String::new(),
        })
    }
}

/// Anti-replay store. Records nonces that have been seen; rejects duplicates.
/// Entries expire when their corresponding envelope would have expired
/// (envelope TTL + slack).
///
/// In-memory only for v1. Production deployments running multiple gateway
/// replicas need a shared store (Redis, KV, or a CosmWasm contract).
#[derive(Default, Clone)]
pub struct NonceStore {
    inner: Arc<Mutex<HashMap<String, i64>>>,
}

impl NonceStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a nonce; reject if already present.
    pub fn record(&self, nonce: &str, exp: i64) -> GatewayResult<()> {
        let mut map = self.inner.lock();
        // Opportunistic GC: drop entries older than `now`.
        let now = now_unix();
        map.retain(|_, e| *e >= now);

        if map.contains_key(nonce) {
            return Err(GatewayError::NonceReplayed);
        }
        map.insert(nonce.to_string(), exp);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }
}

/// Current Unix timestamp in seconds. Wrapped for testability.
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_envelope(ttl: i64) -> PaymentEnvelope {
        PaymentEnvelope::new(
            "juno-1".into(),
            "juno1agentcompanyaddr".into(),
            serde_json::json!({"post_task": {"description": "test"}}),
            vec![Coin {
                denom: "ujuno".into(),
                amount: "100".into(),
            }],
            150_000,
            "11250".into(),
            ttl,
        )
    }

    #[test]
    fn envelope_round_trip_valid() {
        let env = make_envelope(300);
        env.validate().expect("freshly minted envelope must validate");
    }

    #[test]
    fn envelope_tampered_msg_fails_binding() {
        let mut env = make_envelope(300);
        env.msg = serde_json::json!({"evil": true});
        match env.validate() {
            Err(GatewayError::InvalidEnvelope(s)) => assert!(s.contains("binding")),
            other => panic!("expected InvalidEnvelope, got {:?}", other),
        }
    }

    #[test]
    fn envelope_expired_rejected() {
        let env = make_envelope(-1);
        match env.validate() {
            Err(GatewayError::EnvelopeExpired { .. }) => {}
            other => panic!("expected EnvelopeExpired, got {:?}", other),
        }
    }

    #[test]
    fn nonce_store_rejects_replay() {
        let store = NonceStore::new();
        let env = make_envelope(300);
        store
            .record(&env.nonce, env.exp)
            .expect("first record should succeed");
        match store.record(&env.nonce, env.exp) {
            Err(GatewayError::NonceReplayed) => {}
            other => panic!("expected NonceReplayed, got {:?}", other),
        }
    }

    #[test]
    fn nonce_store_distinct_nonces_ok() {
        let store = NonceStore::new();
        let env_a = make_envelope(300);
        let env_b = make_envelope(300);
        assert_ne!(env_a.nonce, env_b.nonce);
        store.record(&env_a.nonce, env_a.exp).unwrap();
        store.record(&env_b.nonce, env_b.exp).unwrap();
        assert_eq!(store.len(), 2);
    }
}
