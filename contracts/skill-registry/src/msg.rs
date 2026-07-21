use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[allow(unused_imports)]
use crate::state::{Config, RegistryStats, SkillEntry};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    /// Native denom for the anti-spam fee (defaults to "ujuno" — this
    /// registry is intended to live on Juno mainnet).
    pub denom: Option<String>,
    pub registration_fee: Uint128,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Publish a new skill entry. Fails with `NameAlreadyClaimed` if
    /// `dapp_name` is already taken by a different publisher — use
    /// `UpdateSkill` instead in that case. Charges `registration_fee` on
    /// first publish only.
    PublishSkill {
        dapp_name: String,
        chain_id: String,
        skill_uri: String,
        skill_hash: String,
    },
    /// Update an existing entry. Only the original `publisher` (or admin,
    /// for dispute resolution) may call this. No fee charged.
    UpdateSkill {
        dapp_name: String,
        chain_id: Option<String>,
        skill_uri: Option<String>,
        skill_hash: Option<String>,
    },
    /// Admin-only: remove an entry (e.g. abandoned/malicious registration).
    RemoveSkill {
        dapp_name: String,
    },
    /// Admin-only: reassign the publisher of a claimed name — dispute
    /// resolution lever so a squatted or abandoned name can be recovered
    /// without a contract migration.
    TransferPublisher {
        dapp_name: String,
        new_publisher: String,
    },
    UpdateConfig {
        admin: Option<String>,
        registration_fee: Option<Uint128>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    #[returns(SkillEntry)]
    GetSkill { dapp_name: String },
    #[returns(Vec<SkillEntry>)]
    ListSkills {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(Vec<SkillEntry>)]
    SearchByChain {
        chain_id: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(RegistryStats)]
    GetStats {},
}
