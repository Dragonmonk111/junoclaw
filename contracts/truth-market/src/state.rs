use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    /// Minimum stake to register as an evaluator (in ujuno)
    pub min_stake: Uint128,
    /// Slash percentage for diverging evaluators (0-100)
    pub slash_percent: u8,
    /// Reward percentage from the eval fee pool for matching evaluators (0-100)
    pub reward_percent: u8,
    /// Token denom (e.g. "ujuno")
    pub denom: String,
    /// Cooldown in seconds before a withdrawn stake is released
    pub unstake_cooldown_secs: u64,
}

#[cw_serde]
pub struct Operator {
    /// Operator's address (also their signing key)
    pub address: Addr,
    /// Staked amount
    pub stake: Uint128,
    /// Total rewards earned
    pub total_rewards: Uint128,
    /// Total amount slashed
    pub total_slashed: Uint128,
    /// Number of epochs participated
    pub epochs_participated: u64,
    /// Number of correct verdicts (matched consensus)
    pub correct_verdicts: u64,
    /// Number of incorrect verdicts (diverged from consensus)
    pub incorrect_verdicts: u64,
    /// Whether currently active
    pub active: bool,
    /// Unstake request timestamp (0 = no pending withdrawal)
    pub unstake_request_time: u64,
}

#[cw_serde]
pub struct VerdictRecord {
    pub operator: Addr,
    pub verdict: String,
    pub batch_height: u64,
}

#[cw_serde]
pub struct EpochResult {
    pub batch_height: u64,
    pub consensus_verdict: String,
    pub messages_hash: String,
    pub total_operators: u64,
    pub matching_operators: u64,
    pub diverging_operators: u64,
    pub rewards_distributed: Uint128,
    pub slashed_amount: Uint128,
    pub finalized: bool,
}

#[cw_serde]
pub struct MarketStats {
    pub total_operators: u64,
    pub active_operators: u64,
    pub total_staked: Uint128,
    pub total_rewards_paid: Uint128,
    pub total_slashed: Uint128,
    pub epochs_finalized: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const OPERATORS: Map<&Addr, Operator> = Map::new("operators");
pub const VERDICTS: Map<(u64, &Addr), VerdictRecord> = Map::new("verdicts");
pub const EPOCHS: Map<u64, EpochResult> = Map::new("epochs");
pub const NEXT_OPERATOR_ID: Item<u64> = Item::new("next_operator_id");
pub const MARKET_STATS: Item<MarketStats> = Item::new("market_stats");
pub const REWARD_POOL: Item<Uint128> = Item::new("reward_pool");
