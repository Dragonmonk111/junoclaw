use cosmwasm_schema::{cw_serde, QueryResponses};

#[allow(unused_imports)]
use crate::state::{Config, LedgerStats};
#[allow(unused_imports)]
use junoclaw_common::{ContractRegistry, PaymentObligation};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub task_ledger: String,
    /// Seconds before a `Pending` obligation can be expired by anyone.
    /// 0 disables expiry. (`timeout_blocks` is accepted as a legacy alias.)
    #[serde(alias = "timeout_blocks")]
    pub timeout_seconds: u64,
    /// Native token denom. Defaults to "ujunox".
    pub denom: Option<String>,
    /// Optional cross-contract registry snapshot. When `None`, `registry`
    /// is initialised with `task_ledger` wired from the required field above
    /// and the other pointers left `None` until `UpdateRegistry` is called.
    pub registry: Option<ContractRegistry>,
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
    /// Permissionless: cancel a `Pending` obligation once
    /// `created_at + timeout_seconds` has passed.
    ExpirePending {
        task_id: u64,
    },
    /// Admin-only: close a `Disputed` obligation, either as paid
    /// (`ConfirmObligation`) or void (`CancelObligation`).
    ResolveDispute {
        task_id: u64,
        resolution: DisputeResolution,
    },
    UpdateConfig {
        admin: Option<String>,
        task_ledger: Option<String>,
        #[serde(alias = "timeout_blocks")]
        timeout_seconds: Option<u64>,
    },
    /// Admin-only: rewire the cross-contract registry. Any field left as
    /// `None` is untouched.
    UpdateRegistry {
        agent_registry: Option<String>,
        task_ledger: Option<String>,
        escrow: Option<String>,
    },
}

#[cw_serde]
pub enum DisputeResolution {
    /// The payee was paid after all: `Disputed` -> `Confirmed`.
    ConfirmObligation,
    /// The payer was right, or the deal is off: `Disputed` -> `Cancelled`.
    CancelObligation,
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
    /// The `tx_hash` the payer attached when confirming, if any.
    #[returns(Option<String>)]
    GetTxHash { task_id: u64 },
    #[returns(LedgerStats)]
    GetStats {},
    #[returns(Vec<PaymentObligation>)]
    ListObligations { start_after: Option<u64>, limit: Option<u32> },
}
