use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[allow(unused_imports)]
use crate::state::{Config, Hire, Listing, MarketplaceStats};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub truth_market: String,
    pub task_ledger: String,
    pub skill_registry: Option<String>,
    /// Native denom for escrow. Defaults to "ujuno".
    pub denom: Option<String>,
    /// Defaults to 3600 (1 hour) when omitted.
    pub cancel_window_secs: Option<u64>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Agent lists a service. `skill_ref` conventionally matches a
    /// `dapp_name` published in `skill-registry`.
    ListService {
        skill_ref: String,
        price: Uint128,
        description: String,
    },
    /// Listing owner (or admin) updates price/description/active flag.
    /// Fields left `None` are unchanged.
    UpdateListing {
        listing_id: u64,
        price: Option<Uint128>,
        description: Option<String>,
        active: Option<bool>,
    },
    /// Listing owner (or admin) deactivates a listing.
    DelistService { listing_id: u64 },
    /// Client hires a listed service, escrowing `listing.price` in the
    /// tx's funds. `task_id` must reference a task already submitted to
    /// `task_ledger` for the work being hired.
    HireService { listing_id: u64, task_id: u64 },
    /// Permissionless: resolve an `Escrowed` hire once its task has
    /// settled in `task_ledger` and (if Completed) `truth_market` has
    /// finalized an epoch for `batch_height`.
    /// - Task Completed + verdict "green" → funds released to agent.
    /// - Task Completed + verdict "red"   → funds returned to client (slash).
    /// - Task Failed/Cancelled            → funds returned to client, no
    ///   verdict lookup required.
    ReleaseOnVerdict { hire_id: u64, batch_height: u64 },
    /// Client (or admin) reclaims escrow after `cancel_window_secs` has
    /// elapsed on an unresolved hire.
    CancelHire { hire_id: u64 },
    UpdateConfig {
        admin: Option<String>,
        truth_market: Option<String>,
        task_ledger: Option<String>,
        skill_registry: Option<String>,
        cancel_window_secs: Option<u64>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    #[returns(Listing)]
    GetListing { listing_id: u64 },
    #[returns(Vec<Listing>)]
    ListListings {
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    #[returns(Vec<Listing>)]
    ListListingsByAgent { agent: String, limit: Option<u32> },
    #[returns(Hire)]
    GetHire { hire_id: u64 },
    #[returns(Option<Hire>)]
    GetHireByTask { task_id: u64 },
    #[returns(Vec<Hire>)]
    ListHiresByClient { client: String, limit: Option<u32> },
    #[returns(Vec<Hire>)]
    ListHiresByAgent { agent: String, limit: Option<u32> },
    #[returns(MarketplaceStats)]
    GetStats {},
}
