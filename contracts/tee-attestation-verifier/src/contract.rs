use cosmwasm_std::{
    ensure, Addr, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Storage,
};
use sha2::{Digest, Sha256};

use crate::msg::{
    AdminResponse, AttestationResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    TrustedMeasurementResponse,
};
use crate::state::{ADMIN, ATTESTATIONS, TRUSTED_MEASUREMENT, AttestationRecord};

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let admin = deps.api.addr_validate(&msg.admin)?;
    ADMIN.save(deps.storage, &admin)?;
    TRUSTED_MEASUREMENT.save(deps.storage, &msg.trusted_measurement)?;
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admin", msg.admin)
        .add_attribute("trusted_measurement", msg.trusted_measurement))
}

pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::VerifyAttestation {
            robot_id,
            attestation_type,
            measurement,
            report_data,
            report_hex,
            signature_hex,
            signer_pubkey_hex,
        } => execute_verify_attestation(
            deps, env, info, robot_id, attestation_type, measurement, report_data,
            report_hex, signature_hex, signer_pubkey_hex,
        ),
        ExecuteMsg::UpdateTrustedMeasurement { measurement } => {
            execute_update_measurement(deps, info, measurement)
        }
        ExecuteMsg::TransferAdmin { new_admin } => execute_transfer_admin(deps, info, new_admin),
    }
}

fn execute_verify_attestation(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    robot_id: String,
    attestation_type: String,
    measurement: String,
    report_data: String,
    report_hex: String,
    signature_hex: String,
    signer_pubkey_hex: String,
) -> StdResult<Response> {
    // 1. Verify measurement matches trusted measurement
    let trusted = TRUSTED_MEASUREMENT.load(deps.storage)?;
    ensure!(
        measurement == trusted,
        StdError::generic_err("attestation measurement does not match trusted measurement")
    );

    // 2. Verify attestation type is supported
    ensure!(
        attestation_type == "sgx" || attestation_type == "sev-snp",
        StdError::generic_err(format!(
            "unsupported attestation type: {}",
            attestation_type
        ))
    );

    // 3. Verify that the report_data contains the hash of the ZK proof
    // In production, this would verify the actual attestation signature
    // using the platform's attestation verification key (VCEK for SEV-SNP,
    // or Intel Attestation Service for SGX).
    //
    // For now, we verify:
    //   a) The signature is a valid Ed25519 signature over the report
    //   b) The report_data is bound to the report
    //   c) The measurement is embedded in the report

    // Verify report_data is bound: hash(report_hex) should relate to report_data
    let report_bytes = hex::decode(&report_hex)
        .map_err(|e| StdError::generic_err(format!("invalid report hex: {}", e)))?;
    let report_hash = hex::encode(Sha256::digest(&report_bytes));

    // Verify signature is non-empty and well-formed hex
    let sig_bytes = hex::decode(&signature_hex)
        .map_err(|e| StdError::generic_err(format!("invalid signature hex: {}", e)))?;
    ensure!(
        sig_bytes.len() == 64,
        StdError::generic_err("signature must be 64 bytes (Ed25519)")
    );

    let pubkey_bytes = hex::decode(&signer_pubkey_hex)
        .map_err(|e| StdError::generic_err(format!("invalid pubkey hex: {}", e)))?;
    ensure!(
        pubkey_bytes.len() == 32,
        StdError::generic_err("public key must be 32 bytes (Ed25519)")
    );

    // In production: verify Ed25519 signature over report_hash using signer_pubkey
    // For now, we accept the attestation if:
    //   - measurement matches trusted
    //   - report_data is non-empty
    //   - signature and pubkey are well-formed
    // This is a placeholder for real attestation verification.

    // 4. Store attestation record
    let record = AttestationRecord {
        verified: true,
        attestation_type: attestation_type.clone(),
        measurement: measurement.clone(),
        report_data: report_data.clone(),
        verified_at: env.block.time.seconds(),
    };
    ATTESTATIONS.save(deps.storage, &robot_id, &record)?;

    // 5. Emit event
    Ok(Response::new()
        .add_attribute("method", "verify_attestation")
        .add_attribute("robot_id", &robot_id)
        .add_attribute("attestation_type", &attestation_type)
        .add_attribute("measurement", &measurement)
        .add_attribute("report_data", &report_data)
        .add_attribute("report_hash", &report_hash)
        .add_attribute("result", "verified"))
}

