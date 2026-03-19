import { useState } from 'react'
import {
  ArrowLeftRight, ArrowDown, Droplets, TrendingUp, Plus, Minus,
  ShieldCheck, AlertTriangle, RefreshCw, ChevronDown, Info,
  Wallet, BarChart3, Activity, Zap, Loader2, Wifi, WifiOff, Check, X,
} from 'lucide-react'
import type {
  DexPoolState, DexSimulation, DexPairInfo, DexSwapAttestation, DexLpPosition,
} from '../types'
import { useDexClient } from '../hooks/useDexClient'
import type { DexState } from '../hooks/useDexClient'

// ── Mock data (until contracts deployed on uni-7) ──

const MOCK_PAIRS: DexPairInfo[] = [
  {
    pair_addr: 'juno1pair_juno_usdc_placeholder',
    token_a: { native: 'ujuno' },
    token_b: { native: 'uusdc' },
    fee_bps: 30,
    factory: 'juno1factory_placeholder',
    junoclaw_contract: 'juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6',
  },
]

const MOCK_POOL: DexPoolState = {
  pair_addr: 'juno1pair_juno_usdc_placeholder',
  reserve_a: '5000000',
  reserve_b: '25000000',
  total_lp_shares: '11180339',
  total_swaps: 42,
  total_volume_a: '15000000',
  total_volume_b: '75000000',
  price_a_per_b: '5.000000',
  price_b_per_a: '0.200000',
  token_a: { native: 'ujuno' },
  token_b: { native: 'uusdc' },
}

const MOCK_ATTESTATIONS: DexSwapAttestation[] = [
  {
    pair: 'juno1pair_juno_usdc_placeholder',
    offer_asset: 'ujuno',
    offer_amount: '100000',
    return_amount: '480384',
    effective_price: '4.80384000',
    price_impact_pct: '1.2340',
    manipulation_flag: false,
    verified: true,
    attestation_hash: '945a53c5c1aab2e99432e659d47633da491fffc399d95cbce66b8e88fae5c0e8',
    timestamp: new Date(Date.now() - 120000).toISOString(),
  },
  {
    pair: 'juno1pair_juno_usdc_placeholder',
    offer_asset: 'uusdc',
    offer_amount: '500000',
    return_amount: '98039',
    effective_price: '0.19607800',
    price_impact_pct: '0.4120',
    manipulation_flag: false,
    verified: true,
    attestation_hash: '7b3f2a1e8c4d5f6a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a',
    timestamp: new Date(Date.now() - 3600000).toISOString(),
  },
]

const MOCK_LP: DexLpPosition = {
  pair_addr: 'juno1pair_juno_usdc_placeholder',
  lp_shares: '2236067',
  share_of_pool_pct: '20.00',
  value_a: '1000000',
  value_b: '5000000',
}

// ── Helpers ──

function denomLabel(denom: string): string {
  if (denom === 'ujuno' || denom === 'ujunox') return 'JUNO'
  if (denom === 'uusdc') return 'USDC'
  if (denom === 'uakt') return 'AKT'
  return denom.replace(/^u/, '').toUpperCase()
}

function formatAmount(raw: string, decimals = 6): string {
  const n = Number(raw) / Math.pow(10, decimals)
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(2) + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(2) + 'K'
  return n.toFixed(decimals > 2 ? 2 : decimals)
}

function formatPrice(price: string): string {
  const n = parseFloat(price)
  return n >= 1 ? n.toFixed(4) : n.toFixed(6)
}

// ── Sub-tabs ──

type DexSubTab = 'swap' | 'pool' | 'liquidity' | 'attestations'

// ── Main DexPanel ──

