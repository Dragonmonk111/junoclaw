// ── Robot Ops — "Cortex Console" ──
//
// Visualizes JunoClaw's two-tier trust architecture for embodied agents:
// per-joint J-Lens verdicts (intent tier — gated), a Truth Market operator
// constellation, and a BFT-batch flight recorder. Per-joint/per-robot
// telemetry has no on-chain analog and stays on the local simulation
// (`useFleetSimulation`, see robot-ops-sim.ts). The Trust Constellation is
// wired to the real truth-market contract on uni-7 (`useTruthMarketLive`) —
// as of 2026-08-17 it legitimately shows 0 registered operators, which is
// itself the operator-bootstrap gap described in the article this panel
// backs (ARTICLE_ROBOT_IS_THE_AGENT_2026_08_17.md §5).

import { useState } from 'react'
import {
  Radio, Activity, Network, Clock3, ShieldCheck, ShieldAlert, ShieldQuestion,
  LayoutGrid, Bone, Zap, Hourglass, Ban, Check, Satellite,
} from 'lucide-react'
import { useFleetSimulation, ACTIONS_BY_MORPHOLOGY, EMERGENCY_STOP, SKELETON_POSES } from '../lib/robot-ops-sim'
import type { ActionRequest, FlightBatch, RobotState, Verdict } from '../lib/robot-ops-sim'
import { useTruthMarketLive } from '../hooks/useTruthMarketLive'
import type { TruthMarketOperator, TruthMarketStats } from '../lib/robot-ops-queries'

function verdictColor(v: Verdict): string {
  if (v === 'green') return '#00d4aa'
  if (v === 'amber') return '#ffb84d'
  return '#ff4d6a'
}

function verdictBg(v: Verdict): string {
  if (v === 'green') return 'rgba(0,212,170,0.12)'
  if (v === 'amber') return 'rgba(255,184,77,0.14)'
  return 'rgba(255,77,106,0.16)'
}

function healthColor(health: number): string {
  if (health >= 80) return '#00d4aa'
  if (health >= 50) return '#ffb84d'
  return '#ff4d6a'
}

function timeAgo(ts: number): string {
  const s = Math.max(0, Math.round((Date.now() - ts) / 1000))
  if (s < 1) return 'now'
  if (s < 60) return `${s}s ago`
  return `${Math.round(s / 60)}m ago`
}

function FleetSelector({
  robots,
  focusedId,
  onFocus,
}: {
  robots: RobotState[]
  focusedId: string
  onFocus: (id: string) => void
}) {
  return (
    <div className="flex gap-2">
      {robots.map((r) => {
        const isFocused = r.id === focusedId
        return (
          <button
            key={r.id}
            onClick={() => onFocus(r.id)}
            className="flex min-w-[130px] flex-col gap-1.5 rounded-lg px-3 py-2 text-left transition-all"
            style={isFocused ? {
              background: 'rgba(255,107,74,0.08)',
              border: '1px solid rgba(255,107,74,0.3)',
            } : {
              background: 'rgba(255,255,255,0.02)',
              border: '1px solid rgba(255,255,255,0.06)',
            }}
          >
            <div className="flex items-center justify-between">
              <span className="text-[11px] font-semibold" style={{ color: '#f0eff8' }}>{r.name}</span>
              <span
                className="h-1.5 w-1.5 rounded-full"
                style={{ background: healthColor(r.health), boxShadow: `0 0 6px ${healthColor(r.health)}` }}
              />
            </div>
            <span className="text-[9px] uppercase tracking-wide" style={{ color: '#6b6a8a' }}>
              {r.morphology}
            </span>
            <div className="h-1 w-full overflow-hidden rounded-full" style={{ background: 'rgba(255,255,255,0.06)' }}>
              <div
                className="h-full rounded-full transition-all"
                style={{ width: `${r.health}%`, background: healthColor(r.health) }}
              />
            </div>
          </button>
        )
      })}
    </div>
  )
}

