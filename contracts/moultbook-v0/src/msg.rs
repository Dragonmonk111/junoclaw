use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Binary;

#[allow(unused_imports)]
use crate::state::{AttestationRef, Config, Disclosure, MoultEntry, MoultKeyState, Stats, Visibility};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    /// Optional whoami / DENS contract address. If set, every `Post` validates
    /// that the sender holds at least one DENS alias. Leave None for devnet.
    pub whoami_contract: Option<String>,
    pub max_size_bytes: u64,
    pub max_refs: u32,
    pub max_content_type_len: u32,
    pub max_group_size: u32,
    /// zk-verifier contract (required for anonymous publishing)
    pub zk_verifier: Option<String>,
    /// agent-registry contract (source of membership merkle root)
    pub agent_registry: Option<String>,
    /// SHA-256 hash of the membership circuit verifying key
    pub membership_vk_hash: Option<String>,
    /// Max entries per moult-key per epoch (default: 10)
    pub entries_per_key_per_epoch: Option<u32>,
    /// Epoch length in blocks (default: 14400 ≈ 24h at 6s/block)
    pub epoch_blocks: Option<u64>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Write a new entry. Author is `info.sender`.
    Post {
        commitment: Binary,
        content_type: String,
        size_bytes: u64,
        attestation_ref: Option<AttestationRef>,
        visibility: Visibility,
        refs: Vec<String>,
    },
    /// Soft-delete: clears commitment, keeps metadata + refs for audit.
    /// Author or admin only.
    Redact { id: String },
    /// Author only. Cannot widen non-Public to Public (one-way narrowing).
    UpdateVisibility { id: String, visibility: Visibility },
    /// Admin only. Empty `whoami_contract` clears the gating.
    /// Empty `membership_vk_hash` clears the configured VK hash.
    UpdateConfig {
        admin: Option<String>,
        whoami_contract: Option<String>,
        max_size_bytes: Option<u64>,
        max_refs: Option<u32>,
        max_group_size: Option<u32>,
        membership_vk_hash: Option<String>,
    },
    /// Publish anonymously using a derived moult-key. Sender is the moult-key.
    /// The Groth16 proof proves the moult-key belongs to a registered agent
    /// without revealing which agent.
    PublishAnon {
        /// SHA-256 of the topic namespace
        topic_hash: String,
        /// IPFS CID of the knowledge content
        content_cid: String,
        /// Groth16 proof (base64-encoded arkworks CanonicalSerialize)
        proof_base64: String,
        /// Public inputs (base64-encoded arkworks CanonicalSerialize)
        public_inputs_base64: String,
    },
    /// Voluntarily link a moultbook entry to your primary identity.
    /// Sender must be the moult-key that authored the entry.
    /// One-way and optional — nobody can force disclosure.
    ///
    /// The disclosure is only persisted if the supplied Groth16 derivation
    /// proof verifies against the configured `zk_verifier` (the membership
    /// circuit in disclosure mode). Verification runs as a `reply_on_success`
    /// sub-message, so an invalid proof rolls the whole tx back.
    VoluntaryDisclose {
        entry_id: String,
        /// The primary agent-registry address to link to
        primary_key: String,
        /// Proof that moult-key derives from primary-key (base64 arkworks Proof<Bn254>)
        derivation_proof_base64: String,
        /// Public inputs for the derivation proof (base64 arkworks [Fr]).
        /// Binds the proof to the moult-key / primary-key pair.
        derivation_public_inputs_base64: String,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    #[returns(MoultEntry)]
    GetEntry { id: String },
    #[returns(EntriesResponse)]
    ListByAuthor {
        author: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(EntriesResponse)]
    ListByRef {
        ref_id: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(Stats)]
    GetStats {},
    /// List entries by moult-key (anonymous author)
    #[returns(EntriesResponse)]
    ListByMoultKey {
        moult_key: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns moult-key rate-limit state
    #[returns(MoultKeyState)]
    MoultKeyStats { moult_key: String },
    /// Check if an entry has a voluntary disclosure
    #[returns(Option<Disclosure>)]
    GetDisclosure { entry_id: String },
    /// List entries by topic hash (e.g. skill endorsements for a specific DAO/proposal).
    /// ADR-005: enables frontend endorsement aggregation queries.
    #[returns(EntriesResponse)]
    ListByTopic {
        topic_hash: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct EntriesResponse {
    pub entries: Vec<MoultEntry>,
}

/// Subset of the upstream `whoami` contract's query API that Moultbook calls
/// for identity gating. Defined locally so the crate has no hard dependency
/// on the whoami crate; the message shape matches `envoylabs/whoami`.
#[cw_serde]
pub enum WhoamiQuery {
    Tokens {
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct WhoamiTokensResponse {
    pub tokens: Vec<String>,
}
