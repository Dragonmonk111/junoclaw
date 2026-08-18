// ── React hook: live truth-market state for the Robot Ops panel ──
//
// Polls the real truth-market contract on uni-7. Kept separate from
// `useFleetSimulation` (robot-ops-sim.ts) because there is no on-chain
// analog for per-joint/per-robot telemetry yet — only the Trust Market
// operator/epoch layer has a real contract behind it.

import { useState, useEffect, useCallback, useRef } from 'react'
import {
  queryTruthMarketOperators,
  queryTruthMarketStats,
  type TruthMarketOperator,
  type TruthMarketStats,
} from '../lib/robot-ops-queries'

export interface TruthMarketLiveState {
  operators: TruthMarketOperator[]
  stats: TruthMarketStats | null
  loading: boolean
  error: string | null
  lastFetched: number | null
  refresh: () => Promise<void>
}

const POLL_INTERVAL = 15_000 // 15 seconds

export function useTruthMarketLive(): TruthMarketLiveState {
  const [operators, setOperators] = useState<TruthMarketOperator[]>([])
  const [stats, setStats] = useState<TruthMarketStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [lastFetched, setLastFetched] = useState<number | null>(null)

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const fetchAll = useCallback(async () => {
    try {
      const [ops, s] = await Promise.all([
        queryTruthMarketOperators(),
        queryTruthMarketStats(),
      ])
      setOperators(ops)
      setStats(s)
      setError(null)
      setLastFetched(Date.now())
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchAll()
    pollRef.current = setInterval(fetchAll, POLL_INTERVAL)
    return () => {
      if (pollRef.current) clearInterval(pollRef.current)
    }
  }, [fetchAll])

  return { operators, stats, loading, error, lastFetched, refresh: fetchAll }
}
