//! ADR-005: Moultbook Endorsement Operator
//!
//! Off-chain WAVS task module that watches for `moultbook_endorsement_ready`
//! events emitted by agent-company contracts after successful zk-verified
//! attestations, and crafts `PublishAnon` transactions against the moultbook
//! contract.
//!
//! Flow:
//! 1. agent-company emits `moultbook_endorsement_ready` event after attestation
//!    with verified proof AND `cfg.moultbook.is_some()`.
//! 2. This operator detects the event via chain event subscription.
//! 3. Operator resolves the endorser's moult-key from the membership tree.
//! 4. Operator generates the Groth16 membership proof (circuits/moultbook-membership).
//! 5. Operator submits `PublishAnon { topic_hash, content_cid, proof_base64, public_inputs_base64 }`
//!    to the moultbook contract on behalf of the moult-key.
//!
//! The operator holds the moult-key signing material and the proving key for
//! the membership circuit. It never reveals the link between the endorser's
//! real identity and their moult-key.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Parsed `moultbook_endorsement_ready` event from on-chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndorsementReadyEvent {
    pub proposal_id: u64,
    pub moultbook_addr: String,
    pub contract_address: String,
}

impl EndorsementReadyEvent {
    /// Try to parse from a raw event attribute map (wasm event attributes).
    pub fn from_attributes(attrs: &[(String, String)]) -> Option<Self> {
        let mut proposal_id = None;
        let mut moultbook = None;
        let mut contract_address = None;

        for (key, value) in attrs {
            match key.as_str() {
                "proposal_id" => proposal_id = value.parse::<u64>().ok(),
                "moultbook" => moultbook = Some(value.clone()),
                "contract_address" => contract_address = Some(value.clone()),
                _ => {}
            }
        }

        Some(Self {
            proposal_id: proposal_id?,
            moultbook_addr: moultbook?,
            contract_address: contract_address?,
        })
    }
}

/// The `PublishAnon` message to be submitted to moultbook-v0.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoultbookExecuteMsg {
    PublishAnon {
        topic_hash: String,
        content_cid: String,
        proof_base64: String,
        public_inputs_base64: String,
    },
}

/// Configuration for the moultbook endorsement operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoultbookOperatorConfig {
    /// The moultbook contract address to target.
    pub moultbook_addr: String,
    /// Topic hash convention: SHA-256 of `"skill_endorsement:{dao_addr}:{proposal_id}"`.
    pub topic_prefix: String,
    /// IPFS gateway for pinning endorsement content before publishing.
    pub ipfs_gateway: Option<String>,
}

/// Build a `PublishAnon` message for a given endorsement event.
///
/// The proof generation is delegated to the circuits crate. This function
/// builds the message payload assuming the proof is already generated.
pub fn build_publish_anon_msg(
    event: &EndorsementReadyEvent,
    topic_hash: &str,
    content_cid: &str,
    proof_base64: &str,
    public_inputs_base64: &str,
) -> MoultbookExecuteMsg {
    info!(
        "Building PublishAnon for proposal {} → moultbook {}",
        event.proposal_id, event.moultbook_addr
    );
    MoultbookExecuteMsg::PublishAnon {
        topic_hash: topic_hash.to_string(),
        content_cid: content_cid.to_string(),
        proof_base64: proof_base64.to_string(),
        public_inputs_base64: public_inputs_base64.to_string(),
    }
}

/// Derive the topic hash for a skill endorsement.
///
/// Convention: `SHA-256("skill_endorsement:{dao_contract}:{proposal_id}")`
/// This matches `junoclaw-common::skill_endorsement_topic()`.
pub fn derive_topic_hash(dao_contract: &str, proposal_id: u64) -> String {
    use sha2::{Digest, Sha256};
    let input = format!("skill_endorsement:{}:{}", dao_contract, proposal_id);
    let hash = Sha256::digest(input.as_bytes());
    format!("sha256:{}", hex::encode(hash))
}

