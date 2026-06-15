use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

/// Total weight must always sum to 10_000 basis points.
pub const TOTAL_WEIGHT: u64 = 10_000;

/// Member role — same taxonomy as agent-company for continuity.
#[cw_serde]
pub enum MemberRole {
    Genesis,
    Bud,
}

/// A node in the trust-tree.
#[cw_serde]
pub struct Member {
    pub addr: Addr,
    pub weight: u64,
    pub role: MemberRole,
    /// Parent in the trust-tree. `None` for the root (Genesis).
    pub parent: Option<Addr>,
    /// Depth from root: Genesis = 0, direct buds = 1, etc.
    pub depth: u32,
    /// Block height at which this member was added.
    pub start_height: u64,
    /// Hex-encoded SHA-256 of the member's MAYO compact public key.
    /// The full PK (4 912 B for MAYO-2) is too large for on-chain storage;
    /// callers provide the full PK during verification and the contract
    /// checks this hash before running the pure-Rust verifier.
    pub mayo_pk_hash: Option<String>,
    /// Hex-encoded SHA-256 of the member's ML-DSA (FIPS 204) public key.
    /// PK is 1 312 / 1 952 / 2 592 B for ML-DSA-44 / 65 / 87 — too large for
    /// on-chain storage, so only the hash is kept. Set/rotated via
    /// `SetMlDsaPk`; the full PK is supplied at `VerifyMlDsaAttestation` time
    /// and checked against this hash before verification.
    pub mldsa_pk_hash: Option<String>,
}

/// Contract configuration.
#[cw_serde]
pub struct Config {
    pub admin: Addr,
    /// Seconds remaining after Sunset is initiated before the contract
    /// is irrevocably dissolved. Default: 86400 (24h at 1s/block).
    pub sunset_grace_seconds: u64,
}

/// Sunset state machine.
#[cw_serde]
pub struct SunsetState {
    pub initiated: bool,
    pub initiated_at: u64,
    pub executed: bool,
}

// ── Storage items ──

pub const CONFIG: Item<Config> = Item::new("config");

/// Sunset state (optional — only created when initiated).
pub const SUNSET: Item<SunsetState> = Item::new("sunset");

/// Member roster: addr -> Member.
pub const MEMBERS: Map<&Addr, Member> = Map::new("members");

/// Children index: parent_addr -> Vec<child_addr>.
pub const CHILDREN: Map<&Addr, Vec<Addr>> = Map::new("children");

/// Total weight cached for O(1) cw4 queries.
pub const TOTAL_WEIGHT_ITEM: Item<u64> = Item::new("total_weight");
