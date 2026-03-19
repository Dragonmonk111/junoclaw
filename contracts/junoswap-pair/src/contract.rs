use cosmwasm_std::{
    coins, entry_point, to_json_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, Event,
    MessageInfo, Response, StdResult, Uint128, Uint256,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::*;
use crate::state::*;
use junoclaw_common::AssetInfo;

const CONTRACT_NAME: &str = "crates.io:junoswap-pair";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let junoclaw = msg
        .junoclaw_contract
        .map(|a| deps.api.addr_validate(&a))
        .transpose()?;

    let config = PairConfig {
        factory: deps.api.addr_validate(&msg.factory)?,
        token_a: msg.token_a,
        token_b: msg.token_b,
        fee_bps: msg.fee_bps,
        junoclaw_contract: junoclaw,
    };
    PAIR_CONFIG.save(deps.storage, &config)?;
    POOL_STATE.save(deps.storage, &PoolState::default())?;

    Ok(Response::new().add_attribute("action", "instantiate_pair"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ProvideLiquidity {} => execute_provide_liquidity(deps, env, info),
        ExecuteMsg::WithdrawLiquidity { lp_amount } => {
            execute_withdraw_liquidity(deps, env, info, lp_amount)
        }
        ExecuteMsg::Swap {
            offer_asset,
            min_return,
        } => execute_swap(deps, env, info, offer_asset, min_return),
    }
}

/// Provide liquidity — send both native tokens to the pair contract.
/// LP shares are minted proportional to the smaller ratio of deposits.
fn execute_provide_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = PAIR_CONFIG.load(deps.storage)?;
    let mut pool = POOL_STATE.load(deps.storage)?;

    let (amount_a, amount_b) = extract_native_amounts(&info.funds, &config)?;

    if amount_a.is_zero() || amount_b.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // Calculate LP shares
    let lp_shares = if pool.total_lp_shares.is_zero() {
        // Initial liquidity: geometric mean
        let product = Uint256::from(amount_a) * Uint256::from(amount_b);
        isqrt_u256(product)
    } else {
        // Proportional: min(a_deposit/a_reserve, b_deposit/b_reserve) * total_shares
        let share_a = amount_a.multiply_ratio(pool.total_lp_shares, pool.reserve_a);
        let share_b = amount_b.multiply_ratio(pool.total_lp_shares, pool.reserve_b);
        share_a.min(share_b)
    };

    if lp_shares.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    pool.reserve_a += amount_a;
    pool.reserve_b += amount_b;
    pool.total_lp_shares += lp_shares;
    POOL_STATE.save(deps.storage, &pool)?;

    // Track user LP balance
    let prev = LP_SHARES
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();
    LP_SHARES.save(deps.storage, &info.sender, &(prev + lp_shares))?;

    let event = Event::new("wasm-provide_liquidity")
        .add_attribute("provider", info.sender.to_string())
        .add_attribute("amount_a", amount_a.to_string())
        .add_attribute("amount_b", amount_b.to_string())
        .add_attribute("lp_shares", lp_shares.to_string())
        .add_attribute("reserve_a", pool.reserve_a.to_string())
        .add_attribute("reserve_b", pool.reserve_b.to_string())
        .add_attribute("block_height", env.block.height.to_string())
        .add_attribute("pair", env.contract.address.to_string());

    Ok(Response::new()
        .add_event(event)
        .add_attribute("action", "provide_liquidity"))
}

/// Withdraw liquidity — burn LP shares, receive proportional reserves.
fn execute_withdraw_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_amount: Uint128,
) -> Result<Response, ContractError> {
    let config = PAIR_CONFIG.load(deps.storage)?;
    let mut pool = POOL_STATE.load(deps.storage)?;

    if lp_amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    let user_lp = LP_SHARES
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();
    if user_lp < lp_amount {
        return Err(ContractError::InsufficientFunds {});
    }

    let withdraw_a = lp_amount.multiply_ratio(pool.reserve_a, pool.total_lp_shares);
    let withdraw_b = lp_amount.multiply_ratio(pool.reserve_b, pool.total_lp_shares);

    pool.reserve_a -= withdraw_a;
    pool.reserve_b -= withdraw_b;
    pool.total_lp_shares -= lp_amount;
    POOL_STATE.save(deps.storage, &pool)?;

    LP_SHARES.save(deps.storage, &info.sender, &(user_lp - lp_amount))?;

    let mut msgs = vec![];
    if !withdraw_a.is_zero() {
        if let AssetInfo::Native(denom) = &config.token_a {
            msgs.push(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(withdraw_a.u128(), denom),
            });
        }
    }
    if !withdraw_b.is_zero() {
        if let AssetInfo::Native(denom) = &config.token_b {
            msgs.push(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(withdraw_b.u128(), denom),
            });
        }
    }

    let event = Event::new("wasm-withdraw_liquidity")
        .add_attribute("provider", info.sender.to_string())
        .add_attribute("lp_burned", lp_amount.to_string())
        .add_attribute("withdraw_a", withdraw_a.to_string())
        .add_attribute("withdraw_b", withdraw_b.to_string())
        .add_attribute("pair", env.contract.address.to_string());

    Ok(Response::new()
        .add_messages(msgs)
        .add_event(event)
        .add_attribute("action", "withdraw_liquidity"))
}

