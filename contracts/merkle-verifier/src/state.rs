use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

/// Admin address
pub const ADMIN: Item<Addr> = Item::new("admin");

/// Composite key: (robot_id, batch_height) -> BatchRoot
pub const ROOTS: Map<(&str, u64), BatchRoot> = Map::new("roots");

#[cw_serde]
pub struct BatchRoot {
    pub merkle_root: String,
    pub cycle_count: u32,
    pub anchored_at: u64,
}
