use cosmwasm_schema::{cw_serde, QueryResponses};

use crate::state::{BankStatus, ConceptThreshold, Config, ProbeBank};

#[cw_serde]
pub struct InstantiateMsg {
    /// DAO / governance address. Only it can activate/deprecate/revoke.
    pub admin: Option<String>,
    /// Minimum number of concepts a bank must cover before activation.
    pub min_concepts: Option<u32>,
    /// Whether activation requires a published calibration hash.
    pub require_calibration: Option<bool>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Anyone may propose a bank for a model. Lands as Proposed; the
    /// admin (DAO) activates it after review + calibration.
    RegisterProbeBank {
        model_id: String,
        model_revision: String,
        weights_hash: String,
        layers: Vec<u32>,
        concepts: Vec<String>,
        method: String,
        bank_hash: String,
        uri: String,
        calibration_hash: String,
        thresholds: Vec<ConceptThreshold>,
    },
    /// Admin: mark a proposed bank Active for its (model_id,
    /// weights_hash). Replaces any existing active bank — the old one
    /// moves to Deprecated automatically.
    ActivateProbeBank {
        bank_hash: String,
    },
    /// Admin: move an active bank to Deprecated (kept for re-audit of
    /// historical activation_root data, rejected for new audits).
    DeprecateBank {
        bank_hash: String,
    },
    /// Admin: revoke a bank entirely (found bad/evasive). Revoked banks
    /// are rejected for audits AND re-audits.
    RevokeBank {
        bank_hash: String,
        reason: String,
    },
    /// Admin-only config update.
    UpdateConfig {
        admin: Option<String>,
        min_concepts: Option<u32>,
        require_calibration: Option<bool>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    /// The bank currently Active for an exact (model_id, weights_hash)
    /// pair. This is THE qualification query: an agent's declared
    /// weights must resolve here before it is eligible for gated work.
    #[returns(ActiveBankResponse)]
    ActiveBank { model_id: String, weights_hash: String },
    #[returns(ProbeBank)]
    GetBank { bank_hash: String },
    #[returns(BankListResponse)]
    ListBanks {
        model_id: Option<String>,
        status: Option<BankStatus>,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Full lifecycle history for a model — every bank ever proposed.
    #[returns(BankHistoryResponse)]
    BankHistory { model_id: String },
}

#[cw_serde]
pub struct ActiveBankResponse {
    pub bank: Option<ProbeBank>,
}

#[cw_serde]
pub struct BankListResponse {
    pub banks: Vec<ProbeBank>,
}

#[cw_serde]
pub struct BankHistoryResponse {
    pub banks: Vec<ProbeBank>,
}
