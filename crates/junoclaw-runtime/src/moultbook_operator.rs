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
use tracing::{info, warn, error};

use ark_bn254::{Bn254, Fr};
use ark_groth16::Groth16;
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};

use moultbook_membership_circuit::{MembershipCircuit, build_merkle_tree, mimc_hash};

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

/// Operator keyring — holds the agent's private key material and the
/// membership Merkle tree needed for proof generation.
///
/// In production, the primary_key would be stored in a TEE enclave or
/// hardware keyring. For now, it's held in memory during operator lifetime.
pub struct OperatorKeyring {
    /// The agent's primary key (field element) — private, never leaves operator
    pub primary_key: Fr,
    /// Derivation salt for moult-key generation
    pub derivation_salt: Fr,
    /// All registered agent leaves (H(pk, 0) for each agent in the group)
    pub member_leaves: Vec<Fr>,
    /// This agent's index in the member_leaves array
    pub member_index: usize,
    /// Tree height (log2 of max group size)
    pub tree_height: usize,
    /// Current epoch number
    pub epoch: u64,
}

/// Generated proof output — base64-encoded proof + public inputs.
#[derive(Debug)]
pub struct ProofOutput {
    pub proof_base64: String,
    pub public_inputs_base64: String,
}

/// Generate a Groth16 membership proof using the moultbook-membership circuit.
///
/// This proves that the operator's primary_key is in the membership tree
/// and derives the moult-key, all without revealing the primary_key.
///
/// CPU-intensive (~2-5 seconds on modern hardware for tree_height=20).
pub fn generate_membership_proof(keyring: &OperatorKeyring) -> Result<ProofOutput, String> {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;

    let zero = Fr::from(0u64);

    // Build the Merkle tree from all member leaves
    let (merkle_root, all_paths, all_bits) =
        build_merkle_tree(&keyring.member_leaves, keyring.tree_height);

    if keyring.member_index >= keyring.member_leaves.len() {
        return Err(format!(
            "member_index {} out of bounds (have {} leaves)",
            keyring.member_index,
            keyring.member_leaves.len()
        ));
    }

    let path = &all_paths[keyring.member_index];
    let bits = &all_bits[keyring.member_index];

    // Derive moult-key and its hash (public inputs)
    let moult_key = mimc_hash(keyring.primary_key, keyring.derivation_salt);
    let moult_key_hash = mimc_hash(moult_key, zero);
    let epoch = Fr::from(keyring.epoch);

    // Build the circuit with concrete witness
    let circuit = MembershipCircuit::new(
        moult_key_hash,
        merkle_root,
        epoch,
        keyring.primary_key,
        keyring.derivation_salt,
        path.clone(),
        bits.clone(),
        keyring.tree_height,
    );

    // Setup — in production, the proving key would be pre-generated and cached.
    // For now, we do trusted setup per-proof (acceptable for testnet).
    let rng = &mut StdRng::seed_from_u64(42);
    let empty_circuit = MembershipCircuit::empty(keyring.tree_height);
    let (pk, _vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, rng)
        .map_err(|e| format!("Setup failed: {}", e))?;

    // Generate proof
    let proof = Groth16::<Bn254>::prove(&pk, circuit, rng)
        .map_err(|e| format!("Prove failed: {}", e))?;

    // Serialize proof to bytes
    let mut proof_bytes = Vec::new();
    proof
        .serialize_compressed(&mut proof_bytes)
        .map_err(|e| format!("Proof serialization failed: {}", e))?;
    let proof_b64 = engine.encode(&proof_bytes);

    // Serialize public inputs: [moult_key_hash, merkle_root, epoch]
    let public_inputs = vec![moult_key_hash, merkle_root, epoch];
    let mut inputs_bytes = Vec::new();
    for input in &public_inputs {
        input
            .serialize_compressed(&mut inputs_bytes)
            .map_err(|e| format!("Input serialization failed: {}", e))?;
    }
    let inputs_b64 = engine.encode(&inputs_bytes);

    Ok(ProofOutput {
        proof_base64: proof_b64,
        public_inputs_base64: inputs_b64,
    })
}