/// XYK constant-product swap with fee.
/// WAVS event emitted on every swap for TEE verification.
fn execute_swap(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    offer_asset: AssetInfo,
    min_return: Option<Uint128>,
) -> Result<Response, ContractError> {
    let config = PAIR_CONFIG.load(deps.storage)?;
    let mut pool = POOL_STATE.load(deps.storage)?;

    if pool.reserve_a.is_zero() || pool.reserve_b.is_zero() {
        return Err(ContractError::EmptyPool {});
    }

    // Determine direction
    let (offer_reserve, return_reserve, _offer_denom, return_denom, is_a_to_b) =
        if offer_asset.denom_key() == config.token_a.denom_key() {
            (
                pool.reserve_a,
                pool.reserve_b,
                &config.token_a,
                &config.token_b,
                true,
            )
        } else if offer_asset.denom_key() == config.token_b.denom_key() {
            (
                pool.reserve_b,
                pool.reserve_a,
                &config.token_b,
                &config.token_a,
                false,
            )
        } else {
            return Err(ContractError::InvalidAsset {
                expected: format!(
                    "{} or {}",
                    config.token_a.denom_key(),
                    config.token_b.denom_key()
                ),
                got: offer_asset.denom_key(),
            });
        };

    let offer_amount = find_native_amount(&info.funds, &offer_asset)?;
    if offer_amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // XYK: return_amount = (offer_amount * return_reserve) / (offer_reserve + offer_amount)
    let fee_amount = offer_amount.multiply_ratio(config.fee_bps as u128, 10000u128);
    let offer_after_fee = offer_amount - fee_amount;

    let return_amount =
        offer_after_fee.multiply_ratio(return_reserve, offer_reserve + offer_after_fee);

    let spread_amount = offer_after_fee.multiply_ratio(return_reserve, offer_reserve) - return_amount;

    if return_amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // Slippage check
    if let Some(min) = min_return {
        if return_amount < min {
            return Err(ContractError::SlippageExceeded {
                return_amount: return_amount.to_string(),
                min_return: min.to_string(),
            });
        }
    }

    // Update reserves
    if is_a_to_b {
        pool.reserve_a += offer_amount;
        pool.reserve_b -= return_amount;
        pool.total_volume_a += offer_amount;
    } else {
        pool.reserve_b += offer_amount;
        pool.reserve_a -= return_amount;
        pool.total_volume_b += offer_amount;
    }
    pool.total_swaps += 1;
    pool.last_swap_block = env.block.height;
    POOL_STATE.save(deps.storage, &pool)?;

    // Send return tokens
    let send_msg = match return_denom {
        AssetInfo::Native(denom) => BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(return_amount.u128(), denom),
        },
        AssetInfo::Cw20(_) => {
            return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
                "CW20 not yet supported",
            )))
        }
    };

    // WAVS verification event — the DEX verifier component watches for these
    let event = Event::new("wasm-swap")
        .add_attribute("pair", env.contract.address.to_string())
        .add_attribute("sender", info.sender.to_string())
        .add_attribute("offer_asset", offer_asset.denom_key())
        .add_attribute("offer_amount", offer_amount.to_string())
        .add_attribute("return_asset", return_denom.denom_key())
        .add_attribute("return_amount", return_amount.to_string())
        .add_attribute("spread_amount", spread_amount.to_string())
        .add_attribute("fee_amount", fee_amount.to_string())
        .add_attribute("reserve_a", pool.reserve_a.to_string())
        .add_attribute("reserve_b", pool.reserve_b.to_string())
        .add_attribute("block_height", env.block.height.to_string())
        .add_attribute("timestamp", env.block.time.seconds().to_string());

    Ok(Response::new()
        .add_message(send_msg)
        .add_event(event)
        .add_attribute("action", "swap")
        .add_attribute("return_amount", return_amount.to_string()))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::PairInfo {} => {
            let config = PAIR_CONFIG.load(deps.storage)?;
            to_json_binary(&PairInfoResponse {
                factory: config.factory,
                token_a: config.token_a,
                token_b: config.token_b,
                fee_bps: config.fee_bps,
                junoclaw_contract: config.junoclaw_contract,
            })
        }
        QueryMsg::Pool {} => {
            let pool = POOL_STATE.load(deps.storage)?;
            let (price_a, price_b) = if pool.reserve_a.is_zero() || pool.reserve_b.is_zero() {
                ("0".to_string(), "0".to_string())
            } else {
                let a = format!(
                    "{:.6}",
                    pool.reserve_b.u128() as f64 / pool.reserve_a.u128() as f64
                );
                let b = format!(
                    "{:.6}",
                    pool.reserve_a.u128() as f64 / pool.reserve_b.u128() as f64
                );
                (a, b)
            };
            to_json_binary(&PoolStateResponse {
                reserve_a: pool.reserve_a,
                reserve_b: pool.reserve_b,
                total_lp_shares: pool.total_lp_shares,
                total_swaps: pool.total_swaps,
                total_volume_a: pool.total_volume_a,
                total_volume_b: pool.total_volume_b,
                price_a_per_b: price_a,
                price_b_per_a: price_b,
            })
        }
        QueryMsg::SimulateSwap {
            offer_asset,
            offer_amount,
        } => {
            let config = PAIR_CONFIG.load(deps.storage)?;
            let pool = POOL_STATE.load(deps.storage)?;

            let (offer_reserve, return_reserve) =
                if offer_asset.denom_key() == config.token_a.denom_key() {
                    (pool.reserve_a, pool.reserve_b)
                } else {
                    (pool.reserve_b, pool.reserve_a)
                };

            let fee_amount =
                offer_amount.multiply_ratio(config.fee_bps as u128, 10000u128);
            let offer_after_fee = offer_amount - fee_amount;
            let return_amount = offer_after_fee
                .multiply_ratio(return_reserve, offer_reserve + offer_after_fee);
            let spread_amount = offer_after_fee.multiply_ratio(return_reserve, offer_reserve)
                - return_amount;

            to_json_binary(&SimulateResponse {
                return_amount,
                spread_amount,
                fee_amount,
            })
        }
        QueryMsg::LpBalance { address } => {
            let addr = deps.api.addr_validate(&address)?;
            let balance = LP_SHARES
                .may_load(deps.storage, &addr)?
                .unwrap_or_default();
            to_json_binary(&balance)
        }
    }
}

