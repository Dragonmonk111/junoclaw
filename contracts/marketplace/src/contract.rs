use cosmwasm_std::{
    entry_point, to_json_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo,
    Order, Response, StdResult, Uint128,
};
use cw2::{get_contract_version, set_contract_version};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{
    Config, Hire, HireStatus, Listing, MarketplaceStats, CONFIG, HIRES, HIRES_BY_AGENT,
    HIRES_BY_CLIENT, HIRES_BY_TASK, LISTINGS, LISTINGS_BY_AGENT, NEXT_HIRE_ID, NEXT_LISTING_ID,
    STATS,
};
use junoclaw_common::TaskRecord;

const CONTRACT_NAME: &str = "crates.io:junoclaw-marketplace";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_DENOM: &str = "ujuno";
const DEFAULT_CANCEL_WINDOW_SECS: u64 = 3600;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = msg
        .admin
        .map(|a| deps.api.addr_validate(&a))
        .transpose()?
        .unwrap_or(info.sender.clone());

    let config = Config {
        admin,
        denom: msg.denom.unwrap_or_else(|| DEFAULT_DENOM.to_string()),
        truth_market: deps.api.addr_validate(&msg.truth_market)?,
        task_ledger: deps.api.addr_validate(&msg.task_ledger)?,
        skill_registry: msg
            .skill_registry
            .map(|a| deps.api.addr_validate(&a))
            .transpose()?,
        cancel_window_secs: msg.cancel_window_secs.unwrap_or(DEFAULT_CANCEL_WINDOW_SECS),
    };
    CONFIG.save(deps.storage, &config)?;
    NEXT_LISTING_ID.save(deps.storage, &1u64)?;
    NEXT_HIRE_ID.save(deps.storage, &1u64)?;
    STATS.save(
        deps.storage,
        &MarketplaceStats {
            total_listings: 0,
            active_listings: 0,
            total_hires: 0,
            total_volume: Uint128::zero(),
            total_released: Uint128::zero(),
            total_refunded: Uint128::zero(),
            total_slashed: Uint128::zero(),
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", config.admin.to_string())
        .add_attribute("denom", config.denom))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ListService {
            skill_ref,
            price,
            description,
        } => execute_list_service(deps, env, info, skill_ref, price, description),
        ExecuteMsg::UpdateListing {
            listing_id,
            price,
            description,
            active,
        } => execute_update_listing(deps, info, listing_id, price, description, active),
        ExecuteMsg::DelistService { listing_id } => execute_delist(deps, info, listing_id),
        ExecuteMsg::HireService { listing_id, task_id } => {
            execute_hire_service(deps, env, info, listing_id, task_id)
        }
        ExecuteMsg::ReleaseOnVerdict {
            hire_id,
            batch_height,
        } => execute_release_on_verdict(deps, env, hire_id, batch_height),
        ExecuteMsg::CancelHire { hire_id } => execute_cancel_hire(deps, env, info, hire_id),
        ExecuteMsg::UpdateConfig {
            admin,
            truth_market,
            task_ledger,
            skill_registry,
            cancel_window_secs,
        } => execute_update_config(
            deps,
            info,
            admin,
            truth_market,
            task_ledger,
            skill_registry,
            cancel_window_secs,
        ),
    }
}

fn execute_list_service(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    skill_ref: String,
    price: Uint128,
    description: String,
) -> Result<Response, ContractError> {
    if price.is_zero() {
        return Err(ContractError::ZeroPrice {});
    }
    if skill_ref.trim().is_empty() {
        return Err(ContractError::EmptySkillRef {});
    }

    let config = CONFIG.load(deps.storage)?;
    if let Some(registry) = &config.skill_registry {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "snake_case")]
        enum SkillRegistryQuery {
            GetSkill { dapp_name: String },
        }
        #[derive(serde::Deserialize)]
        struct SkillEntryView {
            #[allow(dead_code)]
            dapp_name: String,
        }
        let _: SkillEntryView = deps
            .querier
            .query_wasm_smart(
                registry.to_string(),
                &SkillRegistryQuery::GetSkill { dapp_name: skill_ref.clone() },
            )
            .map_err(|_| ContractError::SkillNotRegistered { skill_ref: skill_ref.clone() })?;
    }

    let listing_id = NEXT_LISTING_ID.load(deps.storage)?;
    let listing = Listing {
        id: listing_id,
        agent: info.sender.clone(),
        skill_ref,
        price,
        description,
        active: true,
        created_at: env.block.time.seconds(),
    };
    LISTINGS.save(deps.storage, listing_id, &listing)?;
    NEXT_LISTING_ID.save(deps.storage, &(listing_id + 1))?;
    LISTINGS_BY_AGENT.update(deps.storage, &info.sender, |existing| -> StdResult<_> {
        let mut ids = existing.unwrap_or_default();
        ids.push(listing_id);
        Ok(ids)
    })?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_listings += 1;
        s.active_listings += 1;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "list_service")
        .add_attribute("listing_id", listing_id.to_string())
        .add_attribute("agent", info.sender.to_string())
        .add_attribute("price", price.to_string()))
}

