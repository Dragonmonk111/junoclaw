use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    /// Native token denom used for the anti-spam registration fee (e.g. "ujuno").
    pub denom: String,
    /// Fee charged on the *first* `PublishSkill` for a given `dapp_name`.
    /// Zero disables the fee entirely. Updates to an already-claimed name
    /// never charge a fee again.
    pub registration_fee: Uint128,
}

#[cw_serde]
pub struct SkillEntry {
    pub dapp_name: String,
    /// Address that first published this entry — the only address (besides
    /// admin, for dispute resolution) allowed to call `UpdateSkill` on it.
    pub publisher: Addr,
    /// Chain-id the dApp actually runs on (e.g. "juno-1", "osmosis-1").
    /// The registry itself always lives on Juno; this field is what makes
    /// it useful for *any* interchain dApp, not just Juno-native ones.
    pub chain_id: String,
    /// Where the actual SKILL.md-equivalent content lives: ipfs://, https://, ar://
    pub skill_uri: String,
    /// sha256 of the content at `skill_uri`, so any agent fetching it can
    /// verify integrity before trusting the manual.
    pub skill_hash: String,
    /// Bumped on every `UpdateSkill` call.
    pub version: u64,
    pub updated_at: u64,
}

#[cw_serde]
pub struct RegistryStats {
    pub total_entries: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const SKILLS: Map<&str, SkillEntry> = Map::new("skills");
pub const REGISTRY_STATS: Item<RegistryStats> = Item::new("registry_stats");
