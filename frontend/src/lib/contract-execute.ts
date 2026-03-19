// ── Signed transactions against the agent-company contract ──

import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'
import { CHAIN_CONFIG, CONTRACTS, KEPLR_CHAIN_INFO } from './chain-config'
import type { VoteOption } from '../types'

// ── Keplr window type augmentation ──

interface KeplrWindow {
  keplr?: {
    enable: (chainId: string) => Promise<void>
    experimentalSuggestChain: (chainInfo: unknown) => Promise<void>
    getOfflineSigner: (chainId: string) => unknown
    getKey: (chainId: string) => Promise<{ bech32Address: string; name: string }>
  }
}

declare const window: KeplrWindow & typeof globalThis

// ── Connection state ──

let _signingClient: SigningCosmWasmClient | null = null
let _walletAddress: string | null = null

export function getWalletAddress(): string | null {
  return _walletAddress
}

export function isWalletConnected(): boolean {
  return _signingClient !== null && _walletAddress !== null
}

export async function connectKeplr(): Promise<{ address: string; name: string }> {
  if (!window.keplr) {
    throw new Error('Keplr wallet not found. Please install the Keplr browser extension.')
  }

  // Suggest uni-7 chain if not already added
  try {
    await window.keplr.experimentalSuggestChain(KEPLR_CHAIN_INFO)
  } catch {
    // Chain might already be added — continue
  }

  await window.keplr.enable(CHAIN_CONFIG.chainId)

  const offlineSigner = window.keplr.getOfflineSigner(CHAIN_CONFIG.chainId)
  const keyInfo = await window.keplr.getKey(CHAIN_CONFIG.chainId)

  _signingClient = await SigningCosmWasmClient.connectWithSigner(
    CHAIN_CONFIG.rpc,
    offlineSigner as any,
    { gasPrice: GasPrice.fromString(CHAIN_CONFIG.gasPrice) },
  )
  _walletAddress = keyInfo.bech32Address

  return { address: keyInfo.bech32Address, name: keyInfo.name }
}

export function disconnectWallet() {
  _signingClient = null
  _walletAddress = null
}

function requireSigner(): { client: SigningCosmWasmClient; sender: string } {
  if (!_signingClient || !_walletAddress) {
    throw new Error('Wallet not connected. Call connectKeplr() first.')
  }
  return { client: _signingClient, sender: _walletAddress }
}

// ── Execute messages ──

export async function createProposal(
  kind: Record<string, unknown>,
  contractAddr?: string,
) {
  const { client, sender } = requireSigner()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  return client.execute(
    sender,
    addr,
    { create_proposal: { kind } },
    'auto',
  )
}

export async function castVote(
  proposalId: number,
  vote: VoteOption,
  contractAddr?: string,
) {
  const { client, sender } = requireSigner()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  return client.execute(
    sender,
    addr,
    { cast_vote: { proposal_id: proposalId, vote } },
    'auto',
  )
}

export async function executeProposal(
  proposalId: number,
  contractAddr?: string,
) {
  const { client, sender } = requireSigner()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  return client.execute(
    sender,
    addr,
    { execute_proposal: { proposal_id: proposalId } },
    'auto',
  )
}

export async function expireProposal(
  proposalId: number,
  contractAddr?: string,
) {
  const { client, sender } = requireSigner()
  const addr = contractAddr ?? CONTRACTS.agentCompany
  return client.execute(
    sender,
    addr,
    { expire_proposal: { proposal_id: proposalId } },
    'auto',
  )
}

// ── Proposal kind builders (match contract's ProposalKindMsg snake_case) ──

export function buildFreeTextProposal(title: string, description: string) {
  return { free_text: { title, description } }
}

export function buildWeightChangeProposal(
  members: { addr: string; weight: number; role: string }[],
) {
  return {
    weight_change: {
      members: members.map(m => ({
        addr: m.addr,
        weight: m.weight,
        role: m.role.toLowerCase() === 'subdao' ? 'sub_d_a_o' : m.role.toLowerCase(),
      })),
    },
  }
}

export function buildWavsPushProposal(
  taskDescription: string,
  executionTier: string,
  escrowAmount: string,
) {
  return {
    wavs_push: {
      task_description: taskDescription,
      execution_tier: executionTier.toLowerCase(),
      escrow_amount: escrowAmount,
    },
  }
}

export function buildConfigChangeProposal(
  newAdmin?: string,
  newGovernance?: string,
) {
  return {
    config_change: {
      new_admin: newAdmin ?? null,
      new_governance: newGovernance ?? null,
    },
  }
}

export function buildOutcomeCreateProposal(
  question: string,
  resolutionCriteria: string,
  deadlineBlock: number,
) {
  return {
    outcome_create: {
      question,
      resolution_criteria: resolutionCriteria,
      deadline_block: deadlineBlock,
    },
  }
}

export function buildSortitionRequestProposal(
  count: number,
  purpose: string,
) {
  return {
    sortition_request: { count, purpose },
  }
}