fn execute_update_listing(
    deps: DepsMut,
    info: MessageInfo,
    listing_id: u64,
    price: Option<Uint128>,
    description: Option<String>,
    active: Option<bool>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut listing = LISTINGS
        .may_load(deps.storage, listing_id)?
        .ok_or(ContractError::ListingNotFound { listing_id })?;

    if info.sender != listing.agent && info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let was_active = listing.active;
    if let Some(p) = price {
        if p.is_zero() {
            return Err(ContractError::ZeroPrice {});
        }
        listing.price = p;
    }
    if let Some(d) = description {
        listing.description = d;
    }
    if let Some(a) = active {
        listing.active = a;
    }
    LISTINGS.save(deps.storage, listing_id, &listing)?;

    if was_active != listing.active {
        STATS.update(deps.storage, |mut s| -> StdResult<_> {
            if listing.active {
                s.active_listings += 1;
            } else {
                s.active_listings = s.active_listings.saturating_sub(1);
            }
            Ok(s)
        })?;
    }

    Ok(Response::new()
        .add_attribute("action", "update_listing")
        .add_attribute("listing_id", listing_id.to_string()))
}

fn execute_delist(
    deps: DepsMut,
    info: MessageInfo,
    listing_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut listing = LISTINGS
        .may_load(deps.storage, listing_id)?
        .ok_or(ContractError::ListingNotFound { listing_id })?;

    if info.sender != listing.agent && info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if listing.active {
        listing.active = false;
        LISTINGS.save(deps.storage, listing_id, &listing)?;
        STATS.update(deps.storage, |mut s| -> StdResult<_> {
            s.active_listings = s.active_listings.saturating_sub(1);
            Ok(s)
        })?;
    }

    Ok(Response::new()
        .add_attribute("action", "delist_service")
        .add_attribute("listing_id", listing_id.to_string()))
}

fn execute_hire_service(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    listing_id: u64,
    task_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let listing = LISTINGS
        .may_load(deps.storage, listing_id)?
        .ok_or(ContractError::ListingNotFound { listing_id })?;
    if !listing.active {
        return Err(ContractError::ListingNotActive { listing_id });
    }

    if HIRES_BY_TASK.has(deps.storage, task_id) {
        let existing_hire_id = HIRES_BY_TASK.load(deps.storage, task_id)?;
        return Err(ContractError::TaskAlreadyFunded {
            task_id,
            hire_id: existing_hire_id,
        });
    }

    let sent: Vec<String> = info
        .funds
        .iter()
        .map(|c| format!("{}{}", c.amount, c.denom))
        .collect();
    let matching = info
        .funds
        .iter()
        .find(|c| c.denom == config.denom)
        .cloned();
    if info.funds.len() != 1 || matching.as_ref().map(|c| c.amount) != Some(listing.price) {
        return Err(ContractError::WrongFunds {
            expected: listing.price,
            denom: config.denom.clone(),
            got: sent,
        });
    }

    let hire_id = NEXT_HIRE_ID.load(deps.storage)?;
    let hire = Hire {
        id: hire_id,
        listing_id,
        client: info.sender.clone(),
        agent: listing.agent.clone(),
        amount: listing.price,
        denom: config.denom.clone(),
        task_id,
        status: HireStatus::Escrowed,
        created_at: env.block.time.seconds(),
        resolved_at: None,
        batch_height: None,
    };
    HIRES.save(deps.storage, hire_id, &hire)?;
    NEXT_HIRE_ID.save(deps.storage, &(hire_id + 1))?;
    HIRES_BY_TASK.save(deps.storage, task_id, &hire_id)?;
    HIRES_BY_CLIENT.update(deps.storage, &info.sender, |existing| -> StdResult<_> {
        let mut ids = existing.unwrap_or_default();
        ids.push(hire_id);
        Ok(ids)
    })?;
    HIRES_BY_AGENT.update(deps.storage, &listing.agent, |existing| -> StdResult<_> {
        let mut ids = existing.unwrap_or_default();
        ids.push(hire_id);
        Ok(ids)
    })?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_hires += 1;
        s.total_volume += listing.price;
        Ok(s)
    })?;

    Ok(Response::new()
        .add_attribute("action", "hire_service")
        .add_attribute("hire_id", hire_id.to_string())
        .add_attribute("listing_id", listing_id.to_string())
        .add_attribute("task_id", task_id.to_string())
        .add_attribute("amount", listing.price.to_string()))
}

