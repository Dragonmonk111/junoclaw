import { useState, useEffect, useRef } from 'react'
import {
  Eye, Shield, AlertTriangle, Activity, WifiOff,
  GitBranch, Anchor, Fish, Radio,
  Zap, Lock, Unlock, Globe, Fingerprint,
} from 'lucide-react'

// ── Types for chain-watcher feed messages ──

interface FeedMessage {
  type: 'chain_event' | 'verification' | 'attestation_tx' | 'status'
  timestamp: string
  data: Record<string, unknown>
}

interface GovWatchEvent {
  proposal_id: string
  action_type: string
  actor: string
  proposal_kind: string
  yes_weight: number
  no_weight: number
  total_voted_weight: number
  total_weight: number
  participation_pct: string
  status: string
  blocks_remaining: number
  risk_flags: string[]
  risk_score: number
  risk_level: string
}

interface MigrationEvent {
  proposal_id: number
  contract_addr: string
  new_code_id: number
  title: string
  is_dex_contract: boolean
  is_dao_contract: boolean
  authorized: boolean
  risk_flags: string[]
  risk_score: number
  risk_level: string
}

interface WhaleEvent {
  pair: string
  sender: string
  offer_asset: string
  offer_amount: number
  return_amount: number
  trade_size_pct: string
  whale_tier: string
  price_impact_pct: string
  sandwich_risk: string
  pool_depth_score: string
  block_height: string
  timestamp: string
}

interface IbcEvent {
  channel_id: string
  port_id: string
  counterparty_channel: string
  connection_id: string
  state: string
  packets_sent: number
  packets_recv: number
  packets_timeout: number
  relay_quality: string
  health: string
}

// ── Mock data (until chain-watcher ws://localhost:7778 provides live feed) ──

const MOCK_GOV: GovWatchEvent[] = [
  {
    proposal_id: '5', action_type: 'execute_proposal', actor: 'juno1tvpe...hz5f4m',
    proposal_kind: 'wavs_push', yes_weight: 10000, no_weight: 0,
    total_voted_weight: 10000, total_weight: 10000, participation_pct: '100.00',
    status: 'Executed', blocks_remaining: 0, risk_flags: ['unanimous_vote'],
    risk_score: 20, risk_level: 'low',
  },
  {
    proposal_id: '4', action_type: 'execute_proposal', actor: 'juno1tvpe...hz5f4m',
    proposal_kind: 'wavs_push', yes_weight: 10000, no_weight: 0,
    total_voted_weight: 10000, total_weight: 10000, participation_pct: '100.00',
    status: 'Executed', blocks_remaining: 0,
    risk_flags: ['unanimous_vote', 'fast_execution'],
    risk_score: 45, risk_level: 'medium',
  },
]

const MOCK_MIGRATIONS: MigrationEvent[] = [
  {
    proposal_id: 3, contract_addr: 'juno1k8dxll...stj85k6',
    new_code_id: 63, title: 'Upgrade agent-company to v3',
    is_dex_contract: false, is_dao_contract: true, authorized: true,
    risk_flags: ['dao_contract_migration'], risk_score: 30, risk_level: 'high',
  },
]

const MOCK_WHALES: WhaleEvent[] = [
  {
    pair: 'juno1xn4m...qfr6e98', sender: 'juno1tvpe...hz5f4m',
    offer_asset: 'ujunox', offer_amount: 500_000_000, return_amount: 485_000_000,
    trade_size_pct: '50.0000', whale_tier: 'mega_whale',
    price_impact_pct: '8.4200', sandwich_risk: 'high',
    pool_depth_score: '17.69', block_height: '11900500', timestamp: '1742248000',
  },
]

