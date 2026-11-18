use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

/// Lifecycle of an anchored attestation.
/// pending → finalized   (window passed unchallenged, or clean verdict)
/// pending → challenged  → finalized | fraudulent | unresolved
#[cw_serde]
#[derive(Eq)]
pub enum AttestationStatus {
    /// Inside the challenge window; operator bond locked.
    Pending,
    /// A bonded challenge is open; committee deliberating.
    Challenged,
    /// Survived the window or a challenge; bond returned.
    Finalized,
    /// Committee verdict: committed activations were fabricated.
    /// Operator bond slashed; attestation is void for gating.
    Fraudulent,
    /// Committee failed to reach quorum before the verdict deadline.
    /// Operator bond stays locked until the DAO force-resolves.
    Unresolved,
}

/// A bonded claim that `activation_root` commits to activations produced
/// by the pinned weights on the disclosed prompt/response.
#[cw_serde]
pub struct Attestation {
    /// Hex sha256 — the snapshot/batch root this dispute covers.
    pub id: String,
    pub operator: Addr,
    /// Merkle root over int8 activation leaves (anchored elsewhere;
    /// this contract adjudicates its honesty).
    pub activation_root: String,
    /// Operator escrow backing the claim.
    pub bond: Uint128,
    /// Anchor block time (seconds).
    pub anchored_at: u64,
    /// End of the challenge window (seconds).
    pub challenge_deadline: u64,
    pub status: AttestationStatus,
    /// Declared readout outputs. Present ⇒ the dispute is
    /// machine-adjudicable: the chain can recompute the readout itself
    /// (`ResolveWithRecompute`) or verify a Groth16 proof of it
    /// (`ResolveWithProof`), both in one transaction. Absent ⇒ only
    /// the committee path can resolve the dispute.
    #[serde(default)]
    pub claims: Option<ReadoutClaims>,
}

/// The operator's declared readout, anchored alongside the commitment.
/// These are the values a dispute compares recomputation against.
#[cw_serde]
pub struct ReadoutClaims {
    /// Integer-domain dot product Σ activationsᵢ·bankᵢ (non-negative
    /// in the current jlens.integrity.v1 readout).
    pub score: u64,
    /// SCALE − score.
    pub integrity: u64,
    /// Detection flag: score >= bank threshold.
    pub detection: bool,
    /// Hex sha256 of (bank_vec int8 bytes ‖ threshold big-endian u64) —
    /// binds the claims to a specific registered probe direction.
    pub bank_commitment: String,
}

/// Committee verdict on a dispute.
#[cw_serde]
#[derive(Eq, Copy)]
pub enum Verdict {
    /// Recompute reproduced the committed verdict: attestation honest.
    Clean,
    /// Recompute diverged from committed vectors: attestation fabricated.
    Fraud,
}

#[cw_serde]
pub struct Vote {
    pub verifier: Addr,
    pub verdict: Verdict,
    /// sha256 over the verifier's recompute evidence bundle (disclosure
    /// inputs, recomputed vectors, divergence report). Off-chain artifact,
    /// on-chain commitment.
    pub evidence_hash: String,
    pub voted_at: u64,
}

#[cw_serde]
pub struct Dispute {
    pub attestation_id: String,
    pub challenger: Addr,
    pub challenger_bond: Uint128,
    pub opened_at: u64,
    /// Sortitioned verifier seats. Empty while the drand draw is pending;
    /// filled by `SubmitRandomness` after the committed round lands.
    /// In legacy mode (no drand pubkey configured) the committee is
    /// drawn atomically at open time.
    pub committee: Vec<Addr>,
    /// Verifier pool snapshot taken at open — the draw can't be gamed by
    /// registering new seats after the round is committed.
    #[serde(default)]
    pub eligible: Vec<Addr>,
    /// The drand quicknet round this dispute's draw commits to
    /// (genesis 1692803367, 3s period). 0 = legacy Fiat-Shamir draw.
    #[serde(default)]
    pub drand_round: u64,
    /// The submitted drand signature hex + its submitter, kept for audit.
    #[serde(default)]
    pub randomness_hex: Option<String>,
    #[serde(default)]
    pub randomness_submitter: Option<Addr>,
    pub votes: Vec<Vote>,
    /// Verdict deadline (seconds); after this a short quorum triggers
    /// the Unresolved path via ResolveTimeout.
    pub verdict_deadline: u64,
}

/// A stake-bound verifier seat. In production these are validator
/// seats whose consensus bond doubles as attestation collateral; the
/// contract only tracks the escrowed registration bond.
#[cw_serde]
pub struct Verifier {
    pub addr: Addr,
    pub bond: Uint128,
    pub registered_at: u64,
}

#[cw_serde]
pub struct Config {
    /// DAO / governance — receives slashed-share payouts and force-resolves
    /// unresolved disputes.
    pub admin: Addr,
    /// Bond denomination.
    pub denom: String,
    pub min_operator_bond: Uint128,
    pub min_challenger_bond: Uint128,
    pub min_verifier_bond: Uint128,
    /// How long an attestation can be challenged (seconds).
    pub challenge_window_secs: u64,
    /// How long the committee has to reach quorum (seconds).
    pub verdict_window_secs: u64,
    /// Committee size drawn per dispute.
    pub committee_size: u32,
    /// Share of the committee that must agree on one verdict, in basis
    /// points (e.g. 6700 = 67%).
    pub quorum_bp: u32,
    /// Share of a slashed operator bond paid to the successful
    /// challenger (basis points); remainder goes to the DAO.
    pub challenger_reward_bp: u32,
    /// Share of a slashed challenger bond paid to the exonerated
    /// operator (basis points); remainder goes to the DAO.
    pub operator_reward_bp: u32,
    /// Fraction of a verifier's bond slashed for voting with the
    /// minority (basis points). Honest-majority assumption only —
    /// probabilistic/economic, not cryptographic.
    pub verifier_slash_bp: u32,
    /// Hex-compressed BLS12-381 G2 public key of the drand beacon used
    /// for committee sortition (quicknet). When set, challenges commit
    /// to a future drand round and the committee is drawn trustlessly
    /// from the on-chain-verified signature. When unset, sortition uses
    /// the legacy Fiat-Shamir draw at open time.
    #[serde(default)]
    pub drand_pubkey: Option<String>,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const ATTESTATIONS: Map<&str, Attestation> = Map::new("attestations");
pub const DISPUTES: Map<&str, Dispute> = Map::new("disputes");
pub const VERIFIERS: Map<&str, Verifier> = Map::new("verifiers");
/// verifier addr → number of open-dispute seats held. >0 blocks unbonding;
/// decremented when a dispute resolves or times out.
pub const ACTIVE_SEATS: Map<&str, u64> = Map::new("active_seats");
/// Address of the zk-verifier contract whose `Verify` query the court
/// smart-queries in the `ResolveWithProof` fast-path. Admin-set.
pub const ZK_VERIFIER: Item<Addr> = Item::new("zk_verifier");
