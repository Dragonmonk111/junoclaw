use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Wrong funds: expected at least {expected} {denom}, sent {sent:?}")]
    WrongFunds {
        denom: String,
        expected: Uint128,
        sent: Vec<String>,
    },

    #[error("Attestation already exists: {id}")]
    AttestationExists { id: String },

    #[error("Attestation not found: {id}")]
    AttestationNotFound { id: String },

    #[error("Attestation is not in pending state: {id}")]
    AttestationNotPending { id: String },

    #[error("Challenge window still open: {id}")]
    ChallengeWindowOpen { id: String },

    #[error("Challenge window closed: {id}")]
    ChallengeWindowClosed { id: String },

    #[error("Dispute already open for attestation: {id}")]
    DisputeExists { id: String },

    #[error("No dispute for attestation: {id}")]
    DisputeNotFound { id: String },

    #[error("Not enough registered verifiers: need {need}, have {have}")]
    InsufficientVerifiers { need: u32, have: u32 },

    #[error("Verifier already registered")]
    VerifierExists {},

    #[error("Verifier not registered")]
    VerifierNotFound {},

    #[error("Verifier is seated on an open dispute and cannot unbond")]
    VerifierBusy {},

    #[error("Sender is not on this dispute's committee")]
    NotCommittee {},

    #[error("Committee member already voted")]
    AlreadyVoted {},

    #[error("Verdict window closed; dispute timed out")]
    VerdictWindowClosed {},

    #[error("Verdict window still open")]
    VerdictWindowOpen {},

    #[error("Dispute is not in unresolved state")]
    DisputeNotUnresolved {},

    #[error("No zk-verifier contract configured")]
    ZkVerifierNotConfigured {},

    #[error("Public inputs malformed: {reason}")]
    PublicInputsMalformed { reason: String },

    #[error("Proof public inputs do not bind to this attestation")]
    ProofBindingMismatch {},

    #[error("Proof rejected by zk-verifier: {reason}")]
    ProofRejected { reason: String },

    #[error("Attestation already resolved: {id}")]
    AttestationAlreadyResolved { id: String },

    #[error("Attestation has no declared readout claims; only the committee path can resolve it")]
    NotMachineAdjudicable { id: String },

    #[error("Recompute evidence does not bind to the committed artifacts: {reason}")]
    RecomputeBindingFailed { reason: String },

    #[error("Committee already drawn for attestation: {id}")]
    SortitionAlreadyDone { id: String },

    #[error("Invalid drand signature hex: {reason}")]
    InvalidRandomness { reason: String },

    #[error("Drand signature failed on-chain BLS verification")]
    DrandVerificationFailed {},

    #[error("No drand beacon key configured; sortition is in legacy mode")]
    DrandNotConfigured {},
}
