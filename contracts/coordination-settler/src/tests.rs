#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, Binary};
    use cw_multi_test::{App, ContractWrapper, Executor};

    use crate::contract::{execute, instantiate, query};
    use crate::error::ContractError;
    use crate::msg::{
        BatchResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
        RelayerResponse, ValidatorSetResponse,
    };

    fn setup_contract() -> (App, Addr) {
        let mut app = App::default();

        let contract = ContractWrapper::new(execute, instantiate, query);
        let code_id = app.store_code(Box::new(contract));

        let admin = app.api().addr_make("admin");

        let validators = vec![
            Binary::from(vec![0x11; 48]),
            Binary::from(vec![0x22; 48]),
            Binary::from(vec![0x33; 48]),
            Binary::from(vec![0x44; 48]),
        ];

        let instantiate_msg = InstantiateMsg {
            admin: admin.to_string(),
            validators,
            threshold: 3,
        };

        let addr = app
            .instantiate_contract(
                code_id,
                admin.clone(),
                &instantiate_msg,
                &[],
                "coordination-settler",
                None,
            )
            .unwrap();

        (app, addr)
    }

    #[test]
    fn test_instantiate() {
        let (app, addr) = setup_contract();

        let resp: ConfigResponse = app
            .wrap()
            .query_wasm_smart(&addr, &QueryMsg::Config {})
            .unwrap();

        assert_eq!(resp.threshold, 3);
        assert_eq!(resp.validator_count, 4);
        assert_eq!(resp.relayer_count, 1);
        assert_eq!(resp.latest_height, None);
    }

    #[test]
    fn test_query_validator_set() {
        let (app, addr) = setup_contract();

        let resp: ValidatorSetResponse = app
            .wrap()
            .query_wasm_smart(&addr, &QueryMsg::ValidatorSet {})
            .unwrap();

        assert_eq!(resp.validators.len(), 4);
        assert_eq!(resp.threshold, 3);
    }

    fn compute_cert(messages_hash: &[u8; 32], validators: &[[u8; 48]]) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(messages_hash);
        for v in validators {
            hasher.update(v);
        }
        hasher.finalize().to_vec()
    }

    #[test]
    fn test_submit_batch() {
        let (mut app, addr) = setup_contract();
        let admin = app.api().addr_make("admin");

        let validators: [[u8; 48]; 4] = [
            [0x11; 48], [0x22; 48], [0x33; 48], [0x44; 48],
        ];
        let messages_hash = [0xBB; 32];
        let cert = Binary::from(compute_cert(&messages_hash, &validators));

        let _resp = app
            .execute_contract(
                admin,
                addr.clone(),
                &ExecuteMsg::SubmitBatch {
                    certificate: cert.clone(),
                    messages_hash,
                    commonware_height: 1,
                    timestamp: 1700000000000,
                },
                &[],
            )
            .unwrap();

        let batch: BatchResponse = app
            .wrap()
            .query_wasm_smart(&addr, &QueryMsg::Batch { commonware_height: 1 })
            .unwrap();

        assert_eq!(batch.commonware_height, 1);
        assert_eq!(batch.messages_hash, messages_hash);
    }

    #[test]
    fn test_duplicate_batch_rejected() {
        let (mut app, addr) = setup_contract();
        let admin = app.api().addr_make("admin");

        let validators: [[u8; 48]; 4] = [
            [0x11; 48], [0x22; 48], [0x33; 48], [0x44; 48],
        ];
        let messages_hash = [0xBB; 32];
        let cert = Binary::from(compute_cert(&messages_hash, &validators));

        app.execute_contract(
            admin.clone(),
            addr.clone(),
            &ExecuteMsg::SubmitBatch {
                certificate: cert.clone(),
                messages_hash,
                commonware_height: 1,
                timestamp: 1700000000000,
            },
            &[],
        )
        .unwrap();

        let err = app
            .execute_contract(
                admin,
                addr.clone(),
                &ExecuteMsg::SubmitBatch {
                    certificate: cert,
                    messages_hash,
                    commonware_height: 1,
                    timestamp: 1700000000001,
                },
                &[],
            )
            .unwrap_err();
        let cerr: ContractError = err.downcast().unwrap();
        assert!(matches!(cerr, ContractError::BatchAlreadySettled { .. }));
    }

    #[test]
    fn test_non_relayer_rejected() {
        let (mut app, addr) = setup_contract();
        let random_user = app.api().addr_make("random_user");

        let validators: [[u8; 48]; 4] = [
            [0x11; 48], [0x22; 48], [0x33; 48], [0x44; 48],
        ];
        let messages_hash = [0xBB; 32];
        let cert = Binary::from(compute_cert(&messages_hash, &validators));

        let err = app
            .execute_contract(
                random_user,
                addr.clone(),
                &ExecuteMsg::SubmitBatch {
                    certificate: Binary::from(cert),
                    messages_hash,
                    commonware_height: 1,
                    timestamp: 1700000000000,
                },
                &[],
            )
            .unwrap_err();
        let cerr: ContractError = err.downcast().unwrap();
        assert!(matches!(cerr, ContractError::Unauthorized { .. }));
    }

    #[test]
    fn test_update_validator_set() {
        let (mut app, addr) = setup_contract();
        let admin = app.api().addr_make("admin");

        let new_validators = vec![
            Binary::from(vec![0x11; 48]),
            Binary::from(vec![0x22; 48]),
            Binary::from(vec![0x33; 48]),
            Binary::from(vec![0x44; 48]),
            Binary::from(vec![0x55; 48]),
        ];

        app.execute_contract(
            admin,
            addr.clone(),
            &ExecuteMsg::UpdateValidatorSet {
                validators: new_validators,
                threshold: 4,
            },
            &[],
        )
        .unwrap();

        let resp: ValidatorSetResponse = app
            .wrap()
            .query_wasm_smart(&addr, &QueryMsg::ValidatorSet {})
            .unwrap();

        assert_eq!(resp.validators.len(), 5);
        assert_eq!(resp.threshold, 4);
    }

    #[test]
    fn test_non_admin_cannot_update_validators() {
        let (mut app, addr) = setup_contract();
        let random_user = app.api().addr_make("random_user");

        let err = app
            .execute_contract(
                random_user,
                addr.clone(),
                &ExecuteMsg::UpdateValidatorSet {
                    validators: vec![Binary::from(vec![0x99; 48])],
                    threshold: 1,
                },
                &[],
            )
            .unwrap_err();
        let cerr: ContractError = err.downcast().unwrap();
        assert!(matches!(cerr, ContractError::Unauthorized { .. }));
    }

    #[test]
    fn test_register_and_remove_relayer() {
        let (mut app, addr) = setup_contract();
        let admin = app.api().addr_make("admin");
        let relayer = app.api().addr_make("relayer1");

        app.execute_contract(
            admin.clone(),
            addr.clone(),
            &ExecuteMsg::RegisterRelayer {
                address: relayer.to_string(),
            },
            &[],
        )
        .unwrap();

        let resp: RelayerResponse = app
            .wrap()
            .query_wasm_smart(
                &addr,
                &QueryMsg::Relayer {
                    address: relayer.to_string(),
                },
            )
            .unwrap();
        assert!(resp.is_relayer);

        app.execute_contract(
            admin,
            addr.clone(),
            &ExecuteMsg::RemoveRelayer {
                address: relayer.to_string(),
            },
            &[],
        )
        .unwrap();

        let resp: RelayerResponse = app
            .wrap()
            .query_wasm_smart(
                &addr,
                &QueryMsg::Relayer {
                    address: relayer.to_string(),
                },
            )
            .unwrap();
        assert!(!resp.is_relayer);
    }

    #[test]
    fn test_latest_batch() {
        let (mut app, addr) = setup_contract();
        let admin = app.api().addr_make("admin");

        let validators: [[u8; 48]; 4] = [
            [0x11; 48], [0x22; 48], [0x33; 48], [0x44; 48],
        ];

        for h in 1..=3u64 {
            let messages_hash = [h as u8; 32];
            let cert = Binary::from(compute_cert(&messages_hash, &validators));
            app.execute_contract(
                admin.clone(),
                addr.clone(),
                &ExecuteMsg::SubmitBatch {
                    certificate: cert,
                    messages_hash,
                    commonware_height: h,
                    timestamp: 1700000000000 + h * 1000,
                },
                &[],
            )
            .unwrap();
        }

        let resp: BatchResponse = app
            .wrap()
            .query_wasm_smart(&addr, &QueryMsg::LatestBatch {})
            .unwrap();

        assert_eq!(resp.commonware_height, 3);
    }

    #[test]
    fn test_invalid_certificate_rejected() {
        let (mut app, addr) = setup_contract();
        let admin = app.api().addr_make("admin");

        let err = app
            .execute_contract(
                admin,
                addr.clone(),
                &ExecuteMsg::SubmitBatch {
                    certificate: Binary::from(vec![0xAA; 32]),
                    messages_hash: [0xBB; 32],
                    commonware_height: 1,
                    timestamp: 1700000000000,
                },
                &[],
            )
            .unwrap_err();
        let cerr: ContractError = err.downcast().unwrap();
        assert!(matches!(cerr, ContractError::InvalidCertificate { .. }));
    }

    #[test]
    fn test_update_admin() {
        let (mut app, addr) = setup_contract();
        let admin = app.api().addr_make("admin");
        let new_admin = app.api().addr_make("new_admin");

        app.execute_contract(
            admin,
            addr.clone(),
            &ExecuteMsg::UpdateAdmin {
                new_admin: new_admin.to_string(),
            },
            &[],
        )
        .unwrap();

        let resp: ConfigResponse = app
            .wrap()
            .query_wasm_smart(&addr, &QueryMsg::Config {})
            .unwrap();

        assert_eq!(resp.admin, new_admin.as_str());

        let err = app
            .execute_contract(
                app.api().addr_make("admin"),
                addr.clone(),
                &ExecuteMsg::UpdateValidatorSet {
                    validators: vec![Binary::from(vec![0x99; 48])],
                    threshold: 1,
                },
                &[],
            )
            .unwrap_err();
        let cerr: ContractError = err.downcast().unwrap();
        assert!(matches!(cerr, ContractError::Unauthorized { .. }));
    }
}
