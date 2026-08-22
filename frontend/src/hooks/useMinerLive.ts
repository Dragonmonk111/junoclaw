// ── React hook: live truth market miner state ──

import { useState, useEffect, useCallback, useRef } from 'react'
import {
  queryMinerOperators,
  queryMinerStats,
  queryMinerConfig,
  queryFingerprints,
  type MinerOperator,
  type MinerStats,
  type MinerConfig,
  type FingerprintStats,
} from '../lib/miner-queries'

export interface MinerLiveState {
  operators: MinerOperator[]
  stats: MinerStats | null
  config: MinerConfig | null
  fingerprints: FingerprintStats | null
  loading: boolean
  error: string | null
  lastFetched: number | null
  refresh: () => Promise<void>
}

const POLL_INTERVAL = 15_000

export function useMinerLive(): MinerLiveState {
  const [operators, setOperators] = useState<MinerOperator[]>([])
  const [stats, setStats] = useState<MinerStats | null>(null)
  const [config, setConfig] = useState<MinerConfig | null>(null)
  const [fingerprints, setFingerprints] = useState<FingerprintStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [lastFetched, setLastFetched] = useState<number | null>(null)

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const fetchAll = useCallback(async () => {
    try {
      const [ops, s, cfg, fps] = await Promise.all([
        queryMinerOperators(),
        queryMinerStats(),
        queryMinerConfig(),
        queryFingerprints(),
      ])
      setOperators(ops)
      setStats(s)
      setConfig(cfg)
      setFingerprints(fps)
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

  return { operators, stats, config, fingerprints, loading, error, lastFetched, refresh: fetchAll }
}
