// ── Read-only queries against the agent-company contract ──

import { CosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { CHAIN_CONFIG, CONTRACTS } from './chain-config'
import type { DaoConfig, DaoProposal, DaoMember, VoteOption } from '../types'

let _client: CosmWasmClient | null = null

export async function getClient(): Promise<CosmWasmClient> {
  if (!_client) {
    _client = await CosmWasmClient.connect(CHAIN_CONFIG.rpc)
  }
  return _client
}

// ── Type adapters (contract JSON → frontend types) ──

function adaptMemberRole(role: string): 'agent' | 'human' | 'subdao' {
  if (typeof role === 'object') {
    // cw_serde enums serialize as e.g. "human" or "agent" (lowercase string)
    // but could also be { human: {} } depending on serde config
    const key = Object.keys(role)[0]
    if (key === 'sub_d_a_o' || key === 'sub_dao') return 'subdao'
    return (key as 'agent' | 'human') ?? 'human'
  }
  const r = String(role).toLowerCase()
  if (r === 'sub_d_a_o' || r === 'sub_dao' || r === 'subdao') return 'subdao'
  if (r === 'agent') return 'agent'
  return 'human'
}

function adaptVoteOption(opt: unknown): VoteOption {
  if (typeof opt === 'string') return opt.toLowerCase() as VoteOption
  if (typeof opt === 'object' && opt !== null) {
    const key = Object.keys(opt)[0]
    return (key?.toLowerCase() ?? 'abstain') as VoteOption
  }
  return 'abstain'
}

function adaptProposalStatus(status: unknown): DaoProposal['status'] {
  if (typeof status === 'string') return status.toLowerCase() as DaoProposal['status']
  if (typeof status === 'object' && status !== null) {
    const key = Object.keys(status)[0]
    return (key?.toLowerCase() ?? 'open') as DaoProposal['status']
  }
  return 'open'
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function adaptProposalKind(kind: any): DaoProposal['kind'] {
  // Contract serializes ProposalKind as { variant_name: { ...fields } }
  if (typeof kind !== 'object' || kind === null) {
    return { type: 'free_text', title: 'Unknown', description: '' }
  }
  if ('weight_change' in kind) {
    return {
      type: 'weight_change',
      members: (kind.weight_change.members ?? []).map(adaptMember),
    }
  }
  if ('wavs_push' in kind) {
    return {
      type: 'wavs_push',
      task_description: kind.wavs_push.task_description,
      execution_tier: kind.wavs_push.execution_tier,
      escrow_amount: Number(kind.wavs_push.escrow_amount ?? '0'),
    }
  }
  if ('config_change' in kind) {
    return {
      type: 'config_change',
      new_admin: kind.config_change.new_admin,
      new_governance: kind.config_change.new_governance,
    }
  }
  if ('free_text' in kind) {
    return {
      type: 'free_text',
      title: kind.free_text.title,
      description: kind.free_text.description,
    }
  }
  if ('outcome_create' in kind) {
    return {
      type: 'outcome_create',
      question: kind.outcome_create.question,
      resolution_criteria: kind.outcome_create.resolution_criteria,
      deadline_block: kind.outcome_create.deadline_block,
    }
  }
  if ('outcome_resolve' in kind) {
    return {
      type: 'outcome_resolve',
      market_id: kind.outcome_resolve.market_id,
      outcome: kind.outcome_resolve.outcome,
      attestation_hash: kind.outcome_resolve.attestation_hash,
    }
  }
  // Fallback
  return { type: 'free_text', title: JSON.stringify(kind), description: '' }
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function adaptMember(m: any): DaoMember {
  return {
    addr: typeof m.addr === 'string' ? m.addr : String(m.addr),
    weight: Number(m.weight),
    role: adaptMemberRole(m.role),
  }
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function adaptProposal(p: any): DaoProposal {
  return {
    id: Number(p.id),
    proposer: String(p.proposer),
    kind: adaptProposalKind(p.kind),
    votes: (p.votes ?? []).map((v: any) => ({
      voter: String(v.voter),
      option: adaptVoteOption(v.option),
      weight: Number(v.weight),
      block_height: Number(v.block_height),
    })),
    yes_weight: Number(p.yes_weight),
    no_weight: Number(p.no_weight),
    abstain_weight: Number(p.abstain_weight),
    total_voted_weight: Number(p.total_voted_weight),
    status: adaptProposalStatus(p.status),
    created_at_block: Number(p.created_at_block),
    voting_deadline_block: Number(p.voting_deadline_block),
    min_deadline_block: Number(p.min_deadline_block),
    executed: Boolean(p.executed),
  }
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function adaptVerificationModel(model: any): DaoConfig['verification']['model'] {
  if (typeof model === 'string') {
    const m = model.toLowerCase()
    if (m === 'witness_and_wavs') return 'witness_and_wavs'
    if (m === 'witness') return 'witness'
    if (m === 'wavs') return 'wavs'
    return 'none'
  }
  if (typeof model === 'object' && model !== null) {
    const key = Object.keys(model)[0]?.toLowerCase()
    if (key === 'witness_and_wavs') return 'witness_and_wavs'
    if (key === 'witness') return 'witness'
    if (key === 'wavs') return 'wavs'
    return 'none'
  }
  return 'none'
}

// ── Queries ──

export async function queryConfig(contractAddr?: string): Promise<DaoConfig> {
  const client = await getClient()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any = await client.queryContractSmart(addr, { get_config: {} })
  return {
    name: raw.name,
    admin: String(raw.admin),
    governance: raw.governance ? String(raw.governance) : undefined,
    escrow_contract: String(raw.escrow_contract),
    agent_registry: String(raw.agent_registry),
    task_ledger: raw.task_ledger ? String(raw.task_ledger) : undefined,
    members: (raw.members ?? []).map(adaptMember),
    total_weight: Number(raw.total_weight),
    denom: raw.denom ?? 'ujunox',
    voting_period_blocks: Number(raw.voting_period_blocks),
    quorum_percent: Number(raw.quorum_percent),
    adaptive_threshold_blocks: Number(raw.adaptive_threshold_blocks),
    adaptive_min_blocks: Number(raw.adaptive_min_blocks),
    verification: {
      model: adaptVerificationModel(raw.verification?.model),
      required_attestations: Number(raw.verification?.required_attestations ?? 2),
      total_witnesses: Number(raw.verification?.total_witnesses ?? 3),
      attestation_timeout_blocks: Number(raw.verification?.attestation_timeout_blocks ?? 200),
      auto_release_on_verify: Boolean(raw.verification?.auto_release_on_verify ?? true),
    },
  }
}

export async function queryProposals(
  contractAddr?: string,
  startAfter?: number,
  limit?: number,
): Promise<DaoProposal[]> {
  const client = await getClient()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any[] = await client.queryContractSmart(addr, {
    list_proposals: {
      start_after: startAfter ?? null,
      limit: limit ?? 30,
    },
  })
  return raw.map(adaptProposal)
}

export async function queryProposal(
  proposalId: number,
  contractAddr?: string,
): Promise<DaoProposal> {
  const client = await getClient()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  const raw = await client.queryContractSmart(addr, {
    get_proposal: { proposal_id: proposalId },
  })
  return adaptProposal(raw)
}

export async function queryMembers(contractAddr?: string): Promise<DaoMember[]> {
  const client = await getClient()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any[] = await client.queryContractSmart(addr, { get_members: {} })
  return raw.map(adaptMember)
}

export async function queryMemberEarnings(
  memberAddr: string,
  contractAddr?: string,
): Promise<string> {
  const client = await getClient()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  const raw = await client.queryContractSmart(addr, {
    get_member_earnings: { addr: memberAddr },
  })
  return String(raw)
}

export async function queryAttestations(
  contractAddr?: string,
  startAfter?: number,
  limit?: number,
) {
  const client = await getClient()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  return client.queryContractSmart(addr, {
    list_attestations: {
      start_after: startAfter ?? null,
      limit: limit ?? 30,
    },
  })
}

export async function queryAttestation(
  proposalId: number,
  contractAddr?: string,
) {
  const client = await getClient()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  return client.queryContractSmart(addr, {
    get_attestation: { proposal_id: proposalId },
  })
}

export async function queryBalance(
  address: string,
  denom?: string,
): Promise<string> {
  const client = await getClient()
  const bal = await client.getBalance(address, denom ?? CHAIN_CONFIG.denom)
  return bal.amount
}
