use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, QueryRequest,
    Response, StdResult, WasmQuery,
};
use cw2::set_contract_version;
use cw_storage_plus::Bound;

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, FractionalOwnersResponse, InstantiateMsg, MigrateMsg,
    MachinesResponse, MoultbookCreditScoreInner, MoultbookCreditScoreQuery,
    MoultbookCreditScoreResponse, OwnerFractionResponse, QueryMsg, WorkIntegrityScoreResponse,
};
use crate::state::{
    BY_OWNER, CONFIG, FRACTIONS, MACHINES, NEXT_TOKEN_ID, FractionalOwner,
};

const CONTRACT_NAME: &str = "crates.io:machine-rwa";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const TOTAL_BASIS_POINTS: u32 = 10_000;
const DEFAULT_LIMIT: u32 = 30;
const MAX_LIMIT: u32 = 100;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = deps.api.addr_validate(&msg.admin)?;
    let moultbook_contract = msg
        .moultbook_contract
        .as_deref()
        .map(|s| deps.api.addr_validate(s))
        .transpose()?;

    CONFIG.save(
        deps.storage,
        &crate::state::Config {
            admin,
            moultbook_contract,
        },
    )?;
    NEXT_TOKEN_ID.save(deps.storage, &0)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", info.sender))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Mint {
            model,
            serial_number,
            sensor_suite,
            ipfs_metadata,
            moultbook_author,
        } => execute_mint(deps, env, info, model, serial_number, sensor_suite, ipfs_metadata, moultbook_author),
        ExecuteMsg::Transfer { token_id, to } => execute_transfer(deps, info, token_id, to),
        ExecuteMsg::Fractionalize { token_id, recipients } => {
            execute_fractionalize(deps, info, token_id, recipients)
        }
        ExecuteMsg::TransferFraction {
            token_id,
            to,
            basis_points,
        } => execute_transfer_fraction(deps, info, token_id, to, basis_points),
        ExecuteMsg::Burn { token_id } => execute_burn(deps, info, token_id),
        ExecuteMsg::UpdateConfig {
            admin,
            moultbook_contract,
        } => execute_update_config(deps, info, admin, moultbook_contract),
    }
}

fn execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    model: String,
    serial_number: String,
    sensor_suite: String,
    ipfs_metadata: String,
    moultbook_author: String,
) -> Result<Response, ContractError> {
    if model.trim().is_empty() {
        return Err(ContractError::EmptyModel {});
    }
    if serial_number.trim().is_empty() {
        return Err(ContractError::EmptySerial {});
    }

    let id = NEXT_TOKEN_ID.load(deps.storage)?;
    let token_id = format!("machine-{}", id);
    NEXT_TOKEN_ID.save(deps.storage, &(id + 1))?;

    let machine = crate::state::Machine {
        token_id: token_id.clone(),
        minter: info.sender.clone(),
        model: model.clone(),
        serial_number: serial_number.clone(),
        sensor_suite,
        ipfs_metadata,
        moultbook_author: moultbook_author.clone(),
        minted_at: env.block.height,
        burned: false,
    };

    MACHINES.save(deps.storage, &token_id, &machine)?;
    FRACTIONS.save(deps.storage, (&token_id, &info.sender), &TOTAL_BASIS_POINTS)?;
    BY_OWNER.save(deps.storage, (&info.sender, token_id.as_str()), &())?;

    Ok(Response::new()
        .add_attribute("action", "mint")
        .add_attribute("token_id", token_id)
        .add_attribute("minter", info.sender)
        .add_attribute("model", model)
        .add_attribute("serial", serial_number))
}

fn get_owned_bp(deps: &Deps, token_id: &str, addr: &cosmwasm_std::Addr) -> u32 {
    FRACTIONS
        .may_load(deps.storage, (token_id, addr))
        .unwrap_or(None)
        .unwrap_or(0)
}

fn execute_transfer(
    deps: DepsMut,
    info: MessageInfo,
    token_id: String,
    to: String,
) -> Result<Response, ContractError> {
    let machine = MACHINES
        .may_load(deps.storage, &token_id)?
        .ok_or(ContractError::MachineNotFound { token_id: token_id.clone() })?;
    if machine.burned {
        return Err(ContractError::AlreadyBurned { token_id });
    }

    let to_addr = deps.api.addr_validate(&to)?;
    let owned_bp = get_owned_bp(&deps.as_ref(), &token_id, &info.sender);

    if owned_bp != TOTAL_BASIS_POINTS {
        return Err(ContractError::NotFullOwner {
            token_id,
            owned: owned_bp,
        });
    }

    // Remove from current owner
    FRACTIONS.remove(deps.storage, (&token_id, &info.sender));
    BY_OWNER.remove(deps.storage, (&info.sender, token_id.as_str()));

    // Give to new owner
    FRACTIONS.save(deps.storage, (&token_id, &to_addr), &TOTAL_BASIS_POINTS)?;
    BY_OWNER.save(deps.storage, (&to_addr, token_id.as_str()), &())?;

    Ok(Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("token_id", token_id)
        .add_attribute("from", info.sender)
        .add_attribute("to", to_addr))
}

