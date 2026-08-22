// ── FeePay Panel — Gasless transaction pool monitor ──
//
// Shows live FeePay module status from uni-7: enabled/disabled, registered
// contracts, pool balances, wallet limits. Visualizes the test results from
// Aug 21, 2026: registration + funding + pool accounting all verified on v30,
// gasless tx blocked by GlobalFee ante handler ordering (v31 fix).

import { useState } from 'react'
import {
  Fuel, CheckCircle2, XCircle, RefreshCw, Wallet, Database, AlertTriangle,
  TrendingDown, Layers, Activity,
} from 'lucide-react'
import { useFeePayLive } from '../hooks/useFeePayLive'

function formatAmount(amount: string): string {
  const n = BigInt(amount || '0')
  const whole = n / 1_000_000n
  const frac = n % 1_000_000n
  const fracStr = frac.toString().padStart(6, '0').replace(/0+$/, '')
  return fracStr ? `${whole}.${fracStr}` : whole.toString()
}

function shortAddr(addr: string): string {
  if (addr.length <= 16) return addr
  return `${addr.slice(0, 8)}…${addr.slice(-6)}`
}

function PoolBar({ balance, maxBalance }: { balance: bigint; maxBalance: bigint }) {
  const pct = maxBalance > 0n ? Number((balance * 100n) / maxBalance) : 0
  const color = pct > 50 ? '#00d4aa' : pct > 20 ? '#ffb84d' : '#ff4d6a'
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-full" style={{ background: 'rgba(255,255,255,0.06)' }}>
      <div
        className="h-full rounded-full transition-all"
        style={{ width: `${Math.max(pct, 2)}%`, background: color }}
      />
    </div>
  )
}

