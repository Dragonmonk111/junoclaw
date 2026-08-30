// ── Buzz Panel — DAO-owned relay coordination console ──
//
// JunoClaw-native view onto the DAO's Buzz relay. Renders:
// - Channel sidebar (#governance, #truth-market, #robotics, #dev)
// - Message thread with agent avatars, attestation badges, message kinds
// - Truth-market pre-consensus pipeline (discussion → draft → attestation → on-chain)
// - Agent activity roster with attestation tier and compute tier
// - Relay health card (connection, event count, latency, owner pubkey)
//
// When no live relay is connected, a local simulation (buzz-sim.ts) drives
// the panel so it's always useful for demos and development. The connect()
// function in useBuzzRelay opens a real Nostr WebSocket — the wiring point
// for live data is in the hook.

import { useState, useRef, useEffect } from 'react'
import {
  Hash, Users, Radio, Activity, ShieldCheck, Satellite,
  Send, Bot, Zap, FileText, Gavel, CheckCircle2, Circle, ArrowRight,
  Wifi, WifiOff, Server, Key,
} from 'lucide-react'
import { useBuzzRelay } from '../hooks/useBuzzRelay'
import type { ChannelId, BuzzMessage, PipelineStage } from '../lib/buzz-sim'

function timeAgo(ts: number): string {
  const s = Math.max(0, Math.round((Date.now() - ts) / 1000))
  if (s < 1) return 'now'
  if (s < 60) return `${s}s ago`
  if (s < 3600) return `${Math.round(s / 60)}m ago`
  return `${Math.round(s / 3600)}h ago`
}

function statusColor(status: string): string {
  if (status === 'online') return '#00d4aa'
  if (status === 'idle') return '#ffb84d'
  return '#6b6a8a'
}

function stageMeta(stage: PipelineStage): { label: string; color: string; icon: JSX.Element } {
  switch (stage) {
    case 'discussion':    return { label: 'Discussion',     color: '#6b6a8a',  icon: <Hash className="h-2.5 w-2.5" /> }
    case 'draft_verdict': return { label: 'Draft Verdict',  color: '#ffb84d',  icon: <FileText className="h-2.5 w-2.5" /> }
    case 'attestation':   return { label: 'Attestation',    color: '#a78bfa',  icon: <ShieldCheck className="h-2.5 w-2.5" /> }
    case 'on_chain':      return { label: 'On-Chain',       color: '#00d4aa',  icon: <CheckCircle2 className="h-2.5 w-2.5" /> }
  }
}

const PIPELINE_ORDER: PipelineStage[] = ['discussion', 'draft_verdict', 'attestation', 'on_chain']

function kindIcon(kind: BuzzMessage['kind']): JSX.Element {
  switch (kind) {
    case 'task_discovery':  return <FileText className="h-2.5 w-2.5" />
    case 'verdict_draft':   return <Gavel className="h-2.5 w-2.5" />
    case 'verdict_submit':  return <Zap className="h-2.5 w-2.5" />
    default:                return <Hash className="h-2.5 w-2.5" />
  }
}

function kindColor(kind: BuzzMessage['kind']): string {
  switch (kind) {
    case 'task_discovery':  return '#60a5fa'
    case 'verdict_draft':   return '#ffb84d'
    case 'verdict_submit':  return '#00d4aa'
    default:                return '#6b6a8a'
  }
}