#[derive(serde::Deserialize)]
struct EpochView {
    consensus_verdict: String,
    finalized: bool,
}

fn execute_release_on_verdict(
    deps: DepsMut,
    env: Env,
    hire_id: u64,
    batch_height: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut hire = HIRES
        .may_load(deps.storage, hire_id)?
        .ok_or(ContractError::HireNotFound { hire_id })?;
    if !matches!(hire.status, HireStatus::Escrowed) {
        return Err(ContractError::NotEscrowed { hire_id });
    }

    #[derive(serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    enum TaskLedgerQuery {
        GetTask { task_id: u64 },
    }
    let task: TaskRecord = deps
        .querier
        .query_wasm_smart(
            config.task_ledger.to_string(),
            &TaskLedgerQuery::GetTask { task_id: hire.task_id },
        )
        .map_err(|_| ContractError::TaskNotCompleted { task_id: hire.task_id })?;

    let (recipient, new_status, stats_field): (&Addr, HireStatus, &str) = match task.status {
        junoclaw_common::TaskStatus::Failed | junoclaw_common::TaskStatus::Cancelled => {
            (&hire.client, HireStatus::Refunded, "refunded")
        }
        junoclaw_common::TaskStatus::Pending | junoclaw_common::TaskStatus::Running => {
            return Err(ContractError::TaskNotCompleted { task_id: hire.task_id });
        }
        junoclaw_common::TaskStatus::Completed => {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "snake_case")]
            enum TruthMarketQuery {
                GetEpoch { batch_height: u64 },
            }
            let epoch: EpochView = deps
                .querier
                .query_wasm_smart(
                    config.truth_market.to_string(),
                    &TruthMarketQuery::GetEpoch { batch_height },
                )
                .map_err(|_| ContractError::EpochNotFinalized { batch_height })?;
            if !epoch.finalized {
                return Err(ContractError::EpochNotFinalized { batch_height });
            }
            match epoch.consensus_verdict.as_str() {
                "green" => (&hire.agent, HireStatus::Released, "released"),
                "red" => (&hire.client, HireStatus::Slashed, "slashed"),
                other => {
                    return Err(ContractError::UnknownVerdict {
                        verdict: other.to_string(),
                    })
                }
            }
        }
    };

    let recipient = recipient.clone();
    hire.status = new_status;
    hire.resolved_at = Some(env.block.time.seconds());
    hire.batch_height = Some(batch_height);
    let amount = hire.amount;
    let denom = hire.denom.clone();
    HIRES.save(deps.storage, hire_id, &hire)?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        match stats_field {
            "released" => s.total_released += amount,
            "slashed" => s.total_slashed += amount,
            _ => s.total_refunded += amount,
        }
        Ok(s)
    })?;

    let send = BankMsg::Send {
        to_address: recipient.to_string(),
        amount: vec![Coin { denom, amount }],
    };

    Ok(Response::new()
        .add_message(send)
        .add_attribute("action", "release_on_verdict")
        .add_attribute("hire_id", hire_id.to_string())
        .add_attribute("outcome", stats_field)
        .add_attribute("recipient", recipient.to_string())
        .add_attribute("amount", amount.to_string()))
}

