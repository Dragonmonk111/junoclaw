use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    /// Moultbook contract address for work-integrity score queries.
    /// Optional — if set, GetWorkIntegrityScore queries moultbook.
    pub moultbook_contract: Option<Addr>,
}

#[cw_serde]
pub struct Machine {
    /// Unique token ID (deterministic: "machine-<n>")
    pub token_id: String,
    /// Original minter — the entity that registered this machine
    pub minter: Addr,
    /// Machine model (e.g. "Unitree Go2", "Boston Dynamics Spot")
    pub model: String,
    /// Manufacturer serial number
    pub serial_number: String,
    /// Sensor suite description (e.g. "LiDAR+IMU+stereo+thermal")
    pub sensor_suite: String,
    /// IPFS URI for extended metadata (photos, specs, maintenance log)
    pub ipfs_metadata: String,
    /// Moultbook author address — the agent whose work-integrity score backs this machine
    pub moultbook_author: String,
    /// Block height at mint
    pub minted_at: u64,
    /// Burned?
    pub burned: bool,
}

#[cw_serde]
pub struct FractionalOwner {
    pub owner: Addr,
    /// Ownership in basis points (1 BP = 0.01%, so 10000 BP = 100%)
    pub basis_points: u32,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const MACHINES: Map<&str, Machine> = Map::new("machines");
/// token_id → (owner_addr, basis_points)
pub const FRACTIONS: Map<(&str, &Addr), u32> = Map::new("fractions");
/// owner_addr → token_id (index for listing machines by owner)
pub const BY_OWNER: Map<(&Addr, &str), ()> = Map::new("by_owner_rwa");
pub const NEXT_TOKEN_ID: Item<u64> = Item::new("next_token_id");
