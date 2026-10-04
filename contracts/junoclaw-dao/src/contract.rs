use cosmwasm_std::{
    coins, entry_point, to_json_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo,
    Order, Response, StdResult,
};

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, LockResponse, LockStatsResponse,
    ProposalResponse, ProposalTypeInput, ProposalTypeOutput, ProposalsResponse, QueryMsg,
    TallyResponse, VoteChoiceInput, VoteResponse,
};
use crate::state::{
    Proposal, ProposalStatus, ProposalType, VoteChoice, VoteRecord, ADMIN, COMMUNITY_POOL,
    LOCKED, NEXT_PROPOSAL_ID, PROPOSALS, QUORUM, THRESHOLD, TOTAL_LOCKED, TOTAL_SUPPLY,
    VOTES, VOTE_LOCK, VOTING_DENOM, VOTING_PERIOD,
};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let admin = deps.api.addr_validate(&msg.admin)?;
    let community_pool = deps.api.addr_validate(&msg.community_pool)?;

    if msg.voting_period == 0 {
        return Err(ContractError::InvalidParams {
            reason: "voting_period must be positive".to_string(),
        });
    }

    if msg.quorum == 0 || msg.quorum > 100 {
        return Err(ContractError::InvalidParams {
            reason: "quorum must be 1-100".to_string(),
        });
    }

    if msg.threshold == 0 || msg.threshold > 100 {
        return Err(ContractError::InvalidParams {
            reason: "threshold must be 1-100".to_string(),
        });
    }

    ADMIN.save(deps.storage, &admin)?;
    VOTING_DENOM.save(deps.storage, &msg.voting_denom)?;
    COMMUNITY_POOL.save(deps.storage, &community_pool)?;
    TOTAL_SUPPLY.save(deps.storage, &msg.total_supply)?;
    VOTING_PERIOD.save(deps.storage, &msg.voting_period)?;
    QUORUM.save(deps.storage, &msg.quorum)?;
    THRESHOLD.save(deps.storage, &msg.threshold)?;
    NEXT_PROPOSAL_ID.save(deps.storage, &1u64)?;
    TOTAL_LOCKED.save(deps.storage, &0u128)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", admin.to_string())
        .add_attribute("voting_period", msg.voting_period.to_string())
        .add_attribute("quorum", msg.quorum.to_string())
        .add_attribute("threshold", msg.threshold.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SubmitProposal {
            title,
            description,
            proposal_type,
        } => execute_submit(deps, env, info, title, description, proposal_type),
        ExecuteMsg::Lock {} => execute_lock(deps, env, info),
        ExecuteMsg::Unlock { amount } => execute_unlock(deps, env, info, amount),
        ExecuteMsg::Vote { proposal_id, vote } => {
            execute_vote(deps, env, info, proposal_id, vote)
        }
        ExecuteMsg::ExecuteProposal { proposal_id } => {
            execute_execute(deps, env, info, proposal_id)
        }
        ExecuteMsg::UpdateParams {
            voting_period,
            quorum,
            threshold,
        } => execute_update_params(deps, info, voting_period, quorum, threshold),
        ExecuteMsg::TransferAdmin { new_admin } => {
            execute_transfer_admin(deps, info, new_admin)
        }
    }
}

