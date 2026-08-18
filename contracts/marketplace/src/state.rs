use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    /// Native token denom used for escrow (defaults to "ujuno").
    pub denom: String,
    /// truth-market contract — consulted for the consensus verdict on the
    /// batch_height the agent's work was ordered into.
    pub truth_market: Addr,
    /// task-ledger contract — consulted for task completion status.
    pub task_ledger: Addr,
    /// Optional skill-registry contract — `skill_ref` on a listing may be
    /// cross-checked against a published `dapp_name` there. Not enforced
    /// on-chain in v1; reserved for a future `PublishSkill`-gated listing.
    pub skill_registry: Option<Addr>,
    /// Seconds a client must wait after `HireService` before they can
    /// unilaterally `CancelHire` on a still-unresolved escrow. Protects
    /// agents from a client yanking funds the instant work starts.
    pub cancel_window_secs: u64,
}

#[cw_serde]
pub struct Listing {
    pub id: u64,
    pub agent: Addr,
    /// Free-form skill identifier — conventionally the `dapp_name`
    /// registered in `skill-registry`, but not enforced.
    pub skill_ref: String,
    pub price: Uint128,
    pub description: String,
    pub active: bool,
    pub created_at: u64,
}

#[cw_serde]
pub enum HireStatus {
    /// Funds locked, awaiting task completion + Truth Market verdict.
    Escrowed,
    /// Green verdict + task Completed — funds released to the agent.
    Released,
    /// Red verdict — funds returned to the client, agent's claim rejected.
    Slashed,
    /// Underlying task Failed or was Cancelled in task-ledger — funds
    /// returned to the client, no verdict was ever in dispute.
    Refunded,
    /// Client withdrew after `cancel_window_secs` elapsed with no
    /// resolution — funds returned to the client.
    Cancelled,
}

#[cw_serde]
pub struct Hire {
    pub id: u64,
    pub listing_id: u64,
    pub client: Addr,
    pub agent: Addr,
    pub amount: Uint128,
    pub denom: String,
    /// task-ledger task id this hire is funding. The client (or agent, on
    /// the client's behalf) must supply the id of a task already submitted
    /// to task-ledger for the hired work.
    pub task_id: u64,
    pub status: HireStatus,
    pub created_at: u64,
    pub resolved_at: Option<u64>,
    /// BFT batch height the work's messages were ordered into — supplied
    /// at `ReleaseOnVerdict` time once the coordination mesh has produced
    /// a Truth Market epoch for it.
    pub batch_height: Option<u64>,
}

#[cw_serde]
pub struct MarketplaceStats {
    pub total_listings: u64,
    pub active_listings: u64,
    pub total_hires: u64,
    pub total_volume: Uint128,
    pub total_released: Uint128,
    pub total_refunded: Uint128,
    pub total_slashed: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("config");

pub const LISTINGS: Map<u64, Listing> = Map::new("listings");
pub const LISTINGS_BY_AGENT: Map<&Addr, Vec<u64>> = Map::new("listings_by_agent");
pub const NEXT_LISTING_ID: Item<u64> = Item::new("next_listing_id");

pub const HIRES: Map<u64, Hire> = Map::new("hires");
pub const HIRES_BY_TASK: Map<u64, u64> = Map::new("hires_by_task");
pub const HIRES_BY_CLIENT: Map<&Addr, Vec<u64>> = Map::new("hires_by_client");
pub const HIRES_BY_AGENT: Map<&Addr, Vec<u64>> = Map::new("hires_by_agent");
pub const NEXT_HIRE_ID: Item<u64> = Item::new("next_hire_id");

pub const STATS: Item<MarketplaceStats> = Item::new("stats");
