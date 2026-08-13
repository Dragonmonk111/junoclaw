use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Binary;

/// Instantiate message — called when the contract is deployed.
#[cw_serde]
pub struct InstantiateMsg {
    /// Admin who can update the validator set (typically the DAO)
    pub admin: String,
    /// Initial validator public keys (BLS12-381 compressed, 48 bytes each)
    pub validators: Vec<Binary>,
    /// Threshold number of validators required (e.g. 3 of 4)
    pub threshold: u32,
}

/// Execute messages.
#[cw_serde]
pub enum ExecuteMsg {
    /// Submit a finalized batch with its threshold_simplex certificate.
    /// Only callable by a registered relayer.
    SubmitBatch {
        /// Threshold simplex certificate (~240 bytes, BLS12-381)
        certificate: Binary,
        /// SHA-256 hash of the ordered message batch
        messages_hash: [u8; 32],
        /// Commonware consensus height
        commonware_height: u64,
        /// Unix timestamp (ms) when the batch was finalized
        timestamp: u64,
    },

    /// Update the validator set (admin only).
    UpdateValidatorSet {
        /// New validator public keys
        validators: Vec<Binary>,
        /// New threshold
        threshold: u32,
    },

    /// Update the admin (admin only).
    UpdateAdmin {
        new_admin: String,
    },

    /// Register a relayer authorized to submit batches.
    RegisterRelayer {
        address: String,
    },

    /// Remove a relayer.
    RemoveRelayer {
        address: String,
    },
}

/// Query messages.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get the current config (admin, threshold, validator count)
    #[returns(ConfigResponse)]
    Config {},

    /// Get the current validator set
    #[returns(ValidatorSetResponse)]
    ValidatorSet {},

    /// Get a settled batch by Commonware height
    #[returns(BatchResponse)]
    Batch { commonware_height: u64 },

    /// Get the latest settled batch
    #[returns(BatchResponse)]
    LatestBatch {},

    /// Check if an address is a registered relayer
    #[returns(RelayerResponse)]
    Relayer { address: String },
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub threshold: u32,
    pub validator_count: u32,
    pub relayer_count: u32,
    pub latest_height: Option<u64>,
}

#[cw_serde]
pub struct ValidatorSetResponse {
    pub validators: Vec<Binary>,
    pub threshold: u32,
}

#[cw_serde]
pub struct BatchResponse {
    pub commonware_height: u64,
    pub messages_hash: [u8; 32],
    pub certificate: Binary,
    pub timestamp: u64,
    pub submitter: String,
}

#[cw_serde]
pub struct RelayerResponse {
    pub is_relayer: bool,
}
