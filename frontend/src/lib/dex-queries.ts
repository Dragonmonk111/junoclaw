// ── Read-only queries against Junoswap v2 pair + factory contracts ──

import { CosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { CHAIN_CONFIG, CONTRACTS } from './chain-config'
import type { DexPairInfo, DexPoolState, DexSimulation, DexLpPosition } from '../types'

let _client: CosmWasmClient | null = null

async function getClient(): Promise<CosmWasmClient> {
  if (!_client) {
    _client = await CosmWasmClient.connect(CHAIN_CONFIG.rpc)
  }
  return _client
}

// ── Pair Info ──

export async function queryPairInfo(pairAddr: string): Promise<DexPairInfo> {
  const client = await getClient()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any = await client.queryContractSmart(pairAddr, { pair_info: {} })
  return {
    pair_addr: pairAddr,
    token_a: raw.token_a,
    token_b: raw.token_b,
    fee_bps: Number(raw.fee_bps),
    factory: String(raw.factory),
    junoclaw_contract: raw.junoclaw_contract ? String(raw.junoclaw_contract) : undefined,
  }
}

// ── Pool State ──

export async function queryPoolState(pairAddr: string): Promise<DexPoolState> {
  const client = await getClient()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any = await client.queryContractSmart(pairAddr, { pool: {} })

  // We need token info too for the full DexPoolState
  const pairInfo = await queryPairInfo(pairAddr)

  return {
    pair_addr: pairAddr,
    reserve_a: String(raw.reserve_a),
    reserve_b: String(raw.reserve_b),
    total_lp_shares: String(raw.total_lp_shares),
    total_swaps: Number(raw.total_swaps),
    total_volume_a: String(raw.total_volume_a),
    total_volume_b: String(raw.total_volume_b),
    price_a_per_b: String(raw.price_a_per_b),
    price_b_per_a: String(raw.price_b_per_a),
    token_a: pairInfo.token_a,
    token_b: pairInfo.token_b,
  }
}

// ── Simulate Swap (on-chain) ──

export async function querySimulateSwap(
  pairAddr: string,
  offerAsset: { native: string },
  offerAmount: string,
): Promise<DexSimulation> {
  const client = await getClient()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const raw: any = await client.queryContractSmart(pairAddr, {
    simulate_swap: {
      offer_asset: offerAsset,
      offer_amount: offerAmount,
    },
  })
  return {
    return_amount: String(raw.return_amount),
    spread_amount: String(raw.spread_amount),
    fee_amount: String(raw.fee_amount),
  }
}

// ── LP Balance ──

export async function queryLpBalance(
  pairAddr: string,
  userAddr: string,
): Promise<DexLpPosition> {
  const client = await getClient()
  const lpShares = await client.queryContractSmart(pairAddr, {
    lp_balance: { address: userAddr },
  })

  // Get pool state to compute share percentage and value
  const pool = await queryPoolState(pairAddr)
  const shares = BigInt(String(lpShares))
  const totalShares = BigInt(pool.total_lp_shares || '1')

  const sharePct = totalShares > 0n
    ? Number((shares * 10000n) / totalShares) / 100
    : 0

  const valueA = totalShares > 0n
    ? String((shares * BigInt(pool.reserve_a)) / totalShares)
    : '0'
  const valueB = totalShares > 0n
    ? String((shares * BigInt(pool.reserve_b)) / totalShares)
    : '0'

  return {
    pair_addr: pairAddr,
    lp_shares: String(lpShares),
    share_of_pool_pct: sharePct.toFixed(2),
    value_a: valueA,
    value_b: valueB,
  }
}

// ── All known pairs ──

export function getKnownPairAddresses(): string[] {
  return [
    CONTRACTS.junoswapPairJunoUsdc,
    CONTRACTS.junoswapPairJunoStake,
  ]
}

// ── Batch: fetch all pairs' pool states ──

export async function queryAllPools(): Promise<{ pair: DexPairInfo; pool: DexPoolState }[]> {
  const addrs = getKnownPairAddresses()
  const results = await Promise.all(
    addrs.map(async (addr) => {
      try {
        const [pair, pool] = await Promise.all([
          queryPairInfo(addr),
          queryPoolState(addr),
        ])
        return { pair, pool }
      } catch {
        return null
      }
    }),
  )
  return results.filter((r): r is NonNullable<typeof r> => r !== null)
}