fn execute_cancel_hire(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    hire_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut hire = HIRES
        .may_load(deps.storage, hire_id)?
        .ok_or(ContractError::HireNotFound { hire_id })?;
    if !matches!(hire.status, HireStatus::Escrowed) {
        return Err(ContractError::NotEscrowed { hire_id });
    }
    if info.sender != hire.client && info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let elapsed = env.block.time.seconds().saturating_sub(hire.created_at);
    if elapsed < config.cancel_window_secs {
        return Err(ContractError::CancelWindowNotElapsed {
            elapsed,
            required: config.cancel_window_secs,
        });
    }

    hire.status = HireStatus::Cancelled;
    hire.resolved_at = Some(env.block.time.seconds());
    let amount = hire.amount;
    let denom = hire.denom.clone();
    let client = hire.client.clone();
    HIRES.save(deps.storage, hire_id, &hire)?;

    STATS.update(deps.storage, |mut s| -> StdResult<_> {
        s.total_refunded += amount;
        Ok(s)
    })?;

    let send = BankMsg::Send {
        to_address: client.to_string(),
        amount: vec![Coin { denom, amount }],
    };

    Ok(Response::new()
        .add_message(send)
        .add_attribute("action", "cancel_hire")
        .add_attribute("hire_id", hire_id.to_string())
        .add_attribute("amount", amount.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
    truth_market: Option<String>,
    task_ledger: Option<String>,
    skill_registry: Option<String>,
    cancel_window_secs: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(a) = admin {
        config.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(a) = truth_market {
        config.truth_market = deps.api.addr_validate(&a)?;
    }
    if let Some(a) = task_ledger {
        config.task_ledger = deps.api.addr_validate(&a)?;
    }
    if let Some(a) = skill_registry {
        config.skill_registry = Some(deps.api.addr_validate(&a)?);
    }
    if let Some(w) = cancel_window_secs {
        config.cancel_window_secs = w;
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetListing { listing_id } => {
            to_json_binary(&LISTINGS.load(deps.storage, listing_id)?)
        }
        QueryMsg::ListListings { start_after, limit } => {
            let limit = limit.unwrap_or(20).min(50) as usize;
            let start = start_after.map(cw_storage_plus::Bound::exclusive);
            let listings: Vec<Listing> = LISTINGS
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit)
                .filter_map(|r| r.ok().map(|(_, l)| l))
                .collect();
            to_json_binary(&listings)
        }
        QueryMsg::ListListingsByAgent { agent, limit } => {
            let addr = deps.api.addr_validate(&agent)?;
            let limit = limit.unwrap_or(20).min(50) as usize;
            let ids = LISTINGS_BY_AGENT
                .may_load(deps.storage, &addr)?
                .unwrap_or_default();
            let listings: Vec<Listing> = ids
                .iter()
                .rev()
                .take(limit)
                .filter_map(|id| LISTINGS.may_load(deps.storage, *id).ok().flatten())
                .collect();
            to_json_binary(&listings)
        }
        QueryMsg::GetHire { hire_id } => to_json_binary(&HIRES.load(deps.storage, hire_id)?),
        QueryMsg::GetHireByTask { task_id } => {
            let hire = match HIRES_BY_TASK.may_load(deps.storage, task_id)? {
                Some(hire_id) => HIRES.may_load(deps.storage, hire_id)?,
                None => None,
            };
            to_json_binary(&hire)
        }
        QueryMsg::ListHiresByClient { client, limit } => {
            let addr = deps.api.addr_validate(&client)?;
            let limit = limit.unwrap_or(20).min(50) as usize;
            let ids = HIRES_BY_CLIENT
                .may_load(deps.storage, &addr)?
                .unwrap_or_default();
            let hires: Vec<Hire> = ids
                .iter()
                .rev()
                .take(limit)
                .filter_map(|id| HIRES.may_load(deps.storage, *id).ok().flatten())
                .collect();
            to_json_binary(&hires)
        }
        QueryMsg::ListHiresByAgent { agent, limit } => {
            let addr = deps.api.addr_validate(&agent)?;
            let limit = limit.unwrap_or(20).min(50) as usize;
            let ids = HIRES_BY_AGENT
                .may_load(deps.storage, &addr)?
                .unwrap_or_default();
            let hires: Vec<Hire> = ids
                .iter()
                .rev()
                .take(limit)
                .filter_map(|id| HIRES.may_load(deps.storage, *id).ok().flatten())
                .collect();
            to_json_binary(&hires)
        }
        QueryMsg::GetStats {} => to_json_binary(&STATS.load(deps.storage)?),
    }
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(ContractError::Unauthorized {});
    }
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}