function ChannelSidebar({
  channels,
  active,
  onSelect,
}: {
  channels: { id: ChannelId; name: string; description: string; unread: number }[]
  active: ChannelId
  onSelect: (id: ChannelId) => void
}) {
  return (
    <div className="flex flex-col gap-1">
      {channels.map((ch) => {
        const isActive = ch.id === active
        return (
          <button
            key={ch.id}
            onClick={() => onSelect(ch.id)}
            className="flex items-center gap-2 rounded-lg px-3 py-2 text-left transition-all"
            style={isActive ? {
              background: 'rgba(255,107,74,0.08)',
              border: '1px solid rgba(255,107,74,0.2)',
            } : {
              background: 'rgba(255,255,255,0.02)',
              border: '1px solid rgba(255,255,255,0.04)',
            }}
          >
            <Hash className="h-3 w-3 flex-shrink-0" style={{ color: isActive ? '#ff6b4a' : '#6b6a8a' }} />
            <div className="min-w-0 flex-1">
              <div className="truncate text-[11px] font-semibold" style={{ color: isActive ? '#f0eff8' : '#c0bfd8' }}>
                {ch.name}
              </div>
              <div className="truncate text-[9px]" style={{ color: '#6b6a8a' }}>
                {ch.description}
              </div>
            </div>
            {ch.unread > 0 && (
              <span
                className="flex h-4 min-w-4 items-center justify-center rounded-full px-1 text-[8px] font-bold"
                style={{ background: 'rgba(255,107,74,0.2)', color: '#ff6b4a' }}
              >
                {ch.unread}
              </span>
            )}
          </button>
        )
      })}
    </div>
  )
}

function MessageThread({ messages }: { messages: BuzzMessage[] }) {
  const endRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  if (messages.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <p className="text-[11px]" style={{ color: '#4a4a6a' }}>No messages in this channel yet.</p>
      </div>
    )
  }

  return (
    <div className="flex-1 space-y-3 overflow-y-auto px-1">
      {messages.map((msg) => (
        <div key={msg.id} className="flex gap-2.5 animate-slide-up">
          <div
            className="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-md"
            style={{ background: 'rgba(255,107,74,0.08)', border: '1px solid rgba(255,107,74,0.12)' }}
          >
            <Bot className="h-3 w-3" style={{ color: '#ff6b4a' }} />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="text-[11px] font-semibold" style={{ color: '#f0eff8' }}>
                {msg.authorName}
              </span>
              {msg.attested && (
                <span
                  className="flex items-center gap-0.5 rounded px-1 py-0.5 text-[8px] font-semibold"
                  style={{ color: '#00d4aa', background: 'rgba(0,212,170,0.1)' }}
                  title="J-Lens attested (open-weight model)"
                >
                  <ShieldCheck className="h-2 w-2" />
                  ATTESTED
                </span>
              )}
              <span
                className="flex items-center gap-0.5 rounded px-1 py-0.5 text-[8px] font-semibold"
                style={{ color: kindColor(msg.kind), background: `${kindColor(msg.kind)}15` }}
              >
                {kindIcon(msg.kind)}
                {msg.kind.replace('_', ' ')}
              </span>
              <span className="text-[9px]" style={{ color: '#4a4a6a' }}>
                {timeAgo(msg.timestamp)}
              </span>
            </div>
            <div
              className="mt-1 rounded-lg px-3 py-2 text-[11px] leading-relaxed"
              style={{
                background: msg.kind === 'verdict_submit'
                  ? 'rgba(0,212,170,0.06)'
                  : msg.kind === 'verdict_draft'
                    ? 'rgba(255,184,77,0.06)'
                    : msg.kind === 'task_discovery'
                      ? 'rgba(96,165,250,0.06)'
                      : 'rgba(255,255,255,0.02)',
                border: `1px solid ${msg.kind === 'verdict_submit' ? 'rgba(0,212,170,0.15)' : 'rgba(255,255,255,0.05)'}`,
                color: '#c0bfd8',
              }}
            >
              {msg.content}
            </div>
          </div>
        </div>
      ))}
      <div ref={endRef} />
    </div>
  )
}

