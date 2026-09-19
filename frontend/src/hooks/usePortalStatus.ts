import { useState, useEffect } from 'react'
import { queryFeePayParams, queryAllFeePayContracts } from '../lib/feepay-queries'
import { queryMinerOperators, queryMinerStats } from '../lib/miner-queries'
import { queryTruthMarketOperators } from '../lib/robot-ops-queries'
import { queryAllPools } from '../lib/dex-queries'
import { useBuzzSimulation } from '../lib/buzz-sim'
import { useStore } from '../store'

export interface PortalStatus {
  [key: string]: { label: string; live: boolean; error: boolean }
}

const POLL_INTERVAL = 60_000

export function usePortalStatus(): PortalStatus {
  const agents = useStore((s) => s.agents)
  const daos = useStore((s) => s.daos)
  const buzz = useBuzzSimulation()
  const [status, setStatus] = useState<PortalStatus>({
    chat:        { label: `${agents.length} agents`, live: agents.length > 0, error: false },
    dao:        { label: `${daos.filter(d => d.status === 'active').length} active`, live: daos.length > 0, error: false },
    dex:        { label: 'Loading…', live: false, error: false },
    intel:      { label: 'Live feed', live: true, error: false },
    robotops:   { label: 'Loading…', live: false, error: false },
    feepay:     { label: 'Loading…', live: false, error: false },
    miner:      { label: 'Loading…', live: false, error: false },
    buzz:       { label: buzz.relay.connected ? 'Live relay' : 'Simulated', live: buzz.relay.connected, error: false },
    commonwealth:{ label: 'Digest', live: true, error: false },
    contracts:  { label: 'Registry', live: true, error: false },
    updates:    { label: 'Tracker', live: true, error: false },
  })

  useEffect(() => {
    let cancelled = false

    const fetchStatus = async () => {
      const updates: PortalStatus = {}

      // FeePay
      try {
        const [params, addrs] = await Promise.all([
          queryFeePayParams(),
          queryAllFeePayContracts(),
        ])
        updates.feepay = {
          label: params.enableFeePay ? `${addrs.length} pools` : 'Disabled',
          live: params.enableFeePay,
          error: false,
        }
      } catch {
        updates.feepay = { label: 'Offline', live: false, error: true }
      }

      // Miners (truth market)
      try {
        const [operators, stats] = await Promise.all([
          queryMinerOperators(),
          queryMinerStats(),
        ])
        updates.miner = {
          label: `${stats.activeOperators} / ${stats.totalOperators} operators`,
          live: operators.length > 0,
          error: false,
        }
      } catch {
        updates.miner = { label: 'Offline', live: false, error: true }
      }

      // Robot Ops (truth market operators)
      try {
        const operators = await queryTruthMarketOperators()
        updates.robotops = {
          label: `${operators.length} operators`,
          live: operators.length > 0,
          error: false,
        }
      } catch {
        updates.robotops = { label: 'Offline', live: false, error: true }
      }

      // DEX
      try {
        const pools = await queryAllPools()
        updates.dex = {
          label: `${pools.length} pools`,
          live: pools.length > 0,
          error: false,
        }
      } catch {
        updates.dex = { label: 'Offline', live: false, error: true }
      }

      if (!cancelled) {
        setStatus(prev => ({ ...prev, ...updates }))
      }
    }

    fetchStatus()
    const interval = setInterval(fetchStatus, POLL_INTERVAL)
    return () => { cancelled = true; clearInterval(interval) }
  }, [])

  return status
}
