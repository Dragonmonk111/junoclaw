import { useState, useEffect, useRef } from 'react'
import { useStore } from '../store'
import {
  ExternalLink, Users, Coins, Clock, Building2, Plus, Check, X,
  ThumbsUp, ThumbsDown, Minus, Play, Timer, Shield, Settings, FileText,
  Cpu, ChevronDown, ChevronUp, Zap, Heart, Wheat, GraduationCap, Vote,
  HandCoins, Globe, LayoutGrid, ArrowLeft, TrendingUp, Handshake, Shuffle, ShoppingBasket,
  Archive, UserPlus, Bot, HeartPulse, Loader2, RefreshCw, Wifi, WifiOff
} from 'lucide-react'
import type {
  DaoConfig, DaoInstance, DaoProposal, DaoMember, ProposalKind,
  ProposalStatus, VoteOption
} from '../types'
import { useChainClient } from '../hooks/useChainClient'
import {
  buildFreeTextProposal,
  buildWavsPushProposal,
  buildConfigChangeProposal,
  buildOutcomeCreateProposal,
} from '../lib/contract-execute'

const DAODAO_BASE = 'https://daodao.zone/dao'
const CHAIN_ID = 'uni-7'

// ── Mock data (fallback for new DAOs with no on-chain proposals yet) ──

const CURRENT_BLOCK = 12500

const MOCK_PROPOSALS: DaoProposal[] = [
  {
    id: 1,
    proposer: 'juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m',
    kind: { type: 'wavs_push', task_description: 'Audit smart contract security for v2 migration', execution_tier: 'akash', escrow_amount: 50000 },
    votes: [
      { voter: 'juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m', option: 'yes', weight: 6000, block_height: 12452 },
    ],
    yes_weight: 6000, no_weight: 0, abstain_weight: 0, total_voted_weight: 6000,
    status: 'passed',
    created_at_block: 12450, voting_deadline_block: 12550, min_deadline_block: 12463,
    executed: false,
  },
  {
    id: 2,
    proposer: 'juno1agent01research7x9k4m2q3',
    kind: { type: 'free_text', title: 'Expand research team', description: 'Add 2 new research sub-agents for data analysis tasks. Expected cost: 100k ujunox/month.' },
    votes: [
      { voter: 'juno1agent01research7x9k4m2q3', option: 'yes', weight: 2500, block_height: 12481 },
    ],
    yes_weight: 2500, no_weight: 0, abstain_weight: 0, total_voted_weight: 2500,
    status: 'open',
    created_at_block: 12480, voting_deadline_block: 12580, min_deadline_block: 12493,
    executed: false,
  },
  {
    id: 3,
    proposer: 'juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m',
    kind: { type: 'weight_change', members: [
      { addr: 'juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m', weight: 5000, role: 'human' },
      { addr: 'juno1agent01research7x9k4m2q3', weight: 3000, role: 'agent' },
      { addr: 'juno1subdao0engineering8z3p5', weight: 2000, role: 'subdao' },
    ]},
    votes: [
      { voter: 'juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m', option: 'yes', weight: 6000, block_height: 12301 },
      { voter: 'juno1agent01research7x9k4m2q3', option: 'yes', weight: 2500, block_height: 12303 },
      { voter: 'juno1subdao0engineering8z3p5', option: 'yes', weight: 1500, block_height: 12305 },
    ],
    yes_weight: 10000, no_weight: 0, abstain_weight: 0, total_voted_weight: 10000,
    status: 'executed',
    created_at_block: 12300, voting_deadline_block: 12313, min_deadline_block: 12313,
    executed: true,
  },
]

// ── Helpers ──

const STATUS_STYLES: Record<ProposalStatus, { label: string; color: string; bg: string }> = {
  open:     { label: 'Open',     color: '#fbbf24', bg: 'rgba(251,191,36,0.1)' },
  passed:   { label: 'Passed',   color: '#34d399', bg: 'rgba(52,211,153,0.1)' },
  rejected: { label: 'Rejected', color: '#f87171', bg: 'rgba(248,113,113,0.1)' },
  executed: { label: 'Executed', color: '#a78bfa', bg: 'rgba(167,139,250,0.1)' },
  expired:  { label: 'Expired',  color: '#6b6a8a', bg: 'rgba(107,106,138,0.1)' },
}

const ROLE_STYLES: Record<string, { label: string; color: string }> = {
  human:  { label: 'Human',  color: '#fbbf24' },
  agent:  { label: 'Agent',  color: '#60a5fa' },
  subdao: { label: 'SubDAO', color: '#a78bfa' },
}

function truncAddr(addr: string) {
  return addr.length > 20 ? `${addr.slice(0, 10)}...${addr.slice(-6)}` : addr
}

function kindIcon(kind: ProposalKind) {
  switch (kind.type) {
    case 'weight_change':  return <Users className="h-3.5 w-3.5" />
    case 'wavs_push':      return <Cpu className="h-3.5 w-3.5" />
    case 'config_change':  return <Settings className="h-3.5 w-3.5" />
    case 'free_text':      return <FileText className="h-3.5 w-3.5" />
    case 'outcome_create':  return <TrendingUp className="h-3.5 w-3.5" />
    case 'outcome_resolve': return <Shield className="h-3.5 w-3.5" />
  }
}

function kindTitle(kind: ProposalKind) {
  switch (kind.type) {
    case 'weight_change':  return 'Weight Change'
    case 'wavs_push':      return 'WAVS Push'
    case 'config_change':  return 'Config Change'
    case 'free_text':      return kind.title
    case 'outcome_create':  return 'Outcome: ' + kind.question.slice(0, 40)
    case 'outcome_resolve': return `Resolve Outcome #${kind.market_id}`
  }
}

function kindDetail(kind: ProposalKind) {
  switch (kind.type) {
    case 'weight_change': return `${kind.members.length} members · new weight distribution`
    case 'wavs_push':     return `${kind.task_description} · ${kind.execution_tier} · ${kind.escrow_amount} ujunox`
    case 'config_change': {
      const parts = []
      if (kind.new_admin) parts.push(`admin → ${truncAddr(kind.new_admin)}`)
      if (kind.new_governance) parts.push(`gov → ${truncAddr(kind.new_governance)}`)
      if (kind.new_wavs_operator) parts.push(`wavs op → ${truncAddr(kind.new_wavs_operator)}`)
      return parts.join(', ') || 'No changes specified'
    }
    case 'free_text':      return kind.description
    case 'outcome_create':  return `${kind.resolution_criteria} · deadline block ${kind.deadline_block}`
    case 'outcome_resolve': return `Outcome: ${kind.outcome ? 'YES' : 'NO'} · attestation: ${kind.attestation_hash.slice(0, 12)}...`
  }
}

// ── Sub-components ──

function StatusBadge({ status }: { status: ProposalStatus }) {
  const s = STATUS_STYLES[status]
  return (
    <span className="rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wider"
          style={{ color: s.color, background: s.bg }}>
      {s.label}
    </span>
  )
}

function VoteBar({ yes, no, abstain, total }: { yes: number; no: number; abstain: number; total: number }) {
  const yPct = total > 0 ? (yes / total) * 100 : 0
  const nPct = total > 0 ? (no / total) * 100 : 0
  const aPct = total > 0 ? (abstain / total) * 100 : 0
  return (
    <div className="flex flex-col gap-1">
      <div className="flex h-2 w-full overflow-hidden rounded-full" style={{ background: '#1a1a2e' }}>
        {yPct > 0 && <div style={{ width: `${yPct}%`, background: '#34d399' }} />}
        {nPct > 0 && <div style={{ width: `${nPct}%`, background: '#f87171' }} />}
        {aPct > 0 && <div style={{ width: `${aPct}%`, background: '#6b6a8a' }} />}
      </div>
      <div className="flex gap-3 text-[9px]">
        <span style={{ color: '#34d399' }}>Yes {yPct.toFixed(0)}%</span>
        <span style={{ color: '#f87171' }}>No {nPct.toFixed(0)}%</span>
        <span style={{ color: '#6b6a8a' }}>Abstain {aPct.toFixed(0)}%</span>
      </div>
    </div>
  )
}

function MemberRow({ member, totalWeight }: { member: DaoMember; totalWeight: number }) {
  const pct = (member.weight / totalWeight) * 100
  const role = ROLE_STYLES[member.role] || ROLE_STYLES.human
  return (
    <div className="flex items-center gap-3 py-1.5">
      <span className="rounded px-1.5 py-0.5 text-[8px] font-bold uppercase"
            style={{ color: role.color, background: `${role.color}1a` }}>
        {role.label}
      </span>
      <code className="flex-1 truncate text-[10px] text-[#8a89a6] font-mono">{truncAddr(member.addr)}</code>
      <div className="flex items-center gap-2 w-32">
        <div className="flex-1 h-1.5 rounded-full overflow-hidden" style={{ background: '#1a1a2e' }}>
          <div className="h-full rounded-full" style={{ width: `${pct}%`, background: role.color }} />
        </div>
        <span className="text-[10px] text-[#c0bfd8] font-medium w-10 text-right">{pct.toFixed(1)}%</span>
      </div>
    </div>
  )
}

