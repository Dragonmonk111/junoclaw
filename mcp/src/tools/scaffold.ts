/**
 * CosmWasm Project Scaffold Tool
 *
 * "Describe your DAO" → get a working CosmWasm project.
 *
 * This is juno.new in code form. The AI reads the template,
 * customizes it based on the user's description, and outputs
 * a project structure ready to compile and deploy.
 *
 * The templates come from JunoClaw's 9 DAO archetypes,
 * battle-tested on uni-7 testnet. We open-source them
 * the same way Ethereum open-sourced EIP-712 and ERC-20:
 * as public goods.
 */

export interface DaoTemplate {
  id: string;
  name: string;
  description: string;
  verification: "witness" | "wavs" | "witness_and_wavs";
  defaultVotingPeriod: number;
  defaultQuorum: number;
  features: string[];
}

export const DAO_TEMPLATES: DaoTemplate[] = [
  {
    id: "community_fund",
    name: "Community Fund",
    description: "Pool resources, vote on disbursements. Simple majority governance with transparent treasury.",
    verification: "witness",
    defaultVotingPeriod: 100,
    defaultQuorum: 51,
    features: ["treasury", "proposals", "voting", "disbursements"],
  },
  {
    id: "crop_protection",
    name: "Crop Protection Pool",
    description: "Agricultural mutual insurance. Members pool against crop loss, claims verified by witnesses and WAVS oracles.",
    verification: "witness_and_wavs",
    defaultVotingPeriod: 50,
    defaultQuorum: 51,
    features: ["treasury", "claims", "oracle_verification", "seasonal_pools"],
  },
  {
    id: "credential_verifier",
    name: "Credential Verifier",
    description: "Issue and verify credentials on-chain. WAVS TEE attests document authenticity.",
    verification: "wavs",
    defaultVotingPeriod: 100,
    defaultQuorum: 67,
    features: ["credential_issuance", "verification", "revocation", "tee_attestation"],
  },
  {
    id: "community_vote",
    name: "Community Vote",
    description: "Pure governance. Proposals, discussion, weighted voting. The simplest DAO template.",
    verification: "wavs",
    defaultVotingPeriod: 200,
    defaultQuorum: 33,
    features: ["proposals", "voting", "delegation"],
  },
  {
    id: "mutual_aid",
    name: "Mutual Aid DAO",
    description: "Neighbors helping neighbors. Emergency fund with fast-track disbursement for verified needs.",
    verification: "witness_and_wavs",
    defaultVotingPeriod: 25,
    defaultQuorum: 51,
    features: ["emergency_fund", "fast_track", "witness_verification", "need_assessment"],
  },
  {
    id: "farm_to_table",
    name: "Farm-to-Table Market",
    description: "Direct producer-to-consumer marketplace. Provenance tracking via WAVS attestation.",
    verification: "witness_and_wavs",
    defaultVotingPeriod: 100,
    defaultQuorum: 51,
    features: ["marketplace", "provenance", "reputation", "escrow"],
  },
  {
    id: "sortition_dao",
    name: "Citizens' Assembly",
    description: "Random selection governance. Uses NOIS/drand for verifiable randomness to pick decision-makers.",
    verification: "wavs",
    defaultVotingPeriod: 150,
    defaultQuorum: 67,
    features: ["sortition", "nois_randomness", "rotating_committees", "deliberation"],
  },
  {
    id: "skill_circle",
    name: "Skill-Staking Circle",
    description: "Stake reputation on skills. Members vouch for each other. Trust-tree credential system.",
    verification: "witness_and_wavs",
    defaultVotingPeriod: 100,
    defaultQuorum: 51,
    features: ["skill_attestation", "vouching", "trust_tree", "reputation_staking"],
  },
  {
    id: "verifiable_outcome_market",
    name: "Verifiable Outcome Market",
    description: "Prediction markets with WAVS-verified resolution. Outcomes attested by TEE, not oracles.",
    verification: "wavs",
    defaultVotingPeriod: 100,
    defaultQuorum: 51,
    features: ["outcome_creation", "position_taking", "tee_resolution", "payout"],
  },
];

export interface ScaffoldConfig {
  templateId: string;
  projectName: string;
  chainId: string;
  members: Array<{ address: string; weight: number }>;
  votingPeriod?: number;
  quorum?: number;
}

export interface ScaffoldOutput {
  template: DaoTemplate;
  files: Array<{ path: string; content: string }>;
  deployCommand: string;
  description: string;
}

