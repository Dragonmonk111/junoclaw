import { useState } from 'react'
import {
  Terminal, Search, Send, FileText, Globe, ArrowLeftRight,
  Code, Image, ChevronDown, ChevronRight, Check, X, Clock,
  Loader2, AlertTriangle, Shield,
} from 'lucide-react'
import type { ToolCallRecord, ToolCategory, ToolCallStatus } from '../types'
import { TOOL_CATEGORY_META } from '../types'

// ── Icon resolver ──

const ICON_MAP: Record<string, React.ComponentType<{ className?: string }>> = {
  Terminal, Search, Send, FileText, Globe, ArrowLeftRight, Code, Image,
}

function CategoryIcon({ category, className }: { category: ToolCategory; className?: string }) {
  const meta = TOOL_CATEGORY_META[category]
  const IconComp = ICON_MAP[meta.icon] || Terminal
  return <IconComp className={className} />
}

// ── Status badge ──

function StatusBadge({ status }: { status: ToolCallStatus }) {
  switch (status) {
    case 'pending':
      return (
        <span className="flex items-center gap-1 text-[10px] font-medium text-yellow-400">
          <Clock className="h-2.5 w-2.5" /> Waiting
        </span>
      )
    case 'approved':
    case 'running':
      return (
        <span className="flex items-center gap-1 text-[10px] font-medium text-blue-400">
          <Loader2 className="h-2.5 w-2.5 animate-spin" /> Running
        </span>
      )
    case 'completed':
      return (
        <span className="flex items-center gap-1 text-[10px] font-medium text-green-400">
          <Check className="h-2.5 w-2.5" /> Done
        </span>
      )
    case 'denied':
      return (
        <span className="flex items-center gap-1 text-[10px] font-medium text-red-400">
          <X className="h-2.5 w-2.5" /> Denied
        </span>
      )
    case 'failed':
      return (
        <span className="flex items-center gap-1 text-[10px] font-medium text-red-400">
          <AlertTriangle className="h-2.5 w-2.5" /> Failed
        </span>
      )
    default:
      return null
  }
}

// ── Per-category output renderers ──

function ShellOutput({ output }: { output: unknown }) {
  if (!output) return null
  const text = typeof output === 'string' ? output : JSON.stringify(output, null, 2)
  return (
    <pre className="mt-2 max-h-48 overflow-auto rounded-lg p-3 text-[11px] font-mono leading-relaxed"
         style={{ background: '#020208', color: '#00d4aa', border: '1px solid rgba(0,212,170,0.15)' }}>
      {text}
    </pre>
  )
}

function ChainQueryOutput({ output }: { output: unknown }) {
  if (!output) return null
  if (typeof output === 'object' && output !== null) {
    const entries = Object.entries(output as Record<string, unknown>)
    return (
      <div className="mt-2 space-y-1">
        {entries.map(([key, val]) => (
          <div key={key} className="flex items-baseline gap-2 text-[11px]">
            <span className="font-mono text-[#6b6a8a]">{key}:</span>
            <span className="font-mono text-blue-300">
              {typeof val === 'object' ? JSON.stringify(val) : String(val)}
            </span>
          </div>
        ))}
      </div>
    )
  }
  return <pre className="mt-2 text-[11px] font-mono text-blue-300">{JSON.stringify(output, null, 2)}</pre>
}

