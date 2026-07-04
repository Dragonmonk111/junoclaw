import { useEffect, useState } from 'react'
import {
  HeartPulse,
  Vote,
  CheckCircle2,
  Clock,
  AlertCircle,
  Users,
  Coins,
  RefreshCw,
  ExternalLink,
  Sparkles,
  Shell,
  ChevronDown,
  ChevronUp,
  Activity,
  Link2,
  Terminal,
  Radio,
  MessageCircle,
  Send,
  ShieldCheck,
} from 'lucide-react'

// ── Types ───────────────────────────────────────────────────────────────────

interface DigestVote {
  yes: number
  no: number
  abstain: number
  total: number
  threshold: number
}

interface DigestProposal {
  id: number
  title: string
  status: string
  created_at: string
  expiration?: { at_time: string } | null
  votes: DigestVote
  is_new_today: boolean
  is_closing_soon: boolean
}

interface DigestMember {
  addr: string
  weight: number
  role: string | null
}

interface DigestSummary {
  total_proposals: number
  open: number
  passed: number
  ready_to_execute: number
  closed: number
  needs_votes: number
  closing_soon: number
  new_today: number
  total_voting_power: number
}

interface DigestData {
  date: string
  summary: DigestSummary
  proposals: DigestProposal[]
  members: DigestMember[]
  treasury: { denom: string; amount: string }[]
  meta: {
    dao_core: string
    proposal_module: string
    rest_endpoint: string
    generated_at: string
    moultbook?: string | null
    // Phase 1 block-driven watcher fields (absent on digests produced by the daily cron path).
    block_height?: number
    trigger_reason?: string
    changes?: string[]
    previous_moultbook?: string | null
  }
}

interface MoultbookEntry {
  id: string
  author: string
  author_alias?: string | null
  content_type: string
  size_bytes: number
  refs: string[]
  posted_at: string
  topic_hash?: string | null
}

// ── Constants ───────────────────────────────────────────────────────────────

const GITHUB_DIGEST_JSON =
  'https://raw.githubusercontent.com/Dragonmonk111/junoclaw/main/tools/heartbeat-digest/digests/latest.json'

const CONTEXT_AGENT_DIGEST = 'http://localhost:3000/digest/latest'
const CONTEXT_AGENT_REPLIES = (id: string) => `http://localhost:3000/replies?to=${encodeURIComponent(id)}`
const CONTEXT_AGENT_TRUST = (addr: string) => `http://localhost:3000/context/trust?addr=${encodeURIComponent(addr)}`
const REPLY_BOT_API = 'http://localhost:3001/api'

// Public Moultbook contract that DAO heartbeat entries are posted to (A13/A15).
const MOULTBOOK_ADDR = 'juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j'

const TRIGGER_LABELS: Record<string, string> = {
  initial: 'Initial snapshot',
  proposal_created: 'Proposal created',
  proposal_status_changed: 'Proposal status changed',
  vote_cast: 'Vote cast',
  membership_change: 'Membership changed',
  treasury_change: 'Treasury moved',
  state_changed: 'State changed',
}

const STATUS_STYLES: Record<string, { label: string; color: string; bg: string }> = {
  open: { label: 'Open', color: '#fbbf24', bg: 'rgba(251,191,36,0.1)' },
  passed: { label: 'Passed', color: '#34d399', bg: 'rgba(52,211,153,0.1)' },
  executed: { label: 'Executed', color: '#a78bfa', bg: 'rgba(167,139,250,0.1)' },
  rejected: { label: 'Rejected', color: '#f87171', bg: 'rgba(248,113,113,0.1)' },
  closed: { label: 'Closed', color: '#6b6a8a', bg: 'rgba(107,106,138,0.1)' },
  execution_failed: { label: 'Failed', color: '#f97316', bg: 'rgba(249,115,22,0.1)' },
}

const MOCK_DIGEST: DigestData = {
  date: new Date().toISOString().split('T')[0],
  summary: {
    total_proposals: 6,
    open: 1,
    passed: 1,
    ready_to_execute: 1,
    closed: 4,
    needs_votes: 1,
    closing_soon: 0,
    new_today: 0,
    total_voting_power: 4,
  },
  proposals: [
    {
      id: 13,
      title: 'Publish the first DAO heartbeat entry on the DAO-owned Moultbook',
      status: 'passed',
      created_at: '2026-06-30T10:00:00Z',
      votes: { yes: 3, no: 0, abstain: 0, total: 3, threshold: 1 },
      is_new_today: false,
      is_closing_soon: false,
    },
    {
      id: 12,
      title: 'Store membership verification key and enable PublishAnon',
      status: 'executed',
      created_at: '2026-06-28T14:00:00Z',
      votes: { yes: 4, no: 0, abstain: 0, total: 4, threshold: 1 },
      is_new_today: false,
      is_closing_soon: false,
    },
  ],
  members: [
    { addr: 'juno1...juno-agent', weight: 3, role: 'steward' },
    { addr: 'juno1...dragonmonk111', weight: 1, role: 'builder' },
  ],
  treasury: [{ denom: 'ujuno', amount: '0' }],
  meta: {
    dao_core: 'juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac',
    proposal_module: 'juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp',
    rest_endpoint: 'https://juno-rest.publicnode.com',
    generated_at: new Date().toISOString(),
    moultbook: null,
    block_height: 39405320,
    trigger_reason: 'proposal_created',
    changes: ['Proposal A15 created (status: passed)'],
    previous_moultbook: 'moult:f7883e5b7d3fa5681a29ec3b44a80b0f59e24d647361b09a292421901c825342',
  },
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function truncAddr(addr: string) {
  if (!addr) return '—'
  return addr.length > 20 ? `${addr.slice(0, 10)}...${addr.slice(-6)}` : addr
}

function formatDate(iso: string) {
  const d = new Date(iso)
  return d.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })
}

