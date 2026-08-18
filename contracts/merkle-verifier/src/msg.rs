use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    /// Admin address (governance or attestation contract)
    pub admin: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Anchor a Merkle root for a robot's reflex batch.
    /// Called by the attestation submission flow or governance.
    AnchorRoot {
        robot_id: String,
        batch_height: u64,
        merkle_root: String,
        cycle_count: u32,
    },

    /// Verify a Merkle proof for a specific leaf in a batch.
    /// Anyone can call this — it's a pure verification query.
    /// Returns success if the proof is valid, error otherwise.
    VerifyProof {
        robot_id: String,
        batch_height: u64,
        leaf_hash: String,
        leaf_index: u32,
        proof: Vec<String>,
    },

    /// Transfer admin to a new address.
    TransferAdmin {
        new_admin: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get the anchored Merkle root for a robot's batch
    #[returns(GetRootResponse)]
    GetRoot { robot_id: String, batch_height: u64 },

    /// Get the admin address
    #[returns(AdminResponse)]
    GetAdmin {},
}

#[cw_serde]
pub struct GetRootResponse {
    pub robot_id: String,
    pub batch_height: u64,
    pub merkle_root: String,
    pub cycle_count: u32,
    pub anchored_at: u64,
}

#[cw_serde]
pub struct AdminResponse {
    pub admin: String,
}
