use cosmwasm_std::{
    coins, entry_point, to_json_binary, Binary, BankMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult,
};
use sha2::{Digest, Sha256};

use crate::error::ContractError;
use crate::msg::{
    ClaimStatsResponse, ClaimStatusResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
};
use crate::state::{
    ADMIN, CLAIMED, CLAIM_END, CLAIM_START, COMMUNITY_POOL, DENOM, MERKLE_ROOT, TOTAL_AMOUNT,
    TOTAL_CLAIMED,
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

    if msg.merkle_root.is_empty() {
        return Err(ContractError::InvalidParams {
            reason: "merkle_root must not be empty".to_string(),
        });
    }

    if msg.total_amount == 0 {
        return Err(ContractError::InvalidParams {
            reason: "total_amount must be positive".to_string(),
        });
    }

    if msg.claim_end <= msg.claim_start {
        return Err(ContractError::InvalidParams {
            reason: "claim_end must be after claim_start".to_string(),
        });
    }

    ADMIN.save(deps.storage, &admin)?;
    DENOM.save(deps.storage, &msg.denom)?;
    COMMUNITY_POOL.save(deps.storage, &community_pool)?;
    MERKLE_ROOT.save(deps.storage, &msg.merkle_root)?;
    TOTAL_AMOUNT.save(deps.storage, &msg.total_amount)?;
    TOTAL_CLAIMED.save(deps.storage, &0u128)?;
    CLAIM_START.save(deps.storage, &msg.claim_start)?;
    CLAIM_END.save(deps.storage, &msg.claim_end)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", admin.to_string())
        .add_attribute("total_amount", msg.total_amount.to_string())
        .add_attribute("claim_start", msg.claim_start.to_string())
        .add_attribute("claim_end", msg.claim_end.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Claim {
            amount,
            merkle_proof,
        } => execute_claim(deps, env, info, amount, merkle_proof),
        ExecuteMsg::SweepUnclaimed {} => execute_sweep(deps, env, info),
        ExecuteMsg::UpdateMerkleRoot { merkle_root } => {
            execute_update_root(deps, info, merkle_root)
        }
        ExecuteMsg::TransferAdmin { new_admin } => execute_transfer_admin(deps, info, new_admin),
    }
}

fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: u128,
    merkle_proof: Vec<String>,
) -> Result<Response, ContractError> {
    let claim_start = CLAIM_START.load(deps.storage)?;
    let claim_end = CLAIM_END.load(deps.storage)?;

    if env.block.height < claim_start {
        return Err(ContractError::ClaimNotOpen {});
    }

    if env.block.height >= claim_end {
        return Err(ContractError::ClaimClosed {});
    }

    let claimer = info.sender.clone();
    let claimer_str = claimer.as_str();

    // Check if already claimed
    if CLAIMED.has(deps.storage, claimer_str) {
        return Err(ContractError::AlreadyClaimed {});
    }

    // Verify merkle proof
    let merkle_root = MERKLE_ROOT.load(deps.storage)?;
    let leaf = compute_leaf(claimer_str, amount);
    let computed_root = verify_merkle_proof(&leaf, &merkle_proof)?;

    if computed_root != merkle_root {
        return Err(ContractError::InvalidProof {});
    }

    // Check contract has enough balance
    let denom = DENOM.load(deps.storage)?;
    let balance = deps
        .querier
        .query_balance(env.contract.address.clone(), &denom)?
        .amount
        .u128();
    if balance < amount {
        return Err(ContractError::InsufficientBalance {});
    }

    // Mark as claimed
    CLAIMED.save(deps.storage, claimer_str, &amount)?;

    // Update total claimed
    let total_claimed = TOTAL_CLAIMED.load(deps.storage)?;
    TOTAL_CLAIMED.save(deps.storage, &(total_claimed + amount))?;

    // Send tokens
    let send_msg = BankMsg::Send {
        to_address: claimer.to_string(),
        amount: coins(amount, denom),
    };

    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "claim")
        .add_attribute("claimer", claimer.to_string())
        .add_attribute("amount", amount.to_string()))
}

