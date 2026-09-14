use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Store a serialized Jolt proof blob on-chain.
    StoreProof {
        /// Base64-encoded serialized JoltProof (bincode/postcard format)
        proof_base64: String,
        /// Optional: hash of the program this proof is for
        program_hash: Option<String>,
    },
    /// Store the Jolt verifier preprocessing (the "verifying key") used by
    /// Phase 2 full verification. Admin only.
    StoreVerifyingKey {
        /// Base64-encoded serialized JoltVerifierPreprocessing (bincode 2)
        vk_base64: String,
    },
    /// Verify a stored proof's structure and extract claims.
    /// This deserializes the proof and returns the public claims.
    VerifyProof {
        /// Base64-encoded serialized JoltProof, or None to use stored proof
        proof_base64: Option<String>,
        /// Base64-encoded serialized JoltDevice public I/O (bincode 2).
        /// Required when verifier_mode is "full"; ignored in "structural".
        public_io_base64: Option<String>,
    },
    /// Set verification mode (admin only).
    /// "structural" = Phase 1 (size + magic check)
    /// "full" = Phase 2 (sumcheck + PCS, requires bulk memory wasmvm)
    SetVerifierMode {
        mode: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns whether a proof is stored and its size.
    #[returns(ProofStatusResponse)]
    ProofStatus {},
    /// Returns the admin address.
    #[returns(AdminResponse)]
    Admin {},
}

#[cw_serde]
pub struct ProofStatusResponse {
    pub has_proof: bool,
    pub proof_size_bytes: u64,
    pub program_hash: Option<String>,
    pub proof_sha256: Option<String>,
    pub last_verify_verified: bool,
    pub last_verify_block: u64,
    pub verifier_mode: Option<String>,
}

#[cw_serde]
pub struct AdminResponse {
    pub admin: String,
}

#[cw_serde]
pub struct MigrateMsg {}
