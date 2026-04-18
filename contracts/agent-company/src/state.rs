use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use junoclaw_common::ExecutionTier;

// ──────────────────────────────────────────────
// Member types
// ──────────────────────────────────────────────

#[cw_serde]
pub enum MemberRole {
    Agent,
    Human,
    SubDAO,
}

#[cw_serde]
pub struct Member {
    pub addr: Addr,
    /// Weight in basis points (1 = 0.01%). Sum of all members must equal 10_000.
    pub weight: u64,
    pub role: MemberRole,
}

// ──────────────────────────────────────────────
// Verification (V2 witness + V3 WAVS, DAO-wide default)
// ──────────────────────────────────────────────

#[cw_serde]
pub enum VerificationModel {
    /// No verification required
    None,
    /// V2: M-of-N human witnesses sign attestations
    Witness,
    /// V3: WAVS operators re-verify via API/chain query
    Wavs,
    /// V2+V3 combined
    WitnessAndWavs,
}

#[cw_serde]
pub struct VerificationConfig {
    pub model: VerificationModel,
    /// M in M-of-N witness attestations
    pub required_attestations: u32,
    /// N total witnesses (0 if model is Wavs-only)
    pub total_witnesses: u32,
    /// Blocks witnesses/operators have to submit attestations
    pub attestation_timeout_blocks: u64,
    /// If true, escrow auto-releases on successful verification
    pub auto_release_on_verify: bool,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            model: VerificationModel::WitnessAndWavs,
            required_attestations: 2,
            total_witnesses: 3,
            attestation_timeout_blocks: 200,
            auto_release_on_verify: true,
        }
    }
}

// ──────────────────────────────────────────────
// Config
// ──────────────────────────────────────────────

#[cw_serde]
pub struct Config {
    pub name: String,
    pub admin: Addr,
    /// Optional on-chain governance contract (e.g. DAODAO dao-core).
    pub governance: Option<Addr>,
    pub escrow_contract: Addr,
    pub agent_registry: Addr,
    /// Optional task-ledger address for WavsPush proposals
    pub task_ledger: Option<Addr>,
    pub members: Vec<Member>,
    /// Always 10_000; stored for assertion convenience.
    pub total_weight: u64,
    pub denom: String,
    // ── Governance parameters ──
    /// Blocks from proposal creation to voting deadline (default 100)
    pub voting_period_blocks: u64,
    /// Percentage of total_weight needed for quorum (default 51)
    pub quorum_percent: u64,
    /// If all members vote within this many blocks, deadline shrinks (default 10)
    pub adaptive_threshold_blocks: u64,
    /// Floor: earliest block offset a proposal can finalize after adaptive reduction (default 13)
    pub adaptive_min_blocks: u64,
    /// DAO-wide verification policy for WavsPush proposals
    pub verification: VerificationConfig,
    /// Optional NOIS proxy address for on-chain randomness (sortition, arbitration)
    pub nois_proxy: Option<Addr>,
    /// Supermajority quorum for constitutional proposals — `CodeUpgrade` and
    /// `WeightChange` — default 67. Ordinary proposals use `quorum_percent`.
    pub supermajority_quorum_percent: u64,
    /// Optional Junoswap factory address (set via CodeUpgrade proposal)
    pub dex_factory: Option<Addr>,
}

// ──────────────────────────────────────────────
// Proposal & Voting
// ──────────────────────────────────────────────

#[cw_serde]
pub enum ProposalKind {
    /// Change member weights (replaces old WeightProposal)
    WeightChange { members: Vec<Member> },
    /// Push a company decision to WAVS for off-chain execution
    WavsPush {
        task_description: String,
        execution_tier: ExecutionTier,
        escrow_amount: Uint128,
    },
    /// Change DAO config (admin, governance, etc.)
    ConfigChange {
        new_admin: Option<String>,
        new_governance: Option<String>,
    },
    /// Free-text proposal (signal vote, no on-chain side-effects)
    FreeText {
        title: String,
        description: String,
    },
    /// Create a verifiable outcome market
    OutcomeCreate {
        question: String,
        resolution_criteria: String,
        deadline_block: u64,
    },
    /// Resolve a verifiable outcome market with WAVS-attested outcome
    OutcomeResolve {
        market_id: u64,
        outcome: bool,
        attestation_hash: String,
    },
    /// Request random selection of N members from the DAO (sortition)
    SortitionRequest {
        count: u32,
        purpose: String,
    },
    /// Code upgrade: bundle store/instantiate/migrate actions behind supermajority quorum
    CodeUpgrade {
        title: String,
        description: String,
        actions: Vec<CodeUpgradeAction>,
    },
}

