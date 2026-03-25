use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    /// Admin who can fund/withdraw/update config
    pub admin: Addr,
    /// Amount in native denom to dispense per claim (e.g. 100_000_000 for 100 JUNOX)
    pub drip_amount: u128,
    /// Native denom (e.g. "ujunox")
    pub denom: String,
    /// Total number of claims made
    pub total_claims: u64,
    /// Whether the faucet is active
    pub active: bool,
}

/// Faucet configuration
pub const CONFIG: Item<Config> = Item::new("config");

/// Set of addresses that have already claimed — value is block height of claim
pub const CLAIMED: Map<&Addr, u64> = Map::new("claimed");
