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

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATS: Item<Stats> = Item::new("stats");
pub const ENTRIES: Map<&str, MoultEntry> = Map::new("entries");
pub const BY_AUTHOR: Map<(&Addr, &str), ()> = Map::new("by_author");
pub const BY_REF: Map<(&str, &str), ()> = Map::new("by_ref");
