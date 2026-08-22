// ── React hook: live FeePay state for the FeePay panel ──
//
// Polls the Juno FeePay module on uni-7 via REST. Shows real pool
// balances, registered contracts, and module status.

import { useState, useEffect, useCallback, useRef } from 'react'
import {
  queryFeePayParams,
  queryFeePayContract,
  queryAllFeePayContracts,
  type FeePayParams,
  type FeePayContractInfo,
} from '../lib/feepay-queries'

export interface FeePayLiveState {
  params: FeePayParams | null
  registeredContracts: string[]
  contractDetails: FeePayContractInfo[]
  loading: boolean
  error: string | null
  lastFetched: number | null
  refresh: () => Promise<void>
}

const POLL_INTERVAL = 20_000

export function useFeePayLive(): FeePayLiveState {
  const [params, setParams] = useState<FeePayParams | null>(null)
  const [registeredContracts, setRegisteredContracts] = useState<string[]>([])
  const [contractDetails, setContractDetails] = useState<FeePayContractInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [lastFetched, setLastFetched] = useState<number | null>(null)

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const fetchAll = useCallback(async () => {
    try {
      const [p, addrs] = await Promise.all([
        queryFeePayParams(),
        queryAllFeePayContracts(),
      ])
      setParams(p)
      setRegisteredContracts(addrs)

      const details = await Promise.all(
        addrs.map((addr) => queryFeePayContract(addr).catch(() => null))
      )
      setContractDetails(details.filter((d): d is FeePayContractInfo => d !== null))

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

  return { params, registeredContracts, contractDetails, loading, error, lastFetched, refresh: fetchAll }
}
