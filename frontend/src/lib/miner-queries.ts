// ── Miner queries — truth market operator details + fingerprints ──

import { CosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { CHAIN_CONFIG, CONTRACTS } from './chain-config'

let _client: CosmWasmClient | null = null

async function getClient(): Promise<CosmWasmClient> {
  if (!_client) {
    _client = await CosmWasmClient.connect(CHAIN_CONFIG.rpc)
  }
  return _client
}

export interface MinerOperator {
  address: string
  stake: string
  totalRewards: string
  totalSlashed: string
  epochsParticipated: number
  correctVerdicts: number
  incorrectVerdicts: number
  active: boolean
  accuracy: number
  fingerprint: string | null
}

export interface MinerStats {
  totalOperators: number
  activeOperators: number
  totalStaked: string
  totalRewardsPaid: string
  totalSlashed: string
  epochsFinalized: number
  rewardPool: string
}

export interface MinerConfig {
  admin: string
  minStake: string
  slashPercent: number
  rewardPercent: number
  denom: string
  unstakeCooldownSecs: number
  minOperators: number
}

export interface FingerprintEntry {
  fingerprint: string
  operatorCount: number
}

export interface FingerprintStats {
  fingerprints: FingerprintEntry[]
  operatorsWithoutFingerprint: number
}

export async function queryMinerOperators(): Promise<MinerOperator[]> {
  const client = await getClient()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any = await client.queryContractSmart(CONTRACTS.truthMarket, { list_operators: {} })
  const operators = raw.operators ?? []
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return operators.map((op: any) => ({
    address: String(op.address),
    stake: String(op.stake),
    totalRewards: String(op.total_rewards ?? '0'),
    totalSlashed: String(op.total_slashed ?? '0'),
    epochsParticipated: Number(op.epochs_participated ?? 0),
    correctVerdicts: Number(op.correct_verdicts ?? 0),
    incorrectVerdicts: Number(op.incorrect_verdicts ?? 0),
    active: Boolean(op.active),
    accuracy: Number(op.accuracy ?? 0),
    fingerprint: op.fingerprint ?? null,
  }))
}

export async function queryMinerStats(): Promise<MinerStats> {
  const client = await getClient()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any = await client.queryContractSmart(CONTRACTS.truthMarket, { get_stats: {} })
  return {
    totalOperators: Number(raw.total_operators ?? 0),
    activeOperators: Number(raw.active_operators ?? 0),
    totalStaked: String(raw.total_staked ?? '0'),
    totalRewardsPaid: String(raw.total_rewards_paid ?? '0'),
    totalSlashed: String(raw.total_slashed ?? '0'),
    epochsFinalized: Number(raw.epochs_finalized ?? 0),
    rewardPool: String(raw.reward_pool ?? '0'),
  }
}

export async function queryMinerConfig(): Promise<MinerConfig> {
  const client = await getClient()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any = await client.queryContractSmart(CONTRACTS.truthMarket, { get_config: {} })
  return {
    admin: String(raw.admin ?? ''),
    minStake: String(raw.min_stake ?? '0'),
    slashPercent: Number(raw.slash_percent ?? 0),
    rewardPercent: Number(raw.reward_percent ?? 0),
    denom: String(raw.denom ?? 'ujunox'),
    unstakeCooldownSecs: Number(raw.unstake_cooldown_secs ?? 0),
    minOperators: Number(raw.min_operators ?? 3),
  }
}

export async function queryFingerprints(): Promise<FingerprintStats> {
  const client = await getClient()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any = await client.queryContractSmart(CONTRACTS.truthMarket, { get_fingerprints: {} })
  return {
    fingerprints: (raw.fingerprints ?? []).map((f: any) => ({
      fingerprint: String(f.fingerprint),
      operatorCount: Number(f.operator_count),
    })),
    operatorsWithoutFingerprint: Number(raw.operators_without_fingerprint ?? 0),
  }
}
