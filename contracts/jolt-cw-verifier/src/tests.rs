#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::from_json;

    use super::*;
    use crate::contract::{execute, instantiate, query};
    use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ProofStatusResponse};

    #[test]
    fn instantiate_works() {
        let mut deps = mock_dependencies();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg { admin: None };

        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes[0].value, "instantiate");
    }

    #[test]
    fn store_and_query_proof() {
        let mut deps = mock_dependencies();
        let info = mock_info("creator", &[]);

        // Instantiate
        let init_msg = InstantiateMsg { admin: None };
        instantiate(deps.as_mut(), mock_env(), info.clone(), init_msg).unwrap();

        // Store a dummy proof (just some bytes for Phase 1 testing)
        let dummy_proof = vec![0u8; 75000]; // ~75KB, typical Jolt proof size
        let proof_b64 = cosmwasm_std::to_base64(&dummy_proof);

        let store_msg = ExecuteMsg::StoreProof {
            proof_base64: proof_b64,
            program_hash: Some("sha256:abc123".to_string()),
        };
        let res = execute(deps.as_mut(), mock_env(), info, store_msg).unwrap();
        assert_eq!(res.attributes[1].value, "75000");

        // Query proof status
        let q_res = query(deps.as_ref(), mock_env(), QueryMsg::ProofStatus {}).unwrap();
        let status: ProofStatusResponse = from_json(q_res).unwrap();
        assert!(status.has_proof);
        assert_eq!(status.proof_size_bytes, 75000);
        assert_eq!(status.program_hash, Some("sha256:abc123".to_string()));
        assert!(status.proof_sha256.is_some());
    }

    #[test]
    fn set_verifier_mode_works() {
        let mut deps = mock_dependencies();
        let info = mock_info("creator", &[]);

        instantiate(
            deps.as_mut(),
            mock_env(),
            info.clone(),
            InstantiateMsg { admin: None },
        )
        .unwrap();

        let mode_msg = ExecuteMsg::SetVerifierMode {
            mode: "full".to_string(),
        };
        let res = execute(deps.as_mut(), mock_env(), info, mode_msg).unwrap();
        assert_eq!(res.attributes[0].value, "set_verifier_mode");
        assert_eq!(res.attributes[1].value, "full");

        let q_res = query(deps.as_ref(), mock_env(), QueryMsg::ProofStatus {}).unwrap();
        let status: ProofStatusResponse = from_json(q_res).unwrap();
        assert_eq!(status.verifier_mode, Some("full".to_string()));
    }

    #[test]
    fn set_verifier_mode_unauthorized() {
        let mut deps = mock_dependencies();
        let info = mock_info("creator", &[]);

        instantiate(
            deps.as_mut(),
            mock_env(),
            info.clone(),
            InstantiateMsg { admin: None },
        )
        .unwrap();

        let mode_msg = ExecuteMsg::SetVerifierMode {
            mode: "full".to_string(),
        };
        let non_admin = mock_info("attacker", &[]);
        let res = execute(deps.as_mut(), mock_env(), non_admin, mode_msg);
        assert!(res.is_err());
    }

    #[test]
    fn set_verifier_mode_invalid() {
        let mut deps = mock_dependencies();
        let info = mock_info("creator", &[]);

        instantiate(
            deps.as_mut(),
            mock_env(),
            info.clone(),
            InstantiateMsg { admin: None },
        )
        .unwrap();

        let mode_msg = ExecuteMsg::SetVerifierMode {
            mode: "invalid".to_string(),
        };
        let res = execute(deps.as_mut(), mock_env(), info, mode_msg);
        assert!(res.is_err());
    }

    #[test]
    fn verify_proof_works() {
        let mut deps = mock_dependencies();
        let info = mock_info("creator", &[]);

        // Instantiate
        instantiate(
            deps.as_mut(),
            mock_env(),
            info.clone(),
            InstantiateMsg { admin: None },
        )
        .unwrap();

        // Verify with inline proof
        let dummy_proof = vec![0u8; 75000];
        let proof_b64 = cosmwasm_std::to_base64(&dummy_proof);

        let verify_msg = ExecuteMsg::VerifyProof {
            proof_base64: Some(proof_b64),
            public_io_base64: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, verify_msg).unwrap();
        assert_eq!(res.attributes[2].value, "true");
    }

    #[test]
    fn verify_empty_proof_fails() {
        let mut deps = mock_dependencies();
        let info = mock_info("creator", &[]);

        instantiate(
            deps.as_mut(),
            mock_env(),
            info.clone(),
            InstantiateMsg { admin: None },
        )
        .unwrap();

        let empty_b64 = cosmwasm_std::to_base64(&[]);
        let verify_msg = ExecuteMsg::VerifyProof {
            proof_base64: Some(empty_b64),
            public_io_base64: None,
        };
        let res = execute(deps.as_mut(), mock_env(), info, verify_msg);
        assert!(res.is_err());
    }
}
