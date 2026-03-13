use cosmwasm_schema::{cw_serde, QueryResponses};

#[allow(unused_imports)]
use crate::state::{Config, LedgerStats};
#[allow(unused_imports)]
use junoclaw_common::PaymentObligation;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub task_ledger: String,
    pub timeout_blocks: u64,
    /// Native token denom. Defaults to "ujunox".
    pub denom: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Record a payment obligation (no funds sent to contract).
    /// The payer owes the payee the specified amount.
    Authorize {
        task_id: u64,
        payee: String,
        amount: cosmwasm_std::Uint128,
    },
    /// Payer confirms they have sent funds directly to payee.
    /// Contract records the settlement — no funds flow through it.
    Confirm {
        task_id: u64,
        /// Optional tx hash proving the direct transfer
        tx_hash: Option<String>,
    },
    /// Payer disputes the obligation (e.g. task not completed).
    Dispute {
        task_id: u64,
        reason: String,
    },
    /// Cancel a pending obligation (admin or mutual).
    Cancel {
        task_id: u64,
    },
    /// Attach a WAVS attestation hash to verify the obligation.
    AttachAttestation {
        task_id: u64,
        attestation_hash: String,
    },
    UpdateConfig {
        admin: Option<String>,
        task_ledger: Option<String>,
        timeout_blocks: Option<u64>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    #[returns(PaymentObligation)]
    GetObligation { obligation_id: u64 },
    #[returns(Option<PaymentObligation>)]
    GetObligationByTask { task_id: u64 },
    #[returns(LedgerStats)]
    GetStats {},
    #[returns(Vec<PaymentObligation>)]
    ListObligations { start_after: Option<u64>, limit: Option<u32> },
}
