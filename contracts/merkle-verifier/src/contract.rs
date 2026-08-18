use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use sha2::{Digest, Sha256};

use crate::error::ContractError;
use crate::msg::{
    AdminResponse, ExecuteMsg, GetRootResponse, InstantiateMsg, QueryMsg,
};
use crate::state::{BatchRoot, ADMIN, ROOTS};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let admin = deps.api.addr_validate(&msg.admin)?;
    ADMIN.save(deps.storage, &admin)?;
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", admin.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AnchorRoot {
            robot_id,
            batch_height,
            merkle_root,
            cycle_count,
        } => execute_anchor_root(deps, env, info, robot_id, batch_height, merkle_root, cycle_count),
        ExecuteMsg::VerifyProof {
            robot_id,
            batch_height,
            leaf_hash,
            leaf_index,
            proof,
        } => execute_verify_proof(deps.as_ref(), robot_id, batch_height, leaf_hash, leaf_index, proof),
        ExecuteMsg::TransferAdmin { new_admin } => {
            execute_transfer_admin(deps, info, new_admin)
        }
    }
}

fn execute_anchor_root(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    robot_id: String,
    batch_height: u64,
    merkle_root: String,
    cycle_count: u32,
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

    if cycle_count == 0 {
        return Err(ContractError::InvalidParams {
            reason: "cycle_count must be positive".to_string(),
        });
    }

    let root = BatchRoot {
        merkle_root: merkle_root.clone(),
        cycle_count,
        anchored_at: env.block.height,
    };

    ROOTS.save(deps.storage, (&robot_id, batch_height), &root)?;

    Ok(Response::new()
        .add_attribute("action", "anchor_root")
        .add_attribute("robot_id", &robot_id)
        .add_attribute("batch_height", batch_height.to_string())
        .add_attribute("merkle_root", &merkle_root)
        .add_attribute("cycle_count", cycle_count.to_string()))
}

fn execute_verify_proof(
    deps: Deps,
    robot_id: String,
    batch_height: u64,
    leaf_hash: String,
    leaf_index: u32,
    proof: Vec<String>,
) -> Result<Response, ContractError> {
    let root = ROOTS
        .load(deps.storage, (&robot_id, batch_height))
        .map_err(|_| ContractError::RootNotFound {
            robot_id: robot_id.clone(),
            batch_height,
        })?;

    let computed_root = verify_merkle_proof(&leaf_hash, leaf_index, &proof)?;

    if computed_root != root.merkle_root {
        return Err(ContractError::LeafHashMismatch {
            expected: root.merkle_root,
            got: computed_root,
        });
    }

    Ok(Response::new()
        .add_attribute("action", "verify_proof")
        .add_attribute("robot_id", &robot_id)
        .add_attribute("batch_height", batch_height.to_string())
        .add_attribute("leaf_index", leaf_index.to_string())
        .add_attribute("result", "valid"))
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

/// Verify a Merkle proof using SHA-256.
/// Each proof element is a hex-encoded 32-byte hash.
/// The leaf is hashed with its sibling at each level, alternating
/// left/right based on the bit at each level of the leaf index.
fn verify_merkle_proof(
    leaf_hash: &str,
    leaf_index: u32,
    proof: &[String],
) -> Result<String, ContractError> {
    let mut current = hex::decode(leaf_hash).map_err(|_| ContractError::InvalidProof {
        reason: "leaf_hash is not valid hex".to_string(),
    })?;

    if current.len() != 32 {
        return Err(ContractError::InvalidProof {
            reason: format!("leaf_hash must be 32 bytes, got {}", current.len()),
        });
    }

    for (level, sibling_hex) in proof.iter().enumerate() {
        let sibling = hex::decode(sibling_hex).map_err(|_| ContractError::InvalidProof {
            reason: format!("proof[{}] is not valid hex", level),
        })?;

        if sibling.len() != 32 {
            return Err(ContractError::InvalidProof {
                reason: format!("proof[{}] must be 32 bytes, got {}", level, sibling.len()),
            });
        }

        // Bit at this level determines order: 0 = leaf is left, 1 = leaf is right
        let bit = (leaf_index >> level) & 1;
        let mut hasher = Sha256::new();
        if bit == 0 {
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
        QueryMsg::GetRoot {
            robot_id,
            batch_height,
        } => {
            let root = ROOTS.load(deps.storage, (&robot_id, batch_height)).map_err(|_| {
                cosmwasm_std::StdError::not_found(format!(
                    "root not found for robot {} batch {}",
                    robot_id, batch_height
                ))
            })?;
            let resp = GetRootResponse {
                robot_id,
                batch_height,
                merkle_root: root.merkle_root,
                cycle_count: root.cycle_count,
                anchored_at: root.anchored_at,
            };
            to_json_binary(&resp)
        }
        QueryMsg::GetAdmin {} => {
            let admin = ADMIN.load(deps.storage)?;
            let resp = AdminResponse {
                admin: admin.to_string(),
            };
            to_json_binary(&resp)
        }
    }
}