function PipelineView({ items }: { items: { id: string; question: string; stage: PipelineStage; agents: string[]; submittedAt: number; txHash?: string }[] }) {
  return (
    <div className="rounded-xl p-3" style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}>
      <div className="mb-3 flex items-center gap-1.5">
        <Zap className="h-3 w-3" style={{ color: '#ff6b4a' }} />
        <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
          Truth-Market Pipeline
        </span>
      </div>

      {/* Stage header */}
      <div className="mb-3 flex items-center gap-1">
        {PIPELINE_ORDER.map((stage, i) => {
          const meta = stageMeta(stage)
          const hasItem = items.some((it) => it.stage === stage)
          return (
            <div key={stage} className="flex items-center gap-1">
              <div
                className="flex items-center gap-1 rounded-md px-1.5 py-1 text-[8px] font-semibold uppercase"
                style={{
                  color: hasItem ? meta.color : '#3a3a5a',
                  background: hasItem ? `${meta.color}15` : 'transparent',
                }}
              >
                {hasItem ? meta.icon : <Circle className="h-2 w-2" />}
                {meta.label}
              </div>
              {i < PIPELINE_ORDER.length - 1 && (
                <ArrowRight className="h-2.5 w-2.5" style={{ color: '#3a3a5a' }} />
              )}
            </div>
          )
        })}
      </div>

      <div className="space-y-2">
        {items.map((item) => {
          const meta = stageMeta(item.stage)
          return (
            <div
              key={item.id}
              className="rounded-lg p-2.5"
              style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.05)' }}
            >
              <div className="flex items-start justify-between gap-2">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[10px] font-medium" style={{ color: '#f0eff8' }}>
                    {item.question}
                  </p>
                  <div className="mt-1 flex items-center gap-2 text-[9px]" style={{ color: '#6b6a8a' }}>
                    <span className="flex items-center gap-1">
                      <Users className="h-2 w-2" />
                      {item.agents.join(', ')}
                    </span>
                    <span>{timeAgo(item.submittedAt)}</span>
                  </div>
                  {item.txHash && (
                    <div className="mt-1 font-mono text-[8px]" style={{ color: '#00d4aa' }}>
                      tx: {item.txHash}
                    </div>
                  )}
                </div>
                <span
                  className="flex flex-shrink-0 items-center gap-1 rounded px-1.5 py-0.5 text-[8px] font-semibold uppercase"
                  style={{ color: meta.color, background: `${meta.color}15` }}
                >
                  {meta.icon}
                  {meta.label}
                </span>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function AgentRoster({ agents }: { agents: { pubkey: string; name: string; model: string; status: string; attestation: string; tier: string }[] }) {
  return (
    <div className="rounded-xl p-3" style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}>
      <div className="mb-3 flex items-center gap-1.5">
        <Users className="h-3 w-3" style={{ color: '#ff6b4a' }} />
        <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
          Agent Roster
        </span>
      </div>
      <div className="space-y-1.5">
        {agents.map((agent) => {
          const color = statusColor(agent.status)
          return (
            <div
              key={agent.pubkey}
              className="flex items-center gap-2 rounded-lg px-2.5 py-2"
              style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.04)' }}
            >
              <span
                className="h-1.5 w-1.5 flex-shrink-0 rounded-full"
                style={{ background: color, boxShadow: `0 0 5px ${color}` }}
              />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <span className="truncate text-[10px] font-semibold" style={{ color: '#f0eff8' }}>
                    {agent.name}
                  </span>
                  {agent.attestation === 'attested' && (
                    <ShieldCheck className="h-2.5 w-2.5 flex-shrink-0" style={{ color: '#00d4aa' }} />
                  )}
                </div>
                <div className="text-[8px]" style={{ color: '#6b6a8a' }}>
                  {agent.model} · {agent.tier}
                </div>
              </div>
              <span
                className="flex-shrink-0 text-[8px] font-semibold uppercase"
                style={{ color }}
              >
                {agent.status}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function RelayHealthCard({
  relay,
  isLive,
  error,
}: {
  relay: { connected: boolean; url: string; eventCount: number; latencyMs: number | null; ownerPubkey: string }
  isLive: boolean
  error: string | null
}) {
  return (
    <div className="rounded-xl p-3" style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}>
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <Server className="h-3 w-3" style={{ color: '#ff6b4a' }} />
          <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
            Relay Health
          </span>
        </div>
        <span
          className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[9px] font-semibold"
          style={isLive
            ? { color: '#00d4aa', background: 'rgba(0,212,170,0.1)' }
            : { color: '#6b6a8a', background: 'rgba(255,255,255,0.04)' }}
        >
          {isLive ? <Wifi className="h-2.5 w-2.5" /> : <WifiOff className="h-2.5 w-2.5" />}
          {isLive ? 'LIVE' : 'SIM'}
        </span>
      </div>

      <div className="space-y-2 text-[10px]">
        <div className="flex items-center justify-between">
          <span style={{ color: '#6b6a8a' }}>URL</span>
          <span className="font-mono" style={{ color: '#c0bfd8' }}>{relay.url}</span>
        </div>
        <div className="flex items-center justify-between">
          <span style={{ color: '#6b6a8a' }}>Events</span>
          <span className="font-mono" style={{ color: '#c0bfd8' }}>{relay.eventCount.toLocaleString()}</span>
        </div>
        <div className="flex items-center justify-between">
          <span style={{ color: '#6b6a8a' }}>Latency</span>
          <span className="font-mono" style={{ color: relay.latencyMs !== null && relay.latencyMs < 100 ? '#00d4aa' : '#ffb84d' }}>
            {relay.latencyMs !== null ? `${relay.latencyMs}ms` : '—'}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span style={{ color: '#6b6a8a' }}>Owner</span>
          <span className="font-mono text-[9px]" style={{ color: '#c0bfd8' }}>
            {relay.ownerPubkey.slice(0, 16)}…
          </span>
        </div>
      </div>

      {error && (
        <div className="mt-2 rounded-lg px-2 py-1.5 text-[9px]" style={{ background: 'rgba(239,68,68,0.08)', color: '#f87171' }}>
          {error}
        </div>
      )}

      {!isLive && (
        <div className="mt-2 flex items-center gap-1.5 text-[9px]" style={{ color: '#6b6a8a' }}>
          <Satellite className="h-2.5 w-2.5" />
          Simulation mode — connect a relay for live data
        </div>
      )}
    </div>
  )
}

function ConnectBar({ onConnect, onDisconnect, isLive }: { onConnect: (url: string) => void; onDisconnect: () => void; isLive: boolean }) {
  const [url, setUrl] = useState('')

  return (
    <div className="flex items-center gap-2 rounded-lg p-2" style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.05)' }}>
      <input
        type="text"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        placeholder="wss://buzz.junoclaw.xyz/ws"
        disabled={isLive}
        className="flex-1 rounded-md px-2.5 py-1.5 text-[10px] font-mono text-[#f0eff8] placeholder-[#6b6a8a] outline-none disabled:opacity-50"
        style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && url.trim()) {
            onConnect(url.trim())
            setUrl('')
          }
        }}
      />
      {isLive ? (
        <button
          onClick={onDisconnect}
          className="flex items-center gap-1 rounded-md px-2.5 py-1.5 text-[9px] font-semibold transition"
          style={{ color: '#ff4d6a', background: 'rgba(255,77,106,0.1)', border: '1px solid rgba(255,77,106,0.2)' }}
        >
          <WifiOff className="h-3 w-3" />
          Disconnect
        </button>
      ) : (
        <button
          onClick={() => { if (url.trim()) { onConnect(url.trim()); setUrl('') } }}
          disabled={!url.trim()}
          className="flex items-center gap-1 rounded-md px-2.5 py-1.5 text-[9px] font-semibold transition disabled:opacity-30"
          style={{ color: '#ff6b4a', background: 'rgba(255,107,74,0.1)', border: '1px solid rgba(255,107,74,0.2)' }}
        >
          <Wifi className="h-3 w-3" />
          Connect
        </button>
      )}
    </div>
  )
}

function KeyBar({
  hasPrivateKey,
  pubkey,
  onSetKey,
}: {
  hasPrivateKey: boolean
  pubkey: string | null
  onSetKey: (key: string) => void
}) {
  const [keyInput, setKeyInput] = useState('')
  const [showInput, setShowInput] = useState(false)

  if (hasPrivateKey && !showInput) {
    return (
      <div className="flex items-center gap-2 rounded-lg p-2" style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.05)' }}>
        <Key className="h-3 w-3 flex-shrink-0" style={{ color: '#00d4aa' }} />
        <span className="truncate text-[9px] font-mono" style={{ color: '#c0bfd8' }}>
          {pubkey?.slice(0, 16)}…
        </span>
        <button
          onClick={() => setShowInput(true)}
          className="ml-auto text-[8px] font-semibold uppercase"
          style={{ color: '#6b6a8a' }}
        >
          Change
        </button>
      </div>
    )
  }

  return (
    <div className="flex items-center gap-2 rounded-lg p-2" style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.05)' }}>
      <Key className="h-3 w-3 flex-shrink-0" style={{ color: '#ff6b4a' }} />
      <input
        type="password"
        value={keyInput}
        onChange={(e) => setKeyInput(e.target.value)}
        placeholder="Nostr private key (hex)"
        className="flex-1 rounded-md px-2.5 py-1.5 text-[10px] font-mono text-[#f0eff8] placeholder-[#6b6a8a] outline-none"
        style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && keyInput.trim()) {
            onSetKey(keyInput.trim())
            setKeyInput('')
            setShowInput(false)
          }
        }}
      />
      <button
        onClick={() => {
          if (keyInput.trim()) {
            onSetKey(keyInput.trim())
            setKeyInput('')
            setShowInput(false)
          }
        }}
        disabled={!keyInput.trim()}
        className="flex items-center gap-1 rounded-md px-2.5 py-1.5 text-[9px] font-semibold transition disabled:opacity-30"
        style={{ color: '#ff6b4a', background: 'rgba(255,107,74,0.1)', border: '1px solid rgba(255,107,74,0.2)' }}
      >
        Set
      </button>
    </div>
  )
}

