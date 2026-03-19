// ── React hook for chain state: wallet connection, live queries, polling ──

import { useState, useEffect, useCallback, useRef } from 'react'
import {
  queryConfig,
  queryProposals,
  queryBalance,
  queryAttestations,
} from '../lib/contract-queries'
import {
  connectKeplr,
  disconnectWallet,
  getWalletAddress,
  isWalletConnected,
  castVote as execCastVote,
  executeProposal as execExecuteProposal,
  createProposal as execCreateProposal,
  expireProposal as execExpireProposal,
} from '../lib/contract-execute'
import type { DaoConfig, DaoProposal, VoteOption } from '../types'

export interface ChainState {
  // Wallet
  walletAddress: string | null
  walletName: string | null
  walletConnected: boolean
  walletBalance: string
  connectWallet: () => Promise<void>
  disconnectWalletFn: () => void
  walletError: string | null

  // Chain data
  config: DaoConfig | null
  proposals: DaoProposal[]
  attestations: unknown[]
  loading: boolean
  lastFetched: number | null
  chainError: string | null

  // Actions (require wallet)
  vote: (proposalId: number, option: VoteOption) => Promise<string>
  execute: (proposalId: number) => Promise<string>
  propose: (kind: Record<string, unknown>) => Promise<string>
  expire: (proposalId: number) => Promise<string>
  txPending: boolean
  lastTxHash: string | null
  lastTxError: string | null

  // Manual refresh
  refresh: () => Promise<void>
}

const POLL_INTERVAL = 12_000 // 12 seconds (~2 Juno blocks)

export function useChainClient(contractAddr?: string): ChainState {
  // Wallet state
  const [walletAddress, setWalletAddress] = useState<string | null>(getWalletAddress())
  const [walletName, setWalletName] = useState<string | null>(null)
  const [walletBalance, setWalletBalance] = useState('0')
  const [walletError, setWalletError] = useState<string | null>(null)

  // Chain data
  const [config, setConfig] = useState<DaoConfig | null>(null)
  const [proposals, setProposals] = useState<DaoProposal[]>([])
  const [attestations, setAttestations] = useState<unknown[]>([])
  const [loading, setLoading] = useState(true)
  const [lastFetched, setLastFetched] = useState<number | null>(null)
  const [chainError, setChainError] = useState<string | null>(null)

  // TX state
  const [txPending, setTxPending] = useState(false)
  const [lastTxHash, setLastTxHash] = useState<string | null>(null)
  const [lastTxError, setLastTxError] = useState<string | null>(null)

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // ── Fetch all chain data ──
  const fetchAll = useCallback(async () => {
    try {
      const [cfg, props, atts] = await Promise.all([
        queryConfig(contractAddr),
        queryProposals(contractAddr),
        queryAttestations(contractAddr).catch(() => []),
      ])
      setConfig(cfg)
      setProposals(props)
      setAttestations(atts)
      setLastFetched(Date.now())
      setChainError(null)

      // Fetch wallet balance if connected
      const addr = getWalletAddress()
      if (addr) {
        const bal = await queryBalance(addr).catch(() => '0')
        setWalletBalance(bal)
      }
    } catch (err) {
      setChainError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [contractAddr])

  // ── Initial fetch + polling ──
  useEffect(() => {
    fetchAll()
    pollRef.current = setInterval(fetchAll, POLL_INTERVAL)
    return () => {
      if (pollRef.current) clearInterval(pollRef.current)
    }
  }, [fetchAll])

  // ── Wallet connect ──
  const connectWallet = useCallback(async () => {
    try {
      setWalletError(null)
      const { address, name } = await connectKeplr()
      setWalletAddress(address)
      setWalletName(name)
      const bal = await queryBalance(address).catch(() => '0')
      setWalletBalance(bal)
    } catch (err) {
      setWalletError(err instanceof Error ? err.message : String(err))
    }
  }, [])

  const disconnectWalletFn = useCallback(() => {
    disconnectWallet()
    setWalletAddress(null)
    setWalletName(null)
    setWalletBalance('0')
  }, [])

  // ── TX wrapper ──
  const withTx = useCallback(
    async (fn: () => Promise<{ transactionHash: string }>) => {
      setTxPending(true)
      setLastTxError(null)
      setLastTxHash(null)
      try {
        const result = await fn()
        setLastTxHash(result.transactionHash)
        // Refresh after successful TX
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
  const vote = useCallback(
    (proposalId: number, option: VoteOption) =>
      withTx(() => execCastVote(proposalId, option, contractAddr)),
    [withTx, contractAddr],
  )

  const execute = useCallback(
    (proposalId: number) =>
      withTx(() => execExecuteProposal(proposalId, contractAddr)),
    [withTx, contractAddr],
  )

  const propose = useCallback(
    (kind: Record<string, unknown>) =>
      withTx(() => execCreateProposal(kind, contractAddr)),
    [withTx, contractAddr],
  )

  const expire = useCallback(
    (proposalId: number) =>
      withTx(() => execExpireProposal(proposalId, contractAddr)),
    [withTx, contractAddr],
  )

  return {
    walletAddress,
    walletName,
    walletConnected: isWalletConnected(),
    walletBalance,
    connectWallet,
    disconnectWalletFn,
    walletError,
    config,
    proposals,
    attestations,
    loading,
    lastFetched,
    chainError,
    vote,
    execute,
    propose,
    expire,
    txPending,
    lastTxHash,
    lastTxError,
    refresh: fetchAll,
  }
}
