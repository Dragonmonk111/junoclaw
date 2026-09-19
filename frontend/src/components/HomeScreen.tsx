import { useEffect, useState } from 'react'
import { useStore } from '../store'
import { usePortalStatus } from '../hooks/usePortalStatus'
import { useCountUp } from '../hooks/useCountUp'
import { CrabLogo } from './CrabLogo'
import {
  Bot, Building2, ArrowLeftRight, Shield, Radio,
  Eye, Cpu, Fuel, Pickaxe, FileCode2, HeartPulse, RefreshCw, MessageSquare,
  Lock, Zap, Gauge, CircuitBoard, Gift,
} from 'lucide-react'

// Safety-proof budget: a fused ZK proof lands well inside one Commonware
// block, which is the whole "the robot never waits for the chain" claim.
const PROOF_LATENCY_MS = 187
const BLOCK_BUDGET_MS = 300

// Verification portals — the core mission, shown at the forefront
const VERIFY_PORTALS = [
  { id: 'intel',    label: 'Q-Zeno Truth Market', icon: <Eye className="h-5 w-5" />,     color: '#60a5fa', desc: 'Real-time truth verdicts on agent & robot behavior' },
  { id: 'robotops', label: 'Robot Ops',           icon: <Cpu className="h-5 w-5" />,     color: '#a78bfa', desc: 'Fleet control, safety circuits, J-Lens probes' },
  { id: 'miner',    label: 'Truth Miners',        icon: <Pickaxe className="h-5 w-5" />, color: '#f87171', desc: 'Staked operators adjudicate agent decisions' },
  { id: 'buzz',     label: 'Buzz Relay',          icon: <Radio className="h-5 w-5" />,  color: '#e879f9', desc: 'Attestation channel for agents & operators' },
]

// Ecosystem portals — operational tools, shown in background
const ECOSYSTEM_PORTALS = [
  { id: 'chat',         label: 'Agent Chat',      icon: <MessageSquare className="h-5 w-5" />, color: '#ff6b4a', desc: 'Talk to your AI agents' },
  { id: 'dao',          label: 'DAO Governance',  icon: <Building2 className="h-5 w-5" />,    color: '#00d4aa', desc: 'Proposals, voting, members' },
  { id: 'dex',          label: 'DEX',             icon: <ArrowLeftRight className="h-5 w-5" />, color: '#fbbf24', desc: 'Junoswap v2 trading' },
  { id: 'feepay',       label: 'FeePay',          icon: <Fuel className="h-5 w-5" />,         color: '#34d399', desc: 'Gasless transaction pools' },
  { id: 'commonwealth', label: 'Commonwealth',    icon: <HeartPulse className="h-5 w-5" />,  color: '#fb7185', desc: 'Community health digest' },
  { id: 'contracts',    label: 'Contracts',       icon: <FileCode2 className="h-5 w-5" />,    color: '#94a3b8', desc: 'Deployed contract registry' },
  { id: 'updates',      label: 'Chain Updates',   icon: <RefreshCw className="h-5 w-5" />,    color: '#22d3ee', desc: 'Chain upgrade tracker' },
]