fn execute_submit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    title: String,
    description: String,
    proposal_type: ProposalTypeInput,
) -> Result<Response, ContractError> {
    if title.is_empty() || description.is_empty() {
        return Err(ContractError::InvalidParams {
            reason: "title and description must not be empty".to_string(),
        });
    }

    let voting_period = VOTING_PERIOD.load(deps.storage)?;
    let mut next_id = NEXT_PROPOSAL_ID.load(deps.storage)?;

    let ptype = match proposal_type {
        ProposalTypeInput::Text => ProposalType::Text,
        ProposalTypeInput::Spend {
            recipient,
            amount,
            denom,
        } => {
            if amount == 0 {
                return Err(ContractError::InvalidParams {
                    reason: "spend amount must be positive".to_string(),
                });
            }
            deps.api.addr_validate(&recipient)?;
            ProposalType::Spend {
                recipient,
                amount,
                denom,
            }
        }
        ProposalTypeInput::ParameterChange { key, value } => ProposalType::ParameterChange {
            key,
            value,
        },
    };

    let proposal = Proposal {
        id: next_id,
        title: title.clone(),
        description: description.clone(),
        proposer: info.sender.to_string(),
        proposal_type: ptype,
        status: ProposalStatus::Active,
        yes_votes: 0,
        no_votes: 0,
        abstain_votes: 0,
        created_height: env.block.height,
        voting_end_height: env.block.height + voting_period,
        executed: false,
    };

    PROPOSALS.save(deps.storage, next_id, &proposal)?;
    next_id += 1;
    NEXT_PROPOSAL_ID.save(deps.storage, &next_id)?;

    Ok(Response::new()
        .add_attribute("action", "submit_proposal")
        .add_attribute("proposal_id", proposal.id.to_string())
        .add_attribute("title", title)
        .add_attribute("voting_end_height", proposal.voting_end_height.to_string()))
}

fn execute_vote(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
    vote: VoteChoiceInput,
) -> Result<Response, ContractError> {
    let mut proposal = PROPOSALS
        .load(deps.storage, proposal_id)
        .map_err(|_| ContractError::ProposalNotFound {})?;

    if !matches!(proposal.status, ProposalStatus::Active) {
        return Err(ContractError::ProposalNotActive {});
    }

    if env.block.height >= proposal.voting_end_height {
        return Err(ContractError::VotingEnded {});
    }

    let voter = info.sender.clone();
    let voter_str = voter.as_str();

    if VOTES.has(deps.storage, (proposal_id, voter_str)) {
        return Err(ContractError::AlreadyVoted {});
    }

    // Voting weight = tokens locked in THIS contract. Using the live bank
    // balance instead would let an attacker vote, move tokens to a fresh wallet,
    // and vote again with the same funds.
    let weight = LOCKED
        .may_load(deps.storage, voter_str)?
        .unwrap_or_default();

    if weight == 0 {
        return Err(ContractError::NoLockedTokens {});
    }

    let vote_choice = match vote {
        VoteChoiceInput::Yes => VoteChoice::Yes,
        VoteChoiceInput::No => VoteChoice::No,
        VoteChoiceInput::Abstain => VoteChoice::Abstain,
    };

    match &vote_choice {
        VoteChoice::Yes => proposal.yes_votes += weight,
        VoteChoice::No => proposal.no_votes += weight,
        VoteChoice::Abstain => proposal.abstain_votes += weight,
    }

    let vote_record = VoteRecord {
        voter: voter.to_string(),
        vote: vote_choice,
        weight,
        height: env.block.height,
    };

    VOTES.save(deps.storage, (proposal_id, voter_str), &vote_record)?;

    // Hold the voter's lock until this proposal's voting period ends.
    let lock = VOTE_LOCK.may_load(deps.storage, voter_str)?.unwrap_or_default();
    if proposal.voting_end_height > lock {
        VOTE_LOCK.save(deps.storage, voter_str, &proposal.voting_end_height)?;
    }

    PROPOSALS.save(deps.storage, proposal_id, &proposal)?;

    Ok(Response::new()
        .add_attribute("action", "vote")
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("voter", voter.to_string())
        .add_attribute("weight", weight.to_string()))
}

