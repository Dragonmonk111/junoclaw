use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    ClaimStatusResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StatsResponse,
};
use crate::state::{Config, CLAIMED, CONFIG};

const CONTRACT_NAME: &str = "crates.io:junoclaw-faucet";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    if msg.drip_amount == 0 {
        return Err(ContractError::InvalidDripAmount {});
    }

    let config = Config {
        admin: info.sender.clone(),
        drip_amount: msg.drip_amount,
        denom: msg.denom.clone(),
        total_claims: 0,
        active: true,
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", info.sender)
        .add_attribute("drip_amount", msg.drip_amount.to_string())
        .add_attribute("denom", msg.denom))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Claim {} => execute_claim(deps, env, info),
        ExecuteMsg::Fund {} => execute_fund(deps, info),
        ExecuteMsg::Withdraw { amount } => execute_withdraw(deps, env, info, amount),
        ExecuteMsg::SetActive { active } => execute_set_active(deps, info, active),
        ExecuteMsg::UpdateDrip { drip_amount } => execute_update_drip(deps, info, drip_amount),
        ExecuteMsg::TransferAdmin { new_admin } => execute_transfer_admin(deps, info, new_admin),
    }
}

fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    if !config.active {
        return Err(ContractError::FaucetPaused {});
    }

    // Check if already claimed
    if CLAIMED.has(deps.storage, &info.sender) {
        return Err(ContractError::AlreadyClaimed {
            address: info.sender.to_string(),
        });
    }

    // Check faucet balance
    let balance = deps
        .querier
        .query_balance(env.contract.address.clone(), &config.denom)?;
    let balance_u128 = balance.amount.u128();

    if balance_u128 < config.drip_amount {
        return Err(ContractError::InsufficientFunds {
            balance: balance_u128,
            drip: config.drip_amount,
        });
    }

    // Record claim
    CLAIMED.save(deps.storage, &info.sender, &env.block.height)?;
    config.total_claims += 1;
    CONFIG.save(deps.storage, &config)?;

    // Send tokens
    let send_msg = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: config.denom.clone(),
            amount: Uint128::from(config.drip_amount),
        }],
    };

    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "claim")
        .add_attribute("recipient", info.sender)
        .add_attribute("amount", config.drip_amount.to_string())
        .add_attribute("total_claims", config.total_claims.to_string()))
}

fn execute_fund(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Anyone can fund the faucet — no admin check
    let total_funded: u128 = info
        .funds
        .iter()
        .filter(|c| c.denom == config.denom)
        .map(|c| c.amount.u128())
        .sum();

    Ok(Response::new()
        .add_attribute("action", "fund")
        .add_attribute("funder", info.sender)
        .add_attribute("amount", total_funded.to_string()))
}

fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<u128>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let balance = deps
        .querier
        .query_balance(env.contract.address, &config.denom)?;

    let withdraw_amount = amount.unwrap_or(balance.amount.u128());

    let send_msg = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: config.denom,
            amount: Uint128::from(withdraw_amount),
        }],
    };

    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "withdraw")
        .add_attribute("admin", info.sender)
        .add_attribute("amount", withdraw_amount.to_string()))
}

fn execute_set_active(
    deps: DepsMut,
    info: MessageInfo,
    active: bool,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    config.active = active;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "set_active")
        .add_attribute("active", active.to_string()))
}

fn execute_update_drip(
    deps: DepsMut,
    info: MessageInfo,
    drip_amount: u128,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    if drip_amount == 0 {
        return Err(ContractError::InvalidDripAmount {});
    }

    config.drip_amount = drip_amount;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_drip")
        .add_attribute("drip_amount", drip_amount.to_string()))
}

fn execute_transfer_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let validated = deps.api.addr_validate(&new_admin)?;
    config.admin = validated.clone();
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "transfer_admin")
        .add_attribute("new_admin", validated))
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&query_config(deps, env)?),
        QueryMsg::HasClaimed { address } => to_json_binary(&query_has_claimed(deps, address)?),
        QueryMsg::GetStats {} => to_json_binary(&query_stats(deps, env)?),
    }
}

fn query_config(deps: Deps, env: Env) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let balance = deps
        .querier
        .query_balance(env.contract.address, &config.denom)?;

    Ok(ConfigResponse {
        admin: config.admin,
        drip_amount: config.drip_amount,
        denom: config.denom,
        active: config.active,
        balance: balance.amount.u128(),
    })
}

fn query_has_claimed(deps: Deps, address: String) -> StdResult<ClaimStatusResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let claimed_at = CLAIMED.may_load(deps.storage, &addr)?;

    Ok(ClaimStatusResponse {
        address: addr,
        claimed: claimed_at.is_some(),
        claimed_at_block: claimed_at,
    })
}

fn query_stats(deps: Deps, env: Env) -> StdResult<StatsResponse> {
    let config = CONFIG.load(deps.storage)?;
    let balance = deps
        .querier
        .query_balance(env.contract.address, &config.denom)?;

    Ok(StatsResponse {
        total_claims: config.total_claims,
        drip_amount: config.drip_amount,
        denom: config.denom,
        balance: balance.amount.u128(),
    })
}