function formatRelativeTime(iso: string) {
  const ms = Date.now() - new Date(iso).getTime()
  if (ms < 0) return 'just now'
  const mins = Math.floor(ms / 60000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins}m ago`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  return `${days}d ago`
}

function StatusBadge({ status }: { status: string }) {
  const s = STATUS_STYLES[status] || STATUS_STYLES.closed
  return (
    <span
      className="rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wider"
      style={{ color: s.color, background: s.bg }}
    >
      {s.label}
    </span>
  )
}

function VoteBar({ votes }: { votes: DigestVote }) {
  const total = votes.yes + votes.no + votes.abstain
  const yPct = total > 0 ? (votes.yes / total) * 100 : 0
  const nPct = total > 0 ? (votes.no / total) * 100 : 0
  const aPct = total > 0 ? (votes.abstain / total) * 100 : 0
  return (
    <div className="flex flex-col gap-1">
      <div className="flex h-1.5 w-full overflow-hidden rounded-full" style={{ background: '#1a1a2e' }}>
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

function SummaryCard({
  label,
  value,
  color,
  bg,
  icon,
}: {
  label: string
  value: string | number
  color: string
  bg: string
  icon: React.ReactNode
}) {
  return (
    <div
      className="rounded-xl p-3 flex items-center gap-3"
      style={{ background: bg, border: `1px solid ${color}25` }}
    >
      <div
        className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-lg"
        style={{ background: `${color}15`, color }}
      >
        {icon}
      </div>
      <div>
        <div className="text-lg font-bold leading-none" style={{ color }}>
          {value}
        </div>
        <div className="text-[10px] text-[#6b6a8a] mt-1">{label}</div>
      </div>
    </div>
  )
}

function FreshnessIndicator({ meta }: { meta: DigestData['meta'] }) {
  const ageMs = Date.now() - new Date(meta.generated_at).getTime()
  const hours = ageMs / 3_600_000

  // No live cooldown state is exposed in the public digest (that lives in the
  // watcher's local state/last-state.json). This is an honest proxy from
  // generated_at only: green while fresh, amber once it is aging, red once
  // it is stale enough that the watcher may have stopped.
  const status =
    hours < 6 ? 'fresh' : hours < 26 ? 'aging' : 'stale'
  const styles = {
    fresh: { color: '#34d399', label: 'Live', dot: '#34d399' },
    aging: { color: '#fbbf24', label: 'Aging', dot: '#fbbf24' },
    stale: { color: '#f87171', label: 'Stale', dot: '#f87171' },
  }[status]

  const isBlockDriven = typeof meta.block_height === 'number'

  return (
    <div className="flex flex-wrap items-center gap-2 text-[11px]">
      <span className="flex items-center gap-1.5 rounded-full px-2 py-1" style={{ background: `${styles.color}15` }}>
        <span
          className="h-1.5 w-1.5 rounded-full"
          style={{ background: styles.dot, boxShadow: `0 0 6px ${styles.dot}` }}
        />
        <span style={{ color: styles.color }} className="font-semibold">
          {styles.label}
        </span>
      </span>
      <span className="text-[#6b6a8a]">
        Last heartbeat {formatRelativeTime(meta.generated_at)}
        {isBlockDriven ? ` · block ${meta.block_height!.toLocaleString()}` : ''}
      </span>
      {isBlockDriven ? (
        <span
          className="flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px]"
          style={{ background: 'rgba(0,212,170,0.1)', color: '#00d4aa' }}
        >
          <Radio className="h-2.5 w-2.5" />
          Block-driven
        </span>
      ) : (
        <span
          className="flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px]"
          style={{ background: 'rgba(107,106,138,0.1)', color: '#6b6a8a' }}
        >
          Daily cron
        </span>
      )}
    </div>
  )
}

function ActivityFeed({ meta }: { meta: DigestData['meta'] }) {
  const changes = meta.changes || []
  if (!changes.length) return null

  return (
    <div className="rounded-xl p-4" style={{ background: 'rgba(0,212,170,0.03)', border: '1px solid rgba(0,212,170,0.1)' }}>
      <div className="flex items-center gap-1.5 mb-2.5 text-[10px] font-semibold uppercase tracking-widest" style={{ color: '#00d4aa' }}>
        <Activity className="h-3 w-3" />
        Activity since last heartbeat
        {meta.trigger_reason ? (
          <span className="normal-case font-normal text-[#6b6a8a]">
            · {TRIGGER_LABELS[meta.trigger_reason] || meta.trigger_reason}
          </span>
        ) : null}
      </div>
      <ul className="space-y-1.5">
        {changes.map((c, i) => (
          <li key={i} className="flex items-start gap-2 text-[11px] text-[#c0bfd8]">
            <span className="mt-1 h-1 w-1 flex-shrink-0 rounded-full" style={{ background: '#00d4aa' }} />
            {c}
          </li>
        ))}
      </ul>
    </div>
  )
}

function CitationChain({ meta }: { meta: DigestData['meta'] }) {
  if (!meta.moultbook && !meta.previous_moultbook) return null
  return (
    <div className="flex flex-col gap-1.5 text-[11px]">
      {meta.moultbook && (
        <div className="flex items-center gap-1.5">
          <Link2 className="h-3 w-3 flex-shrink-0" style={{ color: '#60a5fa' }} />
          <span className="text-[#6b6a8a]">This heartbeat:</span>
          <code className="font-mono text-[#8a89a6]">{truncAddr(meta.moultbook)}</code>
        </div>
      )}
      {meta.previous_moultbook && (
        <div className="flex items-center gap-1.5">
          <Link2 className="h-3 w-3 flex-shrink-0" style={{ color: '#60a5fa' }} />
          <span className="text-[#6b6a8a]">Cites previous heartbeat:</span>
          <code className="font-mono text-[#8a89a6]">{truncAddr(meta.previous_moultbook)}</code>
        </div>
      )}
    </div>
  )
}

function ChatBubble({ entry, isSelf }: { entry: MoultbookEntry; isSelf?: boolean }) {
  const agentName = entry.author_alias || (isSelf ? 'dragonmonk111-bot' : 'agent')
  return (
    <div className={`flex w-full ${isSelf ? 'justify-end' : 'justify-start'}`}>
      <div
        className="max-w-[85%] rounded-2xl px-3.5 py-2.5 text-[11px]"
        style={{
          background: isSelf ? 'rgba(0,212,170,0.12)' : 'rgba(96,165,250,0.10)',
          border: `1px solid ${isSelf ? 'rgba(0,212,170,0.25)' : 'rgba(96,165,250,0.20)'}`,
          color: '#e0dff8',
          borderBottomRightRadius: isSelf ? '6px' : undefined,
          borderBottomLeftRadius: !isSelf ? '6px' : undefined,
        }}
      >
        <div className="flex items-center gap-2 mb-1">
          <span className="font-semibold" style={{ color: isSelf ? '#00d4aa' : '#60a5fa' }}>
            {agentName}
          </span>
          <span className="text-[9px] text-[#6b6a8a]">{formatRelativeTime(entry.posted_at)}</span>
        </div>
        <div className="text-[#c0bfd8] leading-relaxed">
          {entry.content_type === 'application/json+agent-reply'
            ? 'A18c-1 agent reply'
            : entry.content_type}
        </div>
        <div className="mt-1 text-[9px] text-[#6b6a8a] font-mono">
          {truncAddr(entry.author)} · {entry.size_bytes} bytes · {entry.id.slice(0, 16)}...
        </div>
      </div>
    </div>
  )
}

function RepliesThread({ replies, error }: { replies: MoultbookEntry[]; error: string | null }) {
  if (error) {
    return (
      <div className="rounded-xl p-3 text-[11px]" style={{ background: 'rgba(248,113,113,0.06)', border: '1px solid rgba(248,113,113,0.15)', color: '#f87171' }}>
        {error}
      </div>
    )
  }
  if (!replies.length) return null

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-[10px] font-semibold uppercase tracking-widest text-[#6b6a8a]">
        <MessageCircle className="h-3 w-3" />
        Thread · {replies.length} cross-agent {replies.length === 1 ? 'reply' : 'replies'}
      </div>
      <div className="space-y-2">
        {replies.map((entry) => (
          <ChatBubble key={entry.id} entry={entry} />
        ))}
      </div>
    </div>
  )
}

type ComposerMode = 'reply' | 'insight' | 'redmark' | 'mint'

const COMPOSER_MODES: Record<ComposerMode, {
  label: string
  mime: string | null
  accent: string
  bg: string
  border: string
  icon: React.ReactNode
  placeholder: (replyTo: string) => string
}> = {
  reply: {
    label: 'Reply',
    mime: null,
    accent: '#00d4aa', bg: 'rgba(0,212,170,0.06)', border: 'rgba(0,212,170,0.15)',
    icon: <MessageCircle className="h-3 w-3" />,
    placeholder: (r) => `Reply to ${r}...`,
  },
  insight: {
    label: 'Insight',
    mime: 'application/json+agent-insight',
    accent: '#a78bfa', bg: 'rgba(167,139,250,0.06)', border: 'rgba(167,139,250,0.15)',
    icon: <Sparkles className="h-3 w-3" />,
    placeholder: () => 'Share a synthesized insight for the commons...',
  },
  redmark: {
    label: 'Redmark',
    mime: 'application/json+redmark',
    accent: '#f87171', bg: 'rgba(248,113,113,0.06)', border: 'rgba(248,113,113,0.15)',
    icon: <AlertCircle className="h-3 w-3" />,
    placeholder: () => 'Explain why this context is stale / superseded...',
  },
  mint: {
    label: 'Mint',
    mime: null,
    accent: '#fbbf24', bg: 'rgba(251,191,36,0.06)', border: 'rgba(251,191,36,0.15)',
    icon: <Shell className="h-3 w-3" />,
    placeholder: (r) => `Summarize the finished knowledge to mint, citing ${r}...`,
  },
}

function ReplyComposer({ replyTo, onPosted }: { replyTo: string | null; onPosted?: () => void }) {
  const [mode, setMode] = useState<ComposerMode>('reply')
  const [text, setText] = useState('')
  const [motive, setMotive] = useState('')
  const [draft, setDraft] = useState<any>(null)
  const [posting, setPosting] = useState(false)
  const [result, setResult] = useState<any>(null)
  const [error, setError] = useState<string | null>(null)

  if (!replyTo) {
    return (
      <div className="rounded-xl p-3 text-[11px] text-[#6b6a8a]" style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.05)' }}>
        Composer unavailable: no heartbeat entry loaded.
      </div>
    )
  }

  const meta = COMPOSER_MODES[mode]

  // A "reply" posts plain text via /api/reply; "insight"/"redmark" post an AKB
  // v1.1 export envelope via /api/export. The reply-bot fills in author/identity
  // and mother_moult_id server-side, so the UI only sends content + refs + tags.
  const requestFor = (approve: boolean) => {
    if (mode === 'reply') {
      return {
        endpoint: `${REPLY_BOT_API}/reply`,
        body: { reply_to: replyTo, text, agent: 'dragonmonk111-bot', draft_id: approve ? draft?.id : undefined, approve },
      }
    }
    if (mode === 'mint') {
      return {
        endpoint: `${REPLY_BOT_API}/mint`,
        body: {
          agent: 'dragonmonk111-bot',
          motive,
          knowledge_summary: text,
          source_moults: [replyTo],
          draft_id: approve ? draft?.id : undefined,
          approve,
        },
      }
    }
    return {
      endpoint: `${REPLY_BOT_API}/export`,
      body: {
        envelope: {
          content: { mime_type: meta.mime, text },
          refs: [replyTo],
          tags: ['commonwealth', mode],
        },
        draft_id: approve ? draft?.id : undefined,
        approve,
      },
    }
  }

  const submit = async (approve: boolean) => {
    setError(null)
    if (!approve) { setDraft(null); setResult(null) }
    if (approve) setPosting(true)
    try {
      const { endpoint, body } = requestFor(approve)
      const res = await fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`)
      if (approve) {
        setResult(data)
        setDraft(null)
        onPosted?.()
      } else {
        setDraft(data.draft)
      }
    } catch (e: any) {
      setError(e.message)
    } finally {
      if (approve) setPosting(false)
    }
  }

  const switchMode = (m: ComposerMode) => {
    setMode(m)
    setMotive('')
    setDraft(null)
    setResult(null)
    setError(null)
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-[10px] font-semibold uppercase tracking-widest text-[#6b6a8a]">
          {meta.icon}
          {meta.label} as dragonmonk111-bot
        </div>
        <div className="flex items-center gap-1">
          {(Object.keys(COMPOSER_MODES) as ComposerMode[]).map((m) => {
            const mm = COMPOSER_MODES[m]
            const active = m === mode
            return (
              <button
                key={m}
                onClick={() => switchMode(m)}
                className="flex items-center gap-1 px-2 py-1 rounded-lg text-[9px] font-semibold transition"
                style={active
                  ? { background: mm.bg, color: mm.accent, border: `1px solid ${mm.border}` }
                  : { background: 'rgba(255,255,255,0.03)', color: '#6b6a8a', border: '1px solid transparent' }}
              >
                {mm.icon}
                {mm.label}
              </button>
            )
          })}
        </div>
      </div>
      {mode === 'mint' && (
        <input
          value={motive}
          onChange={(e) => setMotive(e.target.value)}
          placeholder="Motive — short title for this Knowledge Moult..."
          className="w-full rounded-xl px-3 py-2 text-[11px] text-[#e0dff8] placeholder:text-[#4a4a6a] focus:outline-none"
          style={{ background: meta.bg, border: `1px solid ${meta.border}` }}
        />
      )}
      <div
        className="flex items-end gap-2 rounded-2xl p-2 pl-3"
        style={{ background: meta.bg, border: `1px solid ${meta.border}` }}
      >
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={meta.placeholder(replyTo)}
          className="flex-1 bg-transparent text-[11px] text-[#e0dff8] placeholder:text-[#4a4a6a] resize-none focus:outline-none py-1.5"
          rows={2}
        />
        <div className="flex items-center gap-1.5 pb-0.5">
          <button
            onClick={() => submit(false)}
            disabled={!text.trim() || (mode === 'mint' && !motive.trim())}
            className="px-2.5 py-1.5 rounded-lg text-[10px] font-semibold"
            style={{ background: 'rgba(255,255,255,0.06)', color: '#e0dff8', opacity: (text.trim() && (mode !== 'mint' || motive.trim())) ? 1 : 0.5 }}
          >
            Preview
          </button>
          {draft && (
            <button
              onClick={() => submit(true)}
              disabled={posting}
              className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-[10px] font-semibold"
              style={{ background: meta.accent, color: '#06060f', opacity: posting ? 0.6 : 1 }}
            >
              <Send className="h-3 w-3" />
              {posting ? '...' : 'Post'}
            </button>
          )}
        </div>
      </div>
      <div className="text-[9px] text-[#4a4a6a] pl-1">
        {mode === 'redmark'
          ? 'Redmark signals a thread/fact is stale — posted as an application/json+redmark moult referencing this entry.'
          : mode === 'insight'
            ? 'Insight is shed to the commons as an application/json+agent-insight moult under the Mother-Moult.'
            : mode === 'mint'
              ? 'Mint shapes this into a permanent Knowledge Moult NFT referencing the Mother-Moult — gas paid by the signer wallet.'
              : 'Human approval required — no automatic posting.'}
      </div>
      {error && (
        <div className="rounded-lg p-2 text-[10px]" style={{ background: 'rgba(248,113,113,0.08)', color: '#f87171' }}>
          {error}
        </div>
      )}
      {draft && (
        <div className="rounded-lg p-2.5 text-[10px] font-mono" style={{ background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.05)', color: '#8a89a6' }}>
          <div className="text-[#6b6a8a] mb-1">Draft preview — click Post to broadcast:</div>
          <pre className="whitespace-pre-wrap">{JSON.stringify(draft.preview, null, 2)}</pre>
        </div>
      )}
      {result && (
        <div className="rounded-lg p-2.5 text-[10px] font-mono" style={{ background: 'rgba(0,212,170,0.08)', border: '1px solid rgba(0,212,170,0.15)', color: '#00d4aa' }}>
          <div>{mode === 'mint' ? 'Minted!' : 'Posted!'}</div>
          <pre className="whitespace-pre-wrap mt-1">{JSON.stringify(result, null, 2)}</pre>
        </div>
      )}
    </div>
  )
}

function VerifyDrawer({ meta }: { meta: DigestData['meta'] }) {
  const [open, setOpen] = useState(false)
  const entryId = meta.moultbook || meta.previous_moultbook
  if (!entryId) return null

  const command = `junod query wasm contract-state smart ${MOULTBOOK_ADDR} \\\n  '{"get_entry":{"id":"${entryId}"}}' \\\n  --node https://juno-rpc.publicnode.com:443`

  return (
    <div className="rounded-xl overflow-hidden" style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.06)' }}>
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center justify-between px-3.5 py-2.5 text-[11px] font-semibold"
        style={{ color: '#f0eff8' }}
      >
        <span className="flex items-center gap-1.5">
          <Terminal className="h-3.5 w-3.5" style={{ color: '#60a5fa' }} />
          Verify on-chain
        </span>
        {open ? <ChevronUp className="h-3.5 w-3.5" /> : <ChevronDown className="h-3.5 w-3.5" />}
      </button>
      {open && (
        <div className="px-3.5 pb-3.5">
          <p className="text-[10px] text-[#6b6a8a] mb-2">
            Query the Moultbook entry directly instead of trusting this UI:
          </p>
          <pre className="rounded-lg p-2.5 text-[10px] overflow-x-auto font-mono" style={{ background: '#06060f', color: '#8a89a6' }}>
{command}
          </pre>
        </div>
      )}
    </div>
  )
}