// ── Helpers ──

fn extract_native_amounts(
    funds: &[Coin],
    config: &PairConfig,
) -> Result<(Uint128, Uint128), ContractError> {
    let denom_a = match &config.token_a {
        AssetInfo::Native(d) => d.clone(),
        _ => return Err(ContractError::Std(cosmwasm_std::StdError::generic_err("CW20 not yet supported"))),
    };
    let denom_b = match &config.token_b {
        AssetInfo::Native(d) => d.clone(),
        _ => return Err(ContractError::Std(cosmwasm_std::StdError::generic_err("CW20 not yet supported"))),
    };

    let amount_a = funds
        .iter()
        .find(|c| c.denom == denom_a)
        .map(|c| c.amount)
        .unwrap_or_default();
    let amount_b = funds
        .iter()
        .find(|c| c.denom == denom_b)
        .map(|c| c.amount)
        .unwrap_or_default();

    Ok((amount_a, amount_b))
}

fn find_native_amount(
    funds: &[Coin],
    asset: &AssetInfo,
) -> Result<Uint128, ContractError> {
    match asset {
        AssetInfo::Native(denom) => Ok(funds
            .iter()
            .find(|c| c.denom == *denom)
            .map(|c| c.amount)
            .unwrap_or_default()),
        AssetInfo::Cw20(_) => Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
            "CW20 not yet supported",
        ))),
    }
}

fn isqrt_u256(val: Uint256) -> Uint128 {
    if val.is_zero() {
        return Uint128::zero();
    }
    let mut x = val;
    let mut y = (x + Uint256::one()) >> 1;
    while y < x {
        x = y;
        y = (x + val / x) >> 1;
    }
    // x fits in u128 for reasonable pool sizes
    Uint128::try_from(x).unwrap_or(Uint128::MAX)
}
