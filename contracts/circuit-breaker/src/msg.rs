use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    /// Admin address (governance multisig or DAO)
    pub admin: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Trip the circuit breaker for a robot.
    /// Called by the attestation contract or governance when a safety violation is detected.
    /// Once tripped, the robot's intent-tier is locked (reflexes still run locally).
    TripBreaker {
        robot_id: String,
        reason: String,
        cause_ref: String,
    },

    /// Reset the circuit breaker after the safety violation is resolved.
    /// Admin-only (governance or authorized operator).
    ResetBreaker {
        robot_id: String,
        reset_by: String,
    },

    /// Transfer admin to a new address.
    TransferAdmin {
        new_admin: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get the current circuit breaker state for a robot
    #[returns(GetBreakerResponse)]
    GetBreaker { robot_id: String },

    /// Get the admin address
    #[returns(AdminResponse)]
    GetAdmin {},

    /// Check if a robot's intent-tier is locked (breaker tripped)
    #[returns(IsLockedResponse)]
    IsLocked { robot_id: String },
}

#[cw_serde]
pub struct GetBreakerResponse {
    pub robot_id: String,
    pub state: String,
    pub reason: Option<String>,
    pub tripped_at: Option<u64>,
    pub cause_ref: Option<String>,
    pub reset_at: Option<u64>,
    pub reset_by: Option<String>,
}

#[cw_serde]
pub struct AdminResponse {
    pub admin: String,
}

#[cw_serde]
pub struct IsLockedResponse {
    pub robot_id: String,
    pub is_locked: bool,
    pub reason: Option<String>,
}