function JointGrid({ robot }: { robot: RobotState }) {
  return (
    <div
      className="grid gap-2 rounded-xl p-4"
      style={{
        gridTemplateColumns: `repeat(${robot.cols}, minmax(64px, 1fr))`,
        gridTemplateRows: `repeat(${robot.rows}, 44px)`,
        background: 'rgba(255,255,255,0.015)',
        border: '1px solid rgba(255,255,255,0.05)',
      }}
    >
      {robot.joints.map((j) => (
        <div
          key={j.id}
          title={`${j.label} — ${j.verdict} (${Math.round(j.confidence)}%)`}
          className={`flex flex-col items-center justify-center rounded-md text-center transition-colors ${
            j.verdict !== 'green' ? 'animate-pulse-slow' : ''
          }`}
          style={{
            gridRow: j.row,
            gridColumn: j.col,
            background: verdictBg(j.verdict),
            border: `1px solid ${verdictColor(j.verdict)}55`,
            boxShadow: j.verdict !== 'green' ? `0 0 10px ${verdictColor(j.verdict)}40` : 'none',
          }}
        >
          <span className="text-[8px] font-semibold uppercase tracking-wide" style={{ color: verdictColor(j.verdict) }}>
            {j.label}
          </span>
          <span className="text-[9px] font-mono" style={{ color: '#c0bfd8' }}>
            {Math.round(j.confidence)}%
          </span>
        </div>
      ))}
    </div>
  )
}

function SkeletonView({ robot }: { robot: RobotState }) {
  const pose = SKELETON_POSES[robot.morphology]
  const verdictById = new Map(robot.joints.map((j) => [j.id, j]))
  return (
    <div
      className="flex justify-center rounded-xl p-4"
      style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}
    >
      <svg viewBox={pose.viewBox} style={{ height: 260, maxWidth: '100%' }}>
        {pose.bones.map(([a, b]) => {
          const pa = pose.positions[a]
          const pb = pose.positions[b]
          if (!pa || !pb) return null
          return (
            <line
              key={`${a}-${b}`}
              x1={pa.x} y1={pa.y} x2={pb.x} y2={pb.y}
              stroke="rgba(255,255,255,0.18)"
              strokeWidth={2.5}
              strokeLinecap="round"
            />
          )
        })}
        {Object.entries(pose.positions).map(([id, p]) => {
          const j = verdictById.get(id)
          if (!j) return null
          const color = verdictColor(j.verdict)
          return (
            <g key={id}>
              <circle
                cx={p.x} cy={p.y} r={6.5}
                fill={color}
                opacity={0.9}
                style={{ filter: j.verdict !== 'green' ? `drop-shadow(0 0 5px ${color})` : 'none' }}
              >
                <title>{`${j.label} — ${j.verdict} (${Math.round(j.confidence)}%)`}</title>
              </circle>
            </g>
          )
        })}
      </svg>
    </div>
  )
}

const ACTION_STATUS_META: Record<ActionRequest['status'], { label: string; color: string; icon: JSX.Element }> = {
  pending: { label: 'Pending', color: '#6b6a8a', icon: <Hourglass className="h-2.5 w-2.5" /> },
  probing: { label: 'Probing', color: '#ffb84d', icon: <ShieldQuestion className="h-2.5 w-2.5" /> },
  approved: { label: 'Approved', color: '#00d4aa', icon: <Check className="h-2.5 w-2.5" /> },
  blocked: { label: 'Blocked', color: '#ff4d6a', icon: <Ban className="h-2.5 w-2.5" /> },
}