fn execute_execute(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    let mut proposal = PROPOSALS
        .load(deps.storage, proposal_id)
        .map_err(|_| ContractError::ProposalNotFound {})?;

    if proposal.executed {
        return Err(ContractError::AlreadyExecuted {});
    }

    if env.block.height < proposal.voting_end_height {
        return Err(ContractError::ProposalNotActive {});
    }

    // Finalize status if not done
    if matches!(proposal.status, ProposalStatus::Active) {
        finalize_proposal(deps.as_ref(), &mut proposal)?;
    }

    if !matches!(proposal.status, ProposalStatus::Passed) {
        return Err(ContractError::ProposalNotPassed {});
    }

    proposal.executed = true;
    PROPOSALS.save(deps.storage, proposal_id, &proposal)?;

    let mut response = Response::new()
        .add_attribute("action", "execute_proposal")
        .add_attribute("proposal_id", proposal_id.to_string());

    // Execute the proposal action
    match &proposal.proposal_type {
        ProposalType::Text => {
            // No action needed for text proposals
        }
        ProposalType::Spend {
            recipient,
            amount,
            denom,
        } => {
            // Spendable = contract balance minus locked voting tokens (locked
            // tokens belong to voters, not to the treasury).
            let voting_denom = VOTING_DENOM.load(deps.storage)?;
            let balance = deps
                .querier
                .query_balance(env.contract.address.clone(), denom)?
                .amount
                .u128();
            let reserved = if *denom == voting_denom {
                TOTAL_LOCKED.load(deps.storage)?
            } else {
                0
            };
            let spendable = balance.saturating_sub(reserved);

            if spendable < *amount {
                return Err(ContractError::InsufficientBalance {});
            }

            let send_msg = BankMsg::Send {
                to_address: recipient.clone(),
                amount: coins(*amount, denom.clone()),
            };
            response = response.add_message(send_msg);
        }
        ProposalType::ParameterChange { key, value } => {
            response = response
                .add_attribute("param_key", key)
                .add_attribute("param_value", value);
        }
    }

    Ok(response)
}

fn finalize_proposal(deps: Deps, proposal: &mut Proposal) -> Result<(), ContractError> {
    let total_supply = TOTAL_SUPPLY.load(deps.storage)?;
    let quorum = QUORUM.load(deps.storage)?;
    let threshold = THRESHOLD.load(deps.storage)?;

    let total_votes = proposal.yes_votes + proposal.no_votes + proposal.abstain_votes;
    let quorum_needed = total_supply * quorum as u128 / 100;

    if total_votes < quorum_needed {
        proposal.status = ProposalStatus::Failed;
        return Ok(());
    }

    let cast_votes = proposal.yes_votes + proposal.no_votes;
    if cast_votes == 0 {
        proposal.status = ProposalStatus::Failed;
        return Ok(());
    }

    let threshold_needed = cast_votes * threshold as u128 / 100;

    if proposal.yes_votes >= threshold_needed {
        proposal.status = ProposalStatus::Passed;
    } else {
        proposal.status = ProposalStatus::Rejected;
    }

    Ok(())
}

/// Lock voting tokens. Requires exactly one coin of the voting denom attached.
/// (Contract-internal lock for vote weight — the chain has no consensus staking.)
fn execute_lock(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let voting_denom = VOTING_DENOM.load(deps.storage)?;
    if info.funds.len() != 1 || info.funds[0].denom != voting_denom || info.funds[0].amount.is_zero()
    {
        return Err(ContractError::InvalidFunds {});
    }
    let amount = info.funds[0].amount.u128();
    let locker = info.sender.as_str();

    let locked = LOCKED.may_load(deps.storage, locker)?.unwrap_or_default();
    LOCKED.save(deps.storage, locker, &(locked + amount))?;
    TOTAL_LOCKED.update(deps.storage, |t: u128| -> StdResult<u128> {
        Ok(t + amount)
    })?;

    Ok(Response::new()
        .add_attribute("action", "lock")
        .add_attribute("locker", info.sender)
        .add_attribute("amount", amount.to_string())
        .add_attribute("total_locked", (locked + amount).to_string())
        .add_attribute("height", env.block.height.to_string()))
}

/// Unlock (withdraw) voting tokens. Rejects while the sender has a vote on a
/// proposal whose voting period has not yet ended (that lock is what backs the vote).
fn execute_unlock(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: u128,
) -> Result<Response, ContractError> {
    if amount == 0 {
        return Err(ContractError::InvalidParams {
            reason: "unlock amount must be positive".to_string(),
        });
    }

    let locker = info.sender.as_str();

    let lock_until = VOTE_LOCK.may_load(deps.storage, locker)?.unwrap_or_default();
    if env.block.height < lock_until {
        return Err(ContractError::LockedUntil { until: lock_until });
    }

    let locked = LOCKED.may_load(deps.storage, locker)?.unwrap_or_default();
    if amount > locked {
        return Err(ContractError::InsufficientLocked {});
    }

    LOCKED.save(deps.storage, locker, &(locked - amount))?;
    TOTAL_LOCKED.update(deps.storage, |t: u128| -> StdResult<u128> {
        Ok(t - amount)
    })?;

    let voting_denom = VOTING_DENOM.load(deps.storage)?;
    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(amount, voting_denom),
        })
        .add_attribute("action", "unlock")
        .add_attribute("locker", info.sender)
        .add_attribute("amount", amount.to_string()))
}

