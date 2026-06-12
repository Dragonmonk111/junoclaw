use cosmwasm_schema::{cw_serde, QueryResponses};
use crate::state::MemberRole;

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

    /// Verify a MAYO-2 post-quantum signature on behalf of a member.
    /// Caller provides the full compact public key (4 912 B); the contract
    /// checks the SHA-256 hash against the member's stored `mayo_pk_hash`,
    /// then runs the pure-Rust verifier. Gas cost: ~300 KB peak memory.
    VerifyMayoAttestation {
        addr: String,
        message: Vec<u8>,
        signature: Vec<u8>,
        public_key: Vec<u8>,
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
pub struct MigrateMsg {}
