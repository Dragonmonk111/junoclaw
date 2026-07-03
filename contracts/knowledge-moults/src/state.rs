use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp};
use cw_storage_plus::{Item, Map};

/// Contract configuration. `mother_moult_id` is the Moultbook entry id of the
/// DAO's current canonical Mother-Moult (A18c-4). It is a pointer, not a copy:
/// this contract never re-fetches or validates its content, it only stamps
/// newly-minted Knowledge Moults with whatever is configured at mint time.
/// Superseding the Mother-Moult (via a future DAO proposal) does not rewrite
/// already-minted moults — each one is a permanent snapshot of what the
/// Commonwealth's root knowledge artifact was when it was minted.
#[cw_serde]
pub struct Config {
    pub admin: Addr,
    pub mother_moult_id: String,
    pub max_summary_len: u32,
    pub max_source_moults: u32,
}

/// A single Knowledge Moult: a reproducible, ownable artifact of agentic
/// knowledge, minted when an agent completes a motive. Not a full CW721 —
/// deliberately minimal, mirroring moultbook-v0's style (see ADR-002).
#[cw_serde]
pub struct KnowledgeMoult {
    pub id: String,
    pub owner: Addr,
    pub minter: Addr,
    /// Agent alias, e.g. "hermes", "dragonmonk111-bot". Not identity-gated —
    /// same permissionless-authorship philosophy as Moultbook's `Post`.
    pub agent: String,
    /// What motive/mandate/thread this knowledge came from, e.g. "A18c-3-ui-decision".
    pub motive: String,
    pub knowledge_summary: String,
    /// Moultbook entry ids (`moult:...`) this Knowledge Moult was derived from.
    /// Reproducibility check: anyone can re-fetch these and verify the summary.
    pub source_moults: Vec<String>,
    /// Snapshot of `Config.mother_moult_id` at mint time (see Config doc above).
    pub mother_moult_id: String,
    pub minted_at: Timestamp,
}

#[cw_serde]
pub struct Stats {
    pub total_minted: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATS: Item<Stats> = Item::new("stats");

/// Primary store: id -> KnowledgeMoult.
pub const MOULTS: Map<&str, KnowledgeMoult> = Map::new("moults");

/// Ownership index: (owner, id) -> (). Updated on mint and transfer.
pub const BY_OWNER: Map<(&Addr, &str), ()> = Map::new("by_owner");

/// Authorship index: (agent, id) -> (). Agent alias is a free-text string, not
/// an Addr, so it is keyed as `&str` rather than `&Addr`.
pub const BY_AGENT: Map<(&str, &str), ()> = Map::new("by_agent");
