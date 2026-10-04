#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info, MockApi};
    use cosmwasm_std::{coins, BankMsg, CosmosMsg, DepsMut, Response};
    use super::*;
    use crate::contract::{instantiate, execute, query};
    use crate::msg::{InstantiateMsg, ExecuteMsg, ProposalTypeInput, QueryMsg, LockResponse,
        LockStatsResponse, VoteChoiceInput, VoteResponse};

    fn default_init(api: &MockApi) -> InstantiateMsg {
        InstantiateMsg {
            admin: api.addr_make("admin").to_string(),
            voting_denom: "ujclaw".to_string(),
            community_pool: api.addr_make("pool").to_string(),
            total_supply: 54660000000000,
            voting_period: 1000,
            quorum: 10,
            threshold: 50,
        }
    }

    /// Lock voting tokens in the contract (attach ujclaw funds). This is the
    /// contract-internal lock that provides vote weight — not consensus staking
    /// (the chain has none).
    fn lock(deps: DepsMut, who: &str, amount: u128) -> cosmwasm_std::StdResult<Response> {
        let info = mock_info(who, &coins(amount, "ujclaw"));
        execute(deps, mock_env(), info, ExecuteMsg::Lock {}).map_err(|e| {
            cosmwasm_std::StdError::generic_err(e.to_string())
        })
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        let msg = default_init(&deps.api);
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert!(res.attributes.iter().any(|a| a.key == "action" && a.value == "instantiate"));
    }

    #[test]
    fn submit_text_proposal() {
        let mut deps = mock_dependencies();
        let init = default_init(&deps.api);
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init);

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
        let init = default_init(&deps.api);
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init);

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
        deps.querier.bank.update_balance("voter", coins(1000000, "ujclaw"));
        let init = default_init(&deps.api);
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init);

        lock(deps.as_mut(), "voter", 1000000).unwrap();

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

    /// THE REGRESSION: under the old balance-query design, a voter could vote,
    /// send the same tokens to a fresh wallet, and vote again. With locked
    /// voting power this is impossible — the fresh wallet has no lock, and the
    /// original voter's lock is held until the proposal's voting period ends.
    #[test]
    fn vote_transfer_revote_fails() {
        let mut deps = mock_dependencies();
        deps.querier.bank.update_balance("voter", coins(1000000, "ujclaw"));
        deps.querier.bank.update_balance("voter2", coins(1000000, "ujclaw")); // tokens moved here
        let init = default_init(&deps.api);
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init);

        lock(deps.as_mut(), "voter", 1000000).unwrap();

        let submit_msg = ExecuteMsg::SubmitProposal {
            title: "Test".to_string(),
            description: "Test".to_string(),
            proposal_type: ProposalTypeInput::Text,
        };
        let _ = execute(deps.as_mut(), mock_env(), mock_info("proposer", &[]), submit_msg);

        let vote_msg = ExecuteMsg::Vote { proposal_id: 1, vote: VoteChoiceInput::Yes };
        execute(deps.as_mut(), mock_env(), mock_info("voter", &[]), vote_msg.clone()).unwrap();

        // Attacker moved the tokens to voter2 — voter2 has a balance but NO lock.
        let res = execute(deps.as_mut(), mock_env(), mock_info("voter2", &[]), vote_msg);
        assert!(res.is_err());

        // And the original voter cannot pull their locked tokens out mid-vote.
        let unlock = ExecuteMsg::Unlock { amount: 1000000 };
        let res = execute(deps.as_mut(), mock_env(), mock_info("voter", &[]), unlock);
        assert!(res.is_err()); // LockedUntil
    }

    #[test]
    fn unlock_after_voting_period_works() {
        let mut deps = mock_dependencies();
        deps.querier.bank.update_balance("voter", coins(1000000, "ujclaw"));
        let init = default_init(&deps.api);
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init);

        lock(deps.as_mut(), "voter", 1000000).unwrap();

        let submit_msg = ExecuteMsg::SubmitProposal {
            title: "Test".to_string(),
            description: "Test".to_string(),
            proposal_type: ProposalTypeInput::Text,
        };
        execute(deps.as_mut(), mock_env(), mock_info("proposer", &[]), submit_msg).unwrap();
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("voter", &[]),
            ExecuteMsg::Vote { proposal_id: 1, vote: VoteChoiceInput::Yes },
        )
        .unwrap();

        // Locked during the voting period (end = created 12345 + 1000 = 13345).
        let unlock = ExecuteMsg::Unlock { amount: 1000000 };
        assert!(execute(deps.as_mut(), mock_env(), mock_info("voter", &[]), unlock.clone()).is_err());

        // Unlocked once voting ends — and a BankMsg::Send returns the tokens.
        let mut env = mock_env();
        env.block.height = 13345;
        let res = execute(deps.as_mut(), env, mock_info("voter", &[]), unlock).unwrap();
        assert!(res.messages.iter().any(|m| matches!(
            m.msg,
            CosmosMsg::Bank(BankMsg::Send { ref to_address, ref amount })
                if to_address == "voter" && amount == &coins(1000000, "ujclaw")
        )));
    }

    /// Vote weight must come from the locked amount, not the wallet's live
    /// balance — even if the bank balance is larger.
    #[test]
    fn vote_weight_is_locked_not_balance() {
        let mut deps = mock_dependencies();
        deps.querier.bank.update_balance("voter", coins(5000000, "ujclaw")); // live balance 5M
        let init = default_init(&deps.api);
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init);

        lock(deps.as_mut(), "voter", 500000).unwrap(); // but only 500K locked

        let submit_msg = ExecuteMsg::SubmitProposal {
            title: "Test".to_string(),
            description: "Test".to_string(),
            proposal_type: ProposalTypeInput::Text,
        };
        execute(deps.as_mut(), mock_env(), mock_info("proposer", &[]), submit_msg).unwrap();
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("voter", &[]),
            ExecuteMsg::Vote { proposal_id: 1, vote: VoteChoiceInput::Yes },
        )
        .unwrap();

        let raw = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetVote { proposal_id: 1, voter: "voter".to_string() },
        )
        .unwrap();
        let vote: VoteResponse = cosmwasm_std::from_json(&raw).unwrap();
        assert_eq!(vote.weight, 500000); // locked, not 5M
    }

    /// The treasury can only spend the contract balance minus locked tokens —
    /// a passed Spend proposal cannot drain voters' locked tokens.
    #[test]
    fn spend_cannot_touch_locked_tokens() {
        let mut deps = mock_dependencies();
        deps.querier.bank.update_balance("voter", coins(500000, "ujclaw"));
        // Contract holds 600K: 500K is voter lock, 100K is treasury.
        deps.querier.bank.update_balance(mock_env().contract.address, coins(600000, "ujclaw"));

        let init = InstantiateMsg {
            total_supply: 1000000,
            quorum: 10,
            ..default_init(&deps.api)
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init).unwrap();

        lock(deps.as_mut(), "voter", 500000).unwrap();

        // Proposal asks for 200K — more than the 100K spendable.
        let submit = ExecuteMsg::SubmitProposal {
            title: "Too big".to_string(),
            description: "Try to drain locked tokens".to_string(),
            proposal_type: ProposalTypeInput::Spend {
                recipient: deps.api.addr_make("recipient").to_string(),
                amount: 200000,
                denom: "ujclaw".to_string(),
            },
        };
        execute(deps.as_mut(), mock_env(), mock_info("proposer", &[]), submit).unwrap();
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("voter", &[]),
            ExecuteMsg::Vote { proposal_id: 1, vote: VoteChoiceInput::Yes },
        )
        .unwrap();

        let mut env = mock_env();
        env.block.height = 13345;
        let res = execute(
            deps.as_mut(),
            env,
            mock_info("anyone", &[]),
            ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        );
        assert!(res.is_err()); // InsufficientBalance — locked tokens are untouchable
    }

    /// A spend within the un-locked portion works and sends from the contract.
    #[test]
    fn spend_within_spendable_works() {
        let mut deps = mock_dependencies();
        deps.querier.bank.update_balance("voter", coins(500000, "ujclaw"));
        deps.querier.bank.update_balance(mock_env().contract.address, coins(600000, "ujclaw"));
        let builder = deps.api.addr_make("builder").to_string();

        let init = InstantiateMsg {
            total_supply: 1000000,
            quorum: 10,
            ..default_init(&deps.api)
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init).unwrap();
        lock(deps.as_mut(), "voter", 500000).unwrap();

        let submit = ExecuteMsg::SubmitProposal {
            title: "Grant".to_string(),
            description: "Fund a builder".to_string(),
            proposal_type: ProposalTypeInput::Spend {
                recipient: builder.clone(),
                amount: 100000,
                denom: "ujclaw".to_string(),
            },
        };
        execute(deps.as_mut(), mock_env(), mock_info("proposer", &[]), submit).unwrap();
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("voter", &[]),
            ExecuteMsg::Vote { proposal_id: 1, vote: VoteChoiceInput::Yes },
        )
        .unwrap();

        let mut env = mock_env();
        env.block.height = 13345;
        let res = execute(
            deps.as_mut(),
            env,
            mock_info("anyone", &[]),
            ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        )
        .unwrap();
        assert!(res.messages.iter().any(|m| matches!(
            m.msg,
            CosmosMsg::Bank(BankMsg::Send { ref to_address, ref amount })
                if *to_address == builder && amount == &coins(100000, "ujclaw")
        )));
    }

    #[test]
    fn lock_queries() {
        let mut deps = mock_dependencies();
        let voter = deps.api.addr_make("voter").to_string();
        let voter2 = deps.api.addr_make("voter2").to_string();
        deps.querier.bank.update_balance(&voter, coins(1000000, "ujclaw"));
        deps.querier.bank.update_balance(mock_env().contract.address, coins(700000, "ujclaw"));
        let init = default_init(&deps.api);
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init);

        lock(deps.as_mut(), &voter, 500000).unwrap();
        lock(deps.as_mut(), &voter2, 200000).unwrap();

        let raw = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetLock { address: voter.clone() },
        )
        .unwrap();
        let s: LockResponse = cosmwasm_std::from_json(&raw).unwrap();
        assert_eq!(s.locked, 500000);
        assert_eq!(s.locked_until, 0);

        let raw = query(deps.as_ref(), mock_env(), QueryMsg::GetLockStats {}).unwrap();
        let stats: LockStatsResponse = cosmwasm_std::from_json(&raw).unwrap();
        assert_eq!(stats.total_locked, 700000);
        assert_eq!(stats.contract_balance, 700000);
        assert_eq!(stats.spendable, 0);

        // Locking with the wrong denom or no funds fails.
        assert!(execute(
            deps.as_mut(),
            mock_env(),
            mock_info(&voter, &coins(100, "ujuno")),
            ExecuteMsg::Lock {}
        )
        .is_err());
        assert!(execute(
            deps.as_mut(),
            mock_env(),
            mock_info(&voter, &[]),
            ExecuteMsg::Lock {}
        )
        .is_err());
    }

    #[test]
    fn unauthorized_update_params_fails() {
        let mut deps = mock_dependencies();
        let init = default_init(&deps.api);
        let _ = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), init);

        let update_msg = ExecuteMsg::UpdateParams {
            voting_period: Some(2000),
            quorum: None,
            threshold: None,
        };
        let res = execute(deps.as_mut(), mock_env(), mock_info("not_admin", &[]), update_msg);
        assert!(res.is_err());
    }
}
