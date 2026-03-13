import { CheckCircle, XCircle, Clock, Zap, Link, RefreshCw, AlertTriangle } from 'lucide-react'
import { useStore } from '../store'

interface BackgroundUpdate {
  id: string
  kind: 'task_complete' | 'task_failed' | 'escrow_locked' | 'escrow_released' | 'escrow_slashed' | 'operator_run' | 'chain_tx'
  title: string
  detail: string
  time: string
  txHash?: string
  agentName?: string
}

const MOCK_UPDATES: BackgroundUpdate[] = [
  {
    id: '1',
    kind: 'task_complete',
    title: 'Task completed',
    detail: 'Agent "Researcher" finished: "Summarise WAVS docs" · 1,842 tokens',
    time: '12 min ago',
    agentName: 'Researcher',
  },
  {
    id: '2',
    kind: 'escrow_released',
    title: 'Escrow released',
    detail: 'Task #42 · 250,000 ujunox → Researcher company DAO',
    time: '12 min ago',
    txHash: 'A3F9B2C1D4E5F6078901A2B3C4D5E6F7',
  },
  {
    id: '3',
    kind: 'operator_run',
    title: 'Operator validated',
    detail: 'WAVS operator confirmed task output hash on-chain',
    time: '13 min ago',
    txHash: 'B1C2D3E4F5061728394A5B6C7D8E9F00',
  },
  {
    id: '4',
    kind: 'task_failed',
    title: 'Task failed',
    detail: 'Agent "Coder" — shell execution timed out after 60s',
    time: '1h ago',
    agentName: 'Coder',
  },
  {
    id: '5',
    kind: 'escrow_locked',
    title: 'Escrow locked',
    detail: 'Task #45 · 150,000 ujunox held for "Coder"',
    time: '1h ago',
    txHash: 'C2D3E4F506172839',
  },
  {
    id: '6',
    kind: 'task_complete',
    title: 'Task completed',
    detail: 'Agent "Analyst" — "Weekly token analytics report" · 3,201 tokens',
    time: '4h ago',
    agentName: 'Analyst',
  },
  {
    id: '7',
    kind: 'chain_tx',
    title: 'Agent registered',
    detail: '"Analyst" registered on-chain · agent_id #7',
    time: '6h ago',
    txHash: 'D4E5F6071829304A',
  },
]

function updateIcon(kind: BackgroundUpdate['kind']) {
  switch (kind) {
    case 'task_complete':    return <CheckCircle className="h-3.5 w-3.5" />
    case 'task_failed':      return <XCircle     className="h-3.5 w-3.5" />
    case 'escrow_locked':    return <Clock       className="h-3.5 w-3.5" />
    case 'escrow_released':  return <CheckCircle className="h-3.5 w-3.5" />
    case 'escrow_slashed':   return <AlertTriangle className="h-3.5 w-3.5" />
    case 'operator_run':     return <Zap         className="h-3.5 w-3.5" />
    case 'chain_tx':         return <Link        className="h-3.5 w-3.5" />
  }
}

function updateColors(kind: BackgroundUpdate['kind']): { color: string; bg: string } {
  switch (kind) {
    case 'task_complete':    return { color: '#34d399', bg: 'rgba(52,211,153,0.1)' }
    case 'task_failed':      return { color: '#f87171', bg: 'rgba(248,113,113,0.1)' }
    case 'escrow_locked':    return { color: '#fbbf24', bg: 'rgba(251,191,36,0.1)' }
    case 'escrow_released':  return { color: '#34d399', bg: 'rgba(52,211,153,0.1)' }
    case 'escrow_slashed':   return { color: '#f97316', bg: 'rgba(249,115,22,0.1)' }
    case 'operator_run':     return { color: '#ff6b4a', bg: 'rgba(255,107,74,0.1)' }
    case 'chain_tx':         return { color: '#60a5fa', bg: 'rgba(96,165,250,0.1)' }
  }
}

function kindLabel(kind: BackgroundUpdate['kind']): string {
  switch (kind) {
    case 'task_complete':   return 'Task'
    case 'task_failed':     return 'Task'
    case 'escrow_locked':   return 'Escrow'
    case 'escrow_released': return 'Escrow'
    case 'escrow_slashed':  return 'Escrow'
    case 'operator_run':    return 'WAVS'
    case 'chain_tx':        return 'Chain'
  }
}