fn execute_fractionalize(
    deps: DepsMut,
    info: MessageInfo,
    token_id: String,
    recipients: Vec<(String, u32)>,
) -> Result<Response, ContractError> {
    if recipients.is_empty() {
        return Err(ContractError::EmptyRecipients {});
    }

    let machine = MACHINES
        .may_load(deps.storage, &token_id)?
        .ok_or(ContractError::MachineNotFound { token_id: token_id.clone() })?;
    if machine.burned {
        return Err(ContractError::AlreadyBurned { token_id: token_id.clone() });
    }

    let owned_bp = get_owned_bp(&deps.as_ref(), &token_id, &info.sender);
    if owned_bp != TOTAL_BASIS_POINTS {
        return Err(ContractError::NotFullOwner {
            token_id: token_id.clone(),
            owned: owned_bp,
        });
    }

    let sum: u32 = recipients.iter().map(|(_, bp)| bp).sum();
    if sum != TOTAL_BASIS_POINTS {
        return Err(ContractError::InvalidBasisPointsSum { sum });
    }

    for (_, bp) in &recipients {
        if *bp > TOTAL_BASIS_POINTS {
            return Err(ContractError::BasisPointsTooHigh { bp: *bp });
        }
    }

    // Remove full ownership from caller
    FRACTIONS.remove(deps.storage, (&token_id, &info.sender));
    BY_OWNER.remove(deps.storage, (&info.sender, token_id.as_str()));

    // Distribute fractions
    let recipient_count = recipients.len();
    for (addr_str, bp) in recipients {
        let addr = deps.api.addr_validate(&addr_str)?;
        FRACTIONS.save(deps.storage, (&token_id, &addr), &bp)?;
        BY_OWNER.save(deps.storage, (&addr, token_id.as_str()), &())?;
    }

    Ok(Response::new()
        .add_attribute("action", "fractionalize")
        .add_attribute("token_id", token_id)
        .add_attribute("recipients", recipient_count.to_string()))
}

fn execute_transfer_fraction(
    deps: DepsMut,
    info: MessageInfo,
    token_id: String,
    to: String,
    basis_points: u32,
) -> Result<Response, ContractError> {
    if basis_points == 0 || basis_points > TOTAL_BASIS_POINTS {
        return Err(ContractError::BasisPointsTooHigh { bp: basis_points });
    }

    let machine = MACHINES
        .may_load(deps.storage, &token_id)?
        .ok_or(ContractError::MachineNotFound { token_id: token_id.clone() })?;
    if machine.burned {
        return Err(ContractError::AlreadyBurned { token_id: token_id.clone() });
    }

    let to_addr = deps.api.addr_validate(&to)?;
    let owned_bp = get_owned_bp(&deps.as_ref(), &token_id, &info.sender);

    if owned_bp == 0 {
        return Err(ContractError::NoFraction { token_id: token_id.clone() });
    }
    if basis_points > owned_bp {
        return Err(ContractError::InsufficientFraction {
            requested: basis_points,
            owned: owned_bp,
        });
    }

    // Deduct from sender
    let new_sender_bp = owned_bp - basis_points;
    if new_sender_bp == 0 {
        FRACTIONS.remove(deps.storage, (&token_id, &info.sender));
        BY_OWNER.remove(deps.storage, (&info.sender, token_id.as_str()));
    } else {
        FRACTIONS.save(deps.storage, (&token_id, &info.sender), &new_sender_bp)?;
    }

    // Add to recipient (stack if already owns some)
    let existing_bp = get_owned_bp(&deps.as_ref(), &token_id, &to_addr);
    let new_bp = existing_bp + basis_points;
    FRACTIONS.save(deps.storage, (&token_id, &to_addr), &new_bp)?;
    BY_OWNER.save(deps.storage, (&to_addr, token_id.as_str()), &())?;

    Ok(Response::new()
        .add_attribute("action", "transfer_fraction")
        .add_attribute("token_id", token_id)
        .add_attribute("from", info.sender)
        .add_attribute("to", to_addr)
        .add_attribute("basis_points", basis_points.to_string()))
}

