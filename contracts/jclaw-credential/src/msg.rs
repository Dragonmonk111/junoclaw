use cosmwasm_schema::{cw_serde, QueryResponses};
use crate::state::MemberRole;

/// Supported MAYO parameter sets for on-chain verification.
/// Storage only ever holds a variant-agnostic SHA-256 hash of the compact PK,
/// so adding variants here does not change the `Member` storage shape.
#[cw_serde]
pub enum MayoVariant {
    Mayo1,
    Mayo2,
    Mayo3,
    Mayo5,
}

impl Default for MayoVariant {
    fn default() -> Self {
        MayoVariant::Mayo2
    }
}

impl MayoVariant {
    /// Host-function variant code expected by the `mayo_verify` precompile:
    /// MAYO-1 = 1, MAYO-2 = 2, MAYO-3 = 3, MAYO-5 = 5.
    pub fn to_code(&self) -> u32 {
        match self {
            MayoVariant::Mayo1 => 1,
            MayoVariant::Mayo2 => 2,
            MayoVariant::Mayo3 => 3,
            MayoVariant::Mayo5 => 5,
        }
    }
}

/// Supported ML-DSA (FIPS 204) parameter sets for on-chain verification.
/// As with MAYO, storage only ever holds a variant-agnostic SHA-256 hash of
/// the public key, so adding variants does not change the `Member` shape.
#[cw_serde]
pub enum MlDsaVariant {
    MlDsa44,
    MlDsa65,
    MlDsa87,
}

impl Default for MlDsaVariant {
    fn default() -> Self {
        MlDsaVariant::MlDsa44
    }
}

impl MlDsaVariant {
    /// Host-function variant code expected by the `ml_dsa_verify` precompile:
    /// ML-DSA-44 = 44, ML-DSA-65 = 65, ML-DSA-87 = 87.
    pub fn to_code(&self) -> u32 {
        match self {
            MlDsaVariant::MlDsa44 => 44,
            MlDsaVariant::MlDsa65 => 65,
            MlDsaVariant::MlDsa87 => 87,
        }
    }

    /// FIPS 204-fixed public-key byte length for this variant.
    pub fn pk_len(&self) -> usize {
        match self {
            MlDsaVariant::MlDsa44 => 1312,
            MlDsaVariant::MlDsa65 => 1952,
            MlDsaVariant::MlDsa87 => 2592,
        }
    }
}

/// FIPS 204-fixed public-key byte lengths, in ascending order (44, 65, 87).
pub const MLDSA_PK_LENS: [usize; 3] = [1312, 1952, 2592];

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    /// Genesis member address. Defaults to sender if None.
    pub genesis: Option<String>,
    /// Sunset grace period in seconds. Default: 86400 (24h).
    #[serde(default = "default_sunset_grace")]
    pub sunset_grace_seconds: u64,
}

fn default_sunset_grace() -> u64 {
    86400
}

// ── Execute messages ──

#[cw_serde]
pub enum ExecuteMsg {
    /// Add a new bud under an existing parent member.
    /// Parent's weight is reduced by child_weight; child gets that weight.
    /// Admin-only.
    /// Optional `mayo_pk` attaches a post-quantum MAYO-2 public key to the child.
    /// The contract stores a SHA-256 hash of the compact PK (4 912 B);
    /// the full PK is never kept on-chain.
    Bud {
        parent: String,
        child: String,
        child_weight: u64,
        mayo_pk: Option<Vec<u8>>,
    },

    /// Prune a member and their entire subtree.
    /// Removed weight is returned to the root (Genesis).
    /// Admin-only.
    BreakChannel {
        addr: String,
    },

    /// Initiate sunset (dissolution). Requires all members to have
    /// passed their bud (zero children). Admin-only.
    InitiateSunset {},

    /// Execute sunset after grace period has elapsed. Anyone can call.
    ExecuteSunset {},

    /// Admin-only: transfer admin to a new address.
    TransferAdmin {
        new_admin: String,
    },