export function scaffoldProject(config: ScaffoldConfig): ScaffoldOutput {
  const template = DAO_TEMPLATES.find((t) => t.id === config.templateId);
  if (!template) {
    throw new Error(
      `Unknown template: ${config.templateId}. Available: ${DAO_TEMPLATES.map((t) => t.id).join(", ")}`
    );
  }

  const votingPeriod = config.votingPeriod || template.defaultVotingPeriod;
  const quorum = config.quorum || template.defaultQuorum;
  const contractName = config.projectName.replace(/[^a-z0-9_]/gi, "_").toLowerCase();

  const files: Array<{ path: string; content: string }> = [];

  // Cargo.toml
  files.push({
    path: `${contractName}/Cargo.toml`,
    content: `[package]
name = "${contractName}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
cosmwasm-std = { version = "2.2", features = ["staking"] }
cosmwasm-schema = "2.2"
cw-storage-plus = "2.0"
schemars = "0.8"
serde = { version = "1.0", default-features = false, features = ["derive"] }
thiserror = "2.0"

[profile.release]
opt-level = "z"
strip = true
codegen-units = 1
lto = true
`,
  });

  // .cargo/config.toml
  files.push({
    path: `${contractName}/.cargo/config.toml`,
    content: `[target.wasm32-unknown-unknown]
rustflags = [
  "-C", "target-feature=-bulk-memory,-reference-types",
]
`,
  });

  // src/lib.rs
  files.push({
    path: `${contractName}/src/lib.rs`,
    content: `pub mod contract;
pub mod error;
pub mod msg;
pub mod state;
`,
  });

  // src/error.rs
  files.push({
    path: `${contractName}/src/error.rs`,
    content: `use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Not a member")]
    NotMember {},

    #[error("Already voted")]
    AlreadyVoted {},

    #[error("Voting period ended")]
    VotingEnded {},

    #[error("Proposal not passed")]
    ProposalNotPassed {},
}
`,
  });

  // src/state.rs
  const memberInit = config.members
    .map((m) => `        ("${m.address}", ${m.weight}),`)
    .join("\n");

  files.push({
    path: `${contractName}/src/state.rs`,
    content: `use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub name: String,
    pub admin: Addr,
    pub voting_period: u64,
    pub quorum_percent: u64,
    pub verification: String,
    pub total_weight: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Member {
    pub addr: Addr,
    pub weight: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Addr,
    pub title: String,
    pub description: String,
    pub yes_votes: u64,
    pub no_votes: u64,
    pub status: ProposalStatus,
    pub deadline_block: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ProposalStatus {
    Voting,
    Passed,
    Rejected,
    Executed,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const MEMBERS: Map<&Addr, Member> = Map::new("members");
pub const PROPOSALS: Map<u64, Proposal> = Map::new("proposals");
pub const PROPOSAL_SEQ: Item<u64> = Item::new("proposal_seq");
pub const VOTES: Map<(u64, &Addr), bool> = Map::new("votes");
`,
  });

  // src/msg.rs
  files.push({
    path: `${contractName}/src/msg.rs`,
    content: `use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    pub name: String,
    pub members: Vec<MemberInit>,
    pub voting_period: u64,
    pub quorum_percent: u64,
}

#[cw_serde]
pub struct MemberInit {
    pub addr: String,
    pub weight: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    Propose { title: String, description: String },
    Vote { proposal_id: u64, vote: bool },
    Execute { proposal_id: u64 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(crate::state::Config)]
    Config {},
    #[returns(crate::state::Proposal)]
    Proposal { proposal_id: u64 },
    #[returns(Vec<crate::state::Proposal>)]
    ListProposals { start_after: Option<u64>, limit: Option<u32> },
    #[returns(crate::state::Member)]
    Member { addr: String },
}
`,
  });

  // src/contract.rs
  files.push({
    path: `${contractName}/src/contract.rs`,
    content: `use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    Config, Member, Proposal, ProposalStatus, CONFIG, MEMBERS, PROPOSALS, PROPOSAL_SEQ, VOTES,
};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let mut total_weight = 0u64;
    for m in &msg.members {
        let addr = deps.api.addr_validate(&m.addr)?;
        let member = Member { addr: addr.clone(), weight: m.weight };
        total_weight += m.weight;
        MEMBERS.save(deps.storage, &addr, &member)?;
    }

    let config = Config {
        name: msg.name,
        admin: info.sender,
        voting_period: msg.voting_period,
        quorum_percent: msg.quorum_percent,
        verification: "${template.verification}".to_string(),
        total_weight,
    };
    CONFIG.save(deps.storage, &config)?;
    PROPOSAL_SEQ.save(deps.storage, &0u64)?;

    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Propose { title, description } => {
            execute_propose(deps, env, info, title, description)
        }
        ExecuteMsg::Vote { proposal_id, vote } => {
            execute_vote(deps, env, info, proposal_id, vote)
        }
        ExecuteMsg::Execute { proposal_id } => {
            execute_execute(deps, env, info, proposal_id)
        }
    }
}

fn execute_propose(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    title: String,
    description: String,
) -> Result<Response, ContractError> {
    let member = MEMBERS.may_load(deps.storage, &info.sender)?;
    if member.is_none() {
        return Err(ContractError::NotMember {});
    }

    let config = CONFIG.load(deps.storage)?;
    let id = PROPOSAL_SEQ.load(deps.storage)? + 1;
    PROPOSAL_SEQ.save(deps.storage, &id)?;

    let proposal = Proposal {
        id,
        proposer: info.sender,
        title,
        description,
        yes_votes: 0,
        no_votes: 0,
        status: ProposalStatus::Voting,
        deadline_block: env.block.height + config.voting_period,
    };
    PROPOSALS.save(deps.storage, id, &proposal)?;

    Ok(Response::new()
        .add_attribute("action", "propose")
        .add_attribute("proposal_id", id.to_string()))
}

fn execute_vote(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
    vote: bool,
) -> Result<Response, ContractError> {
    let member = MEMBERS.may_load(deps.storage, &info.sender)?
        .ok_or(ContractError::NotMember {})?;

    let mut proposal = PROPOSALS.load(deps.storage, proposal_id)?;
    if env.block.height > proposal.deadline_block {
        return Err(ContractError::VotingEnded {});
    }
    if VOTES.may_load(deps.storage, (proposal_id, &info.sender))?.is_some() {
        return Err(ContractError::AlreadyVoted {});
    }

    if vote {
        proposal.yes_votes += member.weight;
    } else {
        proposal.no_votes += member.weight;
    }

    VOTES.save(deps.storage, (proposal_id, &info.sender), &vote)?;
    PROPOSALS.save(deps.storage, proposal_id, &proposal)?;

    Ok(Response::new()
        .add_attribute("action", "vote")
        .add_attribute("proposal_id", proposal_id.to_string()))
}

fn execute_execute(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut proposal = PROPOSALS.load(deps.storage, proposal_id)?;

    if env.block.height <= proposal.deadline_block {
        return Err(ContractError::VotingEnded {}); // still in voting
    }

    let quorum_threshold = config.total_weight * config.quorum_percent / 100;
    if proposal.yes_votes >= quorum_threshold {
        proposal.status = ProposalStatus::Passed;
    } else {
        proposal.status = ProposalStatus::Rejected;
    }

    PROPOSALS.save(deps.storage, proposal_id, &proposal)?;

    Ok(Response::new()
        .add_attribute("action", "execute")
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("status", format!("{:?}", proposal.status)))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::Proposal { proposal_id } => {
            to_json_binary(&PROPOSALS.load(deps.storage, proposal_id)?)
        }
        QueryMsg::ListProposals { start_after, limit } => {
            let limit = limit.unwrap_or(30) as usize;
            let start = start_after.map(|s| s + 1).unwrap_or(1);
            let proposals: Vec<Proposal> = (start..start + limit as u64)
                .filter_map(|id| PROPOSALS.may_load(deps.storage, id).ok().flatten())
                .collect();
            to_json_binary(&proposals)
        }
        QueryMsg::Member { addr } => {
            let addr = deps.api.addr_validate(&addr)?;
            to_json_binary(&MEMBERS.load(deps.storage, &addr)?)
        }
    }
}
`,
  });

  // README.md
  files.push({
    path: `${contractName}/README.md`,
    content: `# ${config.projectName}

> Generated by [JunoClaw Cosmos MCP](https://github.com/Dragonmonk111/junoclaw) — AI-native Cosmos tooling

## Template: ${template.name}

${template.description}

## Features
${template.features.map((f) => `- ${f}`).join("\n")}

## Build

\`\`\`bash
cargo build --target wasm32-unknown-unknown --release
wasm-opt -Oz --strip-debug --strip-producers -o ${contractName}_optimized.wasm target/wasm32-unknown-unknown/release/${contractName}.wasm
\`\`\`

## Deploy (uni-7 testnet)

\`\`\`bash
# Via cosmos-mcp, or manually:
# 1. Upload: junod tx wasm store ${contractName}_optimized.wasm --from wallet --gas auto
# 2. Instantiate with your member list and governance params
\`\`\`

## Members

| Address | Weight |
|---------|--------|
${config.members.map((m) => `| \`${m.address}\` | ${m.weight} |`).join("\n")}

## Governance

- **Voting period**: ${votingPeriod} blocks
- **Quorum**: ${quorum}%
- **Verification**: ${template.verification}
`,
  });

  return {
    template,
    files,
    deployCommand: `cosmos-mcp deploy --chain ${config.chainId} --project ./${contractName}`,
    description: `Generated ${template.name} DAO project "${config.projectName}" with ${config.members.length} members, ${votingPeriod}-block voting, ${quorum}% quorum.`,
  };
}

export function listTemplates(): DaoTemplate[] {
  return DAO_TEMPLATES;
}
