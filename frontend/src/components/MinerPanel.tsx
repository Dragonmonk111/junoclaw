// ── Miner Panel — Truth Market mining dashboard ──
//
// Shows live truth market state from uni-7: operators, stakes, rewards,
// fingerprints, and epoch history. Includes a "How to Start Mining" guide
// for the three miner types: Robot (Jetson Orin), GPU (bare-metal), Akash TEE.

import { useState } from 'react'
import {
  Cpu, HardDrive, Cloud, Activity, Users, Coins, TrendingUp, AlertTriangle,
  Fingerprint, RefreshCw, CheckCircle2, XCircle, Timer, BookOpen, Terminal,
} from 'lucide-react'
import { useMinerLive } from '../hooks/useMinerLive'
import type { MinerOperator } from '../lib/miner-queries'

function formatAmount(ustr: string, decimals = 6): string {
  const n = BigInt(ustr || '0')
  const whole = n / BigInt(10 ** decimals)
  const frac = n % BigInt(10 ** decimals)
  return `${whole}.${frac.toString().padStart(decimals, '0').slice(0, 2)}`
}

function shortAddr(addr: string): string {
  if (addr.length <= 12) return addr
  return `${addr.slice(0, 8)}…${addr.slice(-4)}`
}

function accuracyColor(acc: number): string {
  if (acc >= 80) return '#00d4aa'
  if (acc >= 50) return '#ffb84d'
  return '#ff4d6a'
}

function MinerTypeCard({ icon, title, desc, example, color }: {
  icon: React.ReactNode
  title: string
  desc: string
  example: string
  color: string
}) {
  return (
    <div className="rounded-lg p-4" style={{ background: 'rgba(255,255,255,0.03)', border: `1px solid ${color}22` }}>
      <div className="flex items-center gap-2 mb-2">
        <span style={{ color }}>{icon}</span>
        <span className="text-sm font-semibold" style={{ color }}>{title}</span>
      </div>
      <p className="text-xs leading-relaxed mb-2" style={{ color: '#8b8aa8' }}>{desc}</p>
      <div className="rounded px-2 py-1.5" style={{ background: 'rgba(0,0,0,0.3)' }}>
        <p className="text-[10px] font-mono" style={{ color: '#6b6a8a' }}>{example}</p>
      </div>
    </div>
  )
}

function OperatorRow({ op }: { op: MinerOperator }) {
  const [expanded, setExpanded] = useState(false)
  return (
    <div>
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-3 px-3 py-2 rounded text-left transition-all hover:bg-white/[0.03]"
      >
        <span style={{ color: op.active ? '#00d4aa' : '#6b6a8a' }}>
          {op.active ? <CheckCircle2 className="h-3.5 w-3.5" /> : <XCircle className="h-3.5 w-3.5" />}
        </span>
        <span className="text-xs font-mono flex-1" style={{ color: '#c0bfd6' }}>
          {shortAddr(op.address)}
        </span>
        <span className="text-xs" style={{ color: '#8b8aa8' }}>
          {formatAmount(op.stake)} JUNOX
        </span>
        <span className="text-xs font-mono" style={{ color: accuracyColor(op.accuracy) }}>
          {op.accuracy}%
        </span>
        <span className="text-xs" style={{ color: '#6b6a8a' }}>
          {op.epochsParticipated} ep
        </span>
      </button>
      {expanded && (
        <div className="px-6 py-2 text-[11px] grid grid-cols-2 gap-x-4 gap-y-1" style={{ color: '#6b6a8a' }}>
          <span>Rewards: <span style={{ color: '#00d4aa' }}>{formatAmount(op.totalRewards)} JUNOX</span></span>
          <span>Slashed: <span style={{ color: '#ff4d6a' }}>{formatAmount(op.totalSlashed)} JUNOX</span></span>
          <span>Correct: {op.correctVerdicts}</span>
          <span>Incorrect: {op.incorrectVerdicts}</span>
          {op.fingerprint && (
            <span className="col-span-2 font-mono text-[10px]" style={{ color: '#4a4968' }}>
              FP: {op.fingerprint.slice(0, 24)}…
            </span>
          )}
        </div>
      )}
    </div>
  )
}

