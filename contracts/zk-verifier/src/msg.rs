use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Store a Groth16 verification key (arkworks CanonicalSerialize, base64-encoded).
    /// Only admin can call this. One VK per contract instance.
    StoreVk {
        /// Base64-encoded arkworks CanonicalSerialize bytes of VerifyingKey<Bn254>
        vk_base64: String,
    },
    /// Verify a Groth16 proof against the stored VK.
    /// Returns Ok if valid, Err(ProofInvalid) if not.
    VerifyProof {
        /// Base64-encoded arkworks CanonicalSerialize bytes of Proof<Bn254>
        proof_base64: String,
        /// Base64-encoded arkworks CanonicalSerialize bytes of public inputs [Fr]
        public_inputs_base64: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns whether a verification key is stored.
    #[returns(VkStatusResponse)]
    VkStatus {},
    /// Returns the gas estimate for the last verification (if any).
    #[returns(LastVerifyResponse)]
    LastVerify {},
}

#[cw_serde]
pub struct VkStatusResponse {
    pub has_vk: bool,
    pub vk_size_bytes: u64,
}

#[cw_serde]
pub struct LastVerifyResponse {
    pub verified: bool,
    pub block_height: u64,
}

#[cw_serde]
pub struct MigrateMsg {}
