import { useState } from 'react'
import {
  Terminal, Search, Send, FileText, Globe, ArrowLeftRight,
  Code, Image, Shield, Lock, Unlock, Ban, RotateCcw,
  ChevronDown, ChevronRight, Settings2, Plus, X,
} from 'lucide-react'
import { useStore } from '../store'
import type { ToolCategory, SiloApprovalMode, SiloConfig } from '../types'
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

// ── Approval mode visuals ──

const APPROVAL_META: Record<SiloApprovalMode, { label: string; icon: React.ReactNode; color: string; bg: string }> = {
  auto:    { label: 'Auto',    icon: <Unlock className="h-3 w-3" />, color: '#22c55e', bg: 'rgba(34,197,94,0.12)' },
  ask:     { label: 'Ask',     icon: <Shield className="h-3 w-3" />, color: '#fbbf24', bg: 'rgba(251,191,36,0.12)' },
  blocked: { label: 'Blocked', icon: <Ban    className="h-3 w-3" />, color: '#ef4444', bg: 'rgba(239,68,68,0.12)' },
}

// ── Silo Row (one per tool category) ──

interface SiloRowProps {
  config: SiloConfig
  onChangeApproval: (mode: SiloApprovalMode) => void
  onAddScope: (scope: string) => void
  onRemoveScope: (index: number) => void
  onAddBlocklist: (pattern: string) => void
  onRemoveBlocklist: (index: number) => void
  onChangeMaxSpend: (value: number) => void
}

