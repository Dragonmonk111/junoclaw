use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Uint128};
use crate::state::{
    Attestation, CodeUpgradeAction, Config, Member, NoisCallback, PaymentRecord, PendingSortition,
    Proposal, SignRequest, SignRequestStatus, SortitionRound, VerificationConfig, VoteOption,
    WeightProposal,
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
        #[serde(default)]
        new_wavs_operator: Option<String>,
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
    #[serde(default)]
    pub wavs_operator: Option<String>,
    /// Optional `zk-verifier` contract address. When set, attestations
    /// may optionally carry a Groth16 proof bundle that is cross-verified
    /// atomically. When `None`, attestation flow is unchanged (hash-only).
    #[serde(default)]
    pub zk_verifier: Option<String>,
    /// Optional `moultbook-v0` contract address. When set, Skill-Staking
    /// Circle and similar templates can publish anonymous, ZK-protected
    /// endorsements (ADR-005). When `None`, the anonymous endorsement path
    /// is skipped and the DAO pays no additional gas.
    #[serde(default)]
    pub moultbook: Option<String>,
    /// Optional relayer address authorized to submit `RequestSignedTx`
    /// events for the WAVS sealed signer.
    #[serde(default)]
    pub relayer: Option<String>,
    /// Optional sealed-signer (enclave) address. When set, the contract
    /// only requests signatures from this sender.
    #[serde(default)]
    pub sealed_signer: Option<String>,
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

impl Default for InstantiateMsg {
    fn default() -> Self {
        Self {
            name: String::new(),
            admin: None,
            governance: None,
            wavs_operator: None,
            zk_verifier: None,
            moultbook: None,
            relayer: None,
            sealed_signer: None,
            escrow_contract: String::new(),
            agent_registry: String::new(),
            task_ledger: None,
            nois_proxy: None,
            members: vec![],
            denom: None,
            voting_period_blocks: None,
            quorum_percent: None,
            adaptive_threshold_blocks: None,
            adaptive_min_blocks: None,
            verification: None,
            supermajority_quorum_percent: None,
        }
    }
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

    /// Admin-only: rotate the WAVS operator (the off-chain signer whose
    /// address is trusted for `SubmitAttestation` and `SubmitRandomness`).
    /// `new_operator = None` *clears* the operator entirely, disabling WAVS
    /// submissions until a new one is set. Admin-only because this is a
    /// bootstrap / emergency rotation knob; the decentralized rotation path
    /// is a `ConfigChange` governance proposal with `new_wavs_operator`.
    RotateWavsOperator { new_operator: Option<String> },

    /// Admin-only: rotate the zk-verifier contract address. `new_verifier
    /// = None` clears it, reverting `SubmitAttestation` to hash-only mode.
    /// Admin-only for the same reason as `RotateWavsOperator`: a bootstrap
    /// / emergency knob. Decentralized rotation is a future `ConfigChange`
    /// extension.
    RotateZkVerifier { new_verifier: Option<String> },

    /// Admin-only: rotate the moultbook contract address. `new_moultbook =
    /// None` clears it, disabling the anonymous endorsement path entirely
    /// (DAOs reverting to attributed-only reputation pay no extra gas).
    /// Admin-only as a bootstrap / emergency knob; decentralized rotation
    /// is a future `ConfigChange` extension. See ADR-005.
    RotateMoultbook { new_moultbook: Option<String> },

    /// Admin-only: rotate the dedicated relayer address that may submit
    /// `RequestSignedTx` events for the sealed signer to process.
    /// `new_relayer = None` disables the sign-request flow.
    RotateRelayer { new_relayer: Option<String> },

    /// Admin-only: rotate the trusted sealed-signer (enclave) address.
    /// `new_signer = None` disables the sign-request flow.
    RotateSealedSigner { new_signer: Option<String> },

    // ── Sealed-signer relayer ──

    /// Relayer-only: request a `SIGN_MODE_DIRECT` signature for a single
    /// `MsgExecuteContract`. The contract validates the request against the
    /// configured `sealed_signer` and `moultbook` addresses, enforces fee/gas
    /// caps, and emits a `sign_request` event for WAVS to consume.
    RequestSignedTx {
        sender: String,
        contract: String,
        exec_msg_json: String,
        funds_denom: String,
        funds_amount: Uint128,
        gas_limit: u64,
        fee_denom: String,
        fee_amount: Uint128,
        memo: String,
        chain_id: String,
        account_number: u64,
        sequence: u64,
    },

    /// WAVS-only: store the signed `TxRaw` bytes returned by the sealed
    /// signer. Callable only by the configured `wavs_operator`.
    StoreSignedTx {
        id: u64,
        tx_bytes: Binary,
        sign_doc_sha256_hex: String,
    },

    /// Relayer-only: mark a signed tx as broadcast and remove it from
    /// pending storage, allowing a new request to be created.
    AckBroadcastTx { id: u64 },

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

    /// WAVS bridge submits a verification attestation for a WavsPush or OutcomeCreate proposal.
    ///
    /// When `Config.zk_verifier` is wired, callers MAY additionally supply
    /// a Groth16 proof bundle (`proof_base64` + `public_inputs_base64`).
    /// If supplied, the contract fires a sub-message to the zk-verifier
    /// which re-verifies the proof on-chain; sub-message failure reverts
    /// the whole attestation atomically. Supplying only one of the two
    /// proof fields is rejected (`IncompleteZkProofBundle`). Supplying
    /// proof fields without a configured verifier is rejected
    /// (`ZkVerifierNotConfigured`) — fail-closed, never silent-drop.
    SubmitAttestation {
        proposal_id: u64,
        task_type: String,
        data_hash: String,
        attestation_hash: String,
        #[serde(default)]
        proof_base64: Option<String>,
        #[serde(default)]
        public_inputs_base64: Option<String>,
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

    // ── Sealed-signer requests ──

    #[returns(Option<SignRequest>)]
    GetSignRequest { id: u64 },

    #[returns(Vec<SignRequest>)]
    ListSignRequests {
        status: Option<SignRequestStatus>,
        start_after: Option<u64>,
        limit: Option<u32>,
    },

    // ── Attestations ──

    #[returns(Option<Attestation>)]
    GetAttestation { proposal_id: u64 },

    #[returns(Vec<Attestation>)]
    ListAttestations { start_after: Option<u64>, limit: Option<u32> },
}

#[cw_serde]
pub struct MigrateMsg {}
