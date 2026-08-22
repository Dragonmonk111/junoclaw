use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

/// Reward distribution mode for epoch finalization.
#[cw_serde]
pub enum RewardMode {
    /// Equal split among all matching operators (default, backwards-compatible).
    Equal,
    /// Stake-weighted: each matching operator gets a share proportional to their stake.
    StakeWeighted,
    /// Stake × accuracy: share proportional to stake times historical accuracy (Laplace-smoothed).
    /// New operators start at accuracy = 1/1 = 100% (benefit of the doubt).
    StakeTimesAccuracy,
}

impl Default for RewardMode {
    fn default() -> Self {
        RewardMode::Equal
    }
}

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
    /// Minimum number of operators required to finalize an epoch.
    /// Prevents a single operator from trivially self-consensing.
    pub min_operators: u32,
    /// How rewards are distributed among matching operators.
    pub reward_mode: RewardMode,
    /// Per-batch verification fee (in denom's smallest unit).
    /// When >0, PayVerificationFee requires exactly this amount.
    /// 0 = open access mode (no fee enforcement).
    pub verification_fee: Uint128,
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
    /// Self-reported operator fingerprint (model + host hash) for diversity signal.
    /// Not enforced on-chain — relayers use this to detect correlated operators.
    /// None means the operator did not declare a fingerprint.
    pub fingerprint: Option<String>,
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

/// Map from fingerprint string to count of operators with that fingerprint.
/// Used by the GetFingerprints query for relayer-side diversity checks.
pub const FINGERPRINT_COUNTS: Map<&str, u64> = Map::new("fp_counts");
