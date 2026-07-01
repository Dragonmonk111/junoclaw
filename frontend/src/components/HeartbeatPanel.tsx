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
  }
}

// ── Constants ───────────────────────────────────────────────────────────────

const GITHUB_DIGEST_JSON =
  'https://raw.githubusercontent.com/Dragonmonk111/junoclaw/main/tools/heartbeat-digest/digests/latest.json'

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

export function HeartbeatPanel() {
  const [digest, setDigest] = useState<DigestData>(MOCK_DIGEST)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [lastRefresh, setLastRefresh] = useState<string>('mock')

  const loadDigest = async () => {
    setLoading(true)
    setError(null)
    try {
      const res = await fetch(GITHUB_DIGEST_JSON, { cache: 'no-store' })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()
      setDigest(data)
      setLastRefresh('github')
    } catch (e) {
      console.warn('Could not load digest from GitHub, using mock data:', e)
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
            {lastRefresh === 'github' ? `Loaded ${formatDate(digest.date)}` : 'Mock data'}
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
                Generated {new Date(digest.meta.generated_at).toLocaleString()} · {digest.proposals.length} proposals · {digest.members.length} members
              </div>
            </div>
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
