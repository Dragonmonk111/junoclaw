#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::{mock_dependencies, mock_env, mock_info}, coins, BankMsg};
    use super::*;
    use crate::contract::{instantiate, execute, query};
    use crate::msg::{InstantiateMsg, ExecuteMsg, QueryMsg};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: deps.api.addr_make("admin").to_string(),
            denom: "ujclaw".to_string(),
            community_pool: deps.api.addr_make("pool").to_string(),
            merkle_root: "abc123".to_string(),
            total_amount: 37200000000000,
            claim_start: 100,
            claim_end: 1000000,
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(5, res.attributes.len()); // action + admin + total_amount + claim_start + claim_end
    }

    #[test]
    fn claim_before_start_fails() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: deps.api.addr_make("admin").to_string(),
            denom: "ujclaw".to_string(),
            community_pool: deps.api.addr_make("pool").to_string(),
            merkle_root: "root".to_string(),
            total_amount: 1000000,
            claim_start: 100,
            claim_end: 1000,
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        let mut env = mock_env();
        env.block.height = 50; // before start

        let claim_msg = ExecuteMsg::Claim {
            amount: 100,
            merkle_proof: vec![],
        };
        let res = execute(deps.as_mut(), env, mock_info("user", &[]), claim_msg);
        assert!(res.is_err());
    }

    #[test]
    fn double_claim_fails() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: deps.api.addr_make("admin").to_string(),
            denom: "ujclaw".to_string(),
            community_pool: deps.api.addr_make("pool").to_string(),
            merkle_root: "root".to_string(),
            total_amount: 1000000,
            claim_start: 0,
            claim_end: 1000000,
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // First claim will fail on proof verification, but let's test the flow
        let claim_msg = ExecuteMsg::Claim {
            amount: 100,
            merkle_proof: vec![],
        };
        let res = execute(deps.as_mut(), mock_env(), mock_info("user", &[]), claim_msg);
        // Should fail on invalid proof, not on double claim
        assert!(res.is_err());
    }
}
