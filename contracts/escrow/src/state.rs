use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use junoclaw_common::ContractRegistry;

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    pub task_ledger: Addr,
    pub timeout_blocks: u64,
    pub denom: String,
    pub registry: ContractRegistry,
}

#[cw_serde]
pub struct LedgerStats {
    pub total_obligations: u64,
    pub total_pending: Uint128,
    pub total_confirmed: Uint128,
    pub total_disputed: Uint128,
    pub total_cancelled: Uint128,
}

// Keep old alias for migration compatibility
pub type EscrowStats = LedgerStats;

pub const CONFIG: Item<Config> = Item::new("config");
pub const OBLIGATIONS: Map<u64, junoclaw_common::PaymentObligation> = Map::new("obligations");
pub const OBLIGATIONS_BY_TASK: Map<u64, u64> = Map::new("obligations_by_task");
pub const NEXT_OBLIGATION_ID: Item<u64> = Item::new("next_obligation_id");
pub const LEDGER_STATS: Item<LedgerStats> = Item::new("ledger_stats");