function ProposalCard({ proposal, config, currentBlock, onVote, onExecute, txPending }: {
  proposal: DaoProposal
  config: DaoConfig
  currentBlock: number
  onVote?: (proposalId: number, option: VoteOption) => Promise<string>
  onExecute?: (proposalId: number) => Promise<string>
  txPending?: boolean
}) {
  const [expanded, setExpanded] = useState(false)
  const [votingFor, setVotingFor] = useState<VoteOption | null>(null)
  const blocksLeft = Math.max(0, proposal.voting_deadline_block - currentBlock)
  const isAdaptiveReduced = proposal.voting_deadline_block < proposal.created_at_block + config.voting_period_blocks
  const canVote = proposal.status === 'open' && !!onVote
  const canExecute = proposal.status === 'passed' && currentBlock >= proposal.voting_deadline_block && !!onExecute

  const handleVote = async (option: VoteOption) => {
    if (!onVote || txPending) return
    setVotingFor(option)
    try { await onVote(proposal.id, option) } catch { /* handled upstream */ }
    finally { setVotingFor(null) }
  }
  const handleExecute = async () => {
    if (!onExecute || txPending) return
    try { await onExecute(proposal.id) } catch { /* handled upstream */ }
  }

  return (
    <div className="rounded-xl overflow-hidden"
         style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
      {/* Header row */}
      <button className="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-white/[0.02] transition"
              onClick={() => setExpanded(!expanded)}>
        <div className="flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-lg"
             style={{ background: 'rgba(167,139,250,0.1)', color: '#a78bfa' }}>
          {kindIcon(proposal.kind)}
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-[#e0dff8] truncate">#{proposal.id} {kindTitle(proposal.kind)}</span>
            <StatusBadge status={proposal.status} />
            {isAdaptiveReduced && (
              <span className="flex items-center gap-0.5 rounded px-1 py-0.5 text-[8px] font-bold uppercase"
                    style={{ color: '#f59e0b', background: 'rgba(245,158,11,0.1)' }}>
                <Zap className="h-2.5 w-2.5" /> Adaptive
              </span>
            )}
          </div>
          <div className="text-[10px] text-[#6b6a8a] truncate mt-0.5">{kindDetail(proposal.kind)}</div>
        </div>
        <div className="flex items-center gap-2 flex-shrink-0">
          {proposal.status === 'open' && (
            <div className="flex items-center gap-1 text-[10px] text-[#6b6a8a]">
              <Timer className="h-3 w-3" />
              {blocksLeft}b
            </div>
          )}
          {expanded ? <ChevronUp className="h-3.5 w-3.5 text-[#6b6a8a]" /> : <ChevronDown className="h-3.5 w-3.5 text-[#6b6a8a]" />}
        </div>
      </button>

      {/* Vote progress bar (always visible) */}
      <div className="px-4 pb-2">
        <VoteBar yes={proposal.yes_weight} no={proposal.no_weight} abstain={proposal.abstain_weight} total={config.total_weight} />
      </div>

      {/* Expanded detail */}
      {expanded && (
        <div className="px-4 pb-4 space-y-3" style={{ borderTop: '1px solid rgba(255,255,255,0.04)' }}>
          <div className="pt-3 grid grid-cols-2 gap-2 text-[10px]">
            <div>
              <span className="text-[#6b6a8a]">Proposer</span>
              <div className="text-[#a78bfa] font-mono mt-0.5">{truncAddr(proposal.proposer)}</div>
            </div>
            <div>
              <span className="text-[#6b6a8a]">Created</span>
              <div className="text-[#c0bfd8] mt-0.5">Block {proposal.created_at_block}</div>
            </div>
            <div>
              <span className="text-[#6b6a8a]">Deadline</span>
              <div className="text-[#c0bfd8] mt-0.5">
                Block {proposal.voting_deadline_block}
                {isAdaptiveReduced && <span className="text-[#f59e0b] ml-1">(reduced from {proposal.created_at_block + config.voting_period_blocks})</span>}
              </div>
            </div>
            <div>
              <span className="text-[#6b6a8a]">Quorum</span>
              <div className="text-[#c0bfd8] mt-0.5">
                {((proposal.total_voted_weight / config.total_weight) * 100).toFixed(0)}% / {config.quorum_percent}% required
              </div>
            </div>
          </div>

          {/* Votes list */}
          {proposal.votes.length > 0 && (
            <div>
              <div className="text-[9px] text-[#6b6a8a] uppercase tracking-wider mb-1.5">Votes ({proposal.votes.length})</div>
              <div className="space-y-1">
                {proposal.votes.map((v, i) => (
                  <div key={i} className="flex items-center gap-2 text-[10px]">
                    <span className={`flex items-center gap-1 ${v.option === 'yes' ? 'text-[#34d399]' : v.option === 'no' ? 'text-[#f87171]' : 'text-[#6b6a8a]'}`}>
                      {v.option === 'yes' ? <ThumbsUp className="h-2.5 w-2.5" /> : v.option === 'no' ? <ThumbsDown className="h-2.5 w-2.5" /> : <Minus className="h-2.5 w-2.5" />}
                      {v.option.charAt(0).toUpperCase() + v.option.slice(1)}
                    </span>
                    <code className="text-[#8a89a6] font-mono">{truncAddr(v.voter)}</code>
                    <span className="text-[#6b6a8a] ml-auto">{(v.weight / 100).toFixed(1)}% · blk {v.block_height}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Action buttons */}
          {(canVote || canExecute) && (
            <div className="flex gap-2 pt-1">
              {canVote && (
                <>
                  <button onClick={() => handleVote('yes')} disabled={txPending}
                          className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[10px] font-semibold transition hover:opacity-80 disabled:opacity-40"
                          style={{ background: 'rgba(52,211,153,0.1)', border: '1px solid rgba(52,211,153,0.2)', color: '#34d399' }}>
                    {votingFor === 'yes' ? <Loader2 className="h-3 w-3 animate-spin" /> : <ThumbsUp className="h-3 w-3" />} Yes
                  </button>
                  <button onClick={() => handleVote('no')} disabled={txPending}
                          className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[10px] font-semibold transition hover:opacity-80 disabled:opacity-40"
                          style={{ background: 'rgba(248,113,113,0.1)', border: '1px solid rgba(248,113,113,0.2)', color: '#f87171' }}>
                    {votingFor === 'no' ? <Loader2 className="h-3 w-3 animate-spin" /> : <ThumbsDown className="h-3 w-3" />} No
                  </button>
                  <button onClick={() => handleVote('abstain')} disabled={txPending}
                          className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[10px] font-semibold transition hover:opacity-80 disabled:opacity-40"
                          style={{ background: 'rgba(107,106,138,0.1)', border: '1px solid rgba(107,106,138,0.2)', color: '#6b6a8a' }}>
                    {votingFor === 'abstain' ? <Loader2 className="h-3 w-3 animate-spin" /> : <Minus className="h-3 w-3" />} Abstain
                  </button>
                </>
              )}
              {canExecute && (
                <button onClick={handleExecute} disabled={txPending}
                        className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[10px] font-semibold transition hover:opacity-80 disabled:opacity-40"
                        style={{ background: 'rgba(167,139,250,0.1)', border: '1px solid rgba(167,139,250,0.2)', color: '#a78bfa' }}>
                  {txPending ? <Loader2 className="h-3 w-3 animate-spin" /> : <Play className="h-3 w-3" />} Execute
                </button>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function CreateProposalForm({ onClose, onSubmit }: { onClose: () => void; onSubmit?: (kind: Record<string, unknown>) => Promise<string> }) {
  const [kindType, setKindType] = useState<'free_text' | 'wavs_push' | 'weight_change' | 'config_change' | 'outcome_create'>('free_text')
  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [taskDesc, setTaskDesc] = useState('')
  const [execTier, setExecTier] = useState('local')
  const [escrowAmt, setEscrowAmt] = useState('')
  const [newAdmin, setNewAdmin] = useState('')
  const [newGov, setNewGov] = useState('')
  const [newWavsOp, setNewWavsOp] = useState('')
  const [question, setQuestion] = useState('')
  const [resCriteria, setResCriteria] = useState('')
  const [deadlineBlock, setDeadlineBlock] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleSubmit = async () => {
    if (!onSubmit) {
      setError('Connect wallet to submit proposals')
      return
    }
    setSubmitting(true)
    setError(null)
    try {
      let kind: Record<string, unknown>
      switch (kindType) {
        case 'free_text':
          kind = buildFreeTextProposal(title, description)
          break
        case 'wavs_push':
          kind = buildWavsPushProposal(taskDesc, execTier, escrowAmt || '0')
          break
        case 'config_change':
          kind = buildConfigChangeProposal(
            newAdmin || undefined,
            newGov || undefined,
            newWavsOp || undefined,
          )
          break
        case 'outcome_create':
          kind = buildOutcomeCreateProposal(question, resCriteria, Number(deadlineBlock) || 0)
          break
        default:
          setError('Weight change proposals require the full member editor (coming soon)')
          setSubmitting(false)
          return
      }
      await onSubmit(kind)
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSubmitting(false)
    }
  }

  const inputCls = "w-full rounded-lg px-3 py-2 text-xs text-[#e0dff8] placeholder-[#3a3a5a] outline-none"
  const inputStyle = { background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }

  return (
    <div className="rounded-xl p-4 space-y-3"
         style={{ background: '#0a0a18', border: '1px solid rgba(167,139,250,0.2)' }}>
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold text-[#e0dff8]">New Proposal</span>
        <button onClick={onClose} className="text-[#6b6a8a] hover:text-[#f87171] transition">
          <X className="h-4 w-4" />
        </button>
      </div>

      <div className="flex gap-1.5">
        {(['free_text', 'wavs_push', 'weight_change', 'config_change', 'outcome_create'] as const).map((k) => (
          <button key={k}
                  onClick={() => setKindType(k)}
                  className="rounded-md px-2 py-1 text-[9px] font-semibold uppercase transition"
                  style={{
                    background: kindType === k ? 'rgba(167,139,250,0.15)' : 'rgba(255,255,255,0.03)',
                    border: `1px solid ${kindType === k ? 'rgba(167,139,250,0.3)' : 'rgba(255,255,255,0.06)'}`,
                    color: kindType === k ? '#a78bfa' : '#6b6a8a',
                  }}>
            {k.replace('_', ' ')}
          </button>
        ))}
      </div>

      {kindType === 'free_text' && (
        <>
          <input type="text" placeholder="Proposal title"
                 value={title} onChange={(e) => setTitle(e.target.value)}
                 className={inputCls} style={inputStyle} />
          <textarea placeholder="Description..."
                    value={description} onChange={(e) => setDescription(e.target.value)}
                    rows={3}
                    className={inputCls + ' resize-none'} style={inputStyle} />
        </>
      )}

      {kindType === 'wavs_push' && (
        <>
          <input type="text" placeholder="Task description for WAVS execution"
                 value={taskDesc} onChange={(e) => setTaskDesc(e.target.value)}
                 className={inputCls} style={inputStyle} />
          <div className="flex gap-2">
            <select value={execTier} onChange={(e) => setExecTier(e.target.value)}
                    className="flex-1 rounded-lg px-3 py-2 text-xs text-[#e0dff8] outline-none"
                    style={inputStyle}>
              <option value="local">Local</option>
              <option value="akash">Akash (GPU)</option>
            </select>
            <input type="number" placeholder="Escrow (ujunox)"
                   value={escrowAmt} onChange={(e) => setEscrowAmt(e.target.value)}
                   className="w-32 rounded-lg px-3 py-2 text-xs text-[#e0dff8] placeholder-[#3a3a5a] outline-none"
                   style={inputStyle} />
          </div>
        </>
      )}

      {kindType === 'weight_change' && (
        <div className="text-[10px] text-[#6b6a8a] p-3 rounded-lg" style={{ background: '#05050f' }}>
          Weight change proposals require specifying new member weights that sum to 10,000 bp.
          Full form coming in next update.
        </div>
      )}

      {kindType === 'config_change' && (
        <div className="space-y-2">
          <input type="text" placeholder="New admin address (optional)"
                 value={newAdmin} onChange={(e) => setNewAdmin(e.target.value)}
                 className={inputCls} style={inputStyle} />
          <input type="text" placeholder="New governance address (optional)"
                 value={newGov} onChange={(e) => setNewGov(e.target.value)}
                 className={inputCls} style={inputStyle} />
          <input type="text" placeholder="New WAVS operator address (optional)"
                 value={newWavsOp} onChange={(e) => setNewWavsOp(e.target.value)}
                 className={inputCls} style={inputStyle} />
        </div>
      )}

      {kindType === 'outcome_create' && (
        <div className="space-y-2">
          <input type="text" placeholder="Outcome question (e.g. Will Prop 42 pass by June?)"
                 value={question} onChange={(e) => setQuestion(e.target.value)}
                 className={inputCls} style={inputStyle} />
          <input type="text" placeholder="Resolution criteria (how outcome is determined)"
                 value={resCriteria} onChange={(e) => setResCriteria(e.target.value)}
                 className={inputCls} style={inputStyle} />
          <input type="number" placeholder="Deadline block height"
                 value={deadlineBlock} onChange={(e) => setDeadlineBlock(e.target.value)}
                 className={inputCls} style={inputStyle} />
        </div>
      )}

      {error && (
        <div className="text-[10px] text-red-400 px-2">{error}</div>
      )}

      <button onClick={handleSubmit} disabled={submitting}
              className="flex w-full items-center justify-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold transition hover:opacity-80 disabled:opacity-40"
              style={{ background: 'rgba(167,139,250,0.15)', border: '1px solid rgba(167,139,250,0.3)', color: '#a78bfa' }}>
        {submitting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}
        {submitting ? 'Submitting...' : 'Submit Proposal'}
      </button>
    </div>
  )
}

// ── DAO Templates for non-devs ──

const DAO_TEMPLATES = [
  {
    id: 'community_fund',
    name: 'Community Fund',
    desc: 'Members vote on how incoming task payments are split. Funds flow through the contract atomically — never held, never pooled.',
    icon: <HandCoins className="h-5 w-5" />,
    color: '#fbbf24',
    sectors: ['Financial Inclusion', 'Cooperative Treasury'],
    defaults: { voting_period: 100, quorum: 51, verification: 'witness_and_wavs' as const },
  },
  {
    id: 'crop_protection',
    name: 'Crop Protection Pool',
    desc: 'Agent scrapes weather data, WAVS TEE verifies conditions, farmers compensated automatically when thresholds trigger. DAO votes on coverage terms.',
    icon: <Wheat className="h-5 w-5" />,
    color: '#34d399',
    sectors: ['Agriculture', 'Risk Pooling'],
    defaults: { voting_period: 100, quorum: 67, verification: 'witness_and_wavs' as const },
  },
  {
    id: 'credential_verifier',
    name: 'Credential Verifier',
    desc: 'Institutions submit credentials to WAVS TEE, employers query pass/fail without seeing scores. Privacy-first verification.',
    icon: <GraduationCap className="h-5 w-5" />,
    color: '#60a5fa',
    sectors: ['Education', 'Employment'],
    defaults: { voting_period: 200, quorum: 67, verification: 'wavs' as const },
  },
  {
    id: 'community_vote',
    name: 'Community Vote',
    desc: 'Town/village-level decisions on budgets, land use, or local policy. Private ballots via WAVS TEE, publicly verifiable results.',
    icon: <Vote className="h-5 w-5" />,
    color: '#a78bfa',
    sectors: ['Governance', 'Democracy'],
    defaults: { voting_period: 150, quorum: 51, verification: 'wavs' as const },
  },
  {
    id: 'mutual_aid',
    name: 'Mutual Aid DAO',
    desc: 'Peer-to-peer solidarity funding. Need is assessed privately via WAVS TEE \u2014 recipients get support without exposing personal data to the group.',
    icon: <Heart className="h-5 w-5" />,
    color: '#f87171',
    sectors: ['Mutual Aid', 'Humanitarian'],
    defaults: { voting_period: 100, quorum: 67, verification: 'witness_and_wavs' as const },
  },
  {
    id: 'farm_to_table',
    name: 'Farm-to-Table Market',
    desc: 'Agent-to-agent local food trading. Farmer agents list produce, buyer agents purchase, WAVS TEE verifies delivery, payment-ledger records the obligation instantly.',
    icon: <ShoppingBasket className="h-5 w-5" />,
    color: '#f97316',
    sectors: ['Agriculture', 'Commerce'],
    defaults: { voting_period: 100, quorum: 51, verification: 'witness_and_wavs' as const },
  },
  {
    id: 'sortition_dao',
    name: "Citizens' Assembly",
    desc: 'Random selection of community representatives via NOIS/drand. Governs collective decisions without permanent power structures \u2014 no elections, no popularity contests.',
    icon: <Shuffle className="h-5 w-5" />,
    color: '#e879f9',
    sectors: ['Governance', 'Democracy'],
    defaults: { voting_period: 200, quorum: 67, verification: 'wavs' as const },
  },
  {
    id: 'skill_circle',
    name: 'Skill-Staking Circle',
    desc: 'P2P skill exchange between humans and agents. Post what you offer, request what you need. Agents match automatically, WAVS verifies delivery. No money \u2014 just time.',
    icon: <Handshake className="h-5 w-5" />,
    color: '#f472b6',
    sectors: ['Community', 'Skills Exchange'],
    defaults: { voting_period: 75, quorum: 51, verification: 'witness_and_wavs' as const },
  },
  {
    id: 'verifiable_outcome_market',
    name: 'Verifiable Outcome Market',
    desc: 'Collective intelligence for governance. Agents and members signal the most likely outcome before decisions are made. WAVS TEE verifies actual outcomes \u2014 building an on-chain track record of good judgment.',
    icon: <TrendingUp className="h-5 w-5" />,
    color: '#0ea5e9',
    sectors: ['Governance', 'Futarchy'],
    defaults: { voting_period: 150, quorum: 67, verification: 'wavs' as const },
  },
  {
    id: 'health_chw',
    name: 'Community Health Worker DAO',
    desc: 'Coordinate and compensate community health workers. Agents track visits, WAVS TEE verifies service delivery privately, sortition selects peer-review panels. No medical data exposed.',
    icon: <HeartPulse className="h-5 w-5" />,
    color: '#14b8a6',
    sectors: ['Health', 'Community'],
    defaults: { voting_period: 150, quorum: 67, verification: 'witness_and_wavs' as const },
  },
]

// ── WAVS Task definitions per template ──

type WavsTask = { id: string; name: string; desc: string; default_enabled: boolean }

const WAVS_TASKS: Record<string, WavsTask[]> = {
  community_fund: [
    { id: 'track_contributions', name: 'Track Contributions', desc: 'Monitor incoming payments and update treasury balance', default_enabled: true },
    { id: 'propose_releases', name: 'Propose Fund Releases', desc: 'Create WavsPush proposals for task payment disbursements', default_enabled: true },
    { id: 'rotation_schedule', name: 'Manage Task Rotation', desc: 'Track task assignment rotation order and notify next assignee', default_enabled: true },
    { id: 'treasury_report', name: 'Treasury Reports', desc: 'Generate periodic pool balance and activity summaries', default_enabled: false },
  ],
  crop_protection: [
    { id: 'weather_scrape', name: 'Scrape Weather Data', desc: 'Fetch weather data from public APIs, submit to WAVS TEE', default_enabled: true },
    { id: 'threshold_monitor', name: 'Monitor Thresholds', desc: 'WAVS TEE evaluates conditions against coverage triggers', default_enabled: true },
    { id: 'auto_payout', name: 'Trigger Payouts', desc: 'Auto-create verified payout proposal when threshold breached', default_enabled: true },
    { id: 'coverage_review', name: 'Coverage Review', desc: 'Propose seasonal coverage term updates via ConfigChange', default_enabled: false },
  ],
  credential_verifier: [
    { id: 'ingest_credentials', name: 'Ingest Credentials', desc: 'Accept submissions and forward to WAVS TEE for hashing', default_enabled: true },
    { id: 'verify_queries', name: 'Process Queries', desc: 'Respond to employer queries with pass/fail via WAVS attestation', default_enabled: true },
    { id: 'revocation_watch', name: 'Revocation Monitor', desc: 'Watch for credential revocations and update on-chain status', default_enabled: true },
    { id: 'audit_trail', name: 'Audit Trail', desc: 'Log all verification queries with WAVS-attested timestamps', default_enabled: false },
  ],
  community_vote: [
    { id: 'ballot_privacy', name: 'Private Ballots', desc: 'Collect votes via WAVS TEE \u2014 individual votes hidden, only totals published', default_enabled: true },
    { id: 'result_attestation', name: 'Result Attestation', desc: 'WAVS TEE produces verifiable tally with proof of correctness', default_enabled: true },
    { id: 'turnout_monitor', name: 'Turnout Monitor', desc: 'Track participation and send reminders to non-voters', default_enabled: true },
    { id: 'historical_record', name: 'Vote Archive', desc: 'Maintain tamper-proof record of all past votes and outcomes', default_enabled: false },
  ],
  mutual_aid: [
    { id: 'needs_assessment', name: 'Private Needs Assessment', desc: 'WAVS TEE evaluates applicant data \u2014 outputs tier without storing raw data', default_enabled: true },
    { id: 'route_proposals', name: 'Route Fund Proposals', desc: 'Auto-create verified disbursement proposal for eligible recipients', default_enabled: true },
    { id: 'distribution_log', name: 'Distribution History', desc: 'Anonymised record of distributions \u2014 tiers and amounts only', default_enabled: true },
    { id: 'criteria_review', name: 'Review Eligibility', desc: 'ConfigChange proposals when DAO wants to update criteria', default_enabled: false },
  ],
  farm_to_table: [
    { id: 'list_produce', name: 'List Produce', desc: 'Farmer agent publishes listings with item, quantity, price, expiry', default_enabled: true },
    { id: 'auto_purchase', name: 'Auto-Purchase', desc: 'Buyer agent discovers listings and creates a payment obligation', default_enabled: true },
    { id: 'verify_delivery', name: 'Verify Delivery', desc: 'WAVS TEE verifies delivery proof (QR scan, GPS, photo hash)', default_enabled: true },
    { id: 'trigger_payment', name: 'Trigger Payment', desc: 'On WAVS attestation, confirm delivery on payment-ledger \u2014 farmer paid', default_enabled: true },
    { id: 'arbitrate_disputes', name: 'Arbitrate Disputes', desc: 'Create verified dispute proposal with evidence for DAO resolution', default_enabled: false },
  ],
  sortition_dao: [
    { id: 'verify_pool', name: 'Verify Eligible Pool', desc: 'WAVS TEE confirms eligibility of all candidates before selection', default_enabled: true },
    { id: 'drand_selection', name: 'drand Random Selection', desc: 'Request NOIS beacon to randomly select N representatives', default_enabled: true },
    { id: 'term_management', name: 'Term Management', desc: 'Track assembly terms, trigger new selection when term expires', default_enabled: true },
    { id: 'proposal_routing', name: 'Route Proposals', desc: 'Forward community proposals to current assembly', default_enabled: false },
  ],
  skill_circle: [
    { id: 'post_listing', name: 'Post Skill Listing', desc: 'Publish available skills with hours, location radius, and expiry', default_enabled: true },
    { id: 'discover_match', name: 'Discover & Match', desc: 'Scan DAO listings, find compatible exchanges, propose deals', default_enabled: true },
    { id: 'verify_exchange', name: 'Verify Exchange', desc: 'Both agents submit GPS + session hash \u2014 WAVS TEE cross-verifies', default_enabled: true },
    { id: 'reputation_cert', name: 'Issue Reputation', desc: 'WAVS signs completion certificate \u2014 portable across DAOs', default_enabled: true },
    { id: 'arbitrate', name: 'Arbitrate Disputes', desc: 'Sortition randomness selects 3 random members as arbitrators', default_enabled: false },
  ],
  verifiable_outcome_market: [
    { id: 'create_outcome', name: 'Create Outcome Question', desc: 'Translate natural language into a formal outcome question proposal', default_enabled: true },
    { id: 'monitor_resolution', name: 'Monitor Resolution', desc: 'Watch for event data matching resolution criteria', default_enabled: true },
    { id: 'propose_resolution', name: 'Propose Resolution', desc: 'Submit verified resolution with WAVS attestation proving the outcome', default_enabled: true },
    { id: 'update_reputation', name: 'Update Reputation', desc: 'Calculate accuracy changes after resolution for on-chain record', default_enabled: false },
  ],
  health_chw: [
    { id: 'verify_visits', name: 'Verify Visits', desc: 'WAVS TEE verifies CHW visit completion (geolocation + timestamp) without exposing patient data', default_enabled: true },
    { id: 'match_assignments', name: 'Match Assignments', desc: 'Match health workers to tasks based on skills, location, and availability', default_enabled: true },
    { id: 'peer_review_panel', name: 'Peer Review Panel', desc: 'Use sortition to select random peer-review panels for quality assurance', default_enabled: true },
    { id: 'impact_reports', name: 'Impact Reports', desc: 'Generate periodic health outcome and activity summaries', default_enabled: false },
  ],
}

type MemberEntry = { addr: string; role: 'human' | 'agent' | 'subdao'; weight: number }

function DaoTemplateGallery({ onBack }: { onBack: () => void }) {
  const deployDao = useStore(s => s.deployDao)
  const [step, setStep] = useState(1)
  const [selected, setSelected] = useState<string | null>(null)
  const [daoName, setDaoName] = useState('')
  const [votingPeriod, setVotingPeriod] = useState(100)
  const [quorum, setQuorum] = useState(51)
  const [verification, setVerification] = useState<'none' | 'witness' | 'wavs' | 'witness_and_wavs'>('witness')
  const [enabledTasks, setEnabledTasks] = useState<Record<string, boolean>>({})
  const [memberEntries, setMemberEntries] = useState<MemberEntry[]>([
    { addr: '', role: 'human', weight: 10000 },
  ])
  const [deploying, setDeploying] = useState(false)

  const template = DAO_TEMPLATES.find(t => t.id === selected)
  const totalWeight = memberEntries.reduce((sum, m) => sum + m.weight, 0)
  const weightValid = totalWeight === 10000
  const tasks = selected ? (WAVS_TASKS[selected] || []) : []
  const color = template?.color || '#a78bfa'

  const selectTemplate = (id: string) => {
    const t = DAO_TEMPLATES.find(x => x.id === id)!
    setSelected(id)
    setDaoName(t.name + ' DAO')
    setVotingPeriod(t.defaults.voting_period)
    setQuorum(t.defaults.quorum)
    setVerification(t.defaults.verification)
    const taskMap: Record<string, boolean> = {}
    ;(WAVS_TASKS[id] || []).forEach(tk => { taskMap[tk.id] = tk.default_enabled })
    setEnabledTasks(taskMap)
    setStep(2)
  }

  const handleDeploy = () => {
    if (!template || !weightValid || !daoName.trim()) return
    setDeploying(true)
    deployDao({
      name: daoName,
      members: memberEntries.map(m => ({ addr: m.addr, weight: m.weight, role: m.role })),
      template_id: template.id,
      template_color: template.color,
      voting_period_blocks: votingPeriod,
      quorum_percent: quorum,
      verification_model: verification,
    })
    setTimeout(() => { setDeploying(false); onBack() }, 2000)
  }

  const addMember = () => {
    const remaining = 10000 - totalWeight
    setMemberEntries([...memberEntries, { addr: '', role: 'agent', weight: Math.max(0, remaining) }])
  }

  const removeMember = (idx: number) => {
    if (memberEntries.length <= 1) return
    setMemberEntries(memberEntries.filter((_, i) => i !== idx))
  }

  const updateMember = (idx: number, patch: Partial<MemberEntry>) => {
    setMemberEntries(memberEntries.map((m, i) => i === idx ? { ...m, ...patch } : m))
  }

  const canNext = () => {
    switch (step) {
      case 2: return !!daoName.trim()
      case 3: return true
      case 4: return weightValid && memberEntries.every(m => m.addr.trim())
      default: return false
    }
  }

  const stepLabels = ['Template', 'Configure', 'WAVS Tasks', 'Members', 'Deploy']
  const stepHints = ['', 'Set up your DAO', 'Choose your agent tasks', 'Add your team', 'Ready to launch']
  const nameRef = useRef<HTMLInputElement>(null)
  const [fadeKey, setFadeKey] = useState(0)

  useEffect(() => { if (step === 2) setTimeout(() => nameRef.current?.focus(), 80) }, [step])
  useEffect(() => { setFadeKey(k => k + 1) }, [step])

  const nextLabel = step < 5 ? `Next: ${stepLabels[step]} →` : ''

  return (
    <div className="space-y-5">
      <style>{`
        @keyframes fadeSlideIn { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
        @keyframes spin { to { transform: rotate(360deg); } }
      `}</style>
      {/* Header */}
      <div className="flex items-center gap-3">
        <button onClick={() => step === 1 ? onBack() : setStep(step - 1)}
                className="flex h-7 w-7 items-center justify-center rounded-full transition-all hover:bg-white/5"
                style={{ color: '#6b6a8a' }}>
          <ArrowLeft className="h-4 w-4" />
        </button>
        <div className="flex-1 min-w-0">
          <div className="text-xs font-semibold text-[#e0dff8] flex items-center gap-2">
            {step === 1 ? 'What kind of DAO?' : stepHints[step]}
            {template && step > 1 && (
              <span className="text-[10px] font-normal px-2 py-0.5 rounded-full"
                    style={{ background: `${color}15`, color }}>{template.name}</span>
            )}
          </div>
        </div>
      </div>

      {/* Step indicator — continuous progress bar */}
      <div className="relative">
        <div className="flex items-center gap-0">
          {stepLabels.map((label, i) => {
            const n = i + 1
            const done = n < step
            const active = n === step
            return (
              <div key={n} className="flex items-center" style={{ flex: 1 }}>
                {/* Segment bar */}
                <div className="flex-1 h-1 rounded-full overflow-hidden" style={{ background: 'rgba(255,255,255,0.04)' }}>
                  <div className="h-full rounded-full transition-all duration-500 ease-out"
                       style={{
                         width: done ? '100%' : active ? '50%' : '0%',
                         background: `linear-gradient(90deg, ${color}, ${color}99)`,
                       }} />
                </div>
                {/* Node dot */}
                <div className="flex flex-col items-center flex-shrink-0 mx-0.5 relative"
                     style={{ width: i === stepLabels.length - 1 ? undefined : 0 }}>
                  <div className="flex h-5 w-5 items-center justify-center rounded-full text-[8px] font-bold transition-all duration-300"
                       style={{
                         background: done ? color : active ? `${color}25` : '#0a0a18',
                         color: done ? '#fff' : active ? color : '#4a4a6a',
                         border: `2px solid ${done || active ? color : 'rgba(255,255,255,0.06)'}`,
                         boxShadow: active ? `0 0 12px ${color}40` : 'none',
                         transform: `translateX(-50%)`,
                       }}>
                    {done ? <Check className="h-2.5 w-2.5" /> : n}
                  </div>
                  <span className="absolute top-6 text-[7px] whitespace-nowrap font-medium transition-all duration-300"
                        style={{
                          color: active ? color : done ? `${color}99` : '#4a4a6a',
                          transform: 'translateX(-50%)',
                          fontWeight: active ? 700 : 500,
                        }}>{label}</span>
                </div>
              </div>
            )
          })}
        </div>
        <div className="h-5" /> {/* Spacer for labels */}
      </div>

      {/* ── Step 1: Choose Template ── */}
      {step === 1 && (
        <div key={fadeKey} className="grid grid-cols-3 gap-3" style={{ animation: 'fadeSlideIn 0.3s ease-out' }}>
          {DAO_TEMPLATES.map((t) => (
            <button key={t.id} onClick={() => selectTemplate(t.id)}
                    className="group flex flex-col gap-2 rounded-xl p-3.5 text-left transition-all duration-200 hover:scale-[1.03] hover:-translate-y-0.5"
                    style={{
                      background: '#0a0a18',
                      border: '1px solid rgba(255,255,255,0.06)',
                      boxShadow: 'none',
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.borderColor = `${t.color}50`
                      e.currentTarget.style.boxShadow = `0 4px 20px ${t.color}15, inset 0 1px 0 ${t.color}15`
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.borderColor = 'rgba(255,255,255,0.06)'
                      e.currentTarget.style.boxShadow = 'none'
                    }}>
              <div className="flex items-center gap-2.5">
                <div className="flex h-8 w-8 items-center justify-center rounded-lg transition-all duration-200 group-hover:scale-110"
                     style={{ background: `${t.color}1a`, color: t.color }}>
                  {t.icon}
                </div>
                <div className="text-[11px] font-semibold text-[#e0dff8]">{t.name}</div>
              </div>
              <div className="text-[9px] text-[#6b6a8a] leading-relaxed line-clamp-2">{t.desc}</div>
              <div className="flex flex-wrap gap-1 mt-auto">
                {t.sectors.map((s) => (
                  <span key={s} className="rounded-full px-1.5 py-0.5 text-[7px] font-medium"
                        style={{ background: `${t.color}15`, color: t.color }}>{s}</span>
                ))}
              </div>
            </button>
          ))}

          {/* ── $JClaw Governance Actions ── */}
          <div className="flex flex-col items-center justify-center gap-1.5 rounded-xl p-3.5 cursor-default"
               style={{ background: 'rgba(52,211,153,0.04)', border: '1px dashed rgba(52,211,153,0.25)' }}>
            <div className="flex h-8 w-8 items-center justify-center rounded-lg"
                 style={{ background: 'rgba(52,211,153,0.12)', color: '#34d399' }}>
              <Plus className="h-5 w-5" />
            </div>
            <div className="text-[10px] font-semibold text-[#34d399] text-center">Add Template</div>
            <div className="text-[8px] text-[#6b6a8a] text-center leading-tight">via $JClaw governance</div>
          </div>

          <div className="flex flex-col items-center justify-center gap-1.5 rounded-xl p-3.5 cursor-default"
               style={{ background: 'rgba(248,113,113,0.04)', border: '1px dashed rgba(248,113,113,0.25)' }}>
            <div className="flex h-8 w-8 items-center justify-center rounded-lg"
                 style={{ background: 'rgba(248,113,113,0.12)', color: '#f87171' }}>
              <Minus className="h-5 w-5" />
            </div>
            <div className="text-[10px] font-semibold text-[#f87171] text-center">Remove Template</div>
            <div className="text-[8px] text-[#6b6a8a] text-center leading-tight">via $JClaw governance</div>
          </div>

          <div className="flex flex-col items-center justify-center gap-1.5 rounded-xl p-3.5 cursor-default"
               style={{ background: 'rgba(251,191,36,0.04)', border: '1px dashed rgba(251,191,36,0.25)' }}>
            <div className="flex h-8 w-8 items-center justify-center rounded-lg"
                 style={{ background: 'rgba(251,191,36,0.12)', color: '#fbbf24' }}>
              <Settings className="h-5 w-5" />
            </div>
            <div className="text-[10px] font-semibold text-[#fbbf24] text-center">Edit Template</div>
            <div className="text-[8px] text-[#6b6a8a] text-center leading-tight">via $JClaw governance</div>
          </div>
        </div>
      )}

      {/* ── Step 2: Configure ── */}
      {step === 2 && template && (
        <div key={fadeKey} className="rounded-xl p-4 space-y-4" style={{ background: '#0a0a18', border: `1px solid ${color}30`, animation: 'fadeSlideIn 0.3s ease-out' }}>
          <div className="flex items-center gap-3 pb-3" style={{ borderBottom: `1px solid ${color}15` }}>
            <div className="flex h-10 w-10 items-center justify-center rounded-lg"
                 style={{ background: `${color}1a`, color }}>
              {template.icon}
            </div>
            <div className="flex-1 min-w-0">
              <div className="text-[10px] text-[#6b6a8a] line-clamp-2 leading-relaxed">{template.desc}</div>
            </div>
          </div>

          <div>
            <label className="text-[9px] font-semibold uppercase tracking-widest text-[#6b6a8a] mb-1.5 block">DAO Name</label>
            <input ref={nameRef} type="text" value={daoName} onChange={(e) => setDaoName(e.target.value)}
                   placeholder="My Awesome DAO"
                   className="w-full rounded-lg px-3 py-2.5 text-sm text-[#e0dff8] placeholder-[#3a3a5a] outline-none transition-all focus:ring-1"
                   style={{ background: '#05050f', border: `1px solid ${daoName.trim() ? `${color}40` : 'rgba(255,255,255,0.06)'}` }}
                   onKeyDown={(e) => { if (e.key === 'Enter' && daoName.trim()) setStep(3) }} />
            <div className="text-[8px] text-[#3a3a5a] mt-1">This appears on-chain and in DAODAO</div>
          </div>

          <div className="text-[8px] font-semibold uppercase tracking-widest text-[#6b6a8a] mt-1">Governance Parameters</div>
          <div className="grid grid-cols-3 gap-3">
            <div className="rounded-lg p-2.5 transition-all" style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}>
              <label className="text-[8px] text-[#6b6a8a] uppercase mb-1.5 block">Voting Period</label>
              <input type="number" value={votingPeriod} min={10} max={1000} step={10}
                     onChange={(e) => setVotingPeriod(Math.max(10, parseInt(e.target.value) || 10))}
                     className="w-full rounded px-2 py-1.5 text-xs text-[#e0dff8] outline-none text-center font-semibold"
                     style={{ background: 'rgba(255,255,255,0.03)' }} />
              <div className="text-[7px] text-[#3a3a5a] text-center mt-1">≈ {Math.round(votingPeriod * 6 / 60)} min</div>
            </div>
            <div className="rounded-lg p-2.5 transition-all" style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}>
              <label className="text-[8px] text-[#6b6a8a] uppercase mb-1.5 block">Quorum</label>
              <input type="number" value={quorum} min={1} max={100}
                     onChange={(e) => setQuorum(Math.max(1, Math.min(100, parseInt(e.target.value) || 1)))}
                     className="w-full rounded px-2 py-1.5 text-xs text-[#e0dff8] outline-none text-center font-semibold"
                     style={{ background: 'rgba(255,255,255,0.03)' }} />
              <div className="text-[7px] text-[#3a3a5a] text-center mt-1">{quorum > 66 ? 'Supermajority' : quorum > 50 ? 'Simple majority' : 'Low threshold'}</div>
            </div>
            <div className="rounded-lg p-2.5 transition-all" style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}>
              <label className="text-[8px] text-[#6b6a8a] uppercase mb-1.5 block">Verification</label>
              <select value={verification}
                      onChange={(e) => setVerification(e.target.value as typeof verification)}
                      className="w-full rounded px-1.5 py-1.5 text-[10px] text-[#e0dff8] outline-none font-semibold"
                      style={{ background: 'rgba(255,255,255,0.03)' }}>
                <option value="none">None</option>
                <option value="witness">Witness</option>
                <option value="wavs">WAVS TEE</option>
                <option value="witness_and_wavs">W + WAVS</option>
              </select>
              <div className="text-[7px] text-[#3a3a5a] text-center mt-1">{verification === 'wavs' || verification === 'witness_and_wavs' ? 'TEE-attested' : verification === 'witness' ? 'Human witnesses' : 'No verification'}</div>
            </div>
          </div>
        </div>
      )}

      {/* ── Step 3: WAVS Tasks ── */}
      {step === 3 && template && (
        <div key={fadeKey} className="rounded-xl p-4 space-y-3" style={{ background: '#0a0a18', border: `1px solid ${color}30`, animation: 'fadeSlideIn 0.3s ease-out' }}>
          <div className="flex items-center justify-between mb-1">
            <div className="flex items-center gap-2">
              <Cpu className="h-4 w-4" style={{ color }} />
              <span className="text-xs font-semibold text-[#e0dff8]">Agent Tasks</span>
            </div>
            <span className="rounded-full px-2 py-0.5 text-[9px] font-bold"
                  style={{ background: `${color}20`, color }}>
              {Object.values(enabledTasks).filter(Boolean).length} of {tasks.length} active
            </span>
          </div>
          <div className="text-[9px] text-[#6b6a8a] -mt-1 mb-2">Toggle which tasks your agents run autonomously</div>
          <div className="space-y-1.5">
            {tasks.map((task) => {
              const on = !!enabledTasks[task.id]
              return (
                <button key={task.id}
                        onClick={() => setEnabledTasks(prev => ({ ...prev, [task.id]: !prev[task.id] }))}
                        className="flex w-full items-center gap-3 rounded-lg p-3 text-left transition-all duration-200"
                        style={{
                          background: on ? `${color}08` : '#05050f',
                          border: `1px solid ${on ? `${color}30` : 'rgba(255,255,255,0.05)'}`,
                          transform: on ? 'scale(1)' : 'scale(0.99)',
                        }}>
                  <div className="flex h-5 w-5 flex-shrink-0 items-center justify-center rounded transition-all duration-200"
                       style={{
                         background: on ? color : 'transparent',
                         border: `2px solid ${on ? color : 'rgba(255,255,255,0.12)'}`,
                         boxShadow: on ? `0 0 8px ${color}30` : 'none',
                       }}>
                    {on && <Check className="h-3 w-3 text-white" />}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="text-[11px] font-semibold transition-colors duration-200" style={{ color: on ? '#e0dff8' : '#6b6a8a' }}>{task.name}</div>
                    <div className="text-[9px] mt-0.5 transition-colors duration-200" style={{ color: on ? '#8a89a6' : '#4a4a6a' }}>{task.desc}</div>
                  </div>
                  <div className="text-[8px] font-medium flex-shrink-0 transition-all duration-200"
                       style={{ color: on ? color : '#4a4a6a' }}>
                    {on ? 'ON' : 'OFF'}
                  </div>
                </button>
              )
            })}
          </div>
        </div>
      )}

      {/* ── Step 4: Members ── */}
      {step === 4 && template && (
        <div key={fadeKey} className="rounded-xl p-4 space-y-3" style={{ background: '#0a0a18', border: `1px solid ${color}30`, animation: 'fadeSlideIn 0.3s ease-out' }}>
          {/* Weight progress header */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Users className="h-4 w-4" style={{ color }} />
              <span className="text-xs font-semibold text-[#e0dff8]">Members</span>
              <span className="text-[9px] text-[#6b6a8a]">{memberEntries.length} {memberEntries.length === 1 ? 'member' : 'members'}</span>
            </div>
            <button onClick={addMember}
                    className="flex items-center gap-1 rounded-full px-2.5 py-1 text-[9px] font-semibold transition-all hover:opacity-80"
                    style={{ background: `${color}15`, color, border: `1px solid ${color}30` }}>
              <Plus className="h-3 w-3" /> Add
            </button>
          </div>

          {/* Weight allocation bar */}
          <div className="space-y-1">
            <div className="h-2 rounded-full overflow-hidden flex" style={{ background: 'rgba(255,255,255,0.04)' }}>
              {memberEntries.map((m, idx) => {
                const pct = Math.min((m.weight / 10000) * 100, 100)
                const roleColor = ROLE_STYLES[m.role]?.color ?? color
                return pct > 0 ? (
                  <div key={idx} className="h-full transition-all duration-300 first:rounded-l-full last:rounded-r-full"
                       style={{ width: `${pct}%`, background: roleColor, opacity: 0.7 }} />
                ) : null
              })}
            </div>
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                {Object.entries(ROLE_STYLES).map(([key, s]) => {
                  const count = memberEntries.filter(m => m.role === key).length
                  return count > 0 ? (
                    <span key={key} className="flex items-center gap-1 text-[7px]" style={{ color: s.color }}>
                      <span className="h-1.5 w-1.5 rounded-full" style={{ background: s.color }} /> {s.label} ({count})
                    </span>
                  ) : null
                })}
              </div>
              <span className={`text-[9px] font-bold transition-colors ${weightValid ? 'text-[#34d399]' : totalWeight > 10000 ? 'text-[#f87171]' : 'text-[#fbbf24]'}`}>
                {weightValid ? '10,000 bp ✓' : `${totalWeight.toLocaleString()} / 10,000`}
              </span>
            </div>
          </div>

          {/* Member rows */}
          <div className="space-y-1.5">
            {memberEntries.map((m, idx) => {
              const roleColor = ROLE_STYLES[m.role]?.color ?? '#6b6a8a'
              return (
                <div key={idx} className="relative rounded-lg overflow-hidden transition-all"
                     style={{ border: `1px solid ${m.addr.trim() ? `${roleColor}25` : 'rgba(255,255,255,0.05)'}` }}>
                  {/* Background weight bar */}
                  <div className="absolute inset-0 transition-all duration-300"
                       style={{ width: `${Math.min((m.weight / 10000) * 100, 100)}%`, background: `${roleColor}06` }} />
                  <div className="relative flex items-center gap-1.5 p-2.5">
                    <select value={m.role}
                            onChange={(e) => updateMember(idx, { role: e.target.value as MemberEntry['role'] })}
                            className="rounded px-1.5 py-1 text-[8px] font-bold uppercase outline-none flex-shrink-0 transition-colors"
                            style={{ background: `${roleColor}15`, color: roleColor, border: 'none' }}>
                      <option value="human">Human</option>
                      <option value="agent">Agent</option>
                      <option value="subdao">SubDAO</option>
                    </select>
                    <input type="text" value={m.addr} placeholder="juno1..."
                           onChange={(e) => updateMember(idx, { addr: e.target.value })}
                           className="flex-1 min-w-0 rounded px-2 py-1 text-[10px] text-[#e0dff8] placeholder-[#3a3a5a] font-mono outline-none"
                           style={{ background: 'transparent' }} />
                    <input type="number" value={m.weight} min={0} max={10000} step={100}
                           onChange={(e) => updateMember(idx, { weight: Math.max(0, Math.min(10000, parseInt(e.target.value) || 0)) })}
                           className="w-14 rounded px-1 py-1 text-[10px] text-center font-semibold outline-none"
                           style={{ background: 'rgba(255,255,255,0.03)', color: roleColor }} />
                    <span className="text-[9px] font-medium w-8 text-right" style={{ color: roleColor }}>{(m.weight / 100).toFixed(0)}%</span>
                    {memberEntries.length > 1 && (
                      <button onClick={() => removeMember(idx)} className="text-[#4a4a6a] hover:text-[#f87171] transition flex-shrink-0">
                        <X className="h-3 w-3" />
                      </button>
                    )}
                  </div>
                </div>
              )
            })}
          </div>

          {!weightValid && (
            <div className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-[9px]"
                 style={{ background: totalWeight > 10000 ? 'rgba(248,113,113,0.08)' : 'rgba(251,191,36,0.08)', color: totalWeight > 10000 ? '#f87171' : '#fbbf24' }}>
              <Shield className="h-3 w-3 flex-shrink-0" />
              {totalWeight > 10000 ? `${totalWeight - 10000} bp over — reduce weights to continue` : `${10000 - totalWeight} bp remaining — allocate to continue`}
            </div>
          )}
        </div>
      )}

      {/* ── Step 5: Review & Deploy ── */}
      {step === 5 && template && (
        <div key={fadeKey} className="space-y-3" style={{ animation: 'fadeSlideIn 0.3s ease-out' }}>
          {/* Hero banner */}
          <div className="relative rounded-xl overflow-hidden p-4"
               style={{ background: `linear-gradient(135deg, ${color}12, ${color}06)`, border: `1px solid ${color}25` }}>
            <div className="absolute top-0 right-0 w-32 h-32 rounded-full opacity-10"
                 style={{ background: color, filter: 'blur(40px)', transform: 'translate(30%, -30%)' }} />
            <div className="relative flex items-center gap-3">
              <div className="flex h-12 w-12 items-center justify-center rounded-xl"
                   style={{ background: `${color}20`, color, boxShadow: `0 0 20px ${color}15` }}>
                {template.icon}
              </div>
              <div>
                <div className="text-sm font-bold text-[#e0dff8]">{daoName}</div>
                <div className="text-[10px] font-medium" style={{ color }}>{template.name}</div>
              </div>
            </div>
          </div>

          {/* Config summary — compact grid */}
          <div className="grid grid-cols-4 gap-2">
            <div className="rounded-lg p-2.5 text-center" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
              <div className="text-[7px] text-[#6b6a8a] uppercase">Period</div>
              <div className="text-[11px] text-[#e0dff8] font-bold mt-0.5">{votingPeriod}b</div>
            </div>
            <div className="rounded-lg p-2.5 text-center" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
              <div className="text-[7px] text-[#6b6a8a] uppercase">Quorum</div>
              <div className="text-[11px] text-[#e0dff8] font-bold mt-0.5">{quorum}%</div>
            </div>
            <div className="rounded-lg p-2.5 text-center" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
              <div className="text-[7px] text-[#6b6a8a] uppercase">Verify</div>
              <div className="text-[11px] font-bold mt-0.5" style={{ color }}>
                {verification === 'witness_and_wavs' ? 'W+WAVS' : verification === 'wavs' ? 'WAVS' : verification === 'witness' ? 'Witness' : 'None'}
              </div>
            </div>
            <div className="rounded-lg p-2.5 text-center" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
              <div className="text-[7px] text-[#6b6a8a] uppercase">Members</div>
              <div className="text-[11px] text-[#e0dff8] font-bold mt-0.5">{memberEntries.length}</div>
            </div>
          </div>

          {/* Tasks + Members compact */}
          <div className="rounded-xl p-3 space-y-3" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
            <div>
              <div className="text-[8px] text-[#6b6a8a] uppercase tracking-widest mb-1.5">
                Agent Tasks · {Object.values(enabledTasks).filter(Boolean).length} active
              </div>
              <div className="flex flex-wrap gap-1">
                {tasks.filter(t => enabledTasks[t.id]).map(t => (
                  <span key={t.id} className="rounded-full px-2 py-0.5 text-[8px] font-medium"
                        style={{ background: `${color}12`, color, border: `1px solid ${color}20` }}>{t.name}</span>
                ))}
                {Object.values(enabledTasks).filter(Boolean).length === 0 && (
                  <span className="text-[8px] text-[#3a3a5a] italic">No tasks enabled</span>
                )}
              </div>
            </div>

            <div style={{ borderTop: '1px solid rgba(255,255,255,0.04)' }} className="pt-2.5">
              <div className="text-[8px] text-[#6b6a8a] uppercase tracking-widest mb-1.5">Team</div>
              <div className="space-y-1">
                {memberEntries.map((m, i) => {
                  const rc = ROLE_STYLES[m.role]?.color ?? '#6b6a8a'
                  return (
                    <div key={i} className="flex items-center gap-2 text-[10px]">
                      <span className="rounded px-1.5 py-0.5 text-[7px] font-bold uppercase"
                            style={{ color: rc, background: `${rc}15` }}>
                        {ROLE_STYLES[m.role]?.label}
                      </span>
                      <code className="text-[#8a89a6] font-mono truncate flex-1 text-[9px]">{m.addr || 'Not set'}</code>
                      <span className="font-semibold" style={{ color: rc }}>{(m.weight / 100).toFixed(1)}%</span>
                    </div>
                  )
                })}
              </div>
            </div>
          </div>

          {/* Deploy button */}
          <button onClick={handleDeploy}
                  disabled={deploying}
                  className="flex w-full items-center justify-center gap-2 rounded-xl py-3.5 text-sm font-bold text-white transition-all hover:scale-[1.01] hover:-translate-y-px disabled:opacity-40 disabled:hover:scale-100 disabled:hover:translate-y-0"
                  style={{
                    background: `linear-gradient(135deg, ${color}, ${color}cc)`,
                    boxShadow: deploying ? 'none' : `0 4px 20px ${color}30`,
                  }}>
            {deploying ? (
              <><div className="h-4 w-4 rounded-full border-2 border-white/30 border-t-white" style={{ animation: 'spin 0.8s linear infinite' }} /> Deploying to uni-7...</>
            ) : (
              <><Globe className="h-4 w-4" /> Deploy DAO to Chain</>
            )}
          </button>
        </div>
      )}

      {/* Navigation (steps 2-4) */}
      {step >= 2 && step <= 4 && (
        <div className="flex items-center gap-3">
          <button onClick={() => setStep(step - 1)}
                  className="rounded-lg px-4 py-2.5 text-[11px] font-medium transition-all hover:bg-white/5"
                  style={{ color: '#6b6a8a' }}>
            ← Back
          </button>
          <button onClick={() => setStep(step + 1)}
                  disabled={!canNext()}
                  className="flex-1 rounded-lg py-2.5 text-xs font-semibold text-white transition-all hover:opacity-90 hover:scale-[1.005] disabled:opacity-30 disabled:cursor-not-allowed disabled:hover:scale-100"
                  style={{
                    background: canNext() ? `linear-gradient(135deg, ${color}, ${color}cc)` : 'rgba(255,255,255,0.04)',
                    boxShadow: canNext() ? `0 2px 12px ${color}20` : 'none',
                  }}>
            {nextLabel}
          </button>
        </div>
      )}
    </div>
  )
}

// ── Agent Chip (shows local agents in a DAO) ──

function AgentChip({ agentId, onRemove }: { agentId: string; onRemove: () => void }) {
  const agent = useStore(s => s.agents.find(a => a.id === agentId))
  if (!agent) return null
  return (
    <div className="flex items-center gap-1.5 rounded-lg px-2 py-1"
         style={{ background: 'rgba(96,165,250,0.08)', border: '1px solid rgba(96,165,250,0.15)' }}>
      <Bot className="h-3 w-3 text-[#60a5fa]" />
      <span className="text-[10px] text-[#60a5fa] font-medium truncate max-w-[100px]">{agent.name}</span>
      <button onClick={onRemove} className="text-[#6b6a8a] hover:text-[#f87171] transition ml-0.5">
        <X className="h-2.5 w-2.5" />
      </button>
    </div>
  )
}

// ── DAO Dashboard (right panel for a selected DAO) ──

function DaoDashboard({ dao }: { dao: DaoInstance }) {
  const [showCreate, setShowCreate] = useState(false)
  const [showTemplates, setShowTemplates] = useState(false)
  const [showAgentPicker, setShowAgentPicker] = useState(false)
  const agents = useStore(s => s.agents)
  const joinAgent = useStore(s => s.joinAgentToDao)
  const removeAgent = useStore(s => s.removeAgentFromDao)
  const archiveDao = useStore(s => s.archiveDao)

  // ── Live chain data via useChainClient ──
  const chain = useChainClient(dao.chain_address)
  const config = chain.config ?? dao.config
  const proposals = chain.proposals.length > 0 ? chain.proposals : (dao.proposals.length > 0 ? dao.proposals : MOCK_PROPOSALS)
  const currentBlock = chain.lastFetched ? Math.round(Date.now() / 6000) : CURRENT_BLOCK // ~6s blocks, rough estimate
  const isLive = chain.proposals.length > 0 && !chain.chainError

  const daodaoUrl = `${DAODAO_BASE}/${config.admin}?chain=${CHAIN_ID}`
  const availableAgents = agents.filter(a => !dao.agent_ids.includes(a.id))

  return (
    <div className="flex flex-1 flex-col overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-3"
           style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg"
               style={{ background: `${dao.template_color}1a`, border: `1px solid ${dao.template_color}33` }}>
            <Building2 className="h-4 w-4" style={{ color: dao.template_color }} />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="text-sm font-semibold text-[#f0eff8]">{dao.name}</span>
              {dao.status === 'deploying' && (
                <span className="rounded px-1.5 py-0.5 text-[8px] font-bold uppercase animate-pulse"
                      style={{ color: '#fbbf24', background: 'rgba(251,191,36,0.1)' }}>Deploying</span>
              )}
              {dao.status === 'archived' && (
                <span className="rounded px-1.5 py-0.5 text-[8px] font-bold uppercase"
                      style={{ color: '#6b6a8a', background: 'rgba(107,106,138,0.1)' }}>Archived</span>
              )}
            </div>
            <div className="text-[10px] text-[#6b6a8a]">{CHAIN_ID} · {dao.template_id.replace(/_/g, ' ')}</div>
          </div>
        </div>
        <div className="flex gap-1.5">
          <button onClick={() => { setShowTemplates(!showTemplates); setShowCreate(false) }}
                  className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-medium transition hover:opacity-80"
                  style={{ background: 'rgba(255,107,74,0.08)', border: '1px solid rgba(255,107,74,0.15)', color: '#ff6b4a' }}>
            <LayoutGrid className="h-3 w-3" /> New DAO
          </button>
          <button onClick={() => { setShowCreate(!showCreate); setShowTemplates(false) }}
                  className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-medium transition hover:opacity-80"
                  style={{ background: 'rgba(52,211,153,0.1)', border: '1px solid rgba(52,211,153,0.2)', color: '#34d399' }}>
            <Plus className="h-3 w-3" /> Propose
          </button>
          <a href={daodaoUrl} target="_blank" rel="noopener noreferrer"
             className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-medium transition hover:opacity-80"
             style={{ background: 'rgba(167,139,250,0.1)', border: '1px solid rgba(167,139,250,0.2)', color: '#a78bfa' }}>
            DAODAO <ExternalLink className="h-3 w-3" />
          </a>
          <button onClick={() => archiveDao(dao.id)}
                  className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-medium transition hover:opacity-80"
                  style={{ background: 'rgba(248,113,113,0.06)', border: '1px solid rgba(248,113,113,0.12)', color: '#6b6a8a' }}>
            <Archive className="h-3 w-3" />
          </button>
        </div>
      </div>

      <div className="px-5 py-4 space-y-4">
        {showTemplates && <DaoTemplateGallery onBack={() => setShowTemplates(false)} />}
        {showCreate && <CreateProposalForm onClose={() => setShowCreate(false)} onSubmit={chain.walletConnected ? chain.propose : undefined} />}

        {/* Local Agents in this DAO */}
        <div className="rounded-xl p-3" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
          <div className="flex items-center justify-between mb-2">
            <span className="text-[10px] font-semibold text-[#6b6a8a] uppercase tracking-widest">My Agents in this DAO</span>
            <button onClick={() => setShowAgentPicker(!showAgentPicker)}
                    className="flex items-center gap-1 rounded px-2 py-1 text-[9px] font-semibold transition hover:opacity-80"
                    style={{ background: 'rgba(96,165,250,0.08)', border: '1px solid rgba(96,165,250,0.15)', color: '#60a5fa' }}>
              <UserPlus className="h-2.5 w-2.5" /> Add Agent
            </button>
          </div>
          <div className="flex flex-wrap gap-1.5">
            {dao.agent_ids.length === 0 && (
              <span className="text-[10px] text-[#3a3a5a] italic">No local agents assigned yet</span>
            )}
            {dao.agent_ids.map(aid => (
              <AgentChip key={aid} agentId={aid} onRemove={() => removeAgent(dao.id, aid)} />
            ))}
          </div>
          {showAgentPicker && availableAgents.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-1.5 pt-2" style={{ borderTop: '1px solid rgba(255,255,255,0.04)' }}>
              {availableAgents.map(a => (
                <button key={a.id}
                        onClick={() => { joinAgent(dao.id, a.id); setShowAgentPicker(false) }}
                        className="flex items-center gap-1 rounded-lg px-2 py-1 text-[10px] font-medium transition hover:opacity-80"
                        style={{ background: 'rgba(52,211,153,0.08)', border: '1px solid rgba(52,211,153,0.15)', color: '#34d399' }}>
                  <Plus className="h-2.5 w-2.5" /> {a.name}
                </button>
              ))}
            </div>
          )}
          {showAgentPicker && availableAgents.length === 0 && (
            <div className="mt-2 text-[10px] text-[#3a3a5a] italic pt-2" style={{ borderTop: '1px solid rgba(255,255,255,0.04)' }}>
              All agents already assigned. Create more agents in the Chat tab.
            </div>
          )}
        </div>

        {/* Governance Config */}
        <div className="rounded-xl p-3" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[10px] font-semibold text-[#6b6a8a] uppercase tracking-widest">Config</span>
            <span className="rounded px-1.5 py-0.5 text-[8px] font-semibold uppercase"
                  style={{ color: dao.status === 'active' ? '#34d399' : '#fbbf24', background: dao.status === 'active' ? 'rgba(52,211,153,0.08)' : 'rgba(251,191,36,0.08)' }}>
              {dao.status === 'active' ? 'Testnet' : dao.status}
            </span>
          </div>
          <div className="grid grid-cols-2 gap-x-6 gap-y-1.5 text-[10px]">
            <div className="flex justify-between"><span className="text-[#6b6a8a]">Voting period</span><span className="text-[#c0bfd8] font-medium">{config.voting_period_blocks} blocks</span></div>
            <div className="flex justify-between"><span className="text-[#6b6a8a]">Quorum</span><span className="text-[#c0bfd8] font-medium">{config.quorum_percent}%</span></div>
            <div className="flex justify-between"><span className="text-[#6b6a8a]">Adaptive window</span><span className="text-[#c0bfd8] font-medium">{config.adaptive_threshold_blocks} blocks</span></div>
            <div className="flex justify-between"><span className="text-[#6b6a8a]">Adaptive min</span><span className="text-[#c0bfd8] font-medium">{config.adaptive_min_blocks} blocks</span></div>
            <div className="flex justify-between col-span-2">
              <span className="text-[#6b6a8a] flex items-center gap-1"><Shield className="h-3 w-3" /> Verification</span>
              <span className="text-[#a78bfa] font-medium">{config.verification.model} (M={config.verification.required_attestations}/{config.verification.total_witnesses})</span>
            </div>
          </div>
        </div>

        {/* Stats */}
        <div className="grid grid-cols-4 gap-2">
          {[
            { label: 'Members', value: config.members.length, icon: <Users className="h-3 w-3" />, color: '#fbbf24' },
            { label: 'Open', value: proposals.filter(p => p.status === 'open').length, icon: <Clock className="h-3 w-3" />, color: '#fbbf24' },
            { label: 'Passed', value: proposals.filter(p => p.status === 'passed').length, icon: <Check className="h-3 w-3" />, color: '#34d399' },
            { label: 'Executed', value: proposals.filter(p => p.status === 'executed').length, icon: <Coins className="h-3 w-3" />, color: '#a78bfa' },
          ].map((stat) => (
            <div key={stat.label} className="flex flex-col gap-0.5 rounded-lg p-2.5"
                 style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
              <div className="flex h-4 w-4 items-center justify-center rounded"
                   style={{ background: `${stat.color}1a`, color: stat.color }}>{stat.icon}</div>
              <div className="text-sm font-bold text-[#f0eff8]">{stat.value}</div>
              <div className="text-[8px] text-[#6b6a8a]">{stat.label}</div>
            </div>
          ))}
        </div>

        {/* Members */}
        <div>
          <div className="mb-1.5 text-[10px] font-semibold text-[#6b6a8a] uppercase tracking-widest">Members</div>
          <div className="rounded-xl p-2.5" style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
            {config.members.map((m, i) => (
              <MemberRow key={i} member={m} totalWeight={config.total_weight} />
            ))}
          </div>
        </div>

        {/* Chain status banner */}
        <div className="flex items-center gap-2 rounded-lg px-3 py-2 text-[10px]"
             style={{ background: isLive ? 'rgba(52,211,153,0.06)' : 'rgba(251,191,36,0.06)', border: `1px solid ${isLive ? 'rgba(52,211,153,0.15)' : 'rgba(251,191,36,0.15)'}` }}>
          {isLive ? (
            <>
              <Wifi className="h-3 w-3 text-green-400" />
              <span className="text-green-400 font-medium">Live from chain</span>
              <span className="text-[#6b6a8a]">· {proposals.length} proposals · block ~{currentBlock}</span>
              <button onClick={() => chain.refresh()} className="ml-auto text-[#6b6a8a] hover:text-white transition">
                <RefreshCw className={`h-3 w-3 ${chain.loading ? 'animate-spin' : ''}`} />
              </button>
            </>
          ) : chain.loading ? (
            <>
              <Loader2 className="h-3 w-3 text-yellow-400 animate-spin" />
              <span className="text-yellow-400">Connecting to chain...</span>
            </>
          ) : (
            <>
              <WifiOff className="h-3 w-3 text-yellow-400" />
              <span className="text-yellow-400 font-medium">Offline</span>
              <span className="text-[#6b6a8a]">· showing {proposals === MOCK_PROPOSALS ? 'demo' : 'cached'} data</span>
              {chain.chainError && <span className="text-red-400/70 truncate ml-1">{chain.chainError.slice(0, 40)}</span>}
            </>
          )}
        </div>

        {/* TX feedback */}
        {chain.lastTxHash && (
          <div className="flex items-center gap-2 rounded-lg px-3 py-2 text-[10px]"
               style={{ background: 'rgba(52,211,153,0.06)', border: '1px solid rgba(52,211,153,0.15)' }}>
            <Check className="h-3 w-3 text-green-400" />
            <span className="text-green-400">TX confirmed:</span>
            <code className="text-[#c0bfd8] font-mono truncate">{chain.lastTxHash}</code>
          </div>
        )}
        {chain.lastTxError && (
          <div className="flex items-center gap-2 rounded-lg px-3 py-2 text-[10px]"
               style={{ background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)' }}>
            <X className="h-3 w-3 text-red-400" />
            <span className="text-red-400 truncate">{chain.lastTxError}</span>
          </div>
        )}

        {/* Proposals */}
        <div>
          <div className="mb-1.5 text-[10px] font-semibold text-[#6b6a8a] uppercase tracking-widest">
            Proposals ({proposals.length})
          </div>
          <div className="space-y-2">
            {proposals.map((p) => (
              <ProposalCard
                key={p.id}
                proposal={p}
                config={config}
                currentBlock={currentBlock}
                onVote={chain.walletConnected ? chain.vote : undefined}
                onExecute={chain.walletConnected ? chain.execute : undefined}
                txPending={chain.txPending}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}

// ── Main Panel (multi-DAO) ──

export function DaoPanel() {
  const daos = useStore(s => s.daos)
  const activeDaoId = useStore(s => s.activeDaoId)
  const setActiveDao = useStore(s => s.setActiveDao)
  const [showTemplates, setShowTemplates] = useState(false)

  const activeDao = daos.find(d => d.id === activeDaoId)
  const visibleDaos = daos.filter(d => d.status !== 'archived')

  // Empty state: no DAOs created yet
  if (visibleDaos.length === 0 && !showTemplates) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center bg-[#06060f] px-8">
        <div className="flex flex-col items-center gap-4 max-w-md text-center">
          <div className="flex h-16 w-16 items-center justify-center rounded-2xl"
               style={{ background: 'rgba(167,139,250,0.08)', border: '1px solid rgba(167,139,250,0.15)' }}>
            <Building2 className="h-8 w-8 text-[#a78bfa]" />
          </div>
          <div>
            <div className="text-lg font-bold text-[#f0eff8] mb-1">No DAOs Yet</div>
            <div className="text-xs text-[#6b6a8a] leading-relaxed">
              Spin up a DAO to govern any aspect of your community — farm-to-table markets, 
              skill exchanges, mutual aid, citizens' assemblies. Your agents can join multiple 
              DAOs and govern them all locally from this workstation.
            </div>
          </div>
          <button onClick={() => setShowTemplates(true)}
                  className="flex items-center gap-2 rounded-xl px-6 py-3 text-sm font-semibold text-white transition hover:opacity-90"
                  style={{ background: 'linear-gradient(135deg, #a78bfa, #7c3aed)' }}>
            <LayoutGrid className="h-4 w-4" /> Create Your First DAO
          </button>
          <div className="text-[10px] text-[#3a3a5a] mt-2">
            10 templates · 5-step wizard · agents match, verify, and govern autonomously
          </div>
        </div>
      </div>
    )
  }

  // Template gallery fullscreen when no DAOs
  if (visibleDaos.length === 0 && showTemplates) {
    return (
      <div className="flex flex-1 flex-col bg-[#06060f] overflow-y-auto px-6 py-5">
        <DaoTemplateGallery onBack={() => setShowTemplates(false)} />
      </div>
    )
  }

  return (
    <div className="flex flex-1 bg-[#06060f] overflow-hidden">
      {/* Left: DAO Sidebar */}
      <div className="flex flex-col w-56 flex-shrink-0 overflow-y-auto"
           style={{ borderRight: '1px solid rgba(255,255,255,0.05)' }}>
        {/* Sidebar header */}
        <div className="flex items-center justify-between px-3 py-3"
             style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
          <span className="text-[10px] font-semibold text-[#6b6a8a] uppercase tracking-widest">My DAOs</span>
          <button onClick={() => setShowTemplates(!showTemplates)}
                  className="flex items-center gap-1 rounded px-2 py-1 text-[9px] font-semibold transition hover:opacity-80"
                  style={{ background: 'rgba(255,107,74,0.08)', border: '1px solid rgba(255,107,74,0.15)', color: '#ff6b4a' }}>
            <Plus className="h-2.5 w-2.5" />
          </button>
        </div>

        {/* DAO list */}
        <div className="flex-1 px-2 py-2 space-y-1">
          {visibleDaos.map(d => {
            const isActive = d.id === activeDaoId
            const agentCount = d.agent_ids.length
            return (
              <button key={d.id}
                      onClick={() => { setActiveDao(d.id); setShowTemplates(false) }}
                      className="flex w-full items-start gap-2.5 rounded-lg px-2.5 py-2 text-left transition hover:bg-white/[0.03]"
                      style={isActive ? { background: `${d.template_color}0d`, border: `1px solid ${d.template_color}22` } : { border: '1px solid transparent' }}>
                <div className="flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-lg mt-0.5"
                     style={{ background: `${d.template_color}1a`, color: d.template_color }}>
                  <Building2 className="h-3.5 w-3.5" />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-[11px] font-semibold truncate" style={{ color: isActive ? '#f0eff8' : '#c0bfd8' }}>{d.name}</div>
                  <div className="flex items-center gap-2 mt-0.5">
                    <span className="text-[9px] text-[#6b6a8a]">{d.config.members.length} members</span>
                    {agentCount > 0 && (
                      <span className="flex items-center gap-0.5 text-[9px] text-[#60a5fa]">
                        <Bot className="h-2.5 w-2.5" /> {agentCount}
                      </span>
                    )}
                  </div>
                  {d.status === 'deploying' && (
                    <span className="text-[8px] text-[#fbbf24] animate-pulse">deploying...</span>
                  )}
                </div>
              </button>
            )
          })}
        </div>

        {/* Sidebar footer */}
        <div className="px-3 py-2" style={{ borderTop: '1px solid rgba(255,255,255,0.04)' }}>
          <div className="text-[9px] text-[#3a3a5a] leading-relaxed">
            Your main agent governs all DAOs locally. Each DAO runs independently on-chain.
          </div>
        </div>
      </div>

      {/* Right: Active DAO or Template Gallery */}
      {showTemplates ? (
        <div className="flex-1 overflow-y-auto px-5 py-4">
          <DaoTemplateGallery onBack={() => setShowTemplates(false)} />
        </div>
      ) : activeDao ? (
        <DaoDashboard dao={activeDao} />
      ) : (
        <div className="flex flex-1 items-center justify-center">
          <div className="text-center">
            <Building2 className="h-8 w-8 text-[#3a3a5a] mx-auto mb-2" />
            <div className="text-xs text-[#6b6a8a]">Select a DAO from the sidebar</div>
          </div>
        </div>
      )}
    </div>
  )
}
