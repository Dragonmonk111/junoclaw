// ── React hook for DEX state: pool data, swap execution, liquidity ──

import { useState, useEffect, useCallback, useRef } from 'react'
import {
  queryAllPools,
  queryLpBalance,
  querySimulateSwap,
} from '../lib/dex-queries'
import {
  executeSwap,
  executeProvideLiquidity,
  executeWithdrawLiquidity,
} from '../lib/dex-execute'
import { getWalletAddress, isWalletConnected } from '../lib/contract-execute'
import { CONTRACTS } from '../lib/chain-config'
import type { DexPairInfo, DexPoolState, DexSimulation, DexLpPosition } from '../types'

export interface DexState {
  // Pool data
  pairs: { pair: DexPairInfo; pool: DexPoolState }[]
  activePairAddr: string
  activePool: DexPoolState | null
  activePair: DexPairInfo | null
  setActivePair: (addr: string) => void
  loading: boolean
  error: string | null
  lastFetched: number | null

  // User LP position
  lpPosition: DexLpPosition | null

  // Simulation
  simulate: (offerDenom: string, offerAmount: string) => Promise<DexSimulation | null>

  // Actions (require wallet)
  swap: (offerDenom: string, offerAmount: string, minReturn?: string) => Promise<string>
  provideLiquidity: (denomA: string, amountA: string, denomB: string, amountB: string) => Promise<string>
  withdrawLiquidity: (lpAmount: string) => Promise<string>
  txPending: boolean
  lastTxHash: string | null
  lastTxError: string | null

  // Refresh
  refresh: () => Promise<void>
}

const POLL_INTERVAL = 15_000 // 15 seconds

export function useDexClient(): DexState {
  const [pairs, setPairs] = useState<{ pair: DexPairInfo; pool: DexPoolState }[]>([])
  const [activePairAddr, setActivePairAddr] = useState<string>(CONTRACTS.junoswapPairJunoUsdc)
  const [lpPosition, setLpPosition] = useState<DexLpPosition | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [lastFetched, setLastFetched] = useState<number | null>(null)

  const [txPending, setTxPending] = useState(false)
  const [lastTxHash, setLastTxHash] = useState<string | null>(null)
  const [lastTxError, setLastTxError] = useState<string | null>(null)

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Derived
  const activeEntry = pairs.find(p => p.pair.pair_addr === activePairAddr)
  const activePool = activeEntry?.pool ?? null
  const activePair = activeEntry?.pair ?? null

  // ── Fetch all pool data ──
  const fetchAll = useCallback(async () => {
    try {
      const allPools = await queryAllPools()
      setPairs(allPools)
      setLastFetched(Date.now())
      setError(null)

      // Fetch LP position if wallet connected
      const addr = getWalletAddress()
      if (addr) {
        try {
          const lp = await queryLpBalance(activePairAddr, addr)
          setLpPosition(lp)
        } catch {
          setLpPosition(null)
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [activePairAddr])

  // ── Polling ──
  useEffect(() => {
    fetchAll()
    pollRef.current = setInterval(fetchAll, POLL_INTERVAL)
    return () => {
      if (pollRef.current) clearInterval(pollRef.current)
    }
  }, [fetchAll])

  // ── Simulate ──
  const simulate = useCallback(async (offerDenom: string, offerAmount: string): Promise<DexSimulation | null> => {
    if (!offerAmount || Number(offerAmount) <= 0) return null
    try {
      return await querySimulateSwap(activePairAddr, { native: offerDenom }, offerAmount)
    } catch {
      return null
    }
  }, [activePairAddr])

  // ── TX wrapper ──
  const withTx = useCallback(
    async (fn: () => Promise<{ transactionHash: string }>) => {
      if (!isWalletConnected()) throw new Error('Connect wallet first')
      setTxPending(true)
      setLastTxError(null)
      setLastTxHash(null)
      try {
        const result = await fn()
        setLastTxHash(result.transactionHash)
        setTimeout(() => fetchAll(), 2000)
        return result.transactionHash
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err)
        setLastTxError(msg)
        throw err
      } finally {
        setTxPending(false)
      }
    },
    [fetchAll],
  )

  // ── Actions ──
  const swap = useCallback(
    (offerDenom: string, offerAmount: string, minReturn?: string) =>
      withTx(() => executeSwap(activePairAddr, offerDenom, offerAmount, minReturn)),
    [withTx, activePairAddr],
  )

  const provideLiquidity = useCallback(
    (denomA: string, amountA: string, denomB: string, amountB: string) =>
      withTx(() => executeProvideLiquidity(activePairAddr, denomA, amountA, denomB, amountB)),
    [withTx, activePairAddr],
  )

  const withdrawLiquidity = useCallback(
    (lpAmount: string) =>
      withTx(() => executeWithdrawLiquidity(activePairAddr, lpAmount)),
    [withTx, activePairAddr],
  )

  return {
    pairs,
    activePairAddr,
    activePool,
    activePair,
    setActivePair: setActivePairAddr,
    loading,
    error,
    lastFetched,
    lpPosition,
    simulate,
    swap,
    provideLiquidity,
    withdrawLiquidity,
    txPending,
    lastTxHash,
    lastTxError,
    refresh: fetchAll,
  }
}
