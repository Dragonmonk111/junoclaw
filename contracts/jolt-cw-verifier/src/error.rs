use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] cosmwasm_std::StdError),

    #[error("No proof stored. Call StoreProof first or provide proof_base64.")]
    NoProofStored,

    #[error("Proof verification failed: {0}")]
    VerificationFailed(String),

    #[error("Proof deserialization failed: {0}")]
    DeserializationFailed(String),

    #[error("Unauthorized: only admin can call this")]
    Unauthorized,
}