fn execute_sweep(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    let claim_end = CLAIM_END.load(deps.storage)?;
    if env.block.height < claim_end {
        return Err(ContractError::SweepTooEarly {});
    }

    let denom = DENOM.load(deps.storage)?;
    let balance = deps
        .querier
        .query_balance(env.contract.address.clone(), &denom)?
        .amount
        .u128();

    if balance == 0 {
        return Err(ContractError::NothingToSweep {});
    }

    let community_pool = COMMUNITY_POOL.load(deps.storage)?;

    let send_msg = BankMsg::Send {
        to_address: community_pool.to_string(),
        amount: coins(balance, denom.clone()),
    };

    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "sweep_unclaimed")
        .add_attribute("amount", balance.to_string())
        .add_attribute("to", community_pool.to_string()))
}

fn execute_update_root(
    deps: DepsMut,
    info: MessageInfo,
    merkle_root: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized {});
    }

    if merkle_root.is_empty() {
        return Err(ContractError::InvalidParams {
            reason: "merkle_root must not be empty".to_string(),
        });
    }

    MERKLE_ROOT.save(deps.storage, &merkle_root)?;

    Ok(Response::new()
        .add_attribute("action", "update_merkle_root")
        .add_attribute("new_root", merkle_root))
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

/// Compute the leaf hash: SHA-256(address || amount_hex_32_bytes)
fn compute_leaf(address: &str, amount: u128) -> String {
    let mut hasher = Sha256::new();
    hasher.update(address.as_bytes());
    hasher.update(&amount.to_be_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}

/// Verify a merkle proof using SHA-256
/// Each proof element is hex-encoded 32-byte hash
/// Leaf index is always 0 for sorted airdrop tree (we use address-based ordering)
fn verify_merkle_proof(leaf_hash: &str, proof: &[String]) -> Result<String, ContractError> {
    let mut current = hex::decode(leaf_hash).map_err(|_| ContractError::InvalidProof {})?;

    if current.len() != 32 {
        return Err(ContractError::InvalidProof {});
    }

    for (_level, sibling_hex) in proof.iter().enumerate() {
        let sibling = hex::decode(sibling_hex).map_err(|_| ContractError::InvalidProof {})?;

        if sibling.len() != 32 {
            return Err(ContractError::InvalidProof {});
        }

        // Sort pair to determine order (deterministic, no index needed)
        let mut hasher = Sha256::new();
        if current < sibling {
            hasher.update(&current);
            hasher.update(&sibling);
        } else {
            hasher.update(&sibling);
            hasher.update(&current);
        }
        current = hasher.finalize().to_vec();
    }

    Ok(hex::encode(current))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => {
            let admin = ADMIN.load(deps.storage)?;
            let denom = DENOM.load(deps.storage)?;
            let merkle_root = MERKLE_ROOT.load(deps.storage)?;
            let community_pool = COMMUNITY_POOL.load(deps.storage)?;
            let total_amount = TOTAL_AMOUNT.load(deps.storage)?;
            let claim_start = CLAIM_START.load(deps.storage)?;
            let claim_end = CLAIM_END.load(deps.storage)?;

            let resp = ConfigResponse {
                admin: admin.to_string(),
                denom,
                merkle_root,
                community_pool: community_pool.to_string(),
                total_amount,
                claim_start,
                claim_end,
            };
            to_json_binary(&resp)
        }
        QueryMsg::HasClaimed { address } => {
            let claimed_amount = CLAIMED
                .load(deps.storage, &address)
                .unwrap_or(0);
            let resp = ClaimStatusResponse {
                address,
                has_claimed: claimed_amount > 0,
                claimed_amount,
            };
            to_json_binary(&resp)
        }
        QueryMsg::GetStats {} => {
            let total_amount = TOTAL_AMOUNT.load(deps.storage)?;
            let total_claimed = TOTAL_CLAIMED.load(deps.storage)?;
            let resp = ClaimStatsResponse {
                total_amount,
                total_claimed,
                total_unclaimed: total_amount.saturating_sub(total_claimed),
                num_claimers: CLAIMED
                    .keys(deps.storage, None, None, cosmwasm_std::Order::Ascending)
                    .count() as u64,
            };
            to_json_binary(&resp)
        }
    }
}
