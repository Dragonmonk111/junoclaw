// ── Read-only queries against the truth-market + marketplace contracts ──
//
// Backs the Robot Ops panel's "live" mode. As of 2026-08-17 the truth-market
// contract on uni-7 has zero registered operators and zero finalized epochs —
// these queries return that real, empty state rather than fabricating data.
// The panel falls back to the local simulation (`robot-ops-sim.ts`) for
// per-joint/per-robot telemetry, which has no on-chain analog yet.

import { CosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { CHAIN_CONFIG, CONTRACTS } from './chain-config'

let _client: CosmWasmClient | null = null

async function getClient(): Promise<CosmWasmClient> {
  if (!_client) {
    _client = await CosmWasmClient.connect(CHAIN_CONFIG.rpc)
  }
  return _client
}

export interface TruthMarketOperator {
  address: string
  stake: string
  active: boolean
  accuracy: number
  epochsParticipated: number
}

export interface TruthMarketStats {
  totalOperators: number
  activeOperators: number
  totalStaked: string
  epochsFinalized: number
  rewardPool: string
  minOperators: number
}

export async function queryTruthMarketOperators(): Promise<TruthMarketOperator[]> {
  const client = await getClient()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any = await client.queryContractSmart(CONTRACTS.truthMarket, { list_operators: {} })
  const operators = raw.operators ?? []
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return operators.map((op: any) => ({
    address: String(op.address),
    stake: String(op.stake),
    active: Boolean(op.active),
    accuracy: Number(op.accuracy ?? 0),
    epochsParticipated: Number(op.epochs_participated ?? 0),
  }))
}

export async function queryTruthMarketStats(): Promise<TruthMarketStats> {
  const client = await getClient()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [stats, config]: [any, any] = await Promise.all([
    client.queryContractSmart(CONTRACTS.truthMarket, { get_stats: {} }),
    client.queryContractSmart(CONTRACTS.truthMarket, { get_config: {} }),
  ])
  return {
    totalOperators: Number(stats.total_operators ?? 0),
    activeOperators: Number(stats.active_operators ?? 0),
    totalStaked: String(stats.total_staked ?? '0'),
    epochsFinalized: Number(stats.epochs_finalized ?? 0),
    rewardPool: String(stats.reward_pool ?? '0'),
    minOperators: Number(config.min_operators ?? 3),
  }
}

export interface MarketplaceListingLive {
  id: number
  agent: string
  skillRef: string
  price: string
  active: boolean
}

export async function queryMarketplaceListings(limit = 10): Promise<MarketplaceListingLive[]> {
  const client = await getClient()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any[] = await client.queryContractSmart(CONTRACTS.marketplace, {
    list_listings: { start_after: null, limit },
  })
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return raw.map((l: any) => ({
    id: Number(l.id),
    agent: String(l.agent),
    skillRef: String(l.skill_ref),
    price: String(l.price),
    active: Boolean(l.active),
  }))
}
