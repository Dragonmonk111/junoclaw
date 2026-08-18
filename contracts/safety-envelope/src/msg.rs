use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct SafetyEnvelopeParams {
    /// Maximum linear speed (milli-m/s, e.g. 5000 = 5.000 m/s)
    pub max_speed_milli: u64,
    /// Maximum force exerted (milli-Newtons, e.g. 50000 = 50.000 N)
    pub max_force_milli: u64,
    /// Minimum collision distance (milli-meters, e.g. 500 = 0.500 m)
    pub min_collision_distance_milli: u64,
    /// Maximum tilt angle (milli-degrees, e.g. 30000 = 30.000°)
    pub max_tilt_milli_degrees: u64,
    /// Maximum acceleration (milli-m/s², e.g. 3000 = 3.000 m/s²)
    pub max_acceleration_milli: u64,
    /// Whether the robot is permitted to operate in human-proximity zones
    pub human_proximity_allowed: bool,
}

#[cw_serde]
pub struct InstantiateMsg {
    /// Admin address (governance multisig or DAO)
    pub admin: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Set or update the safety envelope for a specific robot.
    /// Only admin (governance) can call this.
    SetEnvelope {
        robot_id: String,
        params: SafetyEnvelopeParams,
    },

    /// Tighten an existing envelope (lower limits only, never relax).
    /// Admin-only. Useful for fleet-level responses to incidents.
    TightenEnvelope {
        robot_id: String,
        params: SafetyEnvelopeParams,
    },

    /// Transfer admin to a new address.
    TransferAdmin {
        new_admin: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get the current safety envelope for a robot
    #[returns(GetEnvelopeResponse)]
    GetEnvelope { robot_id: String },

    /// Get the admin address
    #[returns(AdminResponse)]
    GetAdmin {},

    /// Get the version history count for a robot
    #[returns(VersionCountResponse)]
    GetVersionCount { robot_id: String },
}

#[cw_serde]
pub struct GetEnvelopeResponse {
    pub robot_id: String,
    pub params: SafetyEnvelopeParams,
    pub version: u32,
    pub updated_at: u64,
    pub updated_by: String,
}

#[cw_serde]
pub struct AdminResponse {
    pub admin: String,
}

#[cw_serde]
pub struct VersionCountResponse {
    pub robot_id: String,
    pub count: u32,
}