export function DexPanel() {
  const [subTab, setSubTab] = useState<DexSubTab>('swap')
  const dex = useDexClient()

  // Use live data, fall back to mock
  const isLive = dex.activePool !== null && !dex.error
  const pair = dex.activePair ?? MOCK_PAIRS[0]
  const pool = dex.activePool ?? MOCK_POOL
  const lp = dex.lpPosition ?? MOCK_LP

  const subTabs: { id: DexSubTab; label: string; icon: React.ReactNode }[] = [
    { id: 'swap',         label: 'Swap',         icon: <ArrowLeftRight className="h-3 w-3" /> },
    { id: 'pool',         label: 'Pool',         icon: <BarChart3      className="h-3 w-3" /> },
    { id: 'liquidity',    label: 'Liquidity',    icon: <Droplets       className="h-3 w-3" /> },
    { id: 'attestations', label: 'Attestations', icon: <ShieldCheck    className="h-3 w-3" /> },
  ]

  return (
    <div className="flex flex-1 flex-col bg-[#06060f] overflow-hidden">
      {/* Header */}
      <div className="px-6 py-3" style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-xl"
                 style={{ background: 'rgba(255,107,74,0.1)', border: '1px solid rgba(255,107,74,0.2)' }}>
              <ArrowLeftRight className="h-4.5 w-4.5 text-juno-400" />
            </div>
            <div>
              <div className="text-sm font-bold text-[#f0eff8]">Junoswap v2</div>
              <div className="text-[10px] text-[#6b6a8a]">
                TEE-attested AMM · {denomLabel(pair.token_a.native)}/{denomLabel(pair.token_b.native)}
                <span className="ml-2 text-juno-400">1 {denomLabel(pair.token_a.native)} = {formatPrice(pool.price_a_per_b)} {denomLabel(pair.token_b.native)}</span>
              </div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            {/* Pair selector */}
            {dex.pairs.length > 1 && (
              <select
                value={dex.activePairAddr}
                onChange={(e) => dex.setActivePair(e.target.value)}
                className="rounded-lg px-2 py-1.5 text-[10px] font-semibold text-[#c0bfd8] outline-none"
                style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.08)' }}
              >
                {dex.pairs.map(p => (
                  <option key={p.pair.pair_addr} value={p.pair.pair_addr}>
                    {denomLabel(p.pair.token_a.native)}/{denomLabel(p.pair.token_b.native)}
                  </option>
                ))}
              </select>
            )}
            <span className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-semibold"
                  style={{ color: '#22c55e', background: 'rgba(34,197,94,0.08)', border: '1px solid rgba(34,197,94,0.15)' }}>
              <Activity className="h-2.5 w-2.5" />
              {pool.total_swaps} swaps
            </span>
            <span className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-semibold"
                  style={{ color: isLive ? '#22c55e' : '#fbbf24', background: isLive ? 'rgba(34,197,94,0.08)' : 'rgba(251,191,36,0.08)', border: `1px solid ${isLive ? 'rgba(34,197,94,0.15)' : 'rgba(251,191,36,0.15)'}` }}>
              {isLive ? <Wifi className="h-2.5 w-2.5" /> : dex.loading ? <Loader2 className="h-2.5 w-2.5 animate-spin" /> : <WifiOff className="h-2.5 w-2.5" />}
              {isLive ? 'Live' : dex.loading ? 'Loading...' : 'Offline'}
            </span>
          </div>
        </div>

        {/* Sub-tabs */}
        <div className="flex gap-1 mt-3">
          {subTabs.map((tab) => {
            const isActive = subTab === tab.id
            return (
              <button
                key={tab.id}
                onClick={() => setSubTab(tab.id)}
                className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[11px] font-medium transition-all"
                style={isActive ? {
                  color: '#ff6b4a',
                  background: 'rgba(255,107,74,0.1)',
                  border: '1px solid rgba(255,107,74,0.2)',
                } : {
                  color: '#6b6a8a',
                  background: 'transparent',
                  border: '1px solid transparent',
                }}
              >
                {tab.icon}
                {tab.label}
              </button>
            )
          })}
        </div>
      </div>

      {/* TX feedback */}
      {dex.lastTxHash && (
        <div className="mx-6 mt-3 flex items-center gap-2 rounded-lg px-3 py-2 text-[10px]"
             style={{ background: 'rgba(52,211,153,0.06)', border: '1px solid rgba(52,211,153,0.15)' }}>
          <Check className="h-3 w-3 text-green-400" />
          <span className="text-green-400">TX confirmed:</span>
          <code className="text-[#c0bfd8] font-mono truncate">{dex.lastTxHash}</code>
        </div>
      )}
      {dex.lastTxError && (
        <div className="mx-6 mt-3 flex items-center gap-2 rounded-lg px-3 py-2 text-[10px]"
             style={{ background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)' }}>
          <X className="h-3 w-3 text-red-400" />
          <span className="text-red-400 truncate">{dex.lastTxError}</span>
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-6 py-5">
        {subTab === 'swap'         && <SwapView pair={pair} pool={pool} dex={dex} />}
        {subTab === 'pool'         && <PoolView pool={pool} />}
        {subTab === 'liquidity'    && <LiquidityView pool={pool} lp={lp} dex={dex} />}
        {subTab === 'attestations' && <AttestationsView attestations={MOCK_ATTESTATIONS} />}
      </div>
    </div>
  )
}

// ── Swap View ──

function SwapView({ pair, pool, dex }: { pair: DexPairInfo; pool: DexPoolState; dex: DexState }) {
  const [offerAmount, setOfferAmount] = useState('')
  const [direction, setDirection] = useState<'a_to_b' | 'b_to_a'>('a_to_b')
  const [swapping, setSwapping] = useState(false)

  const offerToken = direction === 'a_to_b' ? pair.token_a : pair.token_b
  const returnToken = direction === 'a_to_b' ? pair.token_b : pair.token_a

  // Simple XYK simulation (matches contract logic)
  const simulation: DexSimulation | null = (() => {
    const amt = parseFloat(offerAmount || '0')
    if (amt <= 0) return null

    const resOffer = direction === 'a_to_b' ? Number(pool.reserve_a) : Number(pool.reserve_b)
    const resReturn = direction === 'a_to_b' ? Number(pool.reserve_b) : Number(pool.reserve_a)
    const offerRaw = amt * 1_000_000

    const returnAmt = Math.floor((offerRaw * resReturn) / (resOffer + offerRaw))
    const idealReturn = Math.floor((offerRaw * resReturn) / resOffer)
    const spread = idealReturn - returnAmt
    const fee = Math.floor(returnAmt * pair.fee_bps / 10000)
    const netReturn = returnAmt - fee

    return {
      return_amount: String(netReturn),
      spread_amount: String(spread),
      fee_amount: String(fee),
    }
  })()

  const priceImpact = simulation ? (() => {
    const offerRaw = parseFloat(offerAmount) * 1_000_000
    const resOffer = direction === 'a_to_b' ? Number(pool.reserve_a) : Number(pool.reserve_b)
    return ((offerRaw / resOffer) * 100).toFixed(2)
  })() : '0.00'

  return (
    <div className="mx-auto max-w-md">
      {/* Swap card */}
      <div className="rounded-2xl overflow-hidden"
           style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>

        {/* From */}
        <div className="p-4">
          <label className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-2 block">
            You Pay
          </label>
          <div className="flex items-center gap-3">
            <input
              type="number"
              value={offerAmount}
              onChange={(e) => setOfferAmount(e.target.value)}
              placeholder="0.00"
              className="flex-1 bg-transparent text-2xl font-bold text-[#f0eff8] outline-none placeholder-[#3a3a5a]"
            />
            <div className="flex items-center gap-1.5 rounded-xl px-3 py-2"
                 style={{ background: 'rgba(255,107,74,0.08)', border: '1px solid rgba(255,107,74,0.15)' }}>
              <Wallet className="h-3 w-3 text-juno-400" />
              <span className="text-xs font-bold text-[#f0eff8]">{denomLabel(offerToken.native)}</span>
              <ChevronDown className="h-3 w-3 text-[#6b6a8a]" />
            </div>
          </div>
        </div>

        {/* Swap direction button */}
        <div className="flex justify-center -my-3 relative z-10">
          <button
            onClick={() => setDirection(d => d === 'a_to_b' ? 'b_to_a' : 'a_to_b')}
            className="flex h-8 w-8 items-center justify-center rounded-xl transition-all hover:rotate-180 hover:bg-juno-500/20"
            style={{ background: '#06060f', border: '1px solid rgba(255,107,74,0.25)' }}
          >
            <ArrowDown className="h-4 w-4 text-juno-400" />
          </button>
        </div>

        {/* To */}
        <div className="p-4" style={{ borderTop: '1px solid rgba(255,255,255,0.04)' }}>
          <label className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-2 block">
            You Receive
          </label>
          <div className="flex items-center gap-3">
            <div className="flex-1 text-2xl font-bold text-[#c0bfd8]">
              {simulation ? formatAmount(simulation.return_amount) : '0.00'}
            </div>
            <div className="flex items-center gap-1.5 rounded-xl px-3 py-2"
                 style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.06)' }}>
              <Wallet className="h-3 w-3 text-[#6b6a8a]" />
              <span className="text-xs font-bold text-[#f0eff8]">{denomLabel(returnToken.native)}</span>
              <ChevronDown className="h-3 w-3 text-[#6b6a8a]" />
            </div>
          </div>
        </div>

        {/* Swap details */}
        {simulation && (
          <div className="px-4 pb-4 space-y-1.5">
            <div className="rounded-xl p-3 space-y-1.5"
                 style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.04)' }}>
              <div className="flex justify-between text-[11px]">
                <span className="text-[#6b6a8a]">Rate</span>
                <span className="text-[#c0bfd8]">
                  1 {denomLabel(offerToken.native)} = {
                    formatPrice(String(Number(simulation.return_amount) / (parseFloat(offerAmount) * 1_000_000)))
                  } {denomLabel(returnToken.native)}
                </span>
              </div>
              <div className="flex justify-between text-[11px]">
                <span className="text-[#6b6a8a]">Fee ({pair.fee_bps / 100}%)</span>
                <span className="text-[#c0bfd8]">{formatAmount(simulation.fee_amount)} {denomLabel(returnToken.native)}</span>
              </div>
              <div className="flex justify-between text-[11px]">
                <span className="text-[#6b6a8a]">Spread</span>
                <span className="text-[#c0bfd8]">{formatAmount(simulation.spread_amount)} {denomLabel(returnToken.native)}</span>
              </div>
              <div className="flex justify-between text-[11px]">
                <span className="text-[#6b6a8a]">Price Impact</span>
                <span className={Number(priceImpact) > 3 ? 'text-red-400 font-semibold' : 'text-green-400'}>
                  {priceImpact}%
                </span>
              </div>
            </div>

            {Number(priceImpact) > 3 && (
              <div className="flex items-center gap-2 rounded-xl px-3 py-2 text-[11px]"
                   style={{ background: 'rgba(239,68,68,0.08)', border: '1px solid rgba(239,68,68,0.15)', color: '#f87171' }}>
                <AlertTriangle className="h-3 w-3 flex-shrink-0" />
                High price impact! Consider reducing your swap amount.
              </div>
            )}
          </div>
        )}

        {/* Swap button */}
        <div className="px-4 pb-4">
          <button
            disabled={!simulation || swapping || dex.txPending}
            onClick={async () => {
              if (!simulation) return
              setSwapping(true)
              try {
                const rawOffer = String(Math.floor(parseFloat(offerAmount) * 1_000_000))
                const minReturn = simulation ? String(Math.floor(Number(simulation.return_amount) * 0.97)) : undefined
                await dex.swap(offerToken.native, rawOffer, minReturn)
                setOfferAmount('')
              } catch { /* error shown via dex.lastTxError */ }
              finally { setSwapping(false) }
            }}
            className="w-full rounded-xl py-3.5 text-sm font-bold text-white transition-all hover:opacity-90 active:scale-[0.98] disabled:opacity-30 disabled:cursor-not-allowed"
            style={{ background: simulation ? 'linear-gradient(135deg, #ff6b4a, #e84e2c)' : '#1a1a2e' }}
          >
            {swapping ? (
              <span className="flex items-center justify-center gap-2">
                <Loader2 className="h-4 w-4 animate-spin" />
                Swapping...
              </span>
            ) : simulation ? (
              <span className="flex items-center justify-center gap-2">
                <Zap className="h-4 w-4" />
                Swap {denomLabel(offerToken.native)} → {denomLabel(returnToken.native)}
              </span>
            ) : (
              'Enter an amount'
            )}
          </button>
          <p className="mt-2 text-center text-[10px] text-[#6b6a8a]">
            <ShieldCheck className="inline h-2.5 w-2.5 mr-0.5 text-juno-400" />
            Every swap is TEE-attested by WAVS operator
          </p>
        </div>
      </div>
    </div>
  )
}

