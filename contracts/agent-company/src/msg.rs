use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use crate::state::{
    Attestation, CodeUpgradeAction, Config, Member, NoisCallback, PaymentRecord, PendingSortition,
    Proposal, SortitionRound, VerificationConfig, VoteOption, WeightProposal,
};
use junoclaw_common::ExecutionTier;

#[cw_serde]
pub struct MemberInput {
    pub addr: String,
    pub weight: u64,
    pub role: crate::state::MemberRole,
}

/// Input representation of ProposalKind using raw strings (not validated Addrs)
#[cw_serde]
pub enum ProposalKindMsg {
    WeightChange { members: Vec<MemberInput> },
    WavsPush {
        task_description: String,
        execution_tier: ExecutionTier,
        escrow_amount: Uint128,
    },
    ConfigChange {
        new_admin: Option<String>,
        new_governance: Option<String>,
    },
    FreeText {
        title: String,
        description: String,
    },
    OutcomeCreate {
        question: String,
        resolution_criteria: String,
        deadline_block: u64,
    },
    OutcomeResolve {
        market_id: u64,
        outcome: bool,
        attestation_hash: String,
    },
    SortitionRequest {
        count: u32,
        purpose: String,
    },
    /// Code upgrade proposal: bundles store/instantiate/migrate/execute actions.
    /// Requires supermajority quorum (default 67%) to pass.
    CodeUpgrade {
        title: String,
        description: String,
        actions: Vec<CodeUpgradeAction>,
    },
}

#[cw_serde]
pub struct InstantiateMsg {
    /// Human-readable company name
    pub name: String,
    /// Defaults to sender if None
    pub admin: Option<String>,
    /// Optional DAODAO dao-core address; if set, weight proposals require governance approval
    pub governance: Option<String>,
    pub escrow_contract: String,
    pub agent_registry: String,
    /// Optional task-ledger address for WavsPush proposals
    pub task_ledger: Option<String>,
    /// Optional NOIS proxy address for on-chain randomness
    pub nois_proxy: Option<String>,
    /// Initial member roster. Weights must sum to 10,000.
    pub members: Vec<MemberInput>,
    /// Native token denom. Defaults to "ujunox".
    pub denom: Option<String>,
    // ── Governance parameters (all optional with defaults) ──
    /// Voting period in blocks (default 100)
    pub voting_period_blocks: Option<u64>,
    /// Quorum percentage of total_weight (default 51)
    pub quorum_percent: Option<u64>,
    /// Adaptive threshold: blocks within which all-vote triggers reduction (default 10)
    pub adaptive_threshold_blocks: Option<u64>,
    /// Adaptive minimum: floor blocks for reduced deadline (default 13)
    pub adaptive_min_blocks: Option<u64>,
    /// DAO-wide verification config (defaults to V2+V3)
    pub verification: Option<VerificationConfig>,
    /// Supermajority quorum for constitutional proposals
    /// (`CodeUpgrade` and `WeightChange`) (default 67)
    pub supermajority_quorum_percent: Option<u64>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Distribute ujuno sent with this message equally-weighted across members.
    DistributePayment { task_id: u64 },

    // ── New general governance ──

    /// Create a new proposal. Any DAO member can propose.
    CreateProposal { kind: ProposalKindMsg },

    /// Cast a vote on an open proposal. Any DAO member, one vote per member.
    CastVote { proposal_id: u64, vote: VoteOption },

    /// Execute a passed proposal after the voting deadline.
    ExecuteProposal { proposal_id: u64 },

    /// Mark an expired proposal (deadline passed without quorum).
    ExpireProposal { proposal_id: u64 },

    // ── Legacy (decode-compatible; disabled in v5 execute path) ──

    /// Legacy message retained for backward decoding compatibility.
    /// Disabled at runtime in v5; use `CreateProposal { kind: WeightChange }`.
    ProposeWeightChange { members: Vec<MemberInput> },

    /// Legacy message retained for backward decoding compatibility.
    /// Disabled at runtime in v5; use `ExecuteProposal { proposal_id }`.
    ExecuteWeightProposal {},

    /// Legacy message retained for backward decoding compatibility.
    /// Disabled at runtime in v5.
    CancelWeightProposal {},

    /// Direct weight update — only callable if governance is None.
    UpdateMembers { members: Vec<MemberInput> },

    /// Transfer admin to a new address (or governance contract).
    TransferAdmin { new_admin: String },

    // ── Randomness ──

    /// NOIS proxy callback delivering randomness for a pending sortition
    NoisReceive { callback: NoisCallback },

    /// WAVS operator submits drand randomness for a pending sortition job
    SubmitRandomness {
        job_id: String,
        randomness_hex: String,
        attestation_hash: String,
    },

    // ── WAVS Attestations ──

    /// WAVS bridge submits a verification attestation for a WavsPush or OutcomeCreate proposal
    SubmitAttestation {
        proposal_id: u64,
        task_type: String,
        data_hash: String,
        attestation_hash: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},

    #[returns(Vec<Member>)]
    GetMembers {},

    // ── New proposal queries ──

    #[returns(Proposal)]
    GetProposal { proposal_id: u64 },

    #[returns(Vec<Proposal>)]
    ListProposals { start_after: Option<u64>, limit: Option<u32> },

    // ── Legacy ──

    #[returns(Option<WeightProposal>)]
    GetPendingProposal {},

    #[returns(PaymentRecord)]
    GetPaymentRecord { task_id: u64 },

    #[returns(Vec<PaymentRecord>)]
    GetPaymentHistory { start_after: Option<u64>, limit: Option<u32> },

    #[returns(cosmwasm_std::Uint128)]
    GetMemberEarnings { addr: String },

    // ── Sortition ──

    #[returns(SortitionRound)]
    GetSortitionRound { round_id: u64 },

    #[returns(Vec<SortitionRound>)]
    ListSortitionRounds { start_after: Option<u64>, limit: Option<u32> },

    #[returns(Option<PendingSortition>)]
    GetPendingSortition { job_id: String },

    // ── Attestations ──

    #[returns(Option<Attestation>)]
    GetAttestation { proposal_id: u64 },

    #[returns(Vec<Attestation>)]
    ListAttestations { start_after: Option<u64>, limit: Option<u32> },
}

#[cw_serde]
pub struct MigrateMsg {}