export function FeePayPanel() {
  const { params, registeredContracts, contractDetails, loading, error, lastFetched, refresh } = useFeePayLive()
  const [selectedContract, setSelectedContract] = useState<string | null>(null)

  const enabled = params?.enableFeePay ?? false
  const maxBalance = contractDetails.reduce((max, c) => {
    const b = BigInt(c.balance || '0')
    return b > max ? b : max
  }, 0n)

  const focused = contractDetails.find((c) => c.contractAddress === selectedContract) ?? contractDetails[0] ?? null

  return (
    <div className="relative flex-1 overflow-y-auto p-5" style={{ background: '#050510' }}>
      <div
        className="pointer-events-none absolute inset-0 opacity-20"
        style={{ background: 'radial-gradient(circle at 30% 0%, rgba(255,107,74,0.15), transparent 55%)' }}
      />

      <div className="relative mx-auto max-w-5xl">
        {/* Header */}
        <header className="mb-5 flex items-center justify-between">
          <div>
            <h2 className="flex items-center gap-2 text-sm font-semibold" style={{ color: '#f0eff8' }}>
              <Fuel className="h-4 w-4" style={{ color: '#ff6b4a' }} />
              FeePay
            </h2>
            <p className="mt-0.5 text-[11px]" style={{ color: '#6b6a8a' }}>
              Gasless transaction pools on uni-7 — registration, funding, and pool accounting verified Aug 21, 2026
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => refresh()}
              className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-semibold transition"
              style={{ color: '#6b6a8a', background: 'rgba(255,255,255,0.03)' }}
            >
              <RefreshCw className={`h-3 w-3 ${loading ? 'animate-spin' : ''}`} />
              Refresh
            </button>
          </div>
        </header>

        {/* Module status banner */}
        <div
          className="mb-4 flex items-center gap-3 rounded-xl p-4"
          style={{
            background: enabled ? 'rgba(0,212,170,0.06)' : 'rgba(255,77,106,0.06)',
            border: `1px solid ${enabled ? 'rgba(0,212,170,0.2)' : 'rgba(255,77,106,0.2)'}`,
          }}
        >
          {enabled ? (
            <CheckCircle2 className="h-5 w-5 flex-shrink-0" style={{ color: '#00d4aa' }} />
          ) : (
            <XCircle className="h-5 w-5 flex-shrink-0" style={{ color: '#ff4d6a' }} />
          )}
          <div className="flex-1">
            <div className="text-[12px] font-semibold" style={{ color: enabled ? '#00d4aa' : '#ff4d6a' }}>
              FeePay Module {enabled ? 'ENABLED' : 'DISABLED'}
            </div>
            <div className="text-[10px]" style={{ color: '#6b6a8a' }}>
              {enabled
                ? 'Gasless transactions are supported. Pool registration and funding confirmed on v30.'
                : 'FeePay module is not active on this chain.'}
            </div>
          </div>
          {lastFetched && (
            <span className="text-[9px]" style={{ color: '#4a4a6a' }}>
              {new Date(lastFetched).toLocaleTimeString()}
            </span>
          )}
        </div>

        {/* v30 limitation banner */}
        <div
          className="mb-4 flex items-start gap-3 rounded-xl p-3"
          style={{ background: 'rgba(255,184,77,0.05)', border: '1px solid rgba(255,184,77,0.15)' }}
        >
          <AlertTriangle className="mt-0.5 h-4 w-4 flex-shrink-0" style={{ color: '#ffb84d' }} />
          <div>
            <div className="text-[11px] font-semibold" style={{ color: '#ffb84d' }}>
              v30: Gasless tx blocked by GlobalFee ante handler ordering
            </div>
            <div className="mt-0.5 text-[10px]" style={{ color: '#8a89a6' }}>
              GlobalFee rejects zero-fee txs before FeePay can escrow. Registration, funding, and pool
              accounting all work. v31 PR #1223 reorders the ante chain so FeePay intercepts first.
            </div>
          </div>
        </div>

        {error && (
          <div
            className="mb-4 rounded-xl p-3 text-[11px]"
            style={{ background: 'rgba(255,77,106,0.08)', border: '1px solid rgba(255,77,106,0.2)', color: '#ff4d6a' }}
          >
            Query error: {error}
          </div>
        )}

        {/* Stats row */}
        <div className="mb-4 grid grid-cols-3 gap-3">
          <div
            className="rounded-xl p-3"
            style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}
          >
            <div className="mb-1 flex items-center gap-1.5">
              <Layers className="h-3 w-3" style={{ color: '#ff6b4a' }} />
              <span className="text-[9px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
                Registered
              </span>
            </div>
            <div className="text-lg font-bold" style={{ color: '#f0eff8' }}>
              {registeredContracts.length}
            </div>
            <div className="text-[9px]" style={{ color: '#4a4a6a' }}>contracts</div>
          </div>

          <div
            className="rounded-xl p-3"
            style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}
          >
            <div className="mb-1 flex items-center gap-1.5">
              <Database className="h-3 w-3" style={{ color: '#ff6b4a' }} />
              <span className="text-[9px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
                Total Pooled
              </span>
            </div>
            <div className="text-lg font-bold" style={{ color: '#f0eff8' }}>
              {formatAmount(
                contractDetails.reduce((sum, c) => sum + BigInt(c.balance || '0'), 0n).toString()
              )}
            </div>
            <div className="text-[9px]" style={{ color: '#4a4a6a' }}>JUNOX</div>
          </div>

          <div
            className="rounded-xl p-3"
            style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}
          >
            <div className="mb-1 flex items-center gap-1.5">
              <Wallet className="h-3 w-3" style={{ color: '#ff6b4a' }} />
              <span className="text-[9px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
                Wallet Limit
              </span>
            </div>
            <div className="text-lg font-bold" style={{ color: '#f0eff8' }}>
              {focused?.walletLimit ?? '—'}
            </div>
            <div className="text-[9px]" style={{ color: '#4a4a6a' }}>txs/wallet</div>
          </div>
        </div>

        {/* Contract list */}
        {contractDetails.length === 0 && !loading ? (
          <div
            className="flex flex-col items-center gap-2 rounded-xl py-12 text-center"
            style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}
          >
            <Fuel className="h-6 w-6" style={{ color: '#4a4a6a' }} />
            <span className="text-[11px] font-semibold" style={{ color: '#6b6a8a' }}>
              No FeePay-registered contracts found
            </span>
            <span className="max-w-[280px] text-[10px]" style={{ color: '#4a4a6a' }}>
              Run deploy/test-feepay-testnet-v2.cjs to register a contract and fund a pool.
            </span>
          </div>
        ) : (
          <div className="space-y-2">
            {contractDetails.map((c) => {
              const isSelected = focused?.contractAddress === c.contractAddress
              const balance = BigInt(c.balance || '0')
              return (
                <button
                  key={c.contractAddress}
                  onClick={() => setSelectedContract(c.contractAddress)}
                  className="w-full rounded-xl p-3 text-left transition-all"
                  style={isSelected ? {
                    background: 'rgba(255,107,74,0.06)',
                    border: '1px solid rgba(255,107,74,0.25)',
                  } : {
                    background: 'rgba(255,255,255,0.015)',
                    border: '1px solid rgba(255,255,255,0.05)',
                  }}
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <Activity className="h-3 w-3" style={{ color: '#ff6b4a' }} />
                      <span className="font-mono text-[11px] font-medium" style={{ color: '#f0eff8' }}>
                        {shortAddr(c.contractAddress)}
                      </span>
                    </div>
                    <div className="flex items-center gap-3">
                      <div className="text-right">
                        <div className="text-[12px] font-bold" style={{ color: '#00d4aa' }}>
                          {formatAmount(c.balance)}
                        </div>
                        <div className="text-[8px] uppercase" style={{ color: '#4a4a6a' }}>JUNOX</div>
                      </div>
                    </div>
                  </div>
                  <div className="mt-2">
                    <PoolBar balance={balance} maxBalance={maxBalance} />
                  </div>
                  <div className="mt-1.5 flex items-center justify-between text-[9px]" style={{ color: '#6b6a8a' }}>
                    <span>Limit: {c.walletLimit} txs/wallet</span>
                    {balance > 0n && (
                      <span className="flex items-center gap-1">
                        <TrendingDown className="h-2.5 w-2.5" />
                        Pool active
                      </span>
                    )}
                  </div>
                </button>
              )
            })}
          </div>
        )}

        {/* Test results summary */}
        <div
          className="mt-5 rounded-xl p-4"
          style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}
        >
          <div className="mb-2 flex items-center gap-1.5">
            <Activity className="h-3 w-3" style={{ color: '#ff6b4a' }} />
            <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
              Test Results — Aug 21, 2026
            </span>
          </div>
          <div className="grid grid-cols-2 gap-2 text-[10px]">
            <div className="flex items-center gap-1.5">
              <CheckCircle2 className="h-3 w-3" style={{ color: '#00d4aa' }} />
              <span style={{ color: '#c0bfd8' }}>FeePay enabled</span>
            </div>
            <div className="flex items-center gap-1.5">
              <CheckCircle2 className="h-3 w-3" style={{ color: '#00d4aa' }} />
              <span style={{ color: '#c0bfd8' }}>Contract registered</span>
            </div>
            <div className="flex items-center gap-1.5">
              <CheckCircle2 className="h-3 w-3" style={{ color: '#00d4aa' }} />
              <span style={{ color: '#c0bfd8' }}>Pool funded (1M ujunox)</span>
            </div>
            <div className="flex items-center gap-1.5">
              <CheckCircle2 className="h-3 w-3" style={{ color: '#00d4aa' }} />
              <span style={{ color: '#c0bfd8' }}>Normal tx succeeded</span>
            </div>
            <div className="flex items-center gap-1.5">
              <XCircle className="h-3 w-3" style={{ color: '#ff4d6a' }} />
              <span style={{ color: '#c0bfd8' }}>Gasless tx blocked (v30)</span>
            </div>
            <div className="flex items-center gap-1.5">
              <AlertTriangle className="h-3 w-3" style={{ color: '#ffb84d' }} />
              <span style={{ color: '#c0bfd8' }}>Fixed in v31 PR #1223</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