function ActionConsole({
  robot,
  actions,
  onSubmit,
}: {
  robot: RobotState
  actions: ActionRequest[]
  onSubmit: (label: string) => void
}) {
  const robotActions = actions.filter((a) => a.robotId === robot.id).slice().reverse()
  const available = ACTIONS_BY_MORPHOLOGY[robot.morphology]

  return (
    <div className="rounded-xl p-3" style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}>
      <div className="mb-2 flex items-center gap-1.5">
        <Zap className="h-3 w-3" style={{ color: '#ff6b4a' }} />
        <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
          Robotic Actions · {robot.name}
        </span>
      </div>

      <div className="mb-3 flex flex-wrap gap-1.5">
        {available.map((label) => {
          const isStop = label === EMERGENCY_STOP
          return (
            <button
              key={label}
              onClick={() => onSubmit(label)}
              className="rounded-md px-2.5 py-1.5 text-[10px] font-semibold transition active:scale-95"
              style={isStop ? {
                color: '#ff4d6a',
                background: 'rgba(255,77,106,0.12)',
                border: '1px solid rgba(255,77,106,0.35)',
              } : {
                color: '#f0eff8',
                background: 'rgba(255,107,74,0.08)',
                border: '1px solid rgba(255,107,74,0.2)',
              }}
            >
              {label}
            </button>
          )
        })}
      </div>

      <div className="max-h-56 space-y-1.5 overflow-y-auto pr-1">
        {robotActions.length === 0 && (
          <p className="text-[10px]" style={{ color: '#4a4a6a' }}>No actions requested yet.</p>
        )}
        {robotActions.map((a) => {
          const meta = ACTION_STATUS_META[a.status]
          const agreeCount = a.votes.filter((v) => v.agrees).length
          return (
            <div
              key={a.id}
              className="flex items-center justify-between rounded-lg px-2.5 py-1.5"
              style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.05)' }}
            >
              <div className="min-w-0">
                <div className="truncate text-[11px] font-medium" style={{ color: '#f0eff8' }}>{a.label}</div>
                <div className="text-[9px]" style={{ color: '#6b6a8a' }}>
                  {a.localReflex
                    ? 'LOCAL REFLEX — no consensus required'
                    : a.status === 'approved' || a.status === 'blocked'
                      ? `${agreeCount}/${a.votes.length} operators agree`
                      : timeAgo(a.submittedAt)}
                </div>
              </div>
              <span
                className="flex flex-shrink-0 items-center gap-1 rounded px-1.5 py-0.5 text-[9px] font-semibold"
                style={{ color: meta.color, background: `${meta.color}22` }}
              >
                {meta.icon}
                {meta.label}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function EkgStrip({
  robot,
  isLive,
  viewedIndex,
  maxIndex,
  onScrub,
  onGoLive,
}: {
  robot: RobotState
  isLive: boolean
  viewedIndex: number
  maxIndex: number
  onScrub: (i: number) => void
  onGoLive: () => void
}) {
  const samples = robot.confidenceHistory
  const w = 300
  const h = 56
  const step = w / (samples.length - 1 || 1)
  const points = samples
    .map((v, i) => `${(i * step).toFixed(1)},${(h - (v / 100) * h).toFixed(1)}`)
    .join(' ')
  const color = healthColor(robot.health)

  return (
    <div className="rounded-xl p-3" style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}>
      <div className="mb-2 flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <Activity className="h-3 w-3" style={{ color: '#ff6b4a' }} />
          <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
            Cognitive EKG · {robot.name}
          </span>
        </div>
        <button
          onClick={onGoLive}
          className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[9px] font-semibold transition"
          style={isLive ? { color: '#00d4aa', background: 'rgba(0,212,170,0.1)' } : { color: '#6b6a8a', background: 'rgba(255,255,255,0.04)' }}
        >
          <Radio className="h-2.5 w-2.5" />
          {isLive ? 'LIVE' : 'PAUSED'}
        </button>
      </div>
      <svg viewBox={`0 0 ${w} ${h}`} className="w-full" style={{ height: 60 }}>
        {[0.25, 0.5, 0.75].map((f) => (
          <line key={f} x1={0} x2={w} y1={h * f} y2={h * f} stroke="rgba(255,255,255,0.05)" strokeWidth={1} />
        ))}
        <polyline
          points={points}
          fill="none"
          stroke={color}
          strokeWidth={1.75}
          style={{ filter: `drop-shadow(0 0 4px ${color}80)` }}
        />
      </svg>
      <input
        type="range"
        min={0}
        max={maxIndex}
        value={viewedIndex}
        onChange={(e) => onScrub(Number(e.target.value))}
        className="mt-1.5 w-full accent-[#ff6b4a]"
      />
    </div>
  )
}

function TrustConstellation({
  operators,
  isLive,
  liveStats,
  liveError,
}: {
  operators: { id: string; label: string; agrees: boolean }[]
  isLive: boolean
  liveStats: TruthMarketStats | null
  liveError: string | null
}) {
  const size = 200
  const cx = size / 2
  const cy = size / 2
  const r = 70
  return (
    <div className="rounded-xl p-3" style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}>
      <div className="mb-2 flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <Network className="h-3 w-3" style={{ color: '#ff6b4a' }} />
          <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
            Trust Constellation
          </span>
        </div>
        <span
          className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[9px] font-semibold"
          style={isLive
            ? { color: '#00d4aa', background: 'rgba(0,212,170,0.1)' }
            : { color: '#6b6a8a', background: 'rgba(255,255,255,0.04)' }}
          title={isLive ? 'Real truth-market contract on uni-7' : 'Local simulation — chain query unavailable/empty'}
        >
          <Satellite className="h-2.5 w-2.5" />
          {isLive ? 'LIVE · uni-7' : 'SIM'}
        </span>
      </div>
      {operators.length === 0 ? (
        <div className="flex flex-col items-center gap-1.5 py-8 text-center">
          <ShieldAlert className="h-5 w-5" style={{ color: '#ff4d6a' }} />
          <span className="text-[10px] font-semibold" style={{ color: '#ff4d6a' }}>
            0 operators registered on-chain
          </span>
          <span className="max-w-[180px] text-[9px]" style={{ color: '#6b6a8a' }}>
            {liveError
              ? `Chain query failed: ${liveError}`
              : `Truth Market requires ${liveStats?.minOperators ?? 3} independent operators before an epoch can finalize.`}
          </span>
        </div>
      ) : (
        <svg viewBox={`0 0 ${size} ${size}`} className="mx-auto" style={{ width: '100%', maxWidth: 220 }}>
          {operators.map((op, i) => {
            const angle = (i / operators.length) * Math.PI * 2 - Math.PI / 2
            const x = cx + r * Math.cos(angle)
            const y = cy + r * Math.sin(angle)
            const color = op.agrees ? '#00d4aa' : '#ff4d6a'
            return (
              <g key={op.id}>
                <line x1={cx} y1={cy} x2={x} y2={y} stroke={color} strokeWidth={op.agrees ? 1 : 1.75} opacity={op.agrees ? 0.5 : 0.9} />
                <circle cx={x} cy={y} r={7} fill={color} opacity={0.9} style={{ filter: `drop-shadow(0 0 4px ${color})` }} />
                <text x={x} y={y + 18} textAnchor="middle" fontSize={7} fill="#8a89a6">
                  {op.label.replace('op-', '')}
                </text>
              </g>
            )
          })}
          <circle cx={cx} cy={cy} r={14} fill="rgba(255,107,74,0.15)" stroke="#ff6b4a" strokeWidth={1} />
          <text x={cx} y={cy + 3} textAnchor="middle" fontSize={6.5} fill="#ff6b4a" fontWeight={700}>
            MARKET
          </text>
        </svg>
      )}
      {isLive && liveStats && operators.length > 0 && (
        <div className="mt-2 flex items-center justify-between text-[9px]" style={{ color: '#6b6a8a' }}>
          <span>{liveStats.activeOperators}/{liveStats.minOperators} min required</span>
          <span>{liveStats.epochsFinalized} epochs finalized</span>
        </div>
      )}
    </div>
  )
}

function FlightRecorder({
  batches,
  selected,
  onSelect,
}: {
  batches: FlightBatch[]
  selected: FlightBatch | null
  onSelect: (b: FlightBatch) => void
}) {
  return (
    <div className="rounded-xl p-3" style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}>
      <div className="mb-2 flex items-center gap-1.5">
        <Clock3 className="h-3 w-3" style={{ color: '#ff6b4a' }} />
        <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
          Flight Recorder
        </span>
      </div>
      <div className="flex gap-1 overflow-x-auto pb-1">
        {batches.map((b) => {
          const isSelected = selected?.batchHeight === b.batchHeight
          const color = b.verdict === 'green' ? '#00d4aa' : '#ff4d6a'
          return (
            <button
              key={b.batchHeight}
              onClick={() => onSelect(b)}
              className="h-6 w-3 flex-shrink-0 rounded-sm transition-transform"
              style={{
                background: color,
                opacity: b.verdict === 'green' ? 0.55 : 0.9,
                transform: isSelected ? 'scaleY(1.25)' : 'scaleY(1)',
                boxShadow: isSelected ? `0 0 6px ${color}` : 'none',
              }}
              title={`Batch ${b.batchHeight} · ${b.verdict}`}
            />
          )
        })}
      </div>
      {selected && (
        <div className="mt-3 rounded-lg p-2.5" style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.05)' }}>
          <div className="flex items-center justify-between">
            <span className="text-[10px] font-mono" style={{ color: '#c0bfd8' }}>batch #{selected.batchHeight}</span>
            <span
              className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase"
              style={{ color: verdictColor(selected.verdict), background: verdictBg(selected.verdict) }}
            >
              {selected.verdict === 'green' ? <ShieldCheck className="h-2.5 w-2.5" /> : <ShieldAlert className="h-2.5 w-2.5" />}
              {selected.verdict}
            </span>
          </div>
          <div className="mt-1 break-all font-mono text-[9px]" style={{ color: '#6b6a8a' }}>{selected.certHash}</div>
          <div className="mt-1 flex items-center justify-between text-[9px]" style={{ color: '#6b6a8a' }}>
            <span>{timeAgo(selected.timestamp)}</span>
            {selected.flaggedRobot && <span style={{ color: '#ff4d6a' }}>flagged: {selected.flaggedRobot}</span>}
          </div>
        </div>
      )}
    </div>
  )
}

