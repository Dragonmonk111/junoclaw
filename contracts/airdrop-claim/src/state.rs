use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

/// Admin who can set merkle root and sweep unclaimed tokens
pub const ADMIN: Item<Addr> = Item::new("admin");

/// The denom of the airdrop token (ujclaw)
pub const DENOM: Item<String> = Item::new("denom");

/// Merkle root of the airdrop distribution (hex-encoded SHA-256)
pub const MERKLE_ROOT: Item<String> = Item::new("merkle_root");

/// Block height when the claim window starts
pub const CLAIM_START: Item<u64> = Item::new("claim_start");

/// Block height when the claim window ends (unclaimed → community pool)
pub const CLAIM_END: Item<u64> = Item::new("claim_end");

/// Community pool address for swept unclaimed tokens
pub const COMMUNITY_POOL: Item<Addr> = Item::new("community_pool");

/// Total airdrop amount
pub const TOTAL_AMOUNT: Item<u128> = Item::new("total_amount");

/// Total claimed so far
pub const TOTAL_CLAIMED: Item<u128> = Item::new("total_claimed");

/// Set of addresses that have already claimed
pub const CLAIMED: Map<&str, u128> = Map::new("claimed");