const MOCK_IBC: IbcEvent[] = [
  {
    channel_id: 'channel-0', port_id: 'transfer',
    counterparty_channel: 'channel-42', connection_id: 'connection-0',
    state: 'STATE_OPEN', packets_sent: 1247, packets_recv: 1243,
    packets_timeout: 4, relay_quality: '99.68', health: 'healthy',
  },
  {
    channel_id: 'channel-1', port_id: 'transfer',
    counterparty_channel: 'channel-88', connection_id: 'connection-1',
    state: 'STATE_OPEN', packets_sent: 312, packets_recv: 310,
    packets_timeout: 2, relay_quality: '99.36', health: 'healthy',
  },
]

// ── Helpers ──

function riskColor(level: string): string {
  if (level === 'critical') return '#ef4444'
  if (level === 'high') return '#f97316'
  if (level === 'medium') return '#fbbf24'
  return '#22c55e'
}

function riskBg(level: string): string {
  if (level === 'critical') return 'rgba(239,68,68,0.08)'
  if (level === 'high') return 'rgba(249,115,22,0.08)'
  if (level === 'medium') return 'rgba(251,191,36,0.08)'
  return 'rgba(34,197,94,0.08)'
}

function riskBorder(level: string): string {
  if (level === 'critical') return 'rgba(239,68,68,0.2)'
  if (level === 'high') return 'rgba(249,115,22,0.2)'
  if (level === 'medium') return 'rgba(251,191,36,0.2)'
  return 'rgba(34,197,94,0.2)'
}

function whaleTierColor(tier: string): string {
  if (tier === 'mega_whale') return '#ef4444'
  if (tier === 'whale') return '#f97316'
  if (tier === 'large') return '#fbbf24'
  return '#6b6a8a'
}

function formatAddr(addr: string): string {
  if (addr.length <= 16) return addr
  return addr.slice(0, 10) + '...' + addr.slice(-6)
}

// ── Sub-tab type ──

type IntelSubTab = 'overview' | 'governance' | 'migrations' | 'whales' | 'ibc'

// ── Main IntelPanel (Qu-Zeno Portal) ──