/// Handle an incoming `moultbook_endorsement_ready` event.
///
/// This is the main entry point called by the WAVS event loop when it detects
/// the trigger event. Returns the built message (caller is responsible for
/// signing and broadcasting).
///
/// # Errors
/// Returns `None` if proof generation is not yet available (placeholder).
pub async fn handle_endorsement_event(
    event: &EndorsementReadyEvent,
    _config: &MoultbookOperatorConfig,
) -> Option<MoultbookExecuteMsg> {
    let topic_hash = derive_topic_hash(&event.contract_address, event.proposal_id);

    // TODO: Generate the moultbook membership proof using the circuits crate.
    // This requires:
    //   1. The endorser's moult-key private key (held in operator keyring)
    //   2. The membership Merkle proof from the moultbook group tree
    //   3. The proving key (pk) for the Groth16 circuit
    //
    // For now, log the trigger and return None. Once the circuits crate
    // exposes `generate_membership_proof(moult_key, merkle_path, pk) -> (proof, inputs)`,
    // this becomes:
    //
    //   let (proof_b64, inputs_b64) = circuits::moultbook_membership::prove(
    //       &moult_key_sk, &merkle_path, &proving_key
    //   )?;
    //   let content_cid = ipfs_pin(endorsement_payload).await?;
    //   Some(build_publish_anon_msg(event, &topic_hash, &content_cid, &proof_b64, &inputs_b64))

    warn!(
        "Moultbook endorsement operator: event received for proposal {} \
         (topic_hash={}), but proof generation not yet wired. \
         Skipping PublishAnon dispatch.",
        event.proposal_id, topic_hash
    );

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_topic_hash_deterministic() {
        let h1 = derive_topic_hash("juno1abc", 42);
        let h2 = derive_topic_hash("juno1abc", 42);
        assert_eq!(h1, h2);
        assert!(h1.starts_with("sha256:"));
        assert_eq!(h1.len(), 7 + 64); // "sha256:" + 64 hex chars
    }

    #[test]
    fn test_derive_topic_hash_different_inputs() {
        let h1 = derive_topic_hash("juno1abc", 42);
        let h2 = derive_topic_hash("juno1abc", 43);
        let h3 = derive_topic_hash("juno1xyz", 42);
        assert_ne!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_event_parse() {
        let attrs = vec![
            ("proposal_id".to_string(), "7".to_string()),
            ("moultbook".to_string(), "juno1moultbook".to_string()),
            ("contract_address".to_string(), "juno1dao".to_string()),
        ];
        let event = EndorsementReadyEvent::from_attributes(&attrs).unwrap();
        assert_eq!(event.proposal_id, 7);
        assert_eq!(event.moultbook_addr, "juno1moultbook");
        assert_eq!(event.contract_address, "juno1dao");
    }

    #[test]
    fn test_event_parse_missing_field() {
        let attrs = vec![
            ("proposal_id".to_string(), "7".to_string()),
            // missing moultbook
            ("contract_address".to_string(), "juno1dao".to_string()),
        ];
        assert!(EndorsementReadyEvent::from_attributes(&attrs).is_none());
    }

    #[test]
    fn test_build_publish_anon_msg() {
        let event = EndorsementReadyEvent {
            proposal_id: 5,
            moultbook_addr: "juno1mb".to_string(),
            contract_address: "juno1dao".to_string(),
        };
        let msg = build_publish_anon_msg(
            &event,
            "sha256:cafe",
            "bafybeig...",
            "proof_b64",
            "inputs_b64",
        );
        match msg {
            MoultbookExecuteMsg::PublishAnon {
                topic_hash,
                content_cid,
                proof_base64,
                public_inputs_base64,
            } => {
                assert_eq!(topic_hash, "sha256:cafe");
                assert_eq!(content_cid, "bafybeig...");
                assert_eq!(proof_base64, "proof_b64");
                assert_eq!(public_inputs_base64, "inputs_b64");
            }
        }
    }
}
