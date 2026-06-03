use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary, Timestamp};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    pub whoami_contract: Option<Addr>,
    pub max_size_bytes: u64,
    pub max_refs: u32,
    pub max_content_type_len: u32,
    pub max_group_size: u32,
    /// zk-verifier contract address (for anonymous publish proof verification)
    pub zk_verifier: Option<Addr>,
    /// agent-registry address (source of membership merkle root)
    pub agent_registry: Option<Addr>,
    /// SHA-256 of the membership circuit verifying key (must match VK stored in zk-verifier)
    pub membership_vk_hash: Option<String>,
    /// Max entries per moult-key per epoch (sybil resistance)
    pub entries_per_key_per_epoch: u32,
    /// Epoch length in blocks
    pub epoch_blocks: u64,
}

#[cw_serde]
pub struct MoultEntry {
    pub id: String,
    pub author: Addr,
    pub author_alias: Option<String>,
    pub commitment: Binary,
    pub content_type: String,
    pub size_bytes: u64,
    pub attestation_ref: Option<AttestationRef>,
    pub visibility: Visibility,
    pub refs: Vec<String>,
    pub posted_at: Timestamp,
    pub redacted_at: Option<Timestamp>,
    /// Topic namespace hash (e.g. "sha256:..."). Present for anonymous
    /// endorsements produced via `PublishAnon`; `None` for legacy entries.
    #[serde(default)]
    pub topic_hash: Option<String>,
}

#[cw_serde]
pub enum AttestationRef {
    ZkProof { verifier: Addr, proof_id: String },
    Tee { quote: Binary, measurement: Binary },
    Bridge { source_chain: String, tx_hash: String },
}

#[cw_serde]
pub enum Visibility {
    Public,
    Group(Vec<Addr>),
    Owner,
}

#[cw_serde]
pub struct Stats {
    pub total_entries: u64,
    pub total_active: u64,
    pub total_redacted: u64,
}

/// Per-moult-key epoch tracking for rate limiting
#[cw_serde]
pub struct MoultKeyState {
    pub entries_this_epoch: u32,
    pub last_epoch: u64,
}

/// Temporary state for pending ZK verification sub-messages
#[cw_serde]
pub struct PendingVerification {
    pub moult_key: Addr,
    pub topic_hash: String,
    pub content_cid: String,
    pub proof: Binary,
    pub public_inputs: Binary,
}

/// Temporary state for a pending voluntary-disclosure ZK verification.
#[cw_serde]
pub struct PendingDisclosure {
    pub entry_id: String,
    pub moult_key: Addr,
    pub primary_key: Addr,
}

/// Voluntary disclosure linking moult-key → primary identity
#[cw_serde]
pub struct Disclosure {
    pub entry_id: String,
    pub primary_key: Addr,
    pub disclosed_at: Timestamp,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATS: Item<Stats> = Item::new("stats");
pub const ENTRIES: Map<&str, MoultEntry> = Map::new("entries");
pub const BY_AUTHOR: Map<(&Addr, &str), ()> = Map::new("by_author");
pub const BY_REF: Map<(&str, &str), ()> = Map::new("by_ref");

pub const MOULT_KEY_STATE: Map<&Addr, MoultKeyState> = Map::new("moult_key_state");
pub const PENDING_VERIFICATION: Item<PendingVerification> = Item::new("pending_verify");
pub const PENDING_DISCLOSURE: Item<PendingDisclosure> = Item::new("pending_disclose");
pub const DISCLOSURES: Map<&str, Disclosure> = Map::new("disclosures");
pub const BY_MOULT_KEY: Map<(&Addr, &str), ()> = Map::new("by_moult_key");
/// Index: topic_hash → entry_id (for ListByTopic aggregation queries)
pub const BY_TOPIC: Map<(&str, &str), ()> = Map::new("by_topic");
