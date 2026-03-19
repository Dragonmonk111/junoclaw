use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};
use junoclaw_common::AssetInfo;

#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub pair_code_id: u64,
    pub default_fee_bps: u16,
    pub junoclaw_contract: Option<Addr>,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const PAIR_COUNT: Item<u64> = Item::new("pair_count");
pub const PAIRS: Map<(&str, &str), Addr> = Map::new("pairs");
pub const ALL_PAIRS: Map<u64, PairRecord> = Map::new("all_pairs");

#[cw_serde]
pub struct PairRecord {
    pub id: u64,
    pub pair_addr: Addr,
    pub token_a: AssetInfo,
    pub token_b: AssetInfo,
    pub created_at: u64,
}