function DexOutput({ output, input }: { output: unknown; input: unknown }) {
  const data = (output || input) as Record<string, unknown> | null
  if (!data) return null

  // Detect swap vs liquidity operation
  const isSwap = data.return_amount !== undefined || data.offer_amount !== undefined
  const isPool = data.reserve_a !== undefined

  if (isSwap) {
    return (
      <div className="mt-2 rounded-lg p-3" style={{ background: 'rgba(255,107,74,0.06)', border: '1px solid rgba(255,107,74,0.15)' }}>
        <div className="flex items-center justify-between text-[11px]">
          <div>
            <span className="text-[#6b6a8a]">Offer:</span>{' '}
            <span className="font-mono text-[#f0eff8]">{String(data.offer_amount ?? '?')}</span>{' '}
            <span className="text-juno-400">{String(data.offer_asset ?? '')}</span>
          </div>
          <ArrowLeftRight className="h-3 w-3 text-[#6b6a8a]" />
          <div>
            <span className="text-[#6b6a8a]">Return:</span>{' '}
            <span className="font-mono text-[#f0eff8]">{String(data.return_amount ?? '?')}</span>{' '}
            <span className="text-juno-400">{String(data.return_asset ?? '')}</span>
          </div>
        </div>
        {!!data.fee_amount && (
          <div className="mt-1 text-[10px] text-[#6b6a8a]">
            Fee: {String(data.fee_amount)} · Spread: {String(data.spread_amount ?? '0')}
          </div>
        )}
        {!!data.price_impact_pct && (
          <div className="mt-1 text-[10px]">
            <span className="text-[#6b6a8a]">Impact:</span>{' '}
            <span className={Number(data.price_impact_pct) > 3 ? 'text-red-400' : 'text-green-400'}>
              {String(data.price_impact_pct)}%
            </span>
          </div>
        )}
      </div>
    )
  }

  if (isPool) {
    return (
      <div className="mt-2 rounded-lg p-3" style={{ background: 'rgba(255,107,74,0.06)', border: '1px solid rgba(255,107,74,0.15)' }}>
        <div className="grid grid-cols-2 gap-2 text-[11px]">
          <div>
            <span className="text-[#6b6a8a]">Reserve A:</span>{' '}
            <span className="font-mono text-[#f0eff8]">{String(data.reserve_a)}</span>
          </div>
          <div>
            <span className="text-[#6b6a8a]">Reserve B:</span>{' '}
            <span className="font-mono text-[#f0eff8]">{String(data.reserve_b)}</span>
          </div>
          {!!data.total_lp_shares && (
            <div className="col-span-2">
              <span className="text-[#6b6a8a]">LP Shares:</span>{' '}
              <span className="font-mono text-[#f0eff8]">{String(data.total_lp_shares)}</span>
            </div>
          )}
        </div>
      </div>
    )
  }

  return <ChainQueryOutput output={output} />
}

function ChainTxOutput({ output }: { output: unknown }) {
  if (!output) return null
  const data = output as Record<string, unknown>
  return (
    <div className="mt-2 rounded-lg p-3" style={{ background: 'rgba(248,113,113,0.06)', border: '1px solid rgba(248,113,113,0.15)' }}>
      {!!data.tx_hash && (
        <div className="text-[11px]">
          <span className="text-[#6b6a8a]">TX:</span>{' '}
          <span className="font-mono text-green-400">{String(data.tx_hash).slice(0, 16)}…</span>
        </div>
      )}
      {!!data.gas_used && (
        <div className="mt-1 text-[10px] text-[#6b6a8a]">
          Gas: {String(data.gas_used)} / {String(data.gas_wanted ?? '?')}
        </div>
      )}
      {!data.tx_hash && (
        <pre className="text-[11px] font-mono text-[#c0bfd8]">{JSON.stringify(output, null, 2)}</pre>
      )}
    </div>
  )
}

function FileOutput({ output, input }: { output: unknown; input: unknown }) {
  const data = (output || input) as Record<string, unknown> | null
  if (!data) return null
  const content = typeof data === 'string' ? data : (data.content as string) || JSON.stringify(data, null, 2)
  const path = String(data.path || data.file_path || '')
  return (
    <div className="mt-2">
      {!!path && <div className="mb-1 text-[10px] font-mono text-[#6b6a8a]">{path}</div>}
      <pre className="max-h-48 overflow-auto rounded-lg p-3 text-[11px] font-mono leading-relaxed"
           style={{ background: '#020208', color: '#a78bfa', border: '1px solid rgba(167,139,250,0.15)' }}>
        {content}
      </pre>
    </div>
  )
}