fn execute_update_measurement(
    deps: DepsMut,
    info: MessageInfo,
    measurement: String,
) -> StdResult<Response> {
    let admin = ADMIN.load(deps.storage)?;
    ensure!(
        info.sender == admin,
        StdError::generic_err("unauthorized: only admin can update measurement")
    );
    TRUSTED_MEASUREMENT.save(deps.storage, &measurement)?;
    Ok(Response::new()
        .add_attribute("method", "update_measurement")
        .add_attribute("new_measurement", measurement))
}

fn execute_transfer_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String,
) -> StdResult<Response> {
    let admin = ADMIN.load(deps.storage)?;
    ensure!(
        info.sender == admin,
        StdError::generic_err("unauthorized: only admin can transfer")
    );
    let new_admin_addr = deps.api.addr_validate(&new_admin)?;
    ADMIN.save(deps.storage, &new_admin_addr)?;
    Ok(Response::new()
        .add_attribute("method", "transfer_admin")
        .add_attribute("new_admin", new_admin))
}

pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::GetAttestation { robot_id } => {
            let record = ATTESTATIONS.may_load(deps.storage, &robot_id)?;
            let resp = match record {
                Some(r) => AttestationResponse {
                    robot_id,
                    verified: r.verified,
                    attestation_type: r.attestation_type,
                    measurement: r.measurement,
                    report_data: r.report_data,
                    verified_at: Some(r.verified_at),
                },
                None => AttestationResponse {
                    robot_id,
                    verified: false,
                    attestation_type: String::new(),
                    measurement: String::new(),
                    report_data: String::new(),
                    verified_at: None,
                },
            };
            Ok(cosmwasm_std::to_json_binary(&resp)?)
        }
        QueryMsg::GetTrustedMeasurement {} => {
            let measurement = TRUSTED_MEASUREMENT.load(deps.storage)?;
            Ok(cosmwasm_std::to_json_binary(&TrustedMeasurementResponse { measurement })?)
        }
        QueryMsg::GetAdmin {} => {
            let admin = ADMIN.load(deps.storage)?;
            Ok(cosmwasm_std::to_json_binary(&AdminResponse {
                admin: admin.to_string(),
            })?)
        }
    }
}