fn execute_update_params(
    deps: DepsMut,
    info: MessageInfo,
    voting_period: Option<u64>,
    quorum: Option<u32>,
    threshold: Option<u32>,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    let mut response = Response::new().add_attribute("action", "update_params");

    if let Some(vp) = voting_period {
        if vp == 0 {
            return Err(ContractError::InvalidParams {
                reason: "voting_period must be positive".to_string(),
            });
        }
        VOTING_PERIOD.save(deps.storage, &vp)?;
        response = response.add_attribute("voting_period", vp.to_string());
    }

    if let Some(q) = quorum {
        if q == 0 || q > 100 {
            return Err(ContractError::InvalidParams {
                reason: "quorum must be 1-100".to_string(),
            });
        }
        QUORUM.save(deps.storage, &q)?;
        response = response.add_attribute("quorum", q.to_string());
    }

    if let Some(t) = threshold {
        if t == 0 || t > 100 {
            return Err(ContractError::InvalidParams {
                reason: "threshold must be 1-100".to_string(),
            });
        }
        THRESHOLD.save(deps.storage, &t)?;
        response = response.add_attribute("threshold", t.to_string());
    }

    Ok(response)
}

fn execute_transfer_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    let new_addr = deps.api.addr_validate(&new_admin)?;
    ADMIN.save(deps.storage, &new_addr)?;

    Ok(Response::new()
        .add_attribute("action", "transfer_admin")
        .add_attribute("new_admin", new_addr.to_string()))
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => {
            let admin = ADMIN.load(deps.storage)?;
            let voting_denom = VOTING_DENOM.load(deps.storage)?;
            let community_pool = COMMUNITY_POOL.load(deps.storage)?;
            let total_supply = TOTAL_SUPPLY.load(deps.storage)?;
            let voting_period = VOTING_PERIOD.load(deps.storage)?;
            let quorum = QUORUM.load(deps.storage)?;
            let threshold = THRESHOLD.load(deps.storage)?;
            let next_proposal_id = NEXT_PROPOSAL_ID.load(deps.storage)?;

            let resp = ConfigResponse {
                admin: admin.to_string(),
                voting_denom,
                community_pool: community_pool.to_string(),
                total_supply,
                voting_period,
                quorum,
                threshold,
                next_proposal_id,
            };
            to_json_binary(&resp)
        }
        QueryMsg::GetProposal { proposal_id } => {
            let proposal = PROPOSALS.load(deps.storage, proposal_id).map_err(|_| {
                cosmwasm_std::StdError::not_found("proposal not found")
            })?;

            let ptype = match proposal.proposal_type {
                ProposalType::Text => ProposalTypeOutput::Text,
                ProposalType::Spend {
                    recipient,
                    amount,
                    denom,
                } => ProposalTypeOutput::Spend {
                    recipient,
                    amount,
                    denom,
                },
                ProposalType::ParameterChange { key, value } => {
                    ProposalTypeOutput::ParameterChange { key, value }
                }
            };

            let status_str = match proposal.status {
                ProposalStatus::Active => "active",
                ProposalStatus::Passed => "passed",
                ProposalStatus::Rejected => "rejected",
                ProposalStatus::Failed => "failed",
            };

            let resp = ProposalResponse {
                id: proposal.id,
                title: proposal.title,
                description: proposal.description,
                proposer: proposal.proposer,
                proposal_type: ptype,
                status: status_str.to_string(),
                yes_votes: proposal.yes_votes,
                no_votes: proposal.no_votes,
                abstain_votes: proposal.abstain_votes,
                created_height: proposal.created_height,
                voting_end_height: proposal.voting_end_height,
                executed: proposal.executed,
            };
            to_json_binary(&resp)
        }
        QueryMsg::ListProposals { start_after, limit } => {
            let limit = limit.unwrap_or(20) as usize;
            let start = start_after.map(|s| cw_storage_plus::Bound::exclusive(s));

            let proposals: Vec<ProposalResponse> = PROPOSALS
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit)
                .filter_map(|item| {
                    let (_, proposal) = item.ok()?;
                    let ptype = match proposal.proposal_type {
                        ProposalType::Text => ProposalTypeOutput::Text,
                        ProposalType::Spend {
                            recipient,
                            amount,
                            denom,
                        } => ProposalTypeOutput::Spend {
                            recipient,
                            amount,
                            denom,
                        },
                        ProposalType::ParameterChange { key, value } => {
                            ProposalTypeOutput::ParameterChange { key, value }
                        }
                    };
                    let status_str = match proposal.status {
                        ProposalStatus::Active => "active",
                        ProposalStatus::Passed => "passed",
                        ProposalStatus::Rejected => "rejected",
                        ProposalStatus::Failed => "failed",
                    };
                    Some(ProposalResponse {
                        id: proposal.id,
                        title: proposal.title,
                        description: proposal.description,
                        proposer: proposal.proposer,
                        proposal_type: ptype,
                        status: status_str.to_string(),
                        yes_votes: proposal.yes_votes,
                        no_votes: proposal.no_votes,
                        abstain_votes: proposal.abstain_votes,
                        created_height: proposal.created_height,
                        voting_end_height: proposal.voting_end_height,
                        executed: proposal.executed,
                    })
                })
                .collect();

            let resp = ProposalsResponse { proposals };
            to_json_binary(&resp)
        }
        QueryMsg::GetVote { proposal_id, voter } => {
            let vote = VOTES
                .load(deps.storage, (proposal_id, &voter))
                .map_err(|_| cosmwasm_std::StdError::not_found("vote not found"))?;

            let vote_str = match vote.vote {
                VoteChoice::Yes => "yes",
                VoteChoice::No => "no",
                VoteChoice::Abstain => "abstain",
            };

            let resp = VoteResponse {
                voter: vote.voter,
                vote: vote_str.to_string(),
                weight: vote.weight,
                height: vote.height,
            };
            to_json_binary(&resp)
        }
        QueryMsg::GetLock { address } => {
            let addr = deps.api.addr_validate(&address)?;
            let locked = LOCKED.may_load(deps.storage, addr.as_str())?.unwrap_or_default();
            let locked_until = VOTE_LOCK
                .may_load(deps.storage, addr.as_str())?
                .unwrap_or_default();
            to_json_binary(&LockResponse {
                address: addr.to_string(),
                locked,
                locked_until,
            })
        }
        QueryMsg::GetLockStats {} => {
            let voting_denom = VOTING_DENOM.load(deps.storage)?;
            let total_locked = TOTAL_LOCKED.load(deps.storage)?;
            let balance = deps
                .querier
                .query_balance(env.contract.address, &voting_denom)?
                .amount
                .u128();
            to_json_binary(&LockStatsResponse {
                total_locked,
                contract_balance: balance,
                spendable: balance.saturating_sub(total_locked),
            })
        }
        QueryMsg::GetTally { proposal_id } => {
            let proposal = PROPOSALS
                .load(deps.storage, proposal_id)
                .map_err(|_| cosmwasm_std::StdError::not_found("proposal not found"))?;

            let mut status = proposal.status.clone();
            if matches!(status, ProposalStatus::Active)
                && env.block.height >= proposal.voting_end_height
            {
                let mut p = proposal.clone();
                finalize_proposal(deps, &mut p).ok();
                status = p.status;
            }

            let status_str = match status {
                ProposalStatus::Active => "active",
                ProposalStatus::Passed => "passed",
                ProposalStatus::Rejected => "rejected",
                ProposalStatus::Failed => "failed",
            };

            let total_votes = proposal.yes_votes + proposal.no_votes + proposal.abstain_votes;
            let resp = TallyResponse {
                proposal_id,
                yes_votes: proposal.yes_votes,
                no_votes: proposal.no_votes,
                abstain_votes: proposal.abstain_votes,
                total_votes,
                status: status_str.to_string(),
            };
            to_json_binary(&resp)
        }
    }
}
