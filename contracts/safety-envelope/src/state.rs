use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

use crate::msg::SafetyEnvelopeParams;

/// Admin address (governance multisig or DAO)
pub const ADMIN: Item<Addr> = Item::new("admin");

/// Current safety envelope per robot: robot_id -> (params, version, updated_at, updated_by)
pub const ENVELOPES: Map<&str, EnvelopeRecord> = Map::new("envelopes");

/// Version count per robot (incremented on each update)
pub const VERSION_COUNTS: Map<&str, u32> = Map::new("version_counts");

#[cw_serde]
pub struct EnvelopeRecord {
    pub params: SafetyEnvelopeParams,
    pub version: u32,
    pub updated_at: u64,
    pub updated_by: String,
}