// ── Pool View ──

function PoolView({ pool }: { pool: DexPoolState }) {
  const tokenA = denomLabel(pool.token_a.native)
  const tokenB = denomLabel(pool.token_b.native)

  const stats = [
    { label: `${tokenA} Reserve`, value: formatAmount(pool.reserve_a), color: '#ff6b4a' },
    { label: `${tokenB} Reserve`, value: formatAmount(pool.reserve_b), color: '#60a5fa' },
    { label: 'LP Shares', value: formatAmount(pool.total_lp_shares), color: '#a78bfa' },
    { label: 'Total Swaps', value: String(pool.total_swaps), color: '#22c55e' },
    { label: `${tokenA} Volume`, value: formatAmount(pool.total_volume_a), color: '#fbbf24' },
    { label: `${tokenB} Volume`, value: formatAmount(pool.total_volume_b), color: '#fbbf24' },
  ]

  return (
    <div className="max-w-2xl mx-auto space-y-4">
      {/* Price banner */}
      <div className="rounded-2xl p-5" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
        <div className="flex items-center justify-between">
          <div>
            <div className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-1">
              {tokenA} / {tokenB} Price
            </div>
            <div className="text-3xl font-bold text-[#f0eff8]">{formatPrice(pool.price_a_per_b)}</div>
            <div className="text-xs text-[#6b6a8a] mt-1">
              1 {tokenB} = {formatPrice(pool.price_b_per_a)} {tokenA}
            </div>
          </div>
          <div className="flex flex-col items-end gap-1">
            <span className="flex items-center gap-1 text-[10px] font-semibold text-green-400">
              <TrendingUp className="h-3 w-3" />
              TEE-attested price
            </span>
            <span className="text-[9px] text-[#4a4a6a]">
              Updated every swap
            </span>
          </div>
        </div>
      </div>

      {/* Stats grid */}
      <div className="grid grid-cols-3 gap-3">
        {stats.map((stat) => (
          <div key={stat.label} className="rounded-xl p-4"
               style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
            <div className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-1">
              {stat.label}
            </div>
            <div className="text-lg font-bold" style={{ color: stat.color }}>
              {stat.value}
            </div>
          </div>
        ))}
      </div>

      {/* Pool info */}
      <div className="rounded-xl p-4"
           style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
        <div className="flex items-center gap-2 text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-3">
          <Info className="h-3 w-3" />
          Pool Details
        </div>
        <div className="space-y-2 text-[11px]">
          <div className="flex justify-between">
            <span className="text-[#6b6a8a]">Pair Address</span>
            <span className="font-mono text-[#c0bfd8]">{pool.pair_addr.slice(0, 20)}...</span>
          </div>
          <div className="flex justify-between">
            <span className="text-[#6b6a8a]">Fee</span>
            <span className="text-[#c0bfd8]">0.30%</span>
          </div>
          <div className="flex justify-between">
            <span className="text-[#6b6a8a]">k (constant product)</span>
            <span className="font-mono text-[#c0bfd8]">
              {(BigInt(pool.reserve_a) * BigInt(pool.reserve_b)).toLocaleString()}
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}

// ── Liquidity View ──

function LiquidityView({ pool, lp, dex }: { pool: DexPoolState; lp: DexLpPosition; dex: DexState }) {
  const [mode, setMode] = useState<'provide' | 'withdraw'>('provide')
  const [amountA, setAmountA] = useState('')
  const [amountB, setAmountB] = useState('')
  const [withdrawPct, setWithdrawPct] = useState(50)
  const [providing, setProviding] = useState(false)
  const [withdrawing, setWithdrawing] = useState(false)

  const tokenA = denomLabel(pool.token_a.native)
  const tokenB = denomLabel(pool.token_b.native)

  // Auto-compute paired amount based on pool ratio
  const handleAmountAChange = (val: string) => {
    setAmountA(val)
    const n = parseFloat(val)
    if (!isNaN(n) && n > 0 && Number(pool.reserve_a) > 0) {
      const ratio = Number(pool.reserve_b) / Number(pool.reserve_a)
      setAmountB((n * ratio).toFixed(6))
    } else {
      setAmountB('')
    }
  }

  return (
    <div className="max-w-lg mx-auto space-y-4">
      {/* Your position */}
      <div className="rounded-2xl p-5" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
        <div className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-3">
          Your LP Position
        </div>
        <div className="grid grid-cols-2 gap-4">
          <div>
            <div className="text-lg font-bold text-[#f0eff8]">{formatAmount(lp.lp_shares)}</div>
            <div className="text-[10px] text-[#6b6a8a]">LP Shares ({lp.share_of_pool_pct}% of pool)</div>
          </div>
          <div className="text-right">
            <div className="text-xs text-[#c0bfd8]">
              {formatAmount(lp.value_a)} {tokenA} + {formatAmount(lp.value_b)} {tokenB}
            </div>
            <div className="text-[10px] text-[#6b6a8a]">Pooled value</div>
          </div>
        </div>
      </div>

      {/* Mode toggle */}
      <div className="flex rounded-xl overflow-hidden"
           style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
        <button
          onClick={() => setMode('provide')}
          className="flex-1 flex items-center justify-center gap-1.5 py-2.5 text-xs font-semibold transition-all"
          style={mode === 'provide' ? {
            color: '#22c55e',
            background: 'rgba(34,197,94,0.1)',
          } : { color: '#6b6a8a' }}
        >
          <Plus className="h-3 w-3" /> Provide
        </button>
        <button
          onClick={() => setMode('withdraw')}
          className="flex-1 flex items-center justify-center gap-1.5 py-2.5 text-xs font-semibold transition-all"
          style={mode === 'withdraw' ? {
            color: '#f87171',
            background: 'rgba(248,113,113,0.1)',
          } : { color: '#6b6a8a' }}
        >
          <Minus className="h-3 w-3" /> Withdraw
        </button>
      </div>

      {/* Provide form */}
      {mode === 'provide' && (
        <div className="rounded-2xl p-4 space-y-3"
             style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-1 block">
              {tokenA} Amount
            </label>
            <input
              type="number"
              value={amountA}
              onChange={(e) => handleAmountAChange(e.target.value)}
              placeholder="0.00"
              className="w-full rounded-xl px-3 py-2.5 text-sm font-mono text-[#f0eff8] placeholder-[#3a3a5a] outline-none"
              style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}
            />
          </div>
          <div className="flex justify-center">
            <Plus className="h-4 w-4 text-[#6b6a8a]" />
          </div>
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-1 block">
              {tokenB} Amount (auto-computed)
            </label>
            <input
              type="number"
              value={amountB}
              readOnly
              placeholder="0.00"
              className="w-full rounded-xl px-3 py-2.5 text-sm font-mono text-[#c0bfd8] placeholder-[#3a3a5a] outline-none cursor-not-allowed"
              style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}
            />
          </div>
          <button
            disabled={!amountA || parseFloat(amountA) <= 0 || providing || dex.txPending}
            onClick={async () => {
              if (!amountA || !amountB) return
              setProviding(true)
              try {
                const rawA = String(Math.floor(parseFloat(amountA) * 1_000_000))
                const rawB = String(Math.floor(parseFloat(amountB) * 1_000_000))
                await dex.provideLiquidity(pool.token_a.native, rawA, pool.token_b.native, rawB)
                setAmountA('')
                setAmountB('')
              } catch { /* error shown via dex.lastTxError */ }
              finally { setProviding(false) }
            }}
            className="w-full rounded-xl py-3 text-sm font-bold text-white transition-all hover:opacity-90 disabled:opacity-30 disabled:cursor-not-allowed"
            style={{ background: amountA ? 'linear-gradient(135deg, #22c55e, #16a34a)' : '#1a1a2e' }}
          >
            {providing ? (
              <span className="flex items-center justify-center gap-2">
                <Loader2 className="h-4 w-4 animate-spin" />
                Providing...
              </span>
            ) : (
              <span><Droplets className="inline h-4 w-4 mr-1" /> Provide Liquidity</span>
            )}
          </button>
        </div>
      )}

      {/* Withdraw form */}
      {mode === 'withdraw' && (
        <div className="rounded-2xl p-4 space-y-3"
             style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-2 block">
              Withdraw {withdrawPct}% of your LP
            </label>
            <input
              type="range"
              min={1} max={100}
              value={withdrawPct}
              onChange={(e) => setWithdrawPct(Number(e.target.value))}
              className="w-full accent-red-500"
            />
            <div className="flex justify-between mt-1">
              {[25, 50, 75, 100].map(pct => (
                <button key={pct}
                  onClick={() => setWithdrawPct(pct)}
                  className="rounded-lg px-2.5 py-1 text-[10px] font-semibold transition"
                  style={withdrawPct === pct ? {
                    color: '#f87171', background: 'rgba(248,113,113,0.15)',
                  } : { color: '#6b6a8a' }}
                >
                  {pct}%
                </button>
              ))}
            </div>
          </div>
          <div className="rounded-xl p-3 text-[11px] space-y-1"
               style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.04)' }}>
            <div className="flex justify-between">
              <span className="text-[#6b6a8a]">You receive</span>
              <span className="text-[#c0bfd8]">
                {formatAmount(String(Math.floor(Number(lp.value_a) * withdrawPct / 100)))} {tokenA}
                {' + '}
                {formatAmount(String(Math.floor(Number(lp.value_b) * withdrawPct / 100)))} {tokenB}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-[#6b6a8a]">LP burned</span>
              <span className="text-[#c0bfd8]">
                {formatAmount(String(Math.floor(Number(lp.lp_shares) * withdrawPct / 100)))}
              </span>
            </div>
          </div>
          <button
            disabled={withdrawing || dex.txPending}
            onClick={async () => {
              setWithdrawing(true)
              try {
                const lpToBurn = String(Math.floor(Number(lp.lp_shares) * withdrawPct / 100))
                await dex.withdrawLiquidity(lpToBurn)
              } catch { /* error shown via dex.lastTxError */ }
              finally { setWithdrawing(false) }
            }}
            className="w-full rounded-xl py-3 text-sm font-bold text-white transition-all hover:opacity-90 disabled:opacity-30 disabled:cursor-not-allowed"
            style={{ background: 'linear-gradient(135deg, #ef4444, #dc2626)' }}
          >
            {withdrawing ? (
              <span className="flex items-center justify-center gap-2">
                <Loader2 className="h-4 w-4 animate-spin" />
                Withdrawing...
              </span>
            ) : (
              <span><Minus className="inline h-4 w-4 mr-1" /> Withdraw {withdrawPct}%</span>
            )}
          </button>
        </div>
      )}
    </div>
  )
}