export function UpdatesPanel() {
  const connected = useStore((s) => s.connected)

  return (
    <div className="flex flex-1 flex-col bg-[#06060f] overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-3"
           style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg"
               style={{ background: 'rgba(96,165,250,0.1)', border: '1px solid rgba(96,165,250,0.2)' }}>
            <RefreshCw className="h-4 w-4" style={{ color: '#60a5fa' }} />
          </div>
          <div>
            <div className="text-sm font-semibold text-[#f0eff8]">Background Updates</div>
            <div className="text-[10px] text-[#6b6a8a]">Tasks · Escrow · Chain events while you were away</div>
          </div>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="h-1.5 w-1.5 rounded-full"
                style={{ background: connected ? '#34d399' : '#6b6a8a',
                         boxShadow: connected ? '0 0 5px rgba(52,211,153,0.5)' : 'none' }} />
          <span className="text-[10px]" style={{ color: connected ? '#34d399' : '#6b6a8a' }}>
            {connected ? 'Live' : 'Offline'}
          </span>
        </div>
      </div>

      <div className="px-6 py-5 space-y-4">
        {/* Summary row */}
        <div className="grid grid-cols-4 gap-2">
          {[
            { label: 'Completed', value: '2', color: '#34d399', bg: 'rgba(52,211,153,0.07)' },
            { label: 'Failed',    value: '1', color: '#f87171', bg: 'rgba(248,113,113,0.07)' },
            { label: 'Escrow',    value: '2', color: '#fbbf24', bg: 'rgba(251,191,36,0.07)' },
            { label: 'Chain txs', value: '3', color: '#60a5fa', bg: 'rgba(96,165,250,0.07)' },
          ].map((s) => (
            <div key={s.label}
                 className="rounded-xl p-3 text-center"
                 style={{ background: s.bg, border: `1px solid ${s.color}20` }}>
              <div className="text-lg font-bold" style={{ color: s.color }}>{s.value}</div>
              <div className="text-[10px] text-[#6b6a8a]">{s.label}</div>
            </div>
          ))}
        </div>

        {/* Feed */}
        <div>
          <div className="mb-3 text-xs font-semibold text-[#6b6a8a] uppercase tracking-widest">
            Event Log
          </div>
          <div className="space-y-1.5">
            {MOCK_UPDATES.map((update) => {
              const { color, bg } = updateColors(update.kind)
              return (
                <div key={update.id}
                     className="flex items-start gap-3 rounded-xl px-3 py-2.5 transition"
                     style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.04)' }}>
                  <div className="mt-0.5 flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-md"
                       style={{ background: bg, color }}>
                    {updateIcon(update.kind)}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-medium text-[#c0bfd8]">{update.title}</span>
                      <span className="rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wider"
                            style={{ color, background: bg }}>
                        {kindLabel(update.kind)}
                      </span>
                    </div>
                    <div className="text-[10px] text-[#6b6a8a] mt-0.5">{update.detail}</div>
                    {update.txHash && (
                      <a href={`https://mintscan.io/juno-testnet/tx/${update.txHash}`}
                         target="_blank"
                         rel="noopener noreferrer"
                         className="mt-1 flex items-center gap-1 text-[10px] transition hover:opacity-80"
                         style={{ color: '#60a5fa' }}>
                        <Link className="h-2.5 w-2.5" />
                        {update.txHash.slice(0, 16)}…
                      </a>
                    )}
                  </div>
                  <div className="flex-shrink-0 flex items-center gap-1 text-[10px] text-[#6b6a8a]">
                    <Clock className="h-2.5 w-2.5" />
                    {update.time}
                  </div>
                </div>
              )
            })}
          </div>
        </div>

        <div className="rounded-xl p-3 text-[11px] text-[#6b6a8a] leading-relaxed"
             style={{ background: 'rgba(96,165,250,0.04)', border: '1px solid rgba(96,165,250,0.1)' }}>
          <span style={{ color: '#60a5fa' }}>ℹ Live updates</span> — Background tasks, escrow events, and
          chain transactions are logged here in real-time via the daemon WebSocket. WAVS operator
          validations appear as they're confirmed on-chain.
        </div>
      </div>
    </div>
  )
}
