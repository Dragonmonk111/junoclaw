use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

// Config/Dispute are referenced by the #[returns(...)] derive below.
#[allow(unused_imports)]
use crate::state::{
    Attestation, AttestationStatus, Config, Dispute, ReadoutClaims, Verdict, Verifier,
};

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
    /// Hex-compressed drand beacon public key (G2). Set it to enable
    /// trustless drand-verified sortition for committees.
    #[serde(default)]
    pub drand_pubkey: Option<String>,
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
    /// root over the int8 leaves. `claims` declares the readout outputs —
    /// supplying it makes the attestation machine-adjudicable via
    /// `ResolveWithRecompute` / `ResolveWithProof`.
    AnchorAttestation {
        id: String,
        activation_root: String,
        #[serde(default)]
        claims: Option<ReadoutClaims>,
    },
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
    /// Permissionless drand submission: posts the drand signature for the
    /// round committed at challenge open. The contract verifies the BLS
    /// signature on-chain against the configured beacon key — nobody can
    /// submit fake randomness — then draws the committee by Fisher-Yates
    /// over sha256(signature). Anyone may call this; a withheld relay
    /// cannot stall the draw.
    SubmitRandomness {
        attestation_id: String,
        /// Hex of the 48-byte BLS12-381 G1 drand signature for the
        /// dispute's committed round.
        signature_hex: String,
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
    /// Admin-only: point the court at a zk-verifier contract instance.
    /// Enables the `ResolveWithProof` fast-path.
    SetZkVerifier { address: String },
    /// Cryptographic fast-path: a Groth16 proof whose public inputs bind
    /// to this attestation's `activation_root` and declared `claims`
    /// resolves it without committee votes — Clean on a live dispute,
    /// early finality on a Pending one (self-verifying receipt).
    /// Permissionless: the proof is the evidence.
    ///
    /// `public_inputs_base64` must decode to exactly 5 arkworks-compressed
    /// BN254 Fr elements (160 bytes) in circuit allocation order:
    /// [merkle_root, bank_commitment, score, integrity, detection].
    /// For proof-resolvable attestations, `activation_root` must be
    /// anchored as lowercase hex of the Fr's compressed 32-byte form.
    ResolveWithProof {
        attestation_id: String,
        proof_base64: String,
        public_inputs_base64: String,
    },
    /// Deterministic fast-path: the chain itself re-runs the readout.
    /// `activations`/`bank_vec` are the canonical int8 vectors (the same
    /// bytes `jlens.integrity.v1` commits); `merkle_path` proves the
    /// activation leaf sits under `activation_root` (sha256 pair-chain,
    /// same convention as the gateway/merkle-verifier). If the inputs
    /// bind correctly the contract recomputes score/integrity/detection
    /// and resolves the attestation — Clean when the recomputation
    /// matches the declared claims, Fraud when it provably doesn't.
    /// Permissionless: the chain is the verifier.
    ResolveWithRecompute {
        attestation_id: String,
        activations: Vec<i8>,
        bank_vec: Vec<i8>,
        threshold: u64,
        leaf_index: u32,
        /// Hex-encoded sibling hashes, leaf level up.
        merkle_path: Vec<String>,
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
        drand_pubkey: Option<String>,
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
    /// Configured zk-verifier contract for the proof fast-path.
    #[returns(ZkVerifierResponse)]
    GetZkVerifier {},
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

#[cw_serde]
pub struct ZkVerifierResponse {
    pub address: Option<String>,
}

/// Wire copy of the zk-verifier contract's `Verify` smart query — kept
/// minimal on purpose; the verifier contract owns the format.
#[cw_serde]
pub enum ZkVerifierQueryMsg {
    Verify {
        proof_base64: String,
        public_inputs_base64: String,
    },
}

#[cw_serde]
pub struct ZkVerifyResponse {
    pub valid: bool,
    pub reason: String,
}