export function MinerPanel() {
  const { operators, stats, config, fingerprints, loading, error, lastFetched, refresh } = useMinerLive()
  const [showGuide, setShowGuide] = useState(false)

  if (loading && !stats) {
    return (
      <div className="flex items-center justify-center h-full">
        <RefreshCw className="h-5 w-5 animate-spin" style={{ color: '#ff6b4a' }} />
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-3" style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
        <div className="flex items-center gap-2">
          <Cpu className="h-4 w-4" style={{ color: '#ff6b4a' }} />
          <span className="text-sm font-semibold" style={{ color: '#f0eff8' }}>Truth Market Miners</span>
          {error && <span className="text-[10px] px-2 py-0.5 rounded" style={{ color: '#ff4d6a', background: 'rgba(255,77,106,0.1)' }}>offline</span>}
        </div>
        <div className="flex items-center gap-3">
          {lastFetched && <span className="text-[10px]" style={{ color: '#4a4968' }}>{new Date(lastFetched).toLocaleTimeString()}</span>}
          <button onClick={refresh} className="p-1 rounded hover:bg-white/[0.05]">
            <RefreshCw className="h-3.5 w-3.5" style={{ color: '#6b6a8a' }} />
          </button>
        </div>
      </div>

      <div className="flex-1 px-5 py-4 space-y-4">
        {/* Stats Grid */}
        {stats && (
          <div className="grid grid-cols-4 gap-3">
            <div className="rounded-lg p-3" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="flex items-center gap-1.5 mb-1">
                <Users className="h-3 w-3" style={{ color: '#6b6a8a' }} />
                <span className="text-[10px] uppercase tracking-wider" style={{ color: '#6b6a8a' }}>Operators</span>
              </div>
              <span className="text-xl font-bold" style={{ color: '#f0eff8' }}>{stats.totalOperators}</span>
              <span className="text-[10px] ml-1" style={{ color: '#4a4968' }}>({stats.activeOperators} active)</span>
            </div>
            <div className="rounded-lg p-3" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="flex items-center gap-1.5 mb-1">
                <Coins className="h-3 w-3" style={{ color: '#6b6a8a' }} />
                <span className="text-[10px] uppercase tracking-wider" style={{ color: '#6b6a8a' }}>Staked</span>
              </div>
              <span className="text-xl font-bold" style={{ color: '#f0eff8' }}>{formatAmount(stats.totalStaked)}</span>
              <span className="text-[10px] ml-1" style={{ color: '#4a4968' }}>JUNOX</span>
            </div>
            <div className="rounded-lg p-3" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="flex items-center gap-1.5 mb-1">
                <TrendingUp className="h-3 w-3" style={{ color: '#6b6a8a' }} />
                <span className="text-[10px] uppercase tracking-wider" style={{ color: '#6b6a8a' }}>Reward Pool</span>
              </div>
              <span className="text-xl font-bold" style={{ color: '#00d4aa' }}>{formatAmount(stats.rewardPool)}</span>
              <span className="text-[10px] ml-1" style={{ color: '#4a4968' }}>JUNOX</span>
            </div>
            <div className="rounded-lg p-3" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <div className="flex items-center gap-1.5 mb-1">
                <Activity className="h-3 w-3" style={{ color: '#6b6a8a' }} />
                <span className="text-[10px] uppercase tracking-wider" style={{ color: '#6b6a8a' }}>Epochs</span>
              </div>
              <span className="text-xl font-bold" style={{ color: '#f0eff8' }}>{stats.epochsFinalized}</span>
              <span className="text-[10px] ml-1" style={{ color: '#4a4968' }}>finalized</span>
            </div>
          </div>
        )}

        {/* Config Summary */}
        {config && (
          <div className="flex items-center gap-4 text-[11px] px-3 py-2 rounded-lg" style={{ background: 'rgba(255,255,255,0.02)' }}>
            <span style={{ color: '#6b6a8a' }}>Min stake: <span style={{ color: '#c0bfd6' }}>{formatAmount(config.minStake)} JUNOX</span></span>
            <span style={{ color: '#6b6a8a' }}>Slash: <span style={{ color: '#ff4d6a' }}>{config.slashPercent}%</span></span>
            <span style={{ color: '#6b6a8a' }}>Reward: <span style={{ color: '#00d4aa' }}>{config.rewardPercent}%</span></span>
            <span style={{ color: '#6b6a8a' }}>Min operators: <span style={{ color: '#c0bfd6' }}>{config.minOperators}</span></span>
            {config.unstakeCooldownSecs > 0 && (
              <span style={{ color: '#6b6a8a' }}>Cooldown: <span style={{ color: '#c0bfd6' }}>{Math.round(config.unstakeCooldownSecs / 3600)}h</span></span>
            )}
          </div>
        )}

        {/* Miner Types Guide */}
        <div>
          <button
            onClick={() => setShowGuide(!showGuide)}
            className="flex items-center gap-2 text-xs font-semibold mb-2"
            style={{ color: '#ff6b4a' }}
          >
            <BookOpen className="h-3.5 w-3.5" />
            {showGuide ? 'Hide' : 'Show'} Mining Guide
          </button>
          {showGuide && (
            <div className="space-y-2 mb-3">
              <MinerTypeCard
                icon={<Cpu className="h-4 w-4" />}
                title="Robot Miner (Jetson Orin)"
                desc="A robot mines truth during idle time. After housekeeping, Rosie sits in her charging dock and evaluates batches from robots on the other side of the planet. 3B model at 15 tok/s on 30W."
                example="junoclaw-miner run --evaluator local --llm-endpoint http://localhost:11434 --llm-model qwen-3b --identity-type robot --hardware jetson-orin"
                color="#00d4aa"
              />
              <MinerTypeCard
                icon={<HardDrive className="h-4 w-4" />}
                title="GPU Miner (Bare-Metal)"
                desc="Anyone with a GPU rig can mine truth. Stake JUNO, run an open-weight model, evaluate batches, earn rewards. Like Bitcoin mining but for robot safety verification."
                example="junoclaw-miner run --evaluator local --llm-endpoint http://localhost:8080 --llm-model llama-70b --identity-type gpu --hardware dgx-spark"
                color="#ff6b4a"
              />
              <MinerTypeCard
                icon={<Cloud className="h-4 w-4" />}
                title="Akash TEE Miner"
                desc="Open-weight model running in a Trusted Execution Environment on Akash. The TEE attests to the exact model and inference — verifiable without owning hardware."
                example="junoclaw-miner run --evaluator akash-tee --llm-endpoint https://akash-deploy.example.com --llm-api-key ak-... --llm-model mistral-8x22b --identity-type akash-tee --hardware akash-h100-tee"
                color="#7c6bff"
              />
              <div className="rounded-lg p-3" style={{ background: 'rgba(255,107,74,0.06)', border: '1px solid rgba(255,107,74,0.15)' }}>
                <div className="flex items-center gap-1.5 mb-1">
                  <AlertTriangle className="h-3 w-3" style={{ color: '#ff6b4a' }} />
                  <span className="text-[11px] font-semibold" style={{ color: '#ff6b4a' }}>Open-weight models only</span>
                </div>
                <p className="text-[10px] leading-relaxed" style={{ color: '#8b8aa8' }}>
                  Only open-weight models (Llama, Qwen, Mistral, DeepSeek) qualify as J-Lens miners.
                  Closed-weight API models (GPT-4o, Claude, Gemini) cannot be verified — the miner
                  can't prove what model ran or that it ran faithfully.
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Operators List */}
        <div>
          <div className="flex items-center gap-2 mb-2">
            <Users className="h-3.5 w-3.5" style={{ color: '#6b6a8a' }} />
            <span className="text-xs font-semibold" style={{ color: '#c0bfd6' }}>Registered Operators</span>
            <span className="text-[10px] px-1.5 py-0.5 rounded" style={{ color: '#6b6a8a', background: 'rgba(255,255,255,0.04)' }}>{operators.length}</span>
          </div>
          {operators.length === 0 ? (
            <div className="text-center py-8 rounded-lg" style={{ background: 'rgba(255,255,255,0.02)' }}>
              <Terminal className="h-6 w-6 mx-auto mb-2" style={{ color: '#4a4968' }} />
              <p className="text-xs" style={{ color: '#6b6a8a' }}>No operators registered yet</p>
              <p className="text-[10px] mt-1" style={{ color: '#4a4968' }}>Be the first to mine truth for robots</p>
            </div>
          ) : (
            <div className="space-y-0.5">
              {operators.map((op) => <OperatorRow key={op.address} op={op} />)}
            </div>
          )}
        </div>

        {/* Fingerprint Diversity */}
        {fingerprints && fingerprints.fingerprints.length > 0 && (
          <div>
            <div className="flex items-center gap-2 mb-2">
              <Fingerprint className="h-3.5 w-3.5" style={{ color: '#6b6a8a' }} />
              <span className="text-xs font-semibold" style={{ color: '#c0bfd6' }}>Fingerprint Diversity</span>
            </div>
            <div className="space-y-1">
              {fingerprints.fingerprints.map((fp, i) => (
                <div key={i} className="flex items-center gap-2 px-3 py-1.5 rounded text-xs" style={{ background: 'rgba(255,255,255,0.02)' }}>
                  <Fingerprint className="h-3 w-3" style={{ color: '#7c6bff' }} />
                  <span className="font-mono text-[10px] flex-1" style={{ color: '#8b8aa8' }}>{fp.fingerprint.slice(0, 20)}…</span>
                  <span style={{ color: '#6b6a8a' }}>{fp.operatorCount} operator{fp.operatorCount !== 1 ? 's' : ''}</span>
                </div>
              ))}
              {fingerprints.operatorsWithoutFingerprint > 0 && (
                <div className="flex items-center gap-2 px-3 py-1.5 rounded text-xs" style={{ background: 'rgba(255,255,255,0.02)' }}>
                  <Timer className="h-3 w-3" style={{ color: '#4a4968' }} />
                  <span style={{ color: '#6b6a8a' }}>{fingerprints.operatorsWithoutFingerprint} without fingerprint</span>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Error */}
        {error && (
          <div className="rounded-lg p-3" style={{ background: 'rgba(255,77,106,0.06)', border: '1px solid rgba(255,77,106,0.15)' }}>
            <div className="flex items-center gap-1.5 mb-1">
              <AlertTriangle className="h-3 w-3" style={{ color: '#ff4d6a' }} />
              <span className="text-[11px] font-semibold" style={{ color: '#ff4d6a' }}>Connection Error</span>
            </div>
            <p className="text-[10px] font-mono" style={{ color: '#8b8aa8' }}>{error}</p>
          </div>
        )}
      </div>
    </div>
  )
}