export function IntelPanel() {
  const [subTab, setSubTab] = useState<IntelSubTab>('overview')
  const [feedConnected, setFeedConnected] = useState(false)
  const [feedEvents, setFeedEvents] = useState<FeedMessage[]>([])
  const [pulse, setPulse] = useState(false)
  const wsRef = useRef<WebSocket | null>(null)

  // ── Chain-watcher WebSocket feed ──
  useEffect(() => {
    let ws: WebSocket | null = null
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null

    function connect() {
      try {
        ws = new WebSocket('ws://localhost:7778')

        ws.onopen = () => {
          setFeedConnected(true)
          wsRef.current = ws
        }

        ws.onmessage = (e) => {
          try {
            const msg: FeedMessage = JSON.parse(e.data)
            setFeedEvents(prev => [msg, ...prev].slice(0, 200))
            setPulse(true)
            setTimeout(() => setPulse(false), 600)
          } catch { /* ignore parse errors */ }
        }

        ws.onclose = () => {
          setFeedConnected(false)
          wsRef.current = null
          reconnectTimer = setTimeout(connect, 5000)
        }

        ws.onerror = () => {
          ws?.close()
        }
      } catch {
        reconnectTimer = setTimeout(connect, 5000)
      }
    }

    connect()

    return () => {
      if (ws) ws.close()
      if (reconnectTimer) clearTimeout(reconnectTimer)
    }
  }, [])

  const subTabs: { id: IntelSubTab; label: string; icon: React.ReactNode }[] = [
    { id: 'overview',    label: 'Overview',    icon: <Eye         className="h-3 w-3" /> },
    { id: 'governance',  label: 'Gov Watch',   icon: <Shield      className="h-3 w-3" /> },
    { id: 'migrations',  label: 'Migrations',  icon: <GitBranch   className="h-3 w-3" /> },
    { id: 'whales',      label: 'Whale Alert',  icon: <Fish        className="h-3 w-3" /> },
    { id: 'ibc',         label: 'IBC Health',  icon: <Globe       className="h-3 w-3" /> },
  ]

  return (
    <div className="flex flex-1 flex-col bg-[#06060f] overflow-hidden">
      {/* ── Qu-Zeno Header ── */}
      <div className="px-6 py-3" style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            {/* Quantum Eye icon */}
            <div className={`relative flex h-10 w-10 items-center justify-center rounded-xl transition-all duration-300 ${pulse ? 'scale-110' : ''}`}
                 style={{
                   background: 'radial-gradient(circle at 40% 40%, rgba(167,139,250,0.25), rgba(96,165,250,0.1))',
                   border: '1px solid rgba(167,139,250,0.3)',
                   boxShadow: pulse ? '0 0 20px rgba(167,139,250,0.4)' : '0 0 8px rgba(167,139,250,0.15)',
                 }}>
              <Eye className="h-5 w-5" style={{ color: '#a78bfa' }} />
              {/* Scan line animation */}
              <div className="absolute inset-0 rounded-xl overflow-hidden pointer-events-none">
                <div className="absolute w-full h-[1px] animate-pulse"
                     style={{ background: 'linear-gradient(90deg, transparent, rgba(167,139,250,0.6), transparent)', top: '50%' }} />
              </div>
            </div>
            <div>
              <div className="flex items-center gap-2">
                <span className="text-sm font-bold text-[#f0eff8]">Qu-Zeno Portal</span>
                <span className="text-[9px] font-mono px-1.5 py-0.5 rounded"
                      style={{ background: 'rgba(167,139,250,0.1)', border: '1px solid rgba(167,139,250,0.2)', color: '#a78bfa' }}>
                  QUANTUM OBSERVER
                </span>
              </div>
              <div className="text-[10px] text-[#6b6a8a]">
                Chain intelligence through continuous observation
                <span className="ml-1 text-[#4a4a6a]">
                  &mdash; what is watched cannot decay unnoticed
                </span>
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <span className="text-[9px] font-mono text-[#4a4a6a]">
              {feedEvents.length} events
            </span>
            <span className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-semibold"
                  style={{
                    color: feedConnected ? '#a78bfa' : '#6b6a8a',
                    background: feedConnected ? 'rgba(167,139,250,0.08)' : 'rgba(255,255,255,0.04)',
                    border: `1px solid ${feedConnected ? 'rgba(167,139,250,0.2)' : 'rgba(255,255,255,0.06)'}`,
                  }}>
              {feedConnected ? <Radio className="h-2.5 w-2.5 animate-pulse" /> : <WifiOff className="h-2.5 w-2.5" />}
              {feedConnected ? 'Observing' : 'Feed offline'}
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
                  color: '#a78bfa',
                  background: 'rgba(167,139,250,0.1)',
                  border: '1px solid rgba(167,139,250,0.2)',
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

      {/* ── Content ── */}
      <div className="flex-1 overflow-y-auto px-6 py-5">
        {subTab === 'overview'   && <OverviewView govEvents={MOCK_GOV} migrations={MOCK_MIGRATIONS} whales={MOCK_WHALES} ibc={MOCK_IBC} feedEvents={feedEvents} />}
        {subTab === 'governance' && <GovernanceView events={MOCK_GOV} />}
        {subTab === 'migrations' && <MigrationView events={MOCK_MIGRATIONS} />}
        {subTab === 'whales'     && <WhaleView events={MOCK_WHALES} />}
        {subTab === 'ibc'        && <IbcView events={MOCK_IBC} />}
      </div>
    </div>
  )
}

// ── Overview Dashboard ──

