#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use super::*;
    use crate::contract::{instantiate, execute};
    use crate::msg::{InstantiateMsg, ExecuteMsg, ProposalTypeInput, VoteChoiceInput};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: "juno1admin".to_string(),
            voting_denom: "ujclaw".to_string(),
            community_pool: "juno1pool".to_string(),
            total_supply: 54660000000000,
            voting_period: 1000,
            quorum: 10,
            threshold: 50,
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert!(res.attributes.iter().any(|a| a.key == "action" && a.value == "instantiate"));
    }

    #[test]
    fn submit_text_proposal() {
        let mut deps = mock_dependencies();
        let init_msg = InstantiateMsg {
            admin: "admin".to_string(),
            voting_denom: "ujclaw".to_string(),
            community_pool: "pool".to_string(),
            total_supply: 54660000000000,
            voting_period: 1000,
            quorum: 10,
            threshold: 50,
        };
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init_msg);

        let submit_msg = ExecuteMsg::SubmitProposal {
            title: "Test Proposal".to_string(),
            description: "This is a test".to_string(),
            proposal_type: ProposalTypeInput::Text,
        };
        let res = execute(deps.as_mut(), mock_env(), mock_info("proposer", &[]), submit_msg);
        assert!(res.is_ok());
        let res = res.unwrap();
        assert!(res.attributes.iter().any(|a| a.key == "proposal_id" && a.value == "1"));
    }

    #[test]
    fn vote_without_tokens_fails() {
        let mut deps = mock_dependencies();
        let init_msg = InstantiateMsg {
            admin: "admin".to_string(),
            voting_denom: "ujclaw".to_string(),
            community_pool: "pool".to_string(),
            total_supply: 54660000000000,
            voting_period: 1000,
            quorum: 10,
            threshold: 50,
        };
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init_msg);

        let submit_msg = ExecuteMsg::SubmitProposal {
            title: "Test".to_string(),
            description: "Test".to_string(),
            proposal_type: ProposalTypeInput::Text,
        };
        let _ = execute(deps.as_mut(), mock_env(), mock_info("proposer", &[]), submit_msg);

        let vote_msg = ExecuteMsg::Vote {
            proposal_id: 1,
            vote: VoteChoiceInput::Yes,
        };
        let res = execute(deps.as_mut(), mock_env(), mock_info("voter", &[]), vote_msg);
        assert!(res.is_err()); // No tokens = can't vote
    }

    #[test]
    fn double_vote_fails() {
        let mut deps = mock_dependencies();
        deps.querier.update_balance("voter", cosmwasm_std::coins(1000000, "ujclaw"));
        let init_msg = InstantiateMsg {
            admin: "admin".to_string(),
            voting_denom: "ujclaw".to_string(),
            community_pool: "pool".to_string(),
            total_supply: 54660000000000,
            voting_period: 1000,
            quorum: 10,
            threshold: 50,
        };
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init_msg);

        let submit_msg = ExecuteMsg::SubmitProposal {
            title: "Test".to_string(),
            description: "Test".to_string(),
            proposal_type: ProposalTypeInput::Text,
        };
        let _ = execute(deps.as_mut(), mock_env(), mock_info("proposer", &[]), submit_msg);

        let vote_msg = ExecuteMsg::Vote {
            proposal_id: 1,
            vote: VoteChoiceInput::Yes,
        };
        let res1 = execute(deps.as_mut(), mock_env(), mock_info("voter", &[]), vote_msg.clone());
        assert!(res1.is_ok());

        let res2 = execute(deps.as_mut(), mock_env(), mock_info("voter", &[]), vote_msg);
        assert!(res2.is_err()); // Already voted
    }

    #[test]
    fn unauthorized_update_params_fails() {
        let mut deps = mock_dependencies();
        let init_msg = InstantiateMsg {
            admin: "admin".to_string(),
            voting_denom: "ujclaw".to_string(),
            community_pool: "pool".to_string(),
            total_supply: 54660000000000,
            voting_period: 1000,
            quorum: 10,
            threshold: 50,
        };
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init_msg);

        let update_msg = ExecuteMsg::UpdateParams {
            voting_period: Some(2000),
            quorum: None,
            threshold: None,
        };
        let res = execute(deps.as_mut(), mock_env(), mock_info("not_admin", &[]), update_msg);
        assert!(res.is_err());
    }
}