fn execute_burn(
    deps: DepsMut,
    info: MessageInfo,
    token_id: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }

    let mut machine = MACHINES
        .may_load(deps.storage, &token_id)?
        .ok_or(ContractError::MachineNotFound { token_id: token_id.clone() })?;

    if machine.burned {
        return Err(ContractError::AlreadyBurned { token_id });
    }

    machine.burned = true;
    MACHINES.save(deps.storage, &token_id, &machine)?;

    Ok(Response::new()
        .add_attribute("action", "burn")
        .add_attribute("token_id", token_id))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
    moultbook_contract: Option<String>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(a) = admin {
        cfg.admin = deps.api.addr_validate(&a)?;
    }
    if let Some(m) = moultbook_contract {
        cfg.moultbook_contract = if m.is_empty() {
            None
        } else {
            Some(deps.api.addr_validate(&m)?)
        };
    }

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetMachine { token_id } => to_json_binary(&query_machine(deps, token_id)?),
        QueryMsg::GetOwnership { token_id } => {
            to_json_binary(&query_ownership(deps, token_id)?)
        }
        QueryMsg::GetOwnerFraction { token_id, owner } => {
            to_json_binary(&query_owner_fraction(deps, token_id, owner)?)
        }
        QueryMsg::ListMachines { start_after, limit } => {
            to_json_binary(&query_list_machines(deps, start_after, limit)?)
        }
        QueryMsg::ListByOwner { owner, start_after, limit } => {
            to_json_binary(&query_list_by_owner(deps, owner, start_after, limit)?)
        }
        QueryMsg::GetWorkIntegrityScore { token_id } => {
            to_json_binary(&query_work_integrity_score(deps, token_id)?)
        }
    }
}

fn query_machine(deps: Deps, token_id: String) -> StdResult<crate::state::Machine> {
    MACHINES.load(deps.storage, &token_id)
}

fn query_ownership(deps: Deps, token_id: String) -> StdResult<FractionalOwnersResponse> {
    let owners = FRACTIONS
        .prefix(&token_id)
        .range(deps.storage, None, None, Order::Ascending)
        .map(|kv| {
            let (addr, bp) = kv?;
            Ok(FractionalOwner { owner: addr, basis_points: bp })
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(FractionalOwnersResponse { owners })
}

fn query_owner_fraction(
    deps: Deps,
    token_id: String,
    owner: String,
) -> StdResult<OwnerFractionResponse> {
    let owner_addr = deps.api.addr_validate(&owner)?;
    let bp = FRACTIONS
        .may_load(deps.storage, (&token_id, &owner_addr))?
        .unwrap_or(0);
    Ok(OwnerFractionResponse { owner, basis_points: bp })
}

fn query_list_machines(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<MachinesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_deref().map(Bound::exclusive);

    let machines = MACHINES
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .filter_map(|r| r.ok().map(|(_, m)| m))
        .collect();

    Ok(MachinesResponse { machines })
}

fn query_list_by_owner(
    deps: Deps,
    owner: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<MachinesResponse> {
    let owner_addr = deps.api.addr_validate(&owner)?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_deref().map(Bound::exclusive);

    let token_ids: Vec<String> = BY_OWNER
        .prefix(&owner_addr)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|kv| {
            let (id, _) = kv?;
            Ok(id)
        })
        .collect::<StdResult<Vec<_>>>()?;

    let machines = token_ids
        .iter()
        .filter_map(|id| MACHINES.may_load(deps.storage, id).ok().flatten())
        .collect();

    Ok(MachinesResponse { machines })
}

fn query_work_integrity_score(
    deps: Deps,
    token_id: String,
) -> StdResult<WorkIntegrityScoreResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let machine = MACHINES.load(deps.storage, &token_id)?;

    let moultbook = cfg
        .moultbook_contract
        .ok_or_else(|| cosmwasm_std::StdError::generic_err("moultbook not configured"))?;

    let query = MoultbookCreditScoreQuery {
        query_credit_score: MoultbookCreditScoreInner {
            author: machine.moultbook_author.clone(),
        },
    };

    let resp: MoultbookCreditScoreResponse = deps.querier.query(&QueryRequest::Wasm(
        WasmQuery::Smart {
            contract_addr: moultbook.to_string(),
            msg: to_json_binary(&query)?,
        },
    ))?;

    Ok(WorkIntegrityScoreResponse {
        token_id,
        moultbook_author: resp.author,
        score: resp.score,
        total_entries: resp.total_entries,
        active_entries: resp.active_entries,
        verified_entries: resp.verified_entries,
    })
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new().add_attribute("action", "migrate"))
}