function SiloRow({
  config,
  onChangeApproval,
  onAddScope,
  onRemoveScope,
  onAddBlocklist,
  onRemoveBlocklist,
  onChangeMaxSpend,
}: SiloRowProps) {
  const [expanded, setExpanded] = useState(false)
  const [newScope, setNewScope] = useState('')
  const [newBlocklist, setNewBlocklist] = useState('')
  const meta = TOOL_CATEGORY_META[config.category]
  const approvalMeta = APPROVAL_META[config.approval]

  const approvalModes: SiloApprovalMode[] = ['auto', 'ask', 'blocked']

  return (
    <div className="rounded-xl overflow-hidden transition-all"
         style={{ border: `1px solid rgba(255,255,255,0.06)`, background: '#0a0a18' }}>
      {/* Header */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-3 px-4 py-3 text-left transition hover:bg-white/[0.02]"
      >
        {expanded
          ? <ChevronDown className="h-3 w-3 text-[#6b6a8a]" />
          : <ChevronRight className="h-3 w-3 text-[#6b6a8a]" />
        }
        <span className="flex items-center gap-2" style={{ color: meta.color }}>
          <CategoryIcon category={config.category} className="h-4 w-4" />
          <span className="text-xs font-semibold">{meta.label}</span>
        </span>
        <span className="ml-auto flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wider"
              style={{ color: approvalMeta.color, background: approvalMeta.bg }}>
          {approvalMeta.icon}
          {approvalMeta.label}
        </span>
      </button>

      {/* Expanded config */}
      {expanded && (
        <div className="px-4 pb-4 space-y-3" style={{ borderTop: '1px solid rgba(255,255,255,0.04)' }}>
          <p className="text-[10px] text-[#6b6a8a] pt-2">{meta.description}</p>

          {/* Approval mode selector */}
          <div>
            <label className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-1 block">
              Approval Mode
            </label>
            <div className="flex gap-1.5">
              {approvalModes.map((mode) => {
                const m = APPROVAL_META[mode]
                const isSelected = config.approval === mode
                return (
                  <button
                    key={mode}
                    onClick={() => onChangeApproval(mode)}
                    className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider transition-all"
                    style={isSelected ? {
                      color: m.color,
                      background: m.bg,
                      border: `1px solid ${m.color}40`,
                    } : {
                      color: '#6b6a8a',
                      background: 'transparent',
                      border: '1px solid rgba(255,255,255,0.06)',
                    }}
                  >
                    {m.icon}
                    {m.label}
                  </button>
                )
              })}
            </div>
          </div>

          {/* Scope (allowed paths/addresses) */}
          {config.approval !== 'blocked' && (
            <div>
              <label className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-1 block">
                Scope {config.category === 'chain_tx' ? '(Allowed Contracts)' : '(Allowed Paths)'}
              </label>
              <div className="flex flex-wrap gap-1.5 mb-1.5">
                {(config.scope || []).map((s, i) => (
                  <span key={i} className="flex items-center gap-1 rounded-md px-2 py-0.5 text-[10px] font-mono"
                        style={{ background: 'rgba(255,255,255,0.04)', color: '#c0bfd8', border: '1px solid rgba(255,255,255,0.06)' }}>
                    {s}
                    <button onClick={() => onRemoveScope(i)} className="text-red-400 hover:text-red-300">
                      <X className="h-2.5 w-2.5" />
                    </button>
                  </span>
                ))}
                {(!config.scope || config.scope.length === 0) && (
                  <span className="text-[10px] text-[#6b6a8a] italic">No restrictions (all allowed)</span>
                )}
              </div>
              <div className="flex gap-1.5">
                <input
                  type="text"
                  value={newScope}
                  onChange={(e) => setNewScope(e.target.value)}
                  placeholder={config.category === 'chain_tx' ? 'juno1abc...' : '/path/to/dir'}
                  className="flex-1 rounded-lg px-2.5 py-1.5 text-[11px] font-mono text-[#f0eff8] placeholder-[#4a4a6a] outline-none"
                  style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && newScope.trim()) {
                      onAddScope(newScope.trim())
                      setNewScope('')
                    }
                  }}
                />
                <button
                  onClick={() => { if (newScope.trim()) { onAddScope(newScope.trim()); setNewScope('') } }}
                  className="rounded-lg px-2.5 py-1.5 text-[#6b6a8a] hover:text-[#c0bfd8] transition"
                  style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.06)' }}
                >
                  <Plus className="h-3 w-3" />
                </button>
              </div>
            </div>
          )}

          {/* Blocklist (for shell silo) */}
          {config.category === 'shell' && config.approval !== 'blocked' && (
            <div>
              <label className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-1 block">
                Blocklist (Denied Patterns)
              </label>
              <div className="flex flex-wrap gap-1.5 mb-1.5">
                {(config.blocklist || []).map((b, i) => (
                  <span key={i} className="flex items-center gap-1 rounded-md px-2 py-0.5 text-[10px] font-mono"
                        style={{ background: 'rgba(239,68,68,0.08)', color: '#f87171', border: '1px solid rgba(239,68,68,0.15)' }}>
                    {b}
                    <button onClick={() => onRemoveBlocklist(i)} className="text-red-300 hover:text-red-200">
                      <X className="h-2.5 w-2.5" />
                    </button>
                  </span>
                ))}
              </div>
              <div className="flex gap-1.5">
                <input
                  type="text"
                  value={newBlocklist}
                  onChange={(e) => setNewBlocklist(e.target.value)}
                  placeholder="e.g., rm -rf, sudo"
                  className="flex-1 rounded-lg px-2.5 py-1.5 text-[11px] font-mono text-[#f0eff8] placeholder-[#4a4a6a] outline-none"
                  style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && newBlocklist.trim()) {
                      onAddBlocklist(newBlocklist.trim())
                      setNewBlocklist('')
                    }
                  }}
                />
                <button
                  onClick={() => { if (newBlocklist.trim()) { onAddBlocklist(newBlocklist.trim()); setNewBlocklist('') } }}
                  className="rounded-lg px-2.5 py-1.5 text-[#6b6a8a] hover:text-[#c0bfd8] transition"
                  style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.06)' }}
                >
                  <Plus className="h-3 w-3" />
                </button>
              </div>
            </div>
          )}

          {/* Max spend (for chain_tx silo) */}
          {config.category === 'chain_tx' && config.approval !== 'blocked' && (
            <div>
              <label className="text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a] mb-1 block">
                Max Spend Per TX (ujunox)
              </label>
              <input
                type="number"
                value={config.max_spend ?? 1_000_000}
                onChange={(e) => onChangeMaxSpend(Number(e.target.value))}
                className="w-40 rounded-lg px-2.5 py-1.5 text-[11px] font-mono text-[#f0eff8] outline-none"
                style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}
              />
              <p className="mt-0.5 text-[9px] text-[#4a4a6a]">
                = {((config.max_spend ?? 1_000_000) / 1_000_000).toFixed(2)} JUNOX
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

// ── Main panel ──

interface SiloConfigPanelProps {
  agentId: string
  agentName: string
  onClose: () => void
}

export function SiloConfigPanel({ agentId, agentName, onClose }: SiloConfigPanelProps) {
  const getAgentSilos = useStore((s) => s.getAgentSilos)
  const setAgentSilo = useStore((s) => s.setAgentSilo)
  const resetAgentSilos = useStore((s) => s.resetAgentSilos)

  const silos = getAgentSilos(agentId)

  const handleAddScope = (category: ToolCategory, scope: string) => {
    const silo = silos.find(s => s.category === category)
    if (!silo) return
    setAgentSilo(agentId, category, { scope: [...(silo.scope || []), scope] })
  }

  const handleRemoveScope = (category: ToolCategory, index: number) => {
    const silo = silos.find(s => s.category === category)
    if (!silo) return
    const updated = [...(silo.scope || [])]
    updated.splice(index, 1)
    setAgentSilo(agentId, category, { scope: updated })
  }

  const handleAddBlocklist = (category: ToolCategory, pattern: string) => {
    const silo = silos.find(s => s.category === category)
    if (!silo) return
    setAgentSilo(agentId, category, { blocklist: [...(silo.blocklist || []), pattern] })
  }

  const handleRemoveBlocklist = (category: ToolCategory, index: number) => {
    const silo = silos.find(s => s.category === category)
    if (!silo) return
    const updated = [...(silo.blocklist || [])]
    updated.splice(index, 1)
    setAgentSilo(agentId, category, { blocklist: updated })
  }

  // Count how many are auto/ask/blocked
  const autoCount = silos.filter(s => s.approval === 'auto').length
  const askCount = silos.filter(s => s.approval === 'ask').length
  const blockedCount = silos.filter(s => s.approval === 'blocked').length

  return (
    <div className="flex flex-col h-full bg-[#06060f]">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-3"
           style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
        <div className="flex items-center gap-2">
          <Settings2 className="h-4 w-4 text-juno-400" />
          <div>
            <div className="text-sm font-semibold text-[#f0eff8]">Tool Silos</div>
            <div className="text-[10px] text-[#6b6a8a]">
              {agentName} — {autoCount} auto · {askCount} ask · {blockedCount} blocked
            </div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => resetAgentSilos(agentId)}
            title="Reset to defaults"
            className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-semibold text-[#6b6a8a] transition hover:text-[#c0bfd8]"
            style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.06)' }}
          >
            <RotateCcw className="h-3 w-3" />
            Reset
          </button>
          <button
            onClick={onClose}
            className="flex items-center justify-center rounded-lg p-1.5 text-[#6b6a8a] transition hover:text-[#f0eff8]"
            style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.06)' }}
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      {/* Security summary */}
      <div className="px-5 py-3" style={{ borderBottom: '1px solid rgba(255,255,255,0.03)' }}>
        <div className="flex gap-3">
          <div className="flex-1 rounded-lg p-2.5 text-center"
               style={{ background: 'rgba(34,197,94,0.06)', border: '1px solid rgba(34,197,94,0.12)' }}>
            <div className="text-lg font-bold text-green-400">{autoCount}</div>
            <div className="text-[9px] font-semibold uppercase tracking-wider text-green-400/60">Auto</div>
          </div>
          <div className="flex-1 rounded-lg p-2.5 text-center"
               style={{ background: 'rgba(251,191,36,0.06)', border: '1px solid rgba(251,191,36,0.12)' }}>
            <div className="text-lg font-bold text-yellow-400">{askCount}</div>
            <div className="text-[9px] font-semibold uppercase tracking-wider text-yellow-400/60">Ask</div>
          </div>
          <div className="flex-1 rounded-lg p-2.5 text-center"
               style={{ background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.12)' }}>
            <div className="text-lg font-bold text-red-400">{blockedCount}</div>
            <div className="text-[9px] font-semibold uppercase tracking-wider text-red-400/60">Blocked</div>
          </div>
        </div>
        <p className="mt-2 text-[10px] text-[#6b6a8a] text-center">
          <Lock className="inline h-2.5 w-2.5 mr-0.5" />
          Each tool category runs in its own isolated silo with independent permissions
        </p>
      </div>

      {/* Silo list */}
      <div className="flex-1 overflow-y-auto px-5 py-3 space-y-2">
        {silos.map((silo) => (
          <SiloRow
            key={silo.category}
            config={silo}
            onChangeApproval={(mode) => setAgentSilo(agentId, silo.category, { approval: mode })}
            onAddScope={(scope) => handleAddScope(silo.category, scope)}
            onRemoveScope={(index) => handleRemoveScope(silo.category, index)}
            onAddBlocklist={(pattern) => handleAddBlocklist(silo.category, pattern)}
            onRemoveBlocklist={(index) => handleRemoveBlocklist(silo.category, index)}
            onChangeMaxSpend={(value) => setAgentSilo(agentId, silo.category, { max_spend: value })}
          />
        ))}
      </div>

      {/* Footer */}
      <div className="px-5 py-3 text-center" style={{ borderTop: '1px solid rgba(255,255,255,0.05)' }}>
        <p className="text-[10px] text-[#6b6a8a]">
          <Shield className="inline h-2.5 w-2.5 mr-0.5 text-juno-400" />
          Silo permissions are saved per-agent and persisted locally
        </p>
      </div>
    </div>
  )
}