function GenericOutput({ output }: { output: unknown }) {
  if (!output) return null
  return (
    <pre className="mt-2 max-h-48 overflow-auto rounded-lg p-3 text-[11px] font-mono text-[#c0bfd8] leading-relaxed"
         style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}>
      {typeof output === 'string' ? output : JSON.stringify(output, null, 2)}
    </pre>
  )
}

function OutputRenderer({ record }: { record: ToolCallRecord }) {
  const cat = record.category || 'shell'
  switch (cat) {
    case 'shell':       return <ShellOutput output={record.output} />
    case 'chain_query': return <ChainQueryOutput output={record.output} />
    case 'chain_tx':    return <ChainTxOutput output={record.output} />
    case 'dex':         return <DexOutput output={record.output} input={record.input} />
    case 'file_io':     return <FileOutput output={record.output} input={record.input} />
    default:            return <GenericOutput output={record.output} />
  }
}

// ── Input renderer (shown before approval or in expanded state) ──

function InputRenderer({ record }: { record: ToolCallRecord }) {
  const cat = record.category || 'shell'

  // Shell: show command prominently
  if (cat === 'shell') {
    const cmd = typeof record.input === 'string'
      ? record.input
      : (record.input as Record<string, unknown>)?.command || JSON.stringify(record.input)
    return (
      <div className="mt-2 rounded-lg p-2.5 font-mono text-[11px]"
           style={{ background: '#020208', color: '#00d4aa', border: '1px solid rgba(0,212,170,0.12)' }}>
        <span className="text-[#6b6a8a]">$</span> {String(cmd)}
      </div>
    )
  }

  // DEX: show swap params
  if (cat === 'dex') {
    const data = record.input as Record<string, unknown> | null
    if (!data) return null
    return (
      <div className="mt-2 rounded-lg p-2.5 text-[11px]"
           style={{ background: 'rgba(255,107,74,0.04)', border: '1px solid rgba(255,107,74,0.1)' }}>
        {!!data.offer_asset && (
          <div>
            <span className="text-[#6b6a8a]">Swap</span>{' '}
            <span className="font-mono text-[#f0eff8]">{String(data.offer_amount ?? '?')}</span>{' '}
            <span className="text-juno-400">{String(data.offer_asset)}</span>
            {!!data.min_return && (
              <span className="text-[#6b6a8a]"> · min return: {String(data.min_return)}</span>
            )}
          </div>
        )}
        {!data.offer_asset && (
          <pre className="font-mono text-[#c0bfd8]">{JSON.stringify(data, null, 2)}</pre>
        )}
      </div>
    )
  }

  // Chain TX: show with warning styling
  if (cat === 'chain_tx') {
    return (
      <div className="mt-2 rounded-lg p-2.5"
           style={{ background: 'rgba(248,113,113,0.04)', border: '1px solid rgba(248,113,113,0.12)' }}>
        <pre className="text-[11px] font-mono text-[#c0bfd8] overflow-x-auto">
          {JSON.stringify(record.input, null, 2)}
        </pre>
      </div>
    )
  }

  // Generic
  return (
    <pre className="mt-2 overflow-x-auto rounded-lg p-2.5 text-[11px] font-mono text-[#c0bfd8]"
         style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}>
      {JSON.stringify(record.input, null, 2)}
    </pre>
  )
}

// ── Main ToolCallBlock component ──

interface ToolCallBlockProps {
  record: ToolCallRecord
  /** If true, show approve/deny buttons */
  isPending?: boolean
  onApprove?: () => void
  onDeny?: () => void
}