// ── Attestations View ──

function AttestationsView({ attestations }: { attestations: DexSwapAttestation[] }) {
  return (
    <div className="max-w-2xl mx-auto space-y-3">
      <div className="flex items-center justify-between mb-2">
        <div className="text-xs font-semibold text-[#f0eff8]">
          <ShieldCheck className="inline h-3.5 w-3.5 mr-1 text-juno-400" />
          TEE-Attested Swap History
        </div>
        <button className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-semibold text-[#6b6a8a] transition hover:text-[#c0bfd8]"
                style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.06)' }}>
          <RefreshCw className="h-2.5 w-2.5" />
          Refresh
        </button>
      </div>

      {attestations.map((att, i) => (
        <div key={i} className="rounded-xl overflow-hidden"
             style={{ background: '#0a0a18', border: `1px solid ${att.manipulation_flag ? 'rgba(239,68,68,0.3)' : 'rgba(255,255,255,0.06)'}` }}>
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-2.5"
               style={{ borderBottom: '1px solid rgba(255,255,255,0.04)' }}>
            <div className="flex items-center gap-2">
              {att.verified ? (
                <span className="flex items-center gap-1 text-[10px] font-semibold text-green-400">
                  <ShieldCheck className="h-3 w-3" /> Verified
                </span>
              ) : (
                <span className="flex items-center gap-1 text-[10px] font-semibold text-yellow-400">
                  <AlertTriangle className="h-3 w-3" /> Unverified
                </span>
              )}
              {att.manipulation_flag && (
                <span className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-semibold"
                      style={{ color: '#f87171', background: 'rgba(239,68,68,0.15)' }}>
                  <AlertTriangle className="h-2.5 w-2.5" /> Manipulation Alert
                </span>
              )}
            </div>
            <span className="text-[10px] text-[#6b6a8a]">
              {new Date(att.timestamp).toLocaleString()}
            </span>
          </div>

          {/* Body */}
          <div className="px-4 py-3 space-y-2">
            <div className="flex items-center justify-between text-[11px]">
              <div>
                <span className="text-[#6b6a8a]">Offer:</span>{' '}
                <span className="font-mono text-[#f0eff8]">{formatAmount(att.offer_amount)}</span>{' '}
                <span className="text-juno-400">{denomLabel(att.offer_asset)}</span>
              </div>
              <ArrowLeftRight className="h-3 w-3 text-[#6b6a8a]" />
              <div>
                <span className="text-[#6b6a8a]">Return:</span>{' '}
                <span className="font-mono text-[#f0eff8]">{formatAmount(att.return_amount)}</span>{' '}
                <span className="text-juno-400">{denomLabel(att.offer_asset === 'ujuno' ? 'uusdc' : 'ujuno')}</span>
              </div>
            </div>

            <div className="flex gap-3 text-[10px]">
              <div>
                <span className="text-[#6b6a8a]">Price:</span>{' '}
                <span className="text-[#c0bfd8]">{att.effective_price}</span>
              </div>
              <div>
                <span className="text-[#6b6a8a]">Impact:</span>{' '}
                <span className={Number(att.price_impact_pct) > 3 ? 'text-red-400' : 'text-green-400'}>
                  {att.price_impact_pct}%
                </span>
              </div>
            </div>

            <div className="text-[9px] font-mono text-[#4a4a6a] truncate">
              attestation: {att.attestation_hash}
            </div>
          </div>
        </div>
      ))}

      {attestations.length === 0 && (
        <div className="text-center py-12 text-[#6b6a8a] text-sm">
          No attestations yet. Swaps will appear here after WAVS operator verification.
        </div>
      )}
    </div>
  )
}