    /// Admin-only: attach or rotate a member's ML-DSA (FIPS 204) public key.
    /// The contract stores only a SHA-256 hash of `mldsa_pk` (PK is
    /// 1 312 / 1 952 / 2 592 B for ML-DSA-44 / 65 / 87 and too large to keep
    /// on-chain). Any supported PK length is accepted; the variant is
    /// disambiguated at verification time.
    SetMlDsaPk {
        addr: String,
        mldsa_pk: Vec<u8>,
    },

    /// Verify a MAYO post-quantum signature on behalf of a member.
    /// Caller provides the full compact public key; the contract checks the
    /// SHA-256 hash against the member's stored `mayo_pk_hash`, then runs the
    /// pure-Rust verifier for the requested `variant` (default MAYO-2).
    VerifyMayoAttestation {
        addr: String,
        message: Vec<u8>,
        signature: Vec<u8>,
        public_key: Vec<u8>,
        #[serde(default)]
        variant: MayoVariant,
    },

    /// Verify an ML-DSA (FIPS 204) post-quantum signature on behalf of a
    /// member. Caller provides the full public key; the contract checks the
    /// SHA-256 hash against the member's stored `mldsa_pk_hash`, then verifies
    /// for the requested `variant` (default ML-DSA-44). With
    /// `--features mldsa-precompile` the work is routed through the
    /// `ml_dsa_verify` host function; otherwise the in-contract `fips204`
    /// verifier runs (integer-only, deterministic, RNG-free).
    VerifyMlDsaAttestation {
        addr: String,
        message: Vec<u8>,
        signature: Vec<u8>,
        public_key: Vec<u8>,
        #[serde(default)]
        variant: MlDsaVariant,
    },
}

// ── Query messages (cw4-compatible + tree extensions) ──

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// cw4-compatible: single member lookup.
    #[returns(MemberResponse)]
    Member {
        addr: String,
    },

    /// cw4-compatible: paginated member list.
    #[returns(ListMembersResponse)]
    ListMembers {
        start_after: Option<String>,
        limit: Option<u32>,
    },

    /// cw4-compatible: total weight.
    #[returns(TotalWeightResponse)]
    TotalWeight {},

    /// Tree-specific: list all children of a member.
    #[returns(ListChildrenResponse)]
    ListChildren {
        addr: String,
    },

    /// Tree-specific: list full ancestry path from root to member.
    #[returns(AncestryResponse)]
    Ancestry {
        addr: String,
    },

    /// Tree-specific: current sunset state.
    #[returns(SunsetStatusResponse)]
    SunsetStatus {},

    /// Get config.
    #[returns(ConfigResponse)]
    Config {},

    /// Get stored MAYO PK hash for a member.
    #[returns(MayoPkHashResponse)]
    MayoPkHash {
        addr: String,
    },

    /// Get stored ML-DSA PK hash for a member.
    #[returns(MlDsaPkHashResponse)]
    MlDsaPkHash {
        addr: String,
    },
}

// ── Response types ──

#[cw_serde]
pub struct MemberResponse {
    pub addr: String,
    pub weight: u64,
    pub role: MemberRole,
    pub parent: Option<String>,
    pub depth: u32,
    pub start_height: u64,
    pub mayo_pk_hash: Option<String>,
    pub mldsa_pk_hash: Option<String>,
}

#[cw_serde]
pub struct ListMembersResponse {
    pub members: Vec<MemberResponse>,
}

#[cw_serde]
pub struct TotalWeightResponse {
    pub weight: u64,
}

#[cw_serde]
pub struct ListChildrenResponse {
    pub children: Vec<MemberResponse>,
}

#[cw_serde]
pub struct AncestryResponse {
    pub path: Vec<MemberResponse>,
}

#[cw_serde]
pub struct SunsetStatusResponse {
    pub initiated: bool,
    pub initiated_at: Option<u64>,
    pub executed: bool,
    pub can_execute: bool,
    pub remaining_grace_seconds: Option<u64>,
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub sunset_grace_seconds: u64,
}

#[cw_serde]
pub struct MayoPkHashResponse {
    pub addr: String,
    pub mayo_pk_hash: Option<String>,
}

#[cw_serde]
pub struct MlDsaPkHashResponse {
    pub addr: String,
    pub mldsa_pk_hash: Option<String>,
}

#[cw_serde]
pub struct MigrateMsg {}