/// Handle an incoming `moultbook_endorsement_ready` event.
///
/// This is the main entry point called by the WAVS event loop when it detects
/// the trigger event. Returns the built message (caller is responsible for
/// signing and broadcasting).
///
/// Requires an `OperatorKeyring` with the agent's key material and membership tree.
/// If no keyring is provided, falls back to logging a warning (no proof generation).
pub async fn handle_endorsement_event(
    event: &EndorsementReadyEvent,
    _config: &MoultbookOperatorConfig,
    keyring: Option<&OperatorKeyring>,
) -> Option<MoultbookExecuteMsg> {
    let topic_hash = derive_topic_hash(&event.contract_address, event.proposal_id);

    let keyring = match keyring {
        Some(k) => k,
        None => {
            warn!(
                "Moultbook endorsement operator: event received for proposal {} \
                 (topic_hash={}), but no keyring configured. \
                 Skipping PublishAnon dispatch.",
                event.proposal_id, topic_hash
            );
            return None;
        }
    };

    info!(
        "Generating membership proof for proposal {} (topic_hash={})",
        event.proposal_id, topic_hash
    );

    match generate_membership_proof(keyring) {
        Ok(proof_output) => {
            info!(
                "Proof generated: {} bytes proof, {} bytes inputs",
                proof_output.proof_base64.len(),
                proof_output.public_inputs_base64.len()
            );

            // TODO: Pin endorsement content to IPFS and get the CID.
            // For now, use a placeholder CID format.
            let content_cid = format!(
                "pending:skill_endorsement:{}:{}",
                event.contract_address, event.proposal_id
            );

            Some(build_publish_anon_msg(
                event,
                &topic_hash,
                &content_cid,
                &proof_output.proof_base64,
                &proof_output.public_inputs_base64,
            ))
        }
        Err(e) => {
            error!(
                "Proof generation failed for proposal {}: {}",
                event.proposal_id, e
            );
            None
        }
    }
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

    #[test]
    fn test_generate_membership_proof_valid() {
        let tree_height = 3; // Small tree for fast test

        // Register 4 agents
        let primary_keys: Vec<Fr> = (1..=4).map(|i| Fr::from(i as u64)).collect();
        let zero = Fr::from(0u64);
        let leaves: Vec<Fr> = primary_keys.iter().map(|k| mimc_hash(*k, zero)).collect();

        // Agent index 1 generates proof
        let keyring = OperatorKeyring {
            primary_key: primary_keys[1],
            derivation_salt: Fr::from(99999u64),
            member_leaves: leaves,
            member_index: 1,
            tree_height,
            epoch: 42,
        };

        let result = generate_membership_proof(&keyring);
        assert!(result.is_ok(), "Proof generation failed: {:?}", result.err());

        let output = result.unwrap();
        assert!(!output.proof_base64.is_empty());
        assert!(!output.public_inputs_base64.is_empty());

        // Proof should be base64-decodable
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        assert!(engine.decode(&output.proof_base64).is_ok());
        assert!(engine.decode(&output.public_inputs_base64).is_ok());
    }

    #[test]
    fn test_generate_membership_proof_out_of_bounds() {
        let tree_height = 3;
        let leaves: Vec<Fr> = (1..=2).map(|i| mimc_hash(Fr::from(i as u64), Fr::from(0u64))).collect();

        let keyring = OperatorKeyring {
            primary_key: Fr::from(1u64),
            derivation_salt: Fr::from(42u64),
            member_leaves: leaves,
            member_index: 99, // Out of bounds
            tree_height,
            epoch: 1,
        };

        let result = generate_membership_proof(&keyring);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of bounds"));
    }

    #[tokio::test]
    async fn test_handle_endorsement_with_keyring() {
        let tree_height = 3;
        let primary_keys: Vec<Fr> = (1..=4).map(|i| Fr::from(i as u64)).collect();
        let zero = Fr::from(0u64);
        let leaves: Vec<Fr> = primary_keys.iter().map(|k| mimc_hash(*k, zero)).collect();

        let keyring = OperatorKeyring {
            primary_key: primary_keys[0],
            derivation_salt: Fr::from(77777u64),
            member_leaves: leaves,
            member_index: 0,
            tree_height,
            epoch: 100,
        };

        let event = EndorsementReadyEvent {
            proposal_id: 7,
            moultbook_addr: "juno1mb".to_string(),
            contract_address: "juno1dao".to_string(),
        };
        let config = MoultbookOperatorConfig {
            moultbook_addr: "juno1mb".to_string(),
            topic_prefix: "skill_endorsement".to_string(),
            ipfs_gateway: None,
        };

        let result = handle_endorsement_event(&event, &config, Some(&keyring)).await;
        assert!(result.is_some(), "Expected PublishAnon message");

        match result.unwrap() {
            MoultbookExecuteMsg::PublishAnon {
                proof_base64,
                public_inputs_base64,
                ..
            } => {
                assert!(!proof_base64.is_empty());
                assert!(!public_inputs_base64.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn test_handle_endorsement_without_keyring() {
        let event = EndorsementReadyEvent {
            proposal_id: 7,
            moultbook_addr: "juno1mb".to_string(),
            contract_address: "juno1dao".to_string(),
        };
        let config = MoultbookOperatorConfig {
            moultbook_addr: "juno1mb".to_string(),
            topic_prefix: "skill_endorsement".to_string(),
            ipfs_gateway: None,
        };

        let result = handle_endorsement_event(&event, &config, None).await;
        assert!(result.is_none(), "Expected None when no keyring");
    }
}
