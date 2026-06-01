// ── Deployed-contract registry (read-only) ──
//
// Surfaces every live uni-7 contract address from `chain-config.ts` — including
// moultbook-v0 and ibc-task-host, which the DAO wizard references indirectly
// but were not otherwise visible in the UI. Each row links to the STAVR LCD
// contract-info endpoint (a verified-live REST path), so addresses are
// independently checkable without trusting a third-party explorer.

import { useState } from 'react'
import { Copy, Check, ExternalLink } from 'lucide-react'
import { CHAIN_CONFIG, CONTRACTS } from '../lib/chain-config'

type ContractRow = {
  key: keyof typeof CONTRACTS
  label: string
  desc: string
  codeId?: string
}

type ContractGroup = {
  layer: string
  rows: ContractRow[]
}

const GROUPS: ContractGroup[] = [
  {
    layer: 'Coordination',
    rows: [
      {
        key: 'agentCompany',
        label: 'Agent Company',
        desc: 'DAO core — proposals, members, attestations, child-contract registry',
      },
    ],
  },
  {
    layer: 'DeFi',
    rows: [
      { key: 'junoswapFactory', label: 'JunoSwap Factory', desc: 'AMM pair factory' },
      { key: 'junoswapPairJunoUsdc', label: 'JunoSwap Pair · JUNO/USDC', desc: 'Constant-product pool' },
      { key: 'junoswapPairJunoStake', label: 'JunoSwap Pair · JUNO/STAKE', desc: 'Constant-product pool' },
    ],
  },
  {
    layer: 'Privacy',
    rows: [
      {
        key: 'moultbookV0',
        label: 'Moultbook v0',
        desc: 'Anonymous publishing + ZK skill endorsement (ADR-005)',
        codeId: '76',
      },
    ],
  },
  {
    layer: 'Bridges',
    rows: [
      {
        key: 'ibcTaskHost',
        label: 'IBC Task Host',
        desc: 'Cross-chain task escrow via ICS-20 / packet-forward middleware',
        codeId: '77',
      },
    ],
  },
]

function truncAddr(addr: string) {
  return addr.length > 24 ? `${addr.slice(0, 14)}…${addr.slice(-8)}` : addr
}

function lcdUrl(addr: string) {
  return `${CHAIN_CONFIG.rest}/cosmwasm/wasm/v1/contract/${addr}`
}

function ContractCard({ row }: { row: ContractRow }) {
  const addr = CONTRACTS[row.key]
  const [copied, setCopied] = useState(false)

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(addr)
      setCopied(true)
      setTimeout(() => setCopied(false), 1200)
    } catch {
      /* clipboard unavailable — non-fatal */
    }
  }

  return (
    <div
      className="rounded-lg p-3"
      style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.05)' }}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="text-xs font-semibold" style={{ color: '#f0eff8' }}>
            {row.label}
          </span>
          {row.codeId && (
            <span
              className="rounded px-1.5 py-0.5 text-[9px] font-mono"
              style={{ background: 'rgba(255,107,74,0.1)', color: '#ff6b4a' }}
            >
              code {row.codeId}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={copy}
            title="Copy address"
            className="flex items-center gap-1 rounded px-1.5 py-0.5 transition hover:bg-white/5"
            style={{ color: copied ? '#00d4aa' : '#6b6a8a' }}
          >
            {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
          </button>
          <a
            href={lcdUrl(addr)}
            target="_blank"
            rel="noreferrer"
            title="View on STAVR LCD"
            className="flex items-center gap-1 rounded px-1.5 py-0.5 transition hover:bg-white/5"
            style={{ color: '#6b6a8a' }}
          >
            <ExternalLink className="h-3 w-3" />
          </a>
        </div>
      </div>
      <p className="mt-1 text-[10px] leading-snug" style={{ color: '#8a89a6' }}>
        {row.desc}
      </p>
      <code
        className="mt-1.5 block break-all font-mono text-[10px]"
        style={{ color: '#c0bfd8' }}
        title={addr}
      >
        {truncAddr(addr)}
      </code>
    </div>
  )
}

export function ContractsPanel() {
  return (
    <div className="flex-1 overflow-y-auto p-5">
      <div className="mx-auto max-w-3xl">
        <header className="mb-4">
          <h2 className="text-sm font-semibold" style={{ color: '#f0eff8' }}>
            Deployed Contracts
          </h2>
          <p className="mt-0.5 text-[11px]" style={{ color: '#6b6a8a' }}>
            Live on {CHAIN_CONFIG.chainName} ({CHAIN_CONFIG.chainId}). Addresses link to the
            STAVR LCD contract-info endpoint for independent verification.
          </p>
        </header>

        <div className="space-y-5">
          {GROUPS.map((group) => (
            <section key={group.layer}>
              <h3
                className="mb-2 text-[10px] font-semibold uppercase tracking-wider"
                style={{ color: '#ff6b4a' }}
              >
                {group.layer}
              </h3>
              <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                {group.rows.map((row) => (
                  <ContractCard key={row.key} row={row} />
                ))}
              </div>
            </section>
          ))}
        </div>
      </div>
    </div>
  )
}