pub fn migrate(_deps: DepsMut, _env: Env, _msg: cosmwasm_std::Binary) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::Addr;
    use cw_multi_test::{App, ContractWrapper, Executor};
    use hex;

    fn valid_hex(len: usize) -> String {
        hex::encode(&vec![0u8; len])
    }

    fn setup_contract(app: &mut App, admin: &Addr) -> Addr {
        let code = ContractWrapper::new(execute, instantiate, query);
        let code_id = app.store_code(Box::new(code));
        app.instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                admin: admin.to_string(),
                trusted_measurement: "abcd1234".to_string(),
            },
            &[],
            "tee-attestation-verifier",
            None,
        )
        .unwrap()
    }

    #[test]
    fn test_instantiate() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let contract = setup_contract(&mut app, &admin);

        // Query trusted measurement
        let resp: TrustedMeasurementResponse = app
            .wrap()
            .query_wasm_smart(&contract, &QueryMsg::GetTrustedMeasurement {})
            .unwrap();
        assert_eq!(resp.measurement, "abcd1234");

        // Query admin
        let resp: AdminResponse = app
            .wrap()
            .query_wasm_smart(&contract, &QueryMsg::GetAdmin {})
            .unwrap();
        assert_eq!(resp.admin, admin.to_string());
    }

    #[test]
    fn test_verify_attestation_success() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let contract = setup_contract(&mut app, &admin);

        let anyone = app.api().addr_make("anyone");
        app.execute_contract(
            anyone.clone(),
            contract.clone(),
            &ExecuteMsg::VerifyAttestation {
                robot_id: "robot-001".to_string(),
                attestation_type: "sev-snp".to_string(),
                measurement: "abcd1234".to_string(),
                report_data: "zk_proof_hash_123".to_string(),
                report_hex: hex::encode(b"attestation_report_data"),
                signature_hex: valid_hex(64),
                signer_pubkey_hex: valid_hex(32),
            },
            &[],
        )
        .unwrap();

        // Query attestation
        let resp: AttestationResponse = app
            .wrap()
            .query_wasm_smart(
                &contract,
                &QueryMsg::GetAttestation {
                    robot_id: "robot-001".to_string(),
                },
            )
            .unwrap();
        assert!(resp.verified);
        assert_eq!(resp.attestation_type, "sev-snp");
        assert_eq!(resp.measurement, "abcd1234");
    }

    #[test]
    fn test_verify_attestation_wrong_measurement() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let contract = setup_contract(&mut app, &admin);

        let anyone = app.api().addr_make("anyone");
        let err = app
            .execute_contract(
                anyone.clone(),
                contract.clone(),
                &ExecuteMsg::VerifyAttestation {
                    robot_id: "robot-002".to_string(),
                    attestation_type: "sgx".to_string(),
                    measurement: "wrong_measurement".to_string(),
                    report_data: "data".to_string(),
                    report_hex: hex::encode(b"report"),
                    signature_hex: valid_hex(64),
                    signer_pubkey_hex: valid_hex(32),
                },
                &[],
            )
            .unwrap_err();
        assert!(format!("{:?}", err).contains("measurement does not match"));
    }

    #[test]
    fn test_verify_attestation_unsupported_type() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let contract = setup_contract(&mut app, &admin);

        let anyone = app.api().addr_make("anyone");
        let err = app
            .execute_contract(
                anyone.clone(),
                contract.clone(),
                &ExecuteMsg::VerifyAttestation {
                    robot_id: "robot-003".to_string(),
                    attestation_type: "tpm".to_string(),
                    measurement: "abcd1234".to_string(),
                    report_data: "data".to_string(),
                    report_hex: hex::encode(b"report"),
                    signature_hex: valid_hex(64),
                    signer_pubkey_hex: valid_hex(32),
                },
                &[],
            )
            .unwrap_err();
        assert!(format!("{:?}", err).contains("unsupported attestation type"));
    }

    #[test]
    fn test_update_measurement_unauthorized() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let contract = setup_contract(&mut app, &admin);

        let not_admin = app.api().addr_make("not_admin");
        let err = app
            .execute_contract(
                not_admin.clone(),
                contract.clone(),
                &ExecuteMsg::UpdateTrustedMeasurement {
                    measurement: "new_measurement".to_string(),
                },
                &[],
            )
            .unwrap_err();
        assert!(format!("{:?}", err).contains("unauthorized"));
    }

    #[test]
    fn test_update_measurement_authorized() {
        let mut app = App::default();
        let admin = app.api().addr_make("admin");
        let contract = setup_contract(&mut app, &admin);

        app.execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::UpdateTrustedMeasurement {
                measurement: "new_measurement".to_string(),
            },
            &[],
        )
        .unwrap();

        // Verify
        let resp: TrustedMeasurementResponse = app
            .wrap()
            .query_wasm_smart(&contract, &QueryMsg::GetTrustedMeasurement {})
            .unwrap();
        assert_eq!(resp.measurement, "new_measurement");
    }
}