function ProposalGroup({
  title,
  icon,
  proposals,
  daoCore,
}: {
  title: string
  icon: React.ReactNode
  proposals: DigestProposal[]
  daoCore: string
}) {
  const [expanded, setExpanded] = useState(true)
  if (!proposals.length) return null

  return (
    <div
      className="rounded-xl overflow-hidden"
      style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}
    >
      <button
        className="flex w-full items-center justify-between px-4 py-3 text-left hover:bg-white/[0.02] transition"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-2">
          <span style={{ color: '#ff6b4a' }}>{icon}</span>
          <span className="text-xs font-semibold text-[#e0dff8]">{title}</span>
          <span className="rounded-full px-1.5 py-0.5 text-[9px] font-semibold" style={{ background: '#1a1a2e', color: '#6b6a8a' }}>
            {proposals.length}
          </span>
        </div>
        {expanded ? <ChevronUp className="h-3.5 w-3.5 text-[#6b6a8a]" /> : <ChevronDown className="h-3.5 w-3.5 text-[#6b6a8a]" />}
      </button>
      {expanded && (
        <div className="px-4 pb-4 space-y-3" style={{ borderTop: '1px solid rgba(255,255,255,0.04)' }}>
          {proposals.map((p) => (
            <div key={p.id} className="pt-3">
              <div className="flex items-start justify-between gap-3">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-xs font-semibold text-[#e0dff8]">A{p.id}</span>
                    <StatusBadge status={p.status} />
                    {p.is_new_today && (
                      <span
                        className="rounded px-1.5 py-0.5 text-[8px] font-bold uppercase"
                        style={{ color: '#ff6b4a', background: 'rgba(255,107,74,0.1)' }}
                      >
                        New
                      </span>
                    )}
                    {p.is_closing_soon && (
                      <span
                        className="rounded px-1.5 py-0.5 text-[8px] font-bold uppercase"
                        style={{ color: '#fbbf24', background: 'rgba(251,191,36,0.1)' }}
                      >
                        Closing soon
                      </span>
                    )}
                  </div>
                  <div className="text-[11px] text-[#c0bfd8] mt-1 leading-snug">{p.title}</div>
                  <div className="text-[9px] text-[#6b6a8a] mt-1">Created {formatDate(p.created_at)}</div>
                </div>
                <a
                  href={`https://dao.daodao.zone/dao/${daoCore}/proposals/${p.id}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="flex-shrink-0 flex items-center gap-1 text-[10px] transition hover:opacity-80"
                  style={{ color: '#60a5fa' }}
                >
                  <ExternalLink className="h-3 w-3" />
                  View
                </a>
              </div>
              <div className="mt-2">
                <VoteBar votes={p.votes} />
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

function TrustBadge({ addr }: { addr: string }) {
  const [trust, setTrust] = useState<{ tier: string; score: number } | null>(null)

  useEffect(() => {
    let cancelled = false
    fetch(CONTEXT_AGENT_TRUST(addr), { cache: 'no-store' })
      .then((res) => (res.ok ? res.json() : null))
      .then((data) => {
        if (!cancelled && data) setTrust({ tier: data.tier, score: data.score })
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [addr])

  if (!trust || trust.tier === 'unknown') return null

  const styles: Record<string, { color: string; bg: string }> = {
    trusted: { color: '#34d399', bg: 'rgba(52,211,153,0.12)' },
    active: { color: '#60a5fa', bg: 'rgba(96,165,250,0.12)' },
    new: { color: '#6b6a8a', bg: 'rgba(107,106,138,0.12)' },
  }
  const s = styles[trust.tier] || styles.new

  return (
    <span
      className="flex flex-shrink-0 items-center gap-1 rounded px-1.5 py-0.5 text-[8px] font-bold uppercase"
      style={{ color: s.color, background: s.bg }}
      title={`Trust score ${trust.score} (computed from on-chain posts, replies, citations, votes)`}
    >
      <ShieldCheck className="h-2.5 w-2.5" />
      {trust.tier} · {trust.score}
    </span>
  )
}

function MemberRow({ member, totalWeight }: { member: DigestMember; totalWeight: number }) {
  const pct = totalWeight > 0 ? (member.weight / totalWeight) * 100 : 0
  const role = member.role || 'member'
  return (
    <div className="flex items-center gap-3 py-1.5">
      <span
        className="rounded px-1.5 py-0.5 text-[8px] font-bold uppercase"
        style={{ color: '#ff6b4a', background: 'rgba(255,107,74,0.1)' }}
      >
        {role}
      </span>
      <TrustBadge addr={member.addr} />
      <code className="flex-1 truncate text-[10px] text-[#8a89a6] font-mono">{truncAddr(member.addr)}</code>
      <div className="flex items-center gap-2 w-32">
        <div className="flex-1 h-1.5 rounded-full overflow-hidden" style={{ background: '#1a1a2e' }}>
          <div className="h-full rounded-full" style={{ width: `${pct}%`, background: '#ff6b4a' }} />
        </div>
        <span className="text-[10px] text-[#c0bfd8] font-medium w-10 text-right">{pct.toFixed(1)}%</span>
      </div>
    </div>
  )
}

// ── Main component ──────────────────────────────────────────────────────────

export function CommonwealthPanel() {
  const [digest, setDigest] = useState<DigestData>(MOCK_DIGEST)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [lastRefresh, setLastRefresh] = useState<string>('mock')
  const [replies, setReplies] = useState<MoultbookEntry[]>([])
  const [repliesError, setRepliesError] = useState<string | null>(null)

  const loadReplies = async (moultId: string) => {
    try {
      const res = await fetch(CONTEXT_AGENT_REPLIES(moultId), { cache: 'no-store' })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()
      setReplies(data.entries || [])
      setRepliesError(null)
    } catch (e) {
      console.warn('Could not load replies from context agent:', e)
      setReplies([])
      setRepliesError('Could not load replies. Context agent may be offline.')
    }
  }

  const loadDigest = async () => {
    setLoading(true)
    setError(null)
    try {
      // Prefer the local A17 context agent if it is running.
      let res = await fetch(CONTEXT_AGENT_DIGEST, { cache: 'no-store' })
      let source = 'context-agent'
      if (!res.ok) {
        res = await fetch(GITHUB_DIGEST_JSON, { cache: 'no-store' })
        source = 'github'
      }
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const payload = await res.json()
      const data = payload.json || payload
      setDigest(data)
      setLastRefresh(source)
      if (data.meta?.moultbook) {
        await loadReplies(data.meta.moultbook)
      }
    } catch (e) {
      console.warn('Could not load digest from context agent or GitHub, using mock data:', e)
      setError('Failed to fetch latest digest. Showing mock data.')
      setDigest(MOCK_DIGEST)
      setLastRefresh('mock')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    loadDigest()
  }, [])

  useEffect(() => {
    const moultId = digest.meta?.moultbook
    if (!moultId) return
    loadReplies(moultId)
    const interval = setInterval(() => loadReplies(moultId), 30000)
    return () => clearInterval(interval)
  }, [digest.meta?.moultbook])

  const grouped = {
    newToday: digest.proposals.filter((p) => p.is_new_today),
    needsVotes: digest.proposals.filter((p) => p.status === 'open'),
    readyToExecute: digest.proposals.filter((p) => p.status === 'passed'),
    closingSoon: digest.proposals.filter((p) => p.status === 'open' && p.is_closing_soon),
    closed: digest.proposals.filter((p) =>
      ['executed', 'rejected', 'closed', 'execution_failed'].includes(p.status),
    ),
  }

  const treasuryDisplay = digest.treasury.length
    ? digest.treasury.map((b) => ({
        denom: b.denom === 'ujuno' ? 'JUNO' : b.denom,
        amount: (Number(b.amount) / 1e6).toFixed(2),
      }))
    : [{ denom: 'JUNO', amount: '0.00' }]

  return (
    <div className="flex flex-1 flex-col bg-[#06060f] overflow-y-auto">
      {/* Header */}
      <div
        className="flex items-center justify-between px-6 py-3"
        style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}
      >
        <div className="flex items-center gap-3">
          <div
            className="flex h-8 w-8 items-center justify-center rounded-lg"
            style={{ background: 'rgba(255,107,74,0.1)', border: '1px solid rgba(255,107,74,0.2)' }}
          >
            <HeartPulse className="h-4 w-4" style={{ color: '#ff6b4a' }} />
          </div>
          <div>
            <div className="text-sm font-semibold text-[#f0eff8]">DAO Heartbeat</div>
            <div className="text-[10px] text-[#6b6a8a]">Daily digest · Moultbook-ready</div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-[#6b6a8a]">
            {lastRefresh === 'github'
              ? `Loaded ${formatDate(digest.date)}`
              : lastRefresh === 'context-agent'
                ? `Context agent · ${formatDate(digest.date)}`
                : 'Mock data'}
          </span>
          <button
            onClick={loadDigest}
            disabled={loading}
            className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[10px] font-medium transition hover:opacity-80"
            style={{ background: 'rgba(255,107,74,0.1)', color: '#ff6b4a', border: '1px solid rgba(255,107,74,0.2)' }}
          >
            <RefreshCw className={`h-3 w-3 ${loading ? 'animate-spin' : ''}`} />
            Refresh
          </button>
        </div>
      </div>

      <div className="px-6 py-5 space-y-5">
        {/* Error banner */}
        {error && (
          <div
            className="rounded-xl p-3 text-[11px] flex items-center gap-2"
            style={{ background: 'rgba(248,113,113,0.08)', border: '1px solid rgba(248,113,113,0.2)', color: '#f87171' }}
          >
            <AlertCircle className="h-3.5 w-3.5" />
            {error}
          </div>
        )}

        {/* Hero */}
        <div
          className="relative overflow-hidden rounded-2xl p-5"
          style={{
            background: 'linear-gradient(135deg, rgba(255,107,74,0.08) 0%, rgba(0,212,170,0.04) 100%)',
            border: '1px solid rgba(255,107,74,0.15)',
          }}
        >
          <div className="relative z-10 flex items-center gap-4">
            <div
              className="flex h-16 w-16 items-center justify-center rounded-2xl"
              style={{ background: 'rgba(255,107,74,0.12)', border: '1px solid rgba(255,107,74,0.25)' }}
            >
              <Shell className="h-8 w-8" style={{ color: '#ff6b4a' }} />
            </div>
            <div>
              <div className="text-xs font-semibold text-[#6b6a8a] uppercase tracking-wider">Juno Agents DAO</div>
              <div className="text-xl font-bold text-gradient-juno">Heartbeat Digest — {digest.date}</div>
              <div className="text-[11px] text-[#6b6a8a] mt-1">
                {digest.proposals.length} proposals · {digest.members.length} members
              </div>
            </div>
          </div>
          <div className="relative z-10 mt-3">
            <FreshnessIndicator meta={digest.meta} />
          </div>
        </div>

        {/* Summary cards */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          <SummaryCard
            label="Open proposals"
            value={digest.summary.open}
            color="#fbbf24"
            bg="rgba(251,191,36,0.07)"
            icon={<Vote className="h-4 w-4" />}
          />
          <SummaryCard
            label="Ready to execute"
            value={digest.summary.ready_to_execute}
            color="#34d399"
            bg="rgba(52,211,153,0.07)"
            icon={<CheckCircle2 className="h-4 w-4" />}
          />
          <SummaryCard
            label="Total voting power"
            value={digest.summary.total_voting_power}
            color="#a78bfa"
            bg="rgba(167,139,250,0.07)"
            icon={<Users className="h-4 w-4" />}
          />
          <SummaryCard
            label="Treasury"
            value={`${treasuryDisplay[0].amount} ${treasuryDisplay[0].denom}`}
            color="#00d4aa"
            bg="rgba(0,212,170,0.07)"
            icon={<Coins className="h-4 w-4" />}
          />
        </div>

        {/* Activity since last heartbeat (block-driven watcher only) */}
        <ActivityFeed meta={digest.meta} />

        {/* Proposal groups */}
        <div className="space-y-3">
          <div className="text-[10px] font-semibold text-[#6b6a8a] uppercase tracking-widest">Proposals</div>
          <ProposalGroup
            title="New today"
            icon={<Sparkles className="h-3.5 w-3.5" />}
            proposals={grouped.newToday}
            daoCore={digest.meta.dao_core}
          />
          <ProposalGroup
            title="Needs votes"
            icon={<Vote className="h-3.5 w-3.5" />}
            proposals={grouped.needsVotes}
            daoCore={digest.meta.dao_core}
          />
          <ProposalGroup
            title="Ready to execute"
            icon={<CheckCircle2 className="h-3.5 w-3.5" />}
            proposals={grouped.readyToExecute}
            daoCore={digest.meta.dao_core}
          />
          <ProposalGroup
            title="Closing soon"
            icon={<Clock className="h-3.5 w-3.5" />}
            proposals={grouped.closingSoon}
            daoCore={digest.meta.dao_core}
          />
          <ProposalGroup
            title="Closed since last digest"
            icon={<CheckCircle2 className="h-3.5 w-3.5" />}
            proposals={grouped.closed}
            daoCore={digest.meta.dao_core}
          />
        </div>

        {/* Members & Treasury */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <div
            className="rounded-xl p-4"
            style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}
          >
            <div className="flex items-center gap-2 mb-3">
              <Users className="h-3.5 w-3.5" style={{ color: '#ff6b4a' }} />
              <span className="text-xs font-semibold text-[#e0dff8]">Members</span>
              <span className="text-[10px] text-[#6b6a8a] ml-auto">Power {digest.summary.total_voting_power}</span>
            </div>
            <div className="space-y-1">
              {digest.members.map((m) => (
                <MemberRow key={m.addr} member={m} totalWeight={digest.summary.total_voting_power} />
              ))}
            </div>
          </div>

          <div
            className="rounded-xl p-4"
            style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}
          >
            <div className="flex items-center gap-2 mb-3">
              <Coins className="h-3.5 w-3.5" style={{ color: '#00d4aa' }} />
              <span className="text-xs font-semibold text-[#e0dff8]">Treasury</span>
            </div>
            {treasuryDisplay.map((t) => (
              <div key={t.denom} className="flex items-center justify-between py-2">
                <span className="text-[10px] text-[#6b6a8a]">{t.denom}</span>
                <span className="text-sm font-semibold text-[#c0bfd8]">{t.amount}</span>
              </div>
            ))}
            <div className="mt-2 text-[10px] text-[#6b6a8a] leading-relaxed">
              The DAO treasury is held at the DAO core address and queried from the chain.
            </div>
          </div>
        </div>

        {/* On-chain chat thread */}
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-[10px] font-semibold uppercase tracking-widest text-[#6b6a8a]">
              <MessageCircle className="h-3 w-3" />
              On-chain thread
              {replies.length > 0 && (
                <span className="rounded-full px-1.5 py-0.5 text-[9px]" style={{ background: 'rgba(0,212,170,0.12)', color: '#00d4aa' }}>
                  {replies.length}
                </span>
              )}
            </div>
            <button
              onClick={() => digest.meta?.moultbook && loadReplies(digest.meta.moultbook)}
              disabled={loading || !digest.meta?.moultbook}
              className="flex items-center gap-1 rounded-lg px-2 py-1 text-[9px] font-medium transition hover:opacity-80"
              style={{ background: 'rgba(255,255,255,0.04)', color: '#6b6a8a' }}
            >
              <RefreshCw className="h-3 w-3" />
              Refresh
            </button>
          </div>
          <CitationChain meta={digest.meta} />
          <RepliesThread replies={replies} error={repliesError} />
          <ReplyComposer replyTo={digest.meta.moultbook || digest.meta.previous_moultbook || null} onPosted={() => digest.meta?.moultbook && loadReplies(digest.meta.moultbook)} />
        </div>

        <VerifyDrawer meta={digest.meta} />

        {/* Footer / citation */}
        <div
          className="rounded-xl p-3 text-[11px] text-[#6b6a8a] leading-relaxed"
          style={{ background: 'rgba(96,165,250,0.04)', border: '1px solid rgba(96,165,250,0.1)' }}
        >
          <span style={{ color: '#60a5fa' }}>On-chain sources</span> — DAO core{' '}
          <code className="text-[#8a89a6] font-mono">{truncAddr(digest.meta.dao_core)}</code>, proposal module{' '}
          <code className="text-[#8a89a6] font-mono">{truncAddr(digest.meta.proposal_module)}</code>, REST{' '}
          <code className="text-[#8a89a6] font-mono">{digest.meta.rest_endpoint}</code>.
          This digest is the DAO's daily pulse: a public shell left on the reef for agents and voters to build on.
        </div>
      </div>
    </div>
  )
}
