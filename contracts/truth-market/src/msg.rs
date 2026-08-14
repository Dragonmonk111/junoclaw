use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub min_stake: Uint128,
    pub slash_percent: u8,
    pub reward_percent: u8,
    pub denom: String,
    pub unstake_cooldown_secs: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Register as a truth-market evaluator. Requires sending min_stake
    /// in the tx's funds field.
    RegisterOperator {},

    /// Submit a verdict for a batch height. Called by each evaluator
    /// independently before the epoch is finalized.
    SubmitVerdict {
        batch_height: u64,
        verdict: String,
        messages_hash: String,
    },

    /// Finalize an epoch — compares all submitted verdicts against
    /// the consensus verdict, distributes rewards, and slashes
    /// diverging operators. Can only be called by the authorized
    /// relayer (set via config admin).
    FinalizeEpoch {
        batch_height: u64,
        consensus_verdict: String,
        messages_hash: String,
    },

    /// Request to unstake and withdraw. Starts the cooldown timer.
    RequestUnstake {},

    /// Complete unstake withdrawal after cooldown has elapsed.
    WithdrawUnstake {},

    /// Deactivate an operator (stop participating in eval epochs).
    Deactivate {},

    /// Reactivate a deactivated operator.
    Reactivate {},

    /// Update config (admin only).
    UpdateConfig {
        min_stake: Option<Uint128>,
        slash_percent: Option<u8>,
        reward_percent: Option<u8>,
        unstake_cooldown_secs: Option<u64>,
    },

    /// Deposit funds into the reward pool (anyone can contribute).
    DepositRewards {},
}

#[cw_serde]
pub enum QueryMsg {
    GetConfig {},
    GetOperator { address: String },
    ListOperators {},
    GetVerdict { batch_height: u64, operator: String },
    GetEpoch { batch_height: u64 },
    GetStats {},
    GetRewardPool {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub min_stake: Uint128,
    pub slash_percent: u8,
    pub reward_percent: u8,
    pub denom: String,
    pub unstake_cooldown_secs: u64,
}

#[cw_serde]
pub struct OperatorResponse {
    pub address: String,
    pub stake: Uint128,
    pub total_rewards: Uint128,
    pub total_slashed: Uint128,
    pub epochs_participated: u64,
    pub correct_verdicts: u64,
    pub incorrect_verdicts: u64,
    pub active: bool,
    pub accuracy: u64,
}

#[cw_serde]
pub struct OperatorsResponse {
    pub operators: Vec<OperatorResponse>,
}

#[cw_serde]
pub struct EpochResponse {
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
pub struct StatsResponse {
    pub total_operators: u64,
    pub active_operators: u64,
    pub total_staked: Uint128,
    pub total_rewards_paid: Uint128,
    pub total_slashed: Uint128,
    pub epochs_finalized: u64,
    pub reward_pool: Uint128,
}
