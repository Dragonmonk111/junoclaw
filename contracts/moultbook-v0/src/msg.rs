use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Binary;

#[allow(unused_imports)]
use crate::state::{AttestationRef, Config, MoultEntry, Stats, Visibility};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    /// Optional whoami / DENS contract address. If set, every `Post` validates
    /// that the sender holds at least one DENS alias. Leave None for devnet.
    pub whoami_contract: Option<String>,
    pub max_size_bytes: u64,
    pub max_refs: u32,
    pub max_content_type_len: u32,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Write a new entry. Author is `info.sender`.
    Post {
        commitment: Binary,
        content_type: String,
        size_bytes: u64,
        attestation_ref: Option<AttestationRef>,
        visibility: Visibility,
        refs: Vec<String>,
    },
    /// Soft-delete: clears commitment, keeps metadata + refs for audit.
    /// Author or admin only.
    Redact { id: String },
    /// Author only. Cannot widen non-Public to Public (one-way narrowing).
    UpdateVisibility { id: String, visibility: Visibility },
    /// Admin only. Empty `whoami_contract` clears the gating.
    UpdateConfig {
        admin: Option<String>,
        whoami_contract: Option<String>,
        max_size_bytes: Option<u64>,
        max_refs: Option<u32>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    #[returns(MoultEntry)]
    GetEntry { id: String },
    #[returns(EntriesResponse)]
    ListByAuthor {
        author: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(EntriesResponse)]
    ListByRef {
        ref_id: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(Stats)]
    GetStats {},
}

#[cw_serde]
pub struct EntriesResponse {
    pub entries: Vec<MoultEntry>,
}

/// Subset of the upstream `whoami` contract's query API that Moultbook calls
/// for identity gating. Defined locally so the crate has no hard dependency
/// on the whoami crate; the message shape matches `envoylabs/whoami`.
#[cw_serde]
pub enum WhoamiQuery {
    Tokens {
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct WhoamiTokensResponse {
    pub tokens: Vec<String>,
}
