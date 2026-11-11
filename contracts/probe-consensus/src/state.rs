use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const REQUIRED: Item<u32> = Item::new("required");
pub const OPERATORS: Map<Addr, ()> = Map::new("operators");

/// Per-(robot, batch, operator) commit — one per operator per batch.
pub const COMMITMENTS: Map<(&str, u64, &Addr), Commit> = Map::new("commitments");

/// Agreed roots: only set once quorum of identical commits exists.
pub const AGREED: Map<(&str, u64), AgreedRoot> = Map::new("agreed");

#[cw_serde]
pub struct Commit {
    pub merkle_root: String,
    pub cycle_count: u32,
    pub committed_at: u64,
}

#[cw_serde]
pub struct AgreedRoot {
    pub merkle_root: String,
    pub cycle_count: u32,
    pub commits: Vec<String>,
    pub agreed_at: u64,
}