export function HomeScreen({ onNavigate, onSelectAgent, onSelectDao, onOpenTool }: {
  onNavigate: (screen: 'discover') => void
  onSelectAgent: (id: string) => void
  onSelectDao: (id: string) => void
  onOpenTool: (tool: string) => void
}) {
  const agents = useStore((s) => s.agents)
  const daos = useStore((s) => s.daos)
  const connected = useStore((s) => s.connected)
  const daemonVersion = useStore((s) => s.daemonVersion)

  const activeAgents = agents.filter((a) => a.is_active)
  const activeDaos = daos.filter((d) => d.status === 'active')
  const wavsVerified = agents.filter((a) => a.wavs_verified)
  const portalStatus = usePortalStatus()

  return (
    <div className="flex flex-col overflow-y-auto" style={{ height: '100%' }}>
      {/* Hero header */}
      <div className="px-8 pt-8 pb-6"
           style={{ background: 'linear-gradient(180deg, rgba(255,107,74,0.06) 0%, transparent 100%)' }}>
        <div className="flex items-center gap-4 mb-2">
          <div className="rounded-2xl p-1.5" style={{ boxShadow: '0 0 32px rgba(255,107,74,0.2)' }}>
            <CrabLogo size={56} />
          </div>
          <div>
            <h1 className="text-2xl font-bold text-white tracking-tight">JunoClaw</h1>
            <p className="text-sm text-[#6b6a8a]">Trust operating system for AI agents & robots</p>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <div className="flex items-center gap-1.5 rounded-full px-3 py-1.5"
                 style={{ background: connected ? 'rgba(0,212,170,0.08)' : 'rgba(255,107,74,0.08)',
                          border: `1px solid ${connected ? 'rgba(0,212,170,0.2)' : 'rgba(255,107,74,0.2)'}` }}>
              <div className={`h-2 w-2 rounded-full ${connected ? 'bg-teal-400' : 'bg-orange-400'} animate-pulse`} />
              <span className="text-xs font-medium" style={{ color: connected ? '#00d4aa' : '#ff6b4a' }}>
                {connected ? `Connected${daemonVersion ? ` · v${daemonVersion}` : ''}` : 'Offline'}
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Airdrop status banner — snapshot done, claim opens at mainnet launch */}
      <div className="px-8 pb-6">
        <div className="relative flex items-center justify-between gap-4 overflow-hidden rounded-2xl p-4 animate-rise"
             style={{ background: 'linear-gradient(135deg, rgba(0,212,170,0.08), rgba(96,165,250,0.06))', border: '1px solid rgba(0,212,170,0.2)' }}>
          {/* Light sweep — keeps the pending claim in peripheral vision */}
          <div className="pointer-events-none absolute inset-y-0 -left-1/3 w-1/3 animate-sweep"
               style={{ background: 'linear-gradient(90deg, transparent, rgba(0,212,170,0.10), transparent)' }} />
          <div className="relative flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl" style={{ background: 'rgba(0,212,170,0.14)' }}>
              <Gift className="h-5 w-5" style={{ color: '#00d4aa' }} />
            </div>
            <div>
              <p className="text-sm font-semibold text-white">Airdrop snapshot complete — 213,385 accounts, 35.04M ujclaw ready</p>
              <p className="text-xs text-[#6b6a8a]">1:1 with staked JUNO at block 41,655,615 · Merkle proofs generated · claim opens at mainnet launch</p>
            </div>
          </div>
          <span className="relative flex-shrink-0 rounded-full px-3 py-1.5 text-xs font-semibold"
                style={{ background: 'rgba(0,212,170,0.12)', color: '#00d4aa', border: '1px solid rgba(0,212,170,0.25)' }}>
            Claim opens at launch
          </span>
        </div>
      </div>

      {/* Mission stat cards — verification-focused */}
      <div className="px-8 pb-6">
        <div className="grid grid-cols-4 gap-4">
          <StatCard
            icon={<Shield className="h-5 w-5" />}
            label="WAVS Verified"
            value={wavsVerified.length}
            color="#60a5fa"
            index={0}
            onClick={() => onNavigate('discover')}
          />
          <StatCard
            icon={<Lock className="h-5 w-5" />}
            label="ZK Proofs"
            value={5}
            color="#a78bfa"
            index={1}
          />
          <StatCard
            icon={<Gauge className="h-5 w-5" />}
            label="Safety Latency"
            value={PROOF_LATENCY_MS}
            suffix="ms"
            color="#00d4aa"
            index={2}
          />
          <StatCard
            icon={<CircuitBoard className="h-5 w-5" />}
            label="Robot Cycles"
            value={2015}
            color="#ff6b4a"
            index={3}
          />
        </div>
      </div>

      {/* Latency budget — the core claim, drawn to scale */}
      <div className="px-8 pb-6">
        <LatencyBudget />
      </div>

      {/* Verification section — the forefront */}
      <div className="px-8 pb-6">
        <div className="flex items-center gap-2 mb-4">
          <Shield className="h-4 w-4 text-[#60a5fa]" />
          <h2 className="text-sm font-semibold text-white uppercase tracking-wider">Verification</h2>
          <span className="text-[10px] text-[#4a4a6a] ml-2">Prove decisions are safe</span>
        </div>
        <div className="grid grid-cols-4 gap-3">
          {VERIFY_PORTALS.map((p, i) => {
            const st = portalStatus[p.id]
            const dotColor = st?.error ? '#ef4444' : st?.live ? '#00d4aa' : '#6b6a8a'
            return (
            <button key={p.id} onClick={() => onOpenTool(p.id)}
                    className="card-lift animate-rise flex flex-col gap-2.5 rounded-2xl p-4 text-left hover:bg-white/5"
                    style={{ background: '#0a0a18', border: `1px solid ${p.color}22`, animationDelay: `${i * 60}ms` }}>
              <div className="flex items-center gap-2.5">
                <div className="flex h-9 w-9 items-center justify-center rounded-xl"
                     style={{ background: `${p.color}14` }}>
                  <span style={{ color: p.color }}>{p.icon}</span>
                </div>
                <span className="text-sm font-semibold text-white">{p.label}</span>
                <span className="ml-auto flex items-center gap-1">
                  <LiveDot color={dotColor} live={!!st?.live} />
                  <span className="text-[10px] text-[#6b6a8a]">{st?.label}</span>
                </span>
              </div>
              <p className="text-xs text-[#6b6a8a] leading-relaxed">{p.desc}</p>
            </button>
            )
          })}
        </div>
      </div>

      {/* Two-column layout */}
      <div className="px-8 pb-8 grid grid-cols-2 gap-6">
        {/* Recent Agents */}
        <div className="rounded-2xl p-5" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-sm font-semibold text-white uppercase tracking-wider">Your Agents</h2>
            <button onClick={() => onNavigate('discover')} className="text-xs text-[#ff6b4a] hover:underline">
              View all →
            </button>
          </div>
          {activeAgents.length === 0 ? (
            <div className="py-8 text-center">
              <Bot className="mx-auto mb-3 h-10 w-10 text-[#2a2a4a]" />
              <p className="text-sm text-[#6b6a8a]">No agents yet</p>
              <button onClick={() => onNavigate('discover')} className="mt-3 text-xs text-[#ff6b4a] hover:underline">
                Create your first agent →
              </button>
            </div>
          ) : (
            <div className="space-y-2">
              {activeAgents.slice(0, 5).map((agent) => (
                <button key={agent.id} onClick={() => onSelectAgent(agent.id)}
                        className="w-full flex items-center gap-3 rounded-xl p-3 transition hover:bg-white/5 text-left">
                  <div className="flex h-9 w-9 items-center justify-center rounded-lg"
                       style={{ background: 'rgba(255,107,74,0.08)' }}>
                    <Bot className="h-4 w-4 text-[#ff6b4a]" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-white truncate">{agent.name}</span>
                      {agent.wavs_verified && <Shield className="h-3 w-3 text-blue-400 flex-shrink-0" />}
                    </div>
                    <p className="text-xs text-[#6b6a8a] truncate">{agent.model} · {agent.personality}</p>
                  </div>
                  <ChevronRight />
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Recent DAOs */}
        <div className="rounded-2xl p-5" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-sm font-semibold text-white uppercase tracking-wider">Your DAOs</h2>
            <button onClick={() => onNavigate('discover')} className="text-xs text-[#ff6b4a] hover:underline">
              View all →
            </button>
          </div>
          {activeDaos.length === 0 ? (
            <div className="py-8 text-center">
              <Building2 className="mx-auto mb-3 h-10 w-10 text-[#2a2a4a]" />
              <p className="text-sm text-[#6b6a8a]">No DAOs yet</p>
              <button onClick={() => onNavigate('discover')} className="mt-3 text-xs text-[#ff6b4a] hover:underline">
                Deploy a DAO →
              </button>
            </div>
          ) : (
            <div className="space-y-2">
              {activeDaos.slice(0, 5).map((dao) => (
                <button key={dao.id} onClick={() => onSelectDao(dao.id)}
                        className="w-full flex items-center gap-3 rounded-xl p-3 transition hover:bg-white/5 text-left">
                  <div className="flex h-9 w-9 items-center justify-center rounded-lg"
                       style={{ background: `${dao.template_color || '#00d4aa'}14` }}>
                    <Building2 className="h-4 w-4" style={{ color: dao.template_color || '#00d4aa' }} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <span className="text-sm font-medium text-white truncate block">{dao.name}</span>
                    <p className="text-xs text-[#6b6a8a]">
                      {dao.config.members.length} members · {dao.proposals.length} proposals
                    </p>
                  </div>
                  <ChevronRight />
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Ecosystem — background portals */}
      <div className="px-8 pb-8">
        <div className="flex items-center gap-2 mb-3">
          <Zap className="h-3.5 w-3.5 text-[#4a4a6a]" />
          <h2 className="text-xs font-semibold text-[#6b6a8a] uppercase tracking-wider">Ecosystem</h2>
        </div>
        <div className="grid grid-cols-7 gap-2">
          {ECOSYSTEM_PORTALS.map((p, i) => {
            const st = portalStatus[p.id]
            const dotColor = st?.error ? '#ef4444' : st?.live ? '#00d4aa' : '#6b6a8a'
            return (
            <button key={p.id} onClick={() => onOpenTool(p.id)}
                    className="card-lift animate-rise flex flex-col items-center gap-1.5 rounded-xl p-3 text-center hover:bg-white/5"
                    style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.03)', animationDelay: `${i * 40}ms` }}>
              <div className="flex h-8 w-8 items-center justify-center rounded-lg"
                   style={{ background: `${p.color}10` }}>
                <span style={{ color: p.color, opacity: 0.8 }}>{p.icon}</span>
              </div>
              <span className="text-[10px] font-medium text-[#8a8aaa]">{p.label}</span>
              <span className="flex items-center gap-1">
                <span className="h-1 w-1 rounded-full"
                      style={{ background: dotColor }} />
              </span>
            </button>
            )
          })}
        </div>
      </div>
    </div>
  )
}

/* Status dot with an outward radar ping while the portal is live. */
function LiveDot({ color, live }: { color: string; live: boolean }) {
  return (
    <span className="relative flex h-1.5 w-1.5 items-center justify-center">
      {live && (
        <span className="absolute h-1.5 w-1.5 rounded-full animate-radar"
              style={{ background: color }} />
      )}
      <span className="relative h-1.5 w-1.5 rounded-full"
            style={{ background: color, boxShadow: live ? `0 0 5px ${color}` : 'none' }} />
    </span>
  )
}

function StatCard({ icon, label, value, suffix, color, index = 0, onClick }: {
  icon: React.ReactNode; label: string; value: number; suffix?: string
  color: string; index?: number; onClick?: () => void
}) {
  const animated = useCountUp(value)
  // Round only at render time so the easing stays smooth.
  const display = Math.round(animated).toLocaleString()

  return (
    <button onClick={onClick}
            className="card-lift animate-rise flex flex-col gap-2 rounded-2xl p-4 text-left hover:bg-white/5"
            style={{ background: '#0a0a18', border: `1px solid ${color}15`, animationDelay: `${index * 60}ms` }}>
      <div className="flex items-center gap-2">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg" style={{ background: `${color}14` }}>
          <span style={{ color }}>{icon}</span>
        </div>
        <span className="text-xs text-[#6b6a8a] uppercase tracking-wider">{label}</span>
      </div>
      <span className="text-2xl font-bold text-white tabular-nums">
        {display}{suffix && <span className="text-base font-semibold text-[#8a8aaa]">{suffix}</span>}
      </span>
    </button>
  )
}

/**
 * Draws the fused safety proof against one Commonware block, to scale.
 *
 * This is the product's central claim in one graphic: the proof finishes
 * inside the block budget, so the robot never blocks on the chain.
 */
function LatencyBudget() {
  const pct = (PROOF_LATENCY_MS / BLOCK_BUDGET_MS) * 100
  const headroom = BLOCK_BUDGET_MS - PROOF_LATENCY_MS

  // Start at zero width, then hand the real value over on the next frame so
  // the CSS transition has something to animate between.
  const [fill, setFill] = useState(0)
  useEffect(() => {
    const id = requestAnimationFrame(() => setFill(pct))
    return () => cancelAnimationFrame(id)
  }, [pct])

  return (
    <div className="animate-rise rounded-2xl p-4"
         style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)', animationDelay: '240ms' }}>
      <div className="mb-3 flex items-center gap-2">
        <Zap className="h-3.5 w-3.5 text-[#00d4aa]" />
        <h2 className="text-xs font-semibold uppercase tracking-wider text-white">Safety budget</h2>
        <span className="text-[10px] text-[#4a4a6a]">Fused ZK proof vs. one block</span>
        <span className="ml-auto text-[10px] font-medium text-[#00d4aa]">
          {headroom}ms headroom
        </span>
      </div>

      <div className="relative h-2.5 w-full overflow-hidden rounded-full"
           style={{ background: 'rgba(255,255,255,0.04)' }}>
        {/* Proof fill — animates out to its share of the block budget */}
        <div className="gauge-fill absolute inset-y-0 left-0 rounded-full"
             style={{
               width: `${fill}%`,
               background: 'linear-gradient(90deg, #00d4aa, #60a5fa)',
               boxShadow: '0 0 12px rgba(0,212,170,0.35)',
             }} />
      </div>

      <div className="mt-2 flex items-center justify-between text-[10px]">
        <span className="font-medium text-[#00d4aa]">{PROOF_LATENCY_MS}ms proof</span>
        <span className="text-[#4a4a6a]">{BLOCK_BUDGET_MS}ms block</span>
      </div>
    </div>
  )
}

function ChevronRight() {
  return (
    <svg className="h-4 w-4 text-[#3a3a5a] flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
    </svg>
  )
}
