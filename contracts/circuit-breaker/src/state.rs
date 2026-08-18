use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

/// Admin address (governance multisig or DAO)
pub const ADMIN: Item<Addr> = Item::new("admin");

/// Circuit breaker state per robot: robot_id -> BreakerRecord
pub const BREAKERS: Map<&str, BreakerRecord> = Map::new("breakers");

#[cw_serde]
pub struct BreakerRecord {
    /// "closed" = intent-tier allowed, "tripped" = locked, "reset" = was tripped, now resolved
    pub state: String,
    /// Reason the breaker tripped (None if closed or reset)
    pub reason: Option<String>,
    /// Block height when the breaker tripped
    pub tripped_at: Option<u64>,
    /// Reference to the attestation or verdict that caused the trip
    pub cause_ref: Option<String>,
    /// Block height when the breaker was reset
    pub reset_at: Option<u64>,
    /// Who authorized the reset
    pub reset_by: Option<String>,
}

impl BreakerRecord {
    pub fn closed() -> Self {
        Self {
            state: "closed".to_string(),
            reason: None,
            tripped_at: None,
            cause_ref: None,
            reset_at: None,
            reset_by: None,
        }
    }

    pub fn is_tripped(&self) -> bool {
        self.state == "tripped"
    }

    pub fn is_closed(&self) -> bool {
        self.state == "closed"
    }
}