export function ToolCallBlock({ record, isPending, onApprove, onDeny }: ToolCallBlockProps) {
  const [expanded, setExpanded] = useState(isPending || false)
  const cat = record.category || 'shell'
  const meta = TOOL_CATEGORY_META[cat]
  const status = record.status || (record.approved ? 'completed' : 'pending')

  const borderColor = isPending ? meta.color : 'rgba(255,255,255,0.06)'
  const bgColor = isPending ? `${meta.color}08` : '#0a0a18'

  return (
    <div className="my-2 rounded-xl overflow-hidden transition-all"
         style={{ border: `1px solid ${borderColor}`, background: bgColor }}>
      {/* Header bar */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left transition hover:bg-white/[0.02]"
      >
        {expanded
          ? <ChevronDown className="h-3 w-3 text-[#6b6a8a]" />
          : <ChevronRight className="h-3 w-3 text-[#6b6a8a]" />
        }
        <span className="flex items-center gap-1.5" style={{ color: meta.color }}>
          <CategoryIcon category={cat} className="h-3 w-3" />
          <span className="text-[11px] font-semibold uppercase tracking-wide">{meta.label}</span>
        </span>
        <span className="ml-1 rounded px-1.5 py-0.5 text-[10px] font-mono text-[#6b6a8a]"
              style={{ background: 'rgba(255,255,255,0.04)' }}>
          {record.tool_name}
        </span>
        <span className="ml-auto flex items-center gap-2">
          <StatusBadge status={status} />
          {record.duration_ms > 0 && (
            <span className="text-[10px] text-[#6b6a8a]">{record.duration_ms}ms</span>
          )}
        </span>
      </button>

      {/* Expanded body */}
      {expanded && (
        <div className="px-3 pb-3" style={{ borderTop: `1px solid ${borderColor}30` }}>
          {/* Input */}
          <InputRenderer record={record} />

          {/* Approval buttons (for pending tool calls) */}
          {isPending && onApprove && onDeny ? (
            <div className="mt-3 flex gap-2">
              <button onClick={onApprove}
                      className="flex-1 flex items-center justify-center gap-1.5 rounded-lg py-2 text-xs font-semibold text-white transition hover:opacity-90"
                      style={{ background: 'linear-gradient(135deg,#22c55e,#16a34a)' }}>
                <Shield className="h-3 w-3" />
                Approve &amp; Run
              </button>
              <button onClick={onDeny}
                      className="flex-1 flex items-center justify-center gap-1.5 rounded-lg py-2 text-xs font-semibold transition hover:opacity-90"
                      style={{ background: 'rgba(239,68,68,0.1)', border: '1px solid rgba(239,68,68,0.25)', color: '#f87171' }}>
                <X className="h-3 w-3" />
                Deny
              </button>
            </div>
          ) : null}

          {/* Output */}
          {record.output ? <OutputRenderer record={record} /> : null}

          {/* Error */}
          {record.error ? (
            <div className="mt-2 rounded-lg px-3 py-2 text-[11px]"
                 style={{ background: 'rgba(239,68,68,0.08)', border: '1px solid rgba(239,68,68,0.2)', color: '#f87171' }}>
              {record.error}
            </div>
          ) : null}
        </div>
      )}
    </div>
  )
}

// ── Pending Tool Call Card (used in ChatPanel for the active pending call) ──

interface PendingToolCallProps {
  toolCall: { taskId: string; toolCallId: string; name: string; args: unknown; category: ToolCategory }
  onApprove: () => void
  onDeny: () => void
}

export function PendingToolCallCard({ toolCall, onApprove, onDeny }: PendingToolCallProps) {
  const record: ToolCallRecord = {
    id: toolCall.toolCallId,
    tool_name: toolCall.name,
    category: toolCall.category,
    input: toolCall.args,
    output: null,
    duration_ms: 0,
    approved: false,
    status: 'pending',
  }

  return (
    <ToolCallBlock
      record={record}
      isPending
      onApprove={onApprove}
      onDeny={onDeny}
    />
  )
}