function OverviewView({
  govEvents, migrations, whales, ibc, feedEvents,
}: {
  govEvents: GovWatchEvent[]
  migrations: MigrationEvent[]
  whales: WhaleEvent[]
  ibc: IbcEvent[]
  feedEvents: FeedMessage[]
}) {
  // Aggregate risk
  const maxGovRisk = govEvents.reduce((max, e) => Math.max(max, e.risk_score), 0)
  const maxMigRisk = migrations.reduce((max, e) => Math.max(max, e.risk_score), 0)
  const whaleCount = whales.filter(w => w.whale_tier !== 'normal').length
  const ibcUnhealthy = ibc.filter(c => c.health !== 'healthy').length

  const overallRisk = Math.max(maxGovRisk, maxMigRisk)
  const overallLevel = overallRisk >= 60 ? 'critical' : overallRisk >= 30 ? 'medium' : 'low'

  const cards = [
    {
      label: 'Governance', icon: <Shield className="h-4 w-4" />,
      value: `${govEvents.length} events`, risk: maxGovRisk > 30 ? 'medium' : 'low',
      detail: `Max risk: ${maxGovRisk}`,
    },
    {
      label: 'Migrations', icon: <GitBranch className="h-4 w-4" />,
      value: `${migrations.length} detected`, risk: maxMigRisk >= 30 ? 'high' : 'low',
      detail: `${migrations.filter(m => m.authorized).length} authorized`,
    },
    {
      label: 'Whale Alerts', icon: <Fish className="h-4 w-4" />,
      value: `${whaleCount} flagged`, risk: whaleCount > 0 ? 'medium' : 'low',
      detail: `${whales.length} total trades`,
    },
    {
      label: 'IBC Channels', icon: <Globe className="h-4 w-4" />,
      value: `${ibc.length} channels`, risk: ibcUnhealthy > 0 ? 'high' : 'low',
      detail: `${ibcUnhealthy} degraded`,
    },
  ]

  return (
    <div className="max-w-3xl mx-auto space-y-5">
      {/* Overall status */}
      <div className="rounded-2xl p-5 relative overflow-hidden"
           style={{ background: '#0a0a18', border: `1px solid ${riskBorder(overallLevel)}` }}>
        <div className="absolute inset-0 pointer-events-none"
             style={{ background: `radial-gradient(ellipse at 20% 50%, ${riskBg(overallLevel)}, transparent 70%)` }} />
        <div className="relative flex items-center justify-between">
          <div className="flex items-center gap-4">
            <div className="flex h-14 w-14 items-center justify-center rounded-2xl"
                 style={{ background: riskBg(overallLevel), border: `1px solid ${riskBorder(overallLevel)}` }}>
              <Eye className="h-7 w-7" style={{ color: riskColor(overallLevel) }} />
            </div>
            <div>
              <div className="text-xs font-semibold uppercase tracking-widest" style={{ color: riskColor(overallLevel) }}>
                Chain State: {overallLevel.toUpperCase()}
              </div>
              <div className="text-[10px] text-[#6b6a8a] mt-0.5">
                The Qu-Zeno observer continuously collapses uncertainty into verified state
              </div>
            </div>
          </div>
          <div className="text-right">
            <div className="text-2xl font-bold font-mono" style={{ color: riskColor(overallLevel) }}>
              {overallRisk}
            </div>
            <div className="text-[9px] text-[#4a4a6a] uppercase tracking-wider">Risk Score</div>
          </div>
        </div>
      </div>

      {/* Module cards grid */}
      <div className="grid grid-cols-2 gap-3">
        {cards.map((card) => (
          <div key={card.label} className="rounded-xl p-4"
               style={{ background: '#0a0a18', border: `1px solid ${riskBorder(card.risk)}` }}>
            <div className="flex items-center justify-between mb-2">
              <div className="flex items-center gap-2">
                <span style={{ color: riskColor(card.risk) }}>{card.icon}</span>
                <span className="text-[11px] font-semibold text-[#f0eff8]">{card.label}</span>
              </div>
              <span className="text-[9px] font-mono px-1.5 py-0.5 rounded"
                    style={{ background: riskBg(card.risk), border: `1px solid ${riskBorder(card.risk)}`, color: riskColor(card.risk) }}>
                {card.risk.toUpperCase()}
              </span>
            </div>
            <div className="text-lg font-bold text-[#f0eff8]">{card.value}</div>
            <div className="text-[10px] text-[#6b6a8a] mt-0.5">{card.detail}</div>
          </div>
        ))}
      </div>

      {/* Live feed ticker */}
      <div className="rounded-xl p-4" style={{ background: '#0a0a18', border: '1px solid rgba(167,139,250,0.1)' }}>
        <div className="flex items-center gap-2 mb-3">
          <Radio className="h-3 w-3 text-[#a78bfa]" />
          <span className="text-[10px] font-semibold uppercase tracking-widest text-[#6b6a8a]">
            Live Observation Feed
          </span>
          <span className="text-[9px] font-mono text-[#4a4a6a]">{feedEvents.length} events</span>
        </div>
        <div className="space-y-1 max-h-48 overflow-y-auto">
          {feedEvents.length === 0 ? (
            <div className="text-[11px] text-[#4a4a6a] text-center py-4">
              <Eye className="h-5 w-5 mx-auto mb-2 text-[#2a2a4a]" />
              Waiting for chain events...
              <div className="text-[9px] mt-1">Connect chain-watcher on ws://localhost:7778</div>
            </div>
          ) : (
            feedEvents.slice(0, 20).map((msg, i) => (
              <div key={i} className="flex items-center gap-2 text-[10px] py-1 px-2 rounded"
                   style={{ background: 'rgba(255,255,255,0.02)' }}>
                <span className="text-[#4a4a6a] font-mono w-16 flex-shrink-0">
                  {new Date(msg.timestamp).toLocaleTimeString()}
                </span>
                <span className={msg.type === 'verification' ? 'text-[#a78bfa]' : msg.type === 'attestation_tx' ? 'text-green-400' : 'text-[#6b6a8a]'}>
                  {msg.type === 'chain_event' ? <Zap className="inline h-2.5 w-2.5" /> :
                   msg.type === 'verification' ? <Fingerprint className="inline h-2.5 w-2.5" /> :
                   msg.type === 'attestation_tx' ? <Lock className="inline h-2.5 w-2.5" /> :
                   <Activity className="inline h-2.5 w-2.5" />}
                </span>
                <span className="text-[#c0bfd8] truncate">
                  {msg.type === 'chain_event' ? `Event: ${(msg.data as Record<string, unknown>).eventType}` :
                   msg.type === 'verification' ? `Verified: proposal ${(msg.data as Record<string, unknown>).proposalId}` :
                   msg.type === 'attestation_tx' ? `Attested: ${String((msg.data as Record<string, unknown>).txHash ?? '').slice(0, 16)}...` :
                   `Status: ${(msg.data as Record<string, unknown>).watcherStatus}`}
                </span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  )
}

// ── Governance Watch View ──

function GovernanceView({ events }: { events: GovWatchEvent[] }) {
  return (
    <div className="max-w-3xl mx-auto space-y-3">
      <div className="flex items-center gap-2 mb-2">
        <Shield className="h-4 w-4 text-[#a78bfa]" />
        <span className="text-xs font-semibold text-[#f0eff8]">Governance Anomaly Detection</span>
        <span className="text-[9px] text-[#4a4a6a]">
          Monitors voting patterns, quorum manipulation, rapid-fire proposals
        </span>
      </div>

      {events.map((evt, i) => (
        <div key={i} className="rounded-xl p-4"
             style={{ background: '#0a0a18', border: `1px solid ${riskBorder(evt.risk_level)}` }}>
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              <span className="text-xs font-bold text-[#f0eff8]">Proposal #{evt.proposal_id}</span>
              <span className="text-[9px] font-mono px-1.5 py-0.5 rounded"
                    style={{ background: 'rgba(255,255,255,0.04)', color: '#6b6a8a' }}>
                {evt.action_type}
              </span>
              <span className="text-[9px] font-mono px-1.5 py-0.5 rounded"
                    style={{ background: 'rgba(255,255,255,0.04)', color: '#6b6a8a' }}>
                {evt.proposal_kind}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[10px] font-mono" style={{ color: riskColor(evt.risk_level) }}>
                RISK: {evt.risk_score}
              </span>
              <span className="text-[9px] font-mono px-1.5 py-0.5 rounded"
                    style={{ background: riskBg(evt.risk_level), border: `1px solid ${riskBorder(evt.risk_level)}`, color: riskColor(evt.risk_level) }}>
                {evt.risk_level.toUpperCase()}
              </span>
            </div>
          </div>

          {/* Stats row */}
          <div className="grid grid-cols-4 gap-2 mb-3">
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Participation</div>
              <div className="text-sm font-bold text-[#f0eff8]">{evt.participation_pct}%</div>
            </div>
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Yes</div>
              <div className="text-sm font-bold text-green-400">{evt.yes_weight}</div>
            </div>
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">No</div>
              <div className="text-sm font-bold text-red-400">{evt.no_weight}</div>
            </div>
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Status</div>
              <div className="text-sm font-bold text-[#c0bfd8]">{evt.status}</div>
            </div>
          </div>

          {/* Risk flags */}
          {evt.risk_flags.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {evt.risk_flags.map((flag) => (
                <span key={flag} className="flex items-center gap-1 text-[9px] font-mono px-2 py-0.5 rounded"
                      style={{ background: riskBg(evt.risk_level), border: `1px solid ${riskBorder(evt.risk_level)}`, color: riskColor(evt.risk_level) }}>
                  <AlertTriangle className="h-2 w-2" />
                  {flag}
                </span>
              ))}
            </div>
          )}

          <div className="mt-2 text-[9px] font-mono text-[#4a4a6a]">
            Actor: {evt.actor}
          </div>
        </div>
      ))}
    </div>
  )
}

// ── Migration Watch View ──

function MigrationView({ events }: { events: MigrationEvent[] }) {
  return (
    <div className="max-w-3xl mx-auto space-y-3">
      <div className="flex items-center gap-2 mb-2">
        <GitBranch className="h-4 w-4 text-[#a78bfa]" />
        <span className="text-xs font-semibold text-[#f0eff8]">Contract Migration Audit</span>
        <span className="text-[9px] text-[#4a4a6a]">
          Verifies code upgrades against DAO approval and known-good code IDs
        </span>
      </div>

      {events.map((evt, i) => (
        <div key={i} className="rounded-xl p-4"
             style={{ background: '#0a0a18', border: `1px solid ${riskBorder(evt.risk_level)}` }}>
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-3">
              {evt.authorized ?
                <Lock className="h-4 w-4 text-green-400" /> :
                <Unlock className="h-4 w-4 text-red-400" />
              }
              <div>
                <div className="text-xs font-bold text-[#f0eff8]">{evt.title}</div>
                <div className="text-[9px] text-[#6b6a8a] font-mono">
                  Proposal #{evt.proposal_id} &middot; Code ID {evt.new_code_id}
                </div>
              </div>
            </div>
            <span className="text-[9px] font-mono px-1.5 py-0.5 rounded"
                  style={{ background: riskBg(evt.risk_level), border: `1px solid ${riskBorder(evt.risk_level)}`, color: riskColor(evt.risk_level) }}>
              {evt.risk_level.toUpperCase()}
            </span>
          </div>

          <div className="grid grid-cols-3 gap-2 mb-3">
            <div className="rounded-lg p-2" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Contract</div>
              <div className="text-[10px] font-mono text-[#c0bfd8]">{formatAddr(evt.contract_addr)}</div>
            </div>
            <div className="rounded-lg p-2" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Type</div>
              <div className="text-[10px] font-bold" style={{ color: evt.is_dao_contract ? '#a78bfa' : evt.is_dex_contract ? '#ff6b4a' : '#6b6a8a' }}>
                {evt.is_dao_contract ? 'DAO Contract' : evt.is_dex_contract ? 'DEX Contract' : 'Unknown'}
              </div>
            </div>
            <div className="rounded-lg p-2" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Authorized</div>
              <div className="text-[10px] font-bold" style={{ color: evt.authorized ? '#22c55e' : '#ef4444' }}>
                {evt.authorized ? 'YES' : 'NO'}
              </div>
            </div>
          </div>

          {evt.risk_flags.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {evt.risk_flags.map((flag) => (
                <span key={flag} className="flex items-center gap-1 text-[9px] font-mono px-2 py-0.5 rounded"
                      style={{ background: riskBg(evt.risk_level), border: `1px solid ${riskBorder(evt.risk_level)}`, color: riskColor(evt.risk_level) }}>
                  <AlertTriangle className="h-2 w-2" />
                  {flag}
                </span>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  )
}

// ── Whale Alert View ──

function WhaleView({ events }: { events: WhaleEvent[] }) {
  return (
    <div className="max-w-3xl mx-auto space-y-3">
      <div className="flex items-center gap-2 mb-2">
        <Fish className="h-4 w-4 text-[#a78bfa]" />
        <span className="text-xs font-semibold text-[#f0eff8]">Whale Trade Intelligence</span>
        <span className="text-[9px] text-[#4a4a6a]">
          Large trade detection, sandwich risk analysis, pool depth monitoring
        </span>
      </div>

      {events.map((evt, i) => (
        <div key={i} className="rounded-xl p-4"
             style={{ background: '#0a0a18', border: `1px solid ${riskBorder(evt.sandwich_risk === 'high' ? 'high' : evt.sandwich_risk === 'medium' ? 'medium' : 'low')}` }}>
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              <Fish className="h-4 w-4" style={{ color: whaleTierColor(evt.whale_tier) }} />
              <span className="text-xs font-bold" style={{ color: whaleTierColor(evt.whale_tier) }}>
                {evt.whale_tier.replace('_', ' ').toUpperCase()}
              </span>
              <span className="text-[9px] font-mono text-[#6b6a8a]">
                Block {evt.block_height}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[9px] font-mono px-1.5 py-0.5 rounded"
                    style={{
                      background: riskBg(evt.sandwich_risk === 'high' ? 'high' : evt.sandwich_risk === 'medium' ? 'medium' : 'low'),
                      border: `1px solid ${riskBorder(evt.sandwich_risk === 'high' ? 'high' : evt.sandwich_risk === 'medium' ? 'medium' : 'low')}`,
                      color: riskColor(evt.sandwich_risk === 'high' ? 'high' : evt.sandwich_risk === 'medium' ? 'medium' : 'low'),
                    }}>
                Sandwich: {evt.sandwich_risk.toUpperCase()}
              </span>
            </div>
          </div>

          <div className="grid grid-cols-4 gap-2 mb-3">
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Offer</div>
              <div className="text-[11px] font-bold text-[#f0eff8]">
                {(evt.offer_amount / 1_000_000).toFixed(1)}
              </div>
              <div className="text-[8px] text-[#4a4a6a]">{evt.offer_asset.replace('u', '').toUpperCase()}</div>
            </div>
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Return</div>
              <div className="text-[11px] font-bold text-[#f0eff8]">
                {(evt.return_amount / 1_000_000).toFixed(1)}
              </div>
            </div>
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Trade Size</div>
              <div className="text-[11px] font-bold" style={{ color: whaleTierColor(evt.whale_tier) }}>
                {parseFloat(evt.trade_size_pct).toFixed(1)}%
              </div>
              <div className="text-[8px] text-[#4a4a6a]">of reserve</div>
            </div>
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Impact</div>
              <div className="text-[11px] font-bold" style={{ color: parseFloat(evt.price_impact_pct) > 5 ? '#ef4444' : parseFloat(evt.price_impact_pct) > 2 ? '#fbbf24' : '#22c55e' }}>
                {parseFloat(evt.price_impact_pct).toFixed(2)}%
              </div>
            </div>
          </div>

          <div className="flex items-center gap-3 text-[9px] font-mono text-[#4a4a6a]">
            <span>Sender: {formatAddr(evt.sender)}</span>
            <span>Pair: {formatAddr(evt.pair)}</span>
            <span>Depth: {parseFloat(evt.pool_depth_score).toFixed(1)}</span>
          </div>
        </div>
      ))}
    </div>
  )
}

// ── IBC Health View ──

function IbcView({ events }: { events: IbcEvent[] }) {
  return (
    <div className="max-w-3xl mx-auto space-y-3">
      <div className="flex items-center gap-2 mb-2">
        <Globe className="h-4 w-4 text-[#a78bfa]" />
        <span className="text-xs font-semibold text-[#f0eff8]">IBC Channel Health Monitor</span>
        <span className="text-[9px] text-[#4a4a6a]">
          Packet relay quality, channel state, timeout detection
        </span>
      </div>

      {events.map((evt, i) => (
        <div key={i} className="rounded-xl p-4"
             style={{ background: '#0a0a18', border: `1px solid ${riskBorder(evt.health === 'healthy' ? 'low' : evt.health === 'degraded' ? 'medium' : 'high')}` }}>
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-3">
              <div className="flex h-8 w-8 items-center justify-center rounded-lg"
                   style={{ background: 'rgba(167,139,250,0.08)', border: '1px solid rgba(167,139,250,0.15)' }}>
                <Anchor className="h-4 w-4 text-[#a78bfa]" />
              </div>
              <div>
                <div className="text-xs font-bold text-[#f0eff8]">{evt.channel_id}</div>
                <div className="text-[9px] text-[#6b6a8a] font-mono">
                  {evt.port_id} &middot; {evt.connection_id}
                </div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[9px] font-mono px-1.5 py-0.5 rounded"
                    style={{
                      background: evt.state === 'STATE_OPEN' ? 'rgba(34,197,94,0.08)' : 'rgba(239,68,68,0.08)',
                      border: `1px solid ${evt.state === 'STATE_OPEN' ? 'rgba(34,197,94,0.15)' : 'rgba(239,68,68,0.15)'}`,
                      color: evt.state === 'STATE_OPEN' ? '#22c55e' : '#ef4444',
                    }}>
                {evt.state.replace('STATE_', '')}
              </span>
            </div>
          </div>

          <div className="grid grid-cols-4 gap-2 mb-3">
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Sent</div>
              <div className="text-sm font-bold text-[#f0eff8]">{evt.packets_sent.toLocaleString()}</div>
            </div>
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Received</div>
              <div className="text-sm font-bold text-green-400">{evt.packets_recv.toLocaleString()}</div>
            </div>
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Timeouts</div>
              <div className="text-sm font-bold" style={{ color: evt.packets_timeout > 10 ? '#ef4444' : evt.packets_timeout > 0 ? '#fbbf24' : '#22c55e' }}>
                {evt.packets_timeout}
              </div>
            </div>
            <div className="rounded-lg p-2 text-center" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="text-[9px] text-[#6b6a8a] uppercase">Relay Quality</div>
              <div className="text-sm font-bold" style={{ color: parseFloat(evt.relay_quality) > 99 ? '#22c55e' : parseFloat(evt.relay_quality) > 95 ? '#fbbf24' : '#ef4444' }}>
                {evt.relay_quality}%
              </div>
            </div>
          </div>

          <div className="text-[9px] font-mono text-[#4a4a6a]">
            Counterparty: {evt.counterparty_channel}
          </div>
        </div>
      ))}
    </div>
  )
}