/// Individual action within a CodeUpgrade proposal
#[cw_serde]
pub enum CodeUpgradeAction {
    /// Upload WASM code to the chain, returning a code_id
    StoreCode {
        /// Label for tracking (e.g. "junoswap-factory")
        label: String,
        /// Base64-encoded WASM binary
        wasm_base64: String,
    },
    /// Instantiate a new contract from an existing code_id
    InstantiateContract {
        label: String,
        code_id: u64,
        /// JSON-encoded instantiate message
        msg_json: String,
        /// Optional admin for the new contract
        admin: Option<String>,
    },
    /// Migrate an existing contract to a new code_id
    MigrateContract {
        contract_addr: String,
        new_code_id: u64,
        /// JSON-encoded migrate message
        msg_json: String,
    },
    /// Execute a message on an existing contract
    ExecuteContract {
        contract_addr: String,
        /// JSON-encoded execute message
        msg_json: String,
    },
    /// Update this DAO's config to wire in new contract addresses
    SetDexFactory {
        factory_addr: String,
    },
}

#[cw_serde]
pub enum VoteOption {
    Yes,
    No,
    Abstain,
}

#[cw_serde]
pub struct Vote {
    pub voter: Addr,
    pub option: VoteOption,
    pub weight: u64,
    pub block_height: u64,
}

#[cw_serde]
#[derive(Copy)]
pub enum ProposalStatus {
    Open,
    Passed,
    Rejected,
    Executed,
    Expired,
}

#[cw_serde]
pub struct Proposal {
    pub id: u64,
    pub proposer: Addr,
    pub kind: ProposalKind,
    pub votes: Vec<Vote>,
    pub yes_weight: u64,
    pub no_weight: u64,
    pub abstain_weight: u64,
    pub total_voted_weight: u64,
    pub status: ProposalStatus,
    pub created_at_block: u64,
    /// Current voting deadline (may shrink via adaptive logic)
    pub voting_deadline_block: u64,
    /// Floor after adaptive reduction
    pub min_deadline_block: u64,
    pub executed: bool,
}

// ──────────────────────────────────────────────
// Legacy weight-change proposal (kept for migration compatibility)
// ──────────────────────────────────────────────

#[cw_serde]
pub struct WeightProposal {
    pub id: u64,
    pub proposer: Addr,
    pub proposed_members: Vec<Member>,
    pub executable_after: u64,
    pub executed: bool,
}

// ──────────────────────────────────────────────
// Payment
// ──────────────────────────────────────────────

#[cw_serde]
pub struct PaymentRecord {
    pub task_id: u64,
    pub total_amount: Uint128,
    pub distributed_at: u64, // block height
}

// ──────────────────────────────────────────────
// Storage keys
// ──────────────────────────────────────────────

pub const CONFIG: Item<Config> = Item::new("config");
/// Legacy single-proposal slot (kept for backward compat)
pub const PROPOSAL: Item<Option<WeightProposal>> = Item::new("proposal");
pub const PROPOSAL_SEQ: Item<u64> = Item::new("proposal_seq");
/// General proposals map (id → Proposal)
pub const PROPOSALS: Map<u64, Proposal> = Map::new("proposals");
/// Per-member cumulative payments received (addr → total_ujuno)
pub const MEMBER_EARNINGS: Map<&Addr, Uint128> = Map::new("member_earnings");
pub const PAYMENT_HISTORY: Map<u64, PaymentRecord> = Map::new("payment_history");

// ──────────────────────────────────────────────
// Randomness & Sortition
// ──────────────────────────────────────────────

#[cw_serde]
pub struct NoisCallback {
    pub job_id: String,
    /// 32 bytes of randomness, hex-encoded (64 chars)
    pub randomness: String,
}

#[cw_serde]
pub struct PendingSortition {
    pub proposal_id: u64,
    pub count: u32,
    pub purpose: String,
    /// Snapshot of eligible member addresses at time of request
    pub eligible: Vec<Addr>,
}

#[cw_serde]
pub struct SortitionRound {
    pub id: u64,
    /// Proposal that triggered this sortition
    pub proposal_id: u64,
    pub purpose: String,
    /// Members selected by the randomness
    pub selected: Vec<Addr>,
    /// Size of eligible pool at time of selection
    pub pool_size: u32,
    /// Source identifier (e.g. "nois:job_123" or "wavs_drand:round_456")
    pub randomness_source: String,
    /// Hex-encoded 32-byte randomness used
    pub randomness_hex: String,
    pub created_at_block: u64,
}

pub const SORTITION_SEQ: Item<u64> = Item::new("sortition_seq");
pub const SORTITION_ROUNDS: Map<u64, SortitionRound> = Map::new("sortition_rounds");
/// Pending sortition requests awaiting randomness: job_id → PendingSortition
pub const PENDING_SORTITION: Map<&str, PendingSortition> = Map::new("pending_sortition");

// ──────────────────────────────────────────────
// WAVS Attestations
// ──────────────────────────────────────────────

#[cw_serde]
pub struct Attestation {
    /// Proposal that triggered the off-chain verification
    pub proposal_id: u64,
    /// Type of verification performed (e.g. "data_verify", "outcome_verify")
    pub task_type: String,
    /// SHA-256 hash of the verified data
    pub data_hash: String,
    /// Attestation hash from the WAVS component (component_id || task_type || data_hash)
    pub attestation_hash: String,
    /// Block height when attestation was submitted
    pub submitted_at_block: u64,
    /// Address that submitted the attestation (bridge/operator)
    pub submitter: Addr,
}

/// Attestations keyed by proposal_id
pub const ATTESTATIONS: Map<u64, Attestation> = Map::new("attestations");