export function RobotOpsPanel() {
  const { state, history, viewedIndex, isLive, scrub, goLive, submitAction } = useFleetSimulation(900)
  const truthMarket = useTruthMarketLive()
  const [focusedId, setFocusedId] = useState(state.robots[0]?.id ?? '')
  const [selectedBatch, setSelectedBatch] = useState<FlightBatch | null>(null)
  const [structureView, setStructureView] = useState<'blocks' | 'skeleton'>('blocks')

  // "Live" means the chain query itself succeeded — not that it returned a
  // non-empty operator list. As of 2026-08-17 the truth-market contract on
  // uni-7 genuinely has 0 registered operators; showing that honestly (not
  // silently falling back to the 5-operator simulation) is the point — see
  // ARTICLE_ROBOT_IS_THE_AGENT_2026_08_17.md §5. The simulation is only the
  // fallback when the live query hasn't resolved yet or actually failed.
  const chainReachable = truthMarket.lastFetched !== null && !truthMarket.error
  const constellationOperators = chainReachable
    ? truthMarket.operators.map((op: TruthMarketOperator) => ({
        id: op.address,
        label: op.address.slice(-8),
        agrees: op.active,
      }))
    : state.operators

  const focusedRobot = state.robots.find((r) => r.id === focusedId) ?? state.robots[0]
  const fleetHealth = state.robots.length
    ? Math.round(state.robots.reduce((s, r) => s + r.health, 0) / state.robots.length)
    : 100
  const auraColor = healthColor(fleetHealth)
  const latestBatch = state.batches[state.batches.length - 1] ?? null
  const activeBatch = selectedBatch ?? latestBatch

  if (!focusedRobot) return null

  return (
    <div className="relative flex-1 overflow-y-auto p-5" style={{ background: '#050510' }}>
      {/* Ambient aura — hue tracks aggregate fleet health */}
      <div
        className="pointer-events-none absolute inset-0 opacity-30 transition-colors duration-1000"
        style={{
          background: `radial-gradient(circle at 20% 0%, ${auraColor}22, transparent 55%)`,
        }}
      />

      <div className="relative mx-auto max-w-7xl">
        <header className="mb-4 flex items-center justify-between">
          <div>
            <h2 className="text-sm font-semibold" style={{ color: '#f0eff8' }}>Robot Ops</h2>
            <p className="mt-0.5 text-[11px]" style={{ color: '#6b6a8a' }}>
              Intent-tier J-Lens gating across the fleet — reflex-tier balance/locomotion loops run
              locally on-device and are not gated here (see Trust OS architecture notes).
            </p>
          </div>
          <div className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5" style={{ background: 'rgba(255,255,255,0.03)' }}>
            <ShieldQuestion className="h-3.5 w-3.5" style={{ color: auraColor }} />
            <span className="text-[10px] font-semibold" style={{ color: auraColor }}>
              Fleet health {fleetHealth}%
            </span>
          </div>
        </header>

        <div className="mb-4">
          <FleetSelector robots={state.robots} focusedId={focusedRobot.id} onFocus={setFocusedId} />
        </div>

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
          <div className="space-y-4">
            <div className="flex items-center justify-end gap-1">
              <button
                onClick={() => setStructureView('blocks')}
                className="flex items-center gap-1 rounded-md px-2 py-1 text-[9px] font-semibold transition"
                style={structureView === 'blocks'
                  ? { color: '#ff6b4a', background: 'rgba(255,107,74,0.12)' }
                  : { color: '#6b6a8a', background: 'rgba(255,255,255,0.03)' }}
              >
                <LayoutGrid className="h-2.5 w-2.5" /> Blocks
              </button>
              <button
                onClick={() => setStructureView('skeleton')}
                className="flex items-center gap-1 rounded-md px-2 py-1 text-[9px] font-semibold transition"
                style={structureView === 'skeleton'
                  ? { color: '#ff6b4a', background: 'rgba(255,107,74,0.12)' }
                  : { color: '#6b6a8a', background: 'rgba(255,255,255,0.03)' }}
              >
                <Bone className="h-2.5 w-2.5" /> Skeleton
              </button>
            </div>
            {structureView === 'blocks' ? <JointGrid robot={focusedRobot} /> : <SkeletonView robot={focusedRobot} />}
            <EkgStrip
              robot={focusedRobot}
              isLive={isLive}
              viewedIndex={viewedIndex}
              maxIndex={history.length - 1}
              onScrub={scrub}
              onGoLive={goLive}
            />
          </div>
          <div className="space-y-4">
            <ActionConsole
              robot={focusedRobot}
              actions={state.actions}
              onSubmit={(label) => submitAction(focusedRobot.id, label)}
            />
          </div>
          <div className="space-y-4">
            <TrustConstellation
              operators={constellationOperators}
              isLive={chainReachable}
              liveStats={truthMarket.stats}
              liveError={truthMarket.error}
            />
            <FlightRecorder
              batches={state.batches}
              selected={activeBatch}
              onSelect={setSelectedBatch}
            />
          </div>
        </div>
      </div>
    </div>
  )
}
