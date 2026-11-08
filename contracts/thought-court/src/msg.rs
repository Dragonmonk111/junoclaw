use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

use crate::state::{Attestation, AttestationStatus, Config, Dispute, Verdict, Verifier};

#[cw_serde]
pub struct InstantiateMsg {
    /// DAO / governance address. Defaults to the instantiator.
    pub admin: Option<String>,
    pub denom: Option<String>,
    pub min_operator_bond: Option<Uint128>,
    pub min_challenger_bond: Option<Uint128>,
    pub min_verifier_bond: Option<Uint128>,
    pub challenge_window_secs: Option<u64>,
    pub verdict_window_secs: Option<u64>,
    pub committee_size: Option<u32>,
    pub quorum_bp: Option<u32>,
    pub challenger_reward_bp: Option<u32>,
    pub operator_reward_bp: Option<u32>,
    pub verifier_slash_bp: Option<u32>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Stake a verifier seat (send >= min_verifier_bond in denom).
    /// Verifier seats are the committee pool; validator bonds are the
    /// intended backer.
    RegisterVerifier {},
    /// Return a verifier's bond. Refused while the verifier sits on an
    /// open dispute's committee.
    RemoveVerifier {},
    /// Anchor an attestation claim (send >= min_operator_bond).
    /// `id` is the snapshot/batch root hex; `activation_root` the Merkle
    /// root over the int8 leaves.
    AnchorAttestation { id: String, activation_root: String },
    /// Open a bonded challenge inside the window (send >=
    /// min_challenger_bond). Draws the committee from the dispute seed.
    OpenChallenge { attestation_id: String },
    /// Committee member submits a verdict plus the hash of its recompute
    /// evidence. Reaching quorum resolves the dispute immediately.
    SubmitVerdict {
        attestation_id: String,
        verdict: Verdict,
        evidence_hash: String,
    },
    /// After the challenge window with no dispute: finalize and return
    /// the operator bond.
    FinalizeAttestation { attestation_id: String },
    /// After the verdict deadline without quorum: mark Unresolved and
    /// refund the challenger. Operator bond stays locked.
    ResolveTimeout { attestation_id: String },
    /// DAO fallback for Unresolved disputes.
    ForceResolve {
        attestation_id: String,
        verdict: Verdict,
    },
    /// Admin-only config update.
    UpdateConfig {
        admin: Option<String>,
        min_operator_bond: Option<Uint128>,
        min_challenger_bond: Option<Uint128>,
        min_verifier_bond: Option<Uint128>,
        challenge_window_secs: Option<u64>,
        verdict_window_secs: Option<u64>,
        committee_size: Option<u32>,
        quorum_bp: Option<u32>,
        challenger_reward_bp: Option<u32>,
        operator_reward_bp: Option<u32>,
        verifier_slash_bp: Option<u32>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    #[returns(Attestation)]
    GetAttestation { attestation_id: String },
    #[returns(Dispute)]
    GetDispute { attestation_id: String },
    #[returns(Verifier)]
    GetVerifier { addr: String },
    #[returns(VerifierListResponse)]
    ListVerifiers {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(AttestationListResponse)]
    ListAttestations {
        status: Option<AttestationStatus>,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Recompute which committee a given seed would draw — anyone can
    /// re-derive the sortition, no trust in the challenger required.
    #[returns(CommitteeResponse)]
    CommitteePreview { seed: String },
}

#[cw_serde]
pub struct VerifierListResponse {
    pub verifiers: Vec<Verifier>,
}

#[cw_serde]
pub struct AttestationListResponse {
    pub attestations: Vec<Attestation>,
}

#[cw_serde]
pub struct CommitteeResponse {
    pub committee: Vec<String>,
}
