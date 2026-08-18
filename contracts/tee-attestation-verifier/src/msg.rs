use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    /// Admin address (governance multisig or DAO)
    pub admin: String,
    /// Trusted attestation measurement (MRENCLAVE for SGX, launch digest for SEV-SNP)
    pub trusted_measurement: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Submit a TEE attestation report for verification.
    /// The report contains:
    /// - attestation_type: "sgx" or "sev-snp"
    /// - measurement: MRENCLAVE / launch digest
    /// - report_data: user data (hash of ZK proof + public inputs)
    /// - report_hex: raw attestation report (hex-encoded)
    /// - signature_hex: attestation signature (hex-encoded)
    /// - signer_pubkey_hex: signer's public key (for SEV-SNP VCEK or SGX quote signing key)
    VerifyAttestation {
        robot_id: String,
        attestation_type: String,
        measurement: String,
        report_data: String,
        report_hex: String,
        signature_hex: String,
        signer_pubkey_hex: String,
    },

    /// Update the trusted measurement (admin only)
    UpdateTrustedMeasurement {
        measurement: String,
    },

    /// Transfer admin
    TransferAdmin {
        new_admin: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get the last attestation verification result for a robot
    #[returns(AttestationResponse)]
    GetAttestation { robot_id: String },

    /// Get the trusted measurement
    #[returns(TrustedMeasurementResponse)]
    GetTrustedMeasurement {},

    /// Get the admin address
    #[returns(AdminResponse)]
    GetAdmin {},
}

#[cw_serde]
pub struct AttestationResponse {
    pub robot_id: String,
    pub verified: bool,
    pub attestation_type: String,
    pub measurement: String,
    pub report_data: String,
    pub verified_at: Option<u64>,
}

#[cw_serde]
pub struct TrustedMeasurementResponse {
    pub measurement: String,
}

#[cw_serde]
pub struct AdminResponse {
    pub admin: String,
}