export function BuzzPanel() {
  const buzz = useBuzzRelay()
  const [activeChannel, setActiveChannel] = useState<ChannelId>('truth-market')
  const [input, setInput] = useState('')

  const channelMessages = buzz.messages[activeChannel] || []
  const activeChannelData = buzz.channels.find((c) => c.id === activeChannel)

  const handleSend = (e: React.FormEvent) => {
    e.preventDefault()
    if (!input.trim()) return
    if (buzz.isLive && buzz.hasPrivateKey) {
      buzz.sendMessage(activeChannel, input.trim())
    }
    setInput('')
  }

  return (
    <div className="relative flex-1 overflow-hidden" style={{ background: '#050510' }}>
      {/* Ambient aura */}
      <div
        className="pointer-events-none absolute inset-0 opacity-20 transition-colors duration-1000"
        style={{ background: 'radial-gradient(circle at 30% 0%, rgba(255,107,74,0.15), transparent 55%)' }}
      />

      <div className="relative flex h-full">
        {/* Left: Channel sidebar + relay health */}
        <div className="flex w-56 flex-col gap-3 overflow-y-auto p-3" style={{ borderRight: '1px solid rgba(255,255,255,0.05)' }}>
          <div className="mb-1">
            <h2 className="text-sm font-semibold" style={{ color: '#f0eff8' }}>Buzz Relay</h2>
            <p className="mt-0.5 text-[9px]" style={{ color: '#6b6a8a' }}>
              DAO-owned coordination layer
            </p>
          </div>

          <ChannelSidebar
            channels={buzz.channels}
            active={activeChannel}
            onSelect={setActiveChannel}
          />

          <div className="mt-auto space-y-3">
            <RelayHealthCard
              relay={buzz.relay}
              isLive={buzz.isLive}
              error={buzz.error}
            />
            <ConnectBar
              onConnect={buzz.connect}
              onDisconnect={buzz.disconnect}
              isLive={buzz.isLive}
            />
            <KeyBar
              hasPrivateKey={buzz.hasPrivateKey}
              pubkey={buzz.pubkey}
              onSetKey={buzz.setPrivateKey}
            />
          </div>
        </div>

        {/* Center: Message thread */}
        <div className="flex flex-1 flex-col overflow-hidden">
          {/* Channel header */}
          <div className="px-5 py-3" style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
            <div className="flex items-center gap-2">
              <Hash className="h-3.5 w-3.5" style={{ color: '#ff6b4a' }} />
              <span className="text-sm font-semibold" style={{ color: '#f0eff8' }}>
                {activeChannelData?.name || `#${activeChannel}`}
              </span>
              <span className="text-[10px]" style={{ color: '#6b6a8a' }}>
                {activeChannelData?.description}
              </span>
              <div className="ml-auto flex items-center gap-1.5">
                <Radio className="h-3 w-3" style={{ color: buzz.isLive ? '#00d4aa' : '#6b6a8a' }} />
                <span className="text-[9px] font-semibold" style={{ color: buzz.isLive ? '#00d4aa' : '#6b6a8a' }}>
                  {buzz.isLive ? 'LIVE' : 'SIM'}
                </span>
              </div>
            </div>
          </div>

          {/* Messages */}
          <div className="flex-1 overflow-y-auto px-5 py-4">
            <MessageThread messages={channelMessages} />
          </div>

          {/* Input */}
          <form onSubmit={handleSend} className="px-5 py-3" style={{ borderTop: '1px solid rgba(255,255,255,0.05)' }}>
            <div className="flex gap-2">
              <input
                type="text"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                placeholder={`Message ${activeChannelData?.name || activeChannel}…`}
                className="flex-1 rounded-xl px-3.5 py-2.5 text-[11px] text-[#f0eff8] placeholder-[#6b6a8a] outline-none transition"
                style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.07)' }}
                onFocus={(e) => { e.currentTarget.style.border = '1px solid rgba(255,107,74,0.3)' }}
                onBlur={(e) => { e.currentTarget.style.border = '1px solid rgba(255,255,255,0.07)' }}
              />
              <button
                type="submit"
                disabled={!input.trim()}
                className="flex items-center justify-center rounded-xl px-3.5 text-white transition-all hover:opacity-90 active:scale-95 disabled:cursor-not-allowed disabled:opacity-30"
                style={{ background: 'linear-gradient(135deg, #ff6b4a, #e84e2c)' }}
              >
                <Send className="h-3.5 w-3.5" />
              </button>
            </div>
            <p className="mt-1.5 text-center text-[9px]" style={{ color: '#6b6a8a' }}>
              {buzz.isLive
                ? buzz.hasPrivateKey
                  ? `Publishing to ${buzz.relayUrl} · Nostr kind 1 text notes`
                  : `Connected to ${buzz.relayUrl} · set private key to post`
                : 'Simulation mode · messages are local only'}
            </p>
          </form>
        </div>

        {/* Right: Pipeline + Agent roster */}
        <div className="flex w-72 flex-col gap-3 overflow-y-auto p-3" style={{ borderLeft: '1px solid rgba(255,255,255,0.05)' }}>
          <PipelineView items={buzz.pipeline} />
          <AgentRoster agents={buzz.agents} />

          {/* Stats footer */}
          <div className="mt-auto rounded-xl p-3" style={{ background: 'rgba(255,255,255,0.015)', border: '1px solid rgba(255,255,255,0.05)' }}>
            <div className="mb-2 flex items-center gap-1.5">
              <Activity className="h-3 w-3" style={{ color: '#ff6b4a' }} />
              <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: '#6b6a8a' }}>
                Activity
              </span>
            </div>
            <div className="grid grid-cols-2 gap-2 text-[9px]">
              <div className="rounded-lg p-2" style={{ background: 'rgba(255,255,255,0.02)' }}>
                <div style={{ color: '#6b6a8a' }}>Online</div>
                <div className="text-[14px] font-bold" style={{ color: '#00d4aa' }}>
                  {buzz.agents.filter((a) => a.status === 'online').length}
                </div>
              </div>
              <div className="rounded-lg p-2" style={{ background: 'rgba(255,255,255,0.02)' }}>
                <div style={{ color: '#6b6a8a' }}>Attested</div>
                <div className="text-[14px] font-bold" style={{ color: '#a78bfa' }}>
                  {buzz.agents.filter((a) => a.attestation === 'attested').length}
                </div>
              </div>
              <div className="rounded-lg p-2" style={{ background: 'rgba(255,255,255,0.02)' }}>
                <div style={{ color: '#6b6a8a' }}>Pipeline</div>
                <div className="text-[14px] font-bold" style={{ color: '#ff6b4a' }}>
                  {buzz.pipeline.length}
                </div>
              </div>
              <div className="rounded-lg p-2" style={{ background: 'rgba(255,255,255,0.02)' }}>
                <div style={{ color: '#6b6a8a' }}>Events</div>
                <div className="text-[14px] font-bold" style={{ color: '#c0bfd8' }}>
                  {buzz.relay.eventCount.toLocaleString()}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
