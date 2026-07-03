use cosmwasm_schema::{cw_serde, QueryResponses};

use crate::state::KnowledgeMoult;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    /// Moultbook entry id of the DAO's current Mother-Moult.
    pub mother_moult_id: String,
    #[serde(default = "default_max_summary_len")]
    pub max_summary_len: u32,
    #[serde(default = "default_max_source_moults")]
    pub max_source_moults: u32,
}

fn default_max_summary_len() -> u32 {
    4096
}

fn default_max_source_moults() -> u32 {
    32
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Mint a Knowledge Moult. Permissionless — like Moultbook's `Post`, any
    /// funded address can call this; the `agent` field is a self-declared
    /// alias, not identity-gated. `owner` defaults to the sender.
    Mint {
        agent: String,
        motive: String,
        knowledge_summary: String,
        source_moults: Vec<String>,
        owner: Option<String>,
    },

    /// Transfer ownership. Only the current owner may call this.
    Transfer { id: String, recipient: String },

    /// Admin-only: point future mints at a new DAO-approved Mother-Moult
    /// version. Does not alter already-minted moults.
    UpdateMotherMoult { mother_moult_id: String },

    /// Admin-only.
    UpdateConfig {
        admin: Option<String>,
        max_summary_len: Option<u32>,
        max_source_moults: Option<u32>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    GetConfig {},

    #[returns(KnowledgeMoult)]
    GetMoult { id: String },

    #[returns(MoultsResponse)]
    ListByOwner {
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },

    #[returns(MoultsResponse)]
    ListByAgent {
        agent: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },

    #[returns(StatsResponse)]
    GetStats {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub mother_moult_id: String,
    pub max_summary_len: u32,
    pub max_source_moults: u32,
}

#[cw_serde]
pub struct MoultsResponse {
    pub moults: Vec<KnowledgeMoult>,
}

#[cw_serde]
pub struct StatsResponse {
    pub total_minted: u64,
}

#[cw_serde]
pub struct MigrateMsg {}
