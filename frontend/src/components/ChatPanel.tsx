import { useState, useRef, useEffect } from 'react'
import { useStore } from '../store'
import { Send, Bot, User, Sparkles, Monitor, Cloud, ShieldCheck, ArrowLeft, GitBranch, Forward } from 'lucide-react'
import type { ChatMessage } from '../types'

export function ChatPanel() {
  const activeAgentId    = useStore((s) => s.activeAgentId)
  const agents           = useStore((s) => s.agents)
  const sessions         = useStore((s) => s.sessions)
  const streamingToken   = useStore((s) => s.streamingToken)
  const isStreaming      = useStore((s) => s.isStreaming)
  const lastError        = useStore((s) => s.lastError)
  const pendingToolCall  = useStore((s) => s.pendingToolCall)
  const approveToolCall  = useStore((s) => s.approveToolCall)
  const denyToolCall     = useStore((s) => s.denyToolCall)
  const sendMessage      = useStore((s) => s.sendMessage)
  const setAgentTier     = useStore((s) => s.setAgentTier)
  const setWavsVerified  = useStore((s) => s.setWavsVerified)
  const setActiveAgent   = useStore((s) => s.setActiveAgent)
  const delegateToAgent  = useStore((s) => s.delegateToAgent)
  const getAgentChain    = useStore((s) => s.getAgentChain)
  const connected        = useStore((s) => s.connected)

  const [input, setInput] = useState('')
  const [showDelegateMenu, setShowDelegateMenu] = useState(false)
  const [delegatePrompt, setDelegatePrompt] = useState('')
  const messagesEndRef = useRef<HTMLDivElement>(null)

  const agent    = agents.find((a) => a.id === activeAgentId)
  const mainAgent = agents.length > 0 ? agents[0] : null
  const isSubAgent = agent && mainAgent && agent.id !== mainAgent.id
  const messages = activeAgentId ? sessions[activeAgentId] || [] : []
  const agentChain = activeAgentId ? getAgentChain(activeAgentId) : []
  const childAgents = agents.filter(a => a.parent_id === activeAgentId)

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, streamingToken])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!input.trim() || !activeAgentId) return
    sendMessage(input.trim())
    setInput('')
  }

  if (!activeAgentId || !agent) {
    return (
      <div className="flex flex-1 items-center justify-center bg-[#06060f]">
        <div className="flex flex-col items-center gap-4 text-center animate-fade-in">
          <div className="flex h-16 w-16 items-center justify-center rounded-2xl"
               style={{ background: 'rgba(255,107,74,0.06)', border: '1px solid rgba(255,107,74,0.12)' }}>
            <Sparkles className="h-7 w-7 text-juno-500" />
          </div>
          <div>
            <p className="text-sm font-medium text-[#c0bfd8]">Select an agent to start</p>
            <p className="mt-1 text-xs text-[#6b6a8a]">Or create one from the sidebar</p>
          </div>
        </div>
      </div>
    )
  }

  const tiers: { id: import('../types').ExecutionTier; label: string; desc: string; icon: React.ReactNode; color: string; bg: string }[] = [
    { id: 'local', label: 'Local',  desc: 'Ollama on your device',         icon: <Monitor className="h-3 w-3" />, color: '#00d4aa', bg: 'rgba(0,212,170,0.12)' },
    { id: 'akash', label: 'Akash',  desc: 'Borrow GPU from Akash Network', icon: <Cloud   className="h-3 w-3" />, color: '#7c3aed', bg: 'rgba(124,58,237,0.12)' },
  ]

  const wavsEnabled = agent.wavs_verified ?? false

  const tierFooterText: Record<string, string> = {
    local: 'Local · Ollama · No data leaves your machine',
    akash: 'Akash Network · Decentralized GPU · Data encrypted in transit',
  }
  const footerWavs = wavsEnabled ? ' · WAVS verified (TEE attested)' : ''

  return (
    <div className="flex flex-1 flex-col bg-[#06060f]">
      {/* Agent header */}
      <div className="px-6 py-3"
           style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>

        {/* Hierarchy breadcrumb — shows full chain */}
        {agentChain.length > 1 && (
          <div className="mb-2 flex items-center gap-1 flex-wrap">
            {agentChain.map((node, i) => {
              const isLast = i === agentChain.length - 1
              return (
                <span key={node.id} className="flex items-center gap-1">
                  {i === 0 && <ArrowLeft className="h-3 w-3 text-[#6b6a8a]" />}
                  {isLast ? (
                    <span className="flex items-center gap-1 text-[10px] font-semibold text-juno-400">
                      <GitBranch className="h-2.5 w-2.5" />
                      {node.name}
                    </span>
                  ) : (
                    <>
                      <button
                        onClick={() => setActiveAgent(node.id)}
                        className="rounded-md px-1.5 py-0.5 text-[10px] font-medium transition hover:bg-[#16162b] text-[#6b6a8a]"
                      >
                        {node.name}
                      </button>
                      <span className="text-[10px] text-[#3a3a5a]">/</span>
                    </>
                  )}
                </span>
              )
            })}
          </div>
        )}

        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg"
                 style={isSubAgent
                   ? { background: 'rgba(124,58,237,0.1)', border: '1px solid rgba(124,58,237,0.2)' }
                   : { background: 'rgba(255,107,74,0.1)', border: '1px solid rgba(255,107,74,0.2)' }
                 }>
              {isSubAgent
                ? <GitBranch className="h-4 w-4 text-purple-400" />
                : <Bot className="h-4 w-4 text-juno-400" />
              }
            </div>
            <div>
              <div className="text-sm font-semibold text-[#f0eff8]">{agent.name}</div>
              <div className="text-[10px] text-[#6b6a8a]">
                {agent.description || agent.model}
                {agent.description && <span className="ml-1.5 text-[#4a4a6a]">· {agent.model}</span>}
              </div>
            </div>
          </div>

          {/* Compute tier + WAVS verification */}
          <div className="flex items-center gap-2">
            <div className="flex items-center gap-1 rounded-lg p-0.5"
                 style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
              {tiers.map((tier) => {
                const isSelected = agent.default_tier === tier.id
                return (
                  <button
                    key={tier.id}
                    onClick={() => setAgentTier(agent.id, tier.id)}
                    title={tier.desc}
                    className="flex items-center gap-1 rounded-md px-2.5 py-1.5 text-[10px] font-semibold uppercase tracking-wider transition-all"
                    style={isSelected ? {
                      color: tier.color,
                      background: tier.bg,
                      boxShadow: `0 0 8px ${tier.bg}`,
                    } : {
                      color: '#6b6a8a',
                    }}
                  >
                    <span style={{ color: isSelected ? tier.color : '#6b6a8a' }}>{tier.icon}</span>
                    {tier.label}
                  </button>
                )
              })}
            </div>

            {/* WAVS verification toggle */}
            <button
              onClick={() => setWavsVerified(agent.id, !wavsEnabled)}
              title={wavsEnabled ? 'WAVS verification ON — results are TEE attested' : 'WAVS verification OFF — click to enable'}
              className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-semibold uppercase tracking-wider transition-all"
              style={wavsEnabled ? {
                color: '#ff6b4a',
                background: 'rgba(255,107,74,0.12)',
                border: '1px solid rgba(255,107,74,0.25)',
                boxShadow: '0 0 8px rgba(255,107,74,0.12)',
              } : {
                color: '#6b6a8a',
                background: 'transparent',
                border: '1px solid rgba(255,255,255,0.06)',
              }}
            >
              <ShieldCheck className="h-3 w-3" />
              WAVS
            </button>
          </div>
        </div>
      </div>

      {/* Connection warning */}
      {!connected && (
        <div className="flex items-center gap-2 px-6 py-2 text-[11px]"
             style={{ background: 'rgba(251,191,36,0.06)', borderBottom: '1px solid rgba(251,191,36,0.15)', color: '#fbbf24' }}>
          <span className="h-1.5 w-1.5 rounded-full bg-yellow-500 animate-pulse" />
          Daemon offline — messages will be sent when connected
        </div>
      )}

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-4 py-5 md:px-8">
        {messages.length === 0 && (
          <div className="flex h-full items-center justify-center">
            <div className="text-center animate-fade-in">
              <p className="text-sm text-[#6b6a8a]">
                Start a conversation with <span className="text-juno-400 font-medium">{agent.name}</span>
              </p>
            </div>
          </div>
        )}

        {messages.map((msg: ChatMessage) => (
          <MessageBubble key={msg.id} message={msg} />
        ))}

        {/* Thinking indicator — shown before first token arrives */}
        {isStreaming && !streamingToken && (
          <div className="mb-5 flex gap-3 animate-fade-in">
            <div className="flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-lg"
                 style={{ background: 'rgba(255,107,74,0.1)', border: '1px solid rgba(255,107,74,0.15)' }}>
              <Bot className="h-3.5 w-3.5 text-juno-400" />
            </div>
            <div className="flex items-center gap-2 rounded-xl px-4 py-3"
                 style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
              <span className="text-xs text-[#6b6a8a]">Thinking</span>
              <span className="flex gap-1">
                {[0,1,2].map(i => (
                  <span key={i} className="h-1 w-1 rounded-full bg-juno-500 animate-bounce"
                        style={{ animationDelay: `${i * 0.15}s` }} />
                ))}
              </span>
            </div>
          </div>
        )}

        {/* Streaming tokens */}
        {streamingToken && (
          <div className="mb-5 flex gap-3 animate-fade-in">
            <div className="flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-lg"
                 style={{ background: 'rgba(255,107,74,0.1)', border: '1px solid rgba(255,107,74,0.15)' }}>
              <Bot className="h-3.5 w-3.5 text-juno-400" />
            </div>
            <div className="max-w-2xl rounded-xl px-4 py-3 text-sm leading-relaxed text-[#c0bfd8]"
                 style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.06)' }}>
              {streamingToken}
              <span className="ml-0.5 inline-block h-3.5 w-[2px] translate-y-[1px] bg-juno-400 cursor-blink" />
            </div>
          </div>
        )}

        {/* Tool Call Approval Card */}
        {pendingToolCall && (
          <div className="mb-5 animate-fade-in rounded-xl overflow-hidden"
               style={{ border: '1px solid rgba(255,107,74,0.35)', background: 'rgba(255,107,74,0.05)' }}>
            <div className="flex items-center gap-2 px-4 py-2.5"
                 style={{ borderBottom: '1px solid rgba(255,107,74,0.15)', background: 'rgba(255,107,74,0.08)' }}>
              <span className="text-xs font-semibold tracking-wide text-juno-400 uppercase">⚡ Tool Request</span>
              <span className="ml-auto rounded px-2 py-0.5 text-[10px] font-mono text-juno-300"
                    style={{ background: 'rgba(255,107,74,0.15)' }}>{pendingToolCall.name}</span>
            </div>
            <div className="px-4 py-3">
              <pre className="mb-3 overflow-x-auto rounded-lg p-3 text-xs text-[#c0bfd8] font-mono"
                   style={{ background: '#05050f', border: '1px solid rgba(255,255,255,0.06)' }}>
                {JSON.stringify(pendingToolCall.args, null, 2)}
              </pre>
              <div className="flex gap-2">
                <button onClick={approveToolCall}
                        className="flex-1 rounded-lg py-2 text-xs font-semibold text-white transition hover:opacity-90"
                        style={{ background: 'linear-gradient(135deg,#22c55e,#16a34a)' }}>
                  ✓ Approve &amp; Run
                </button>
                <button onClick={denyToolCall}
                        className="flex-1 rounded-lg py-2 text-xs font-semibold transition hover:opacity-90"
                        style={{ background: 'rgba(239,68,68,0.1)', border: '1px solid rgba(239,68,68,0.25)', color: '#f87171' }}>
                  ✕ Deny
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Error banner */}
        {lastError && (
          <div className="mb-4 rounded-xl px-4 py-3 text-xs animate-fade-in"
               style={{ background: 'rgba(239,68,68,0.08)', border: '1px solid rgba(239,68,68,0.2)', color: '#f87171' }}>
            ⚠ {lastError}
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Delegate to sub-agent */}
      {childAgents.length > 0 && (
        <div className="px-4 md:px-8" style={{ borderTop: '1px solid rgba(255,255,255,0.03)' }}>
          <button
            onClick={() => setShowDelegateMenu(!showDelegateMenu)}
            className="flex items-center gap-1.5 py-2 text-[10px] font-semibold uppercase tracking-wider transition hover:text-juno-400"
            style={{ color: showDelegateMenu ? '#ff6b4a' : '#6b6a8a' }}
          >
            <Forward className="h-3 w-3" />
            Delegate to sub-agent ({childAgents.length})
          </button>

          {showDelegateMenu && (
            <div className="mb-3 rounded-xl p-3 animate-slide-up"
                 style={{ background: '#0a0a18', border: '1px solid rgba(255,107,74,0.15)' }}>
              <input
                type="text"
                value={delegatePrompt}
                onChange={(e) => setDelegatePrompt(e.target.value)}
                placeholder="Prompt to delegate..."
                className="mb-2 w-full rounded-lg px-3 py-2 text-xs text-white placeholder-[#6b6a8a] outline-none"
                style={{ background: '#16162b', border: '1px solid rgba(255,255,255,0.06)' }}
                autoFocus
              />
              <div className="flex flex-wrap gap-1.5">
                {childAgents.map((child) => (
                  <button
                    key={child.id}
                    onClick={() => {
                      if (!delegatePrompt.trim()) return
                      delegateToAgent(agent.id, child.id, delegatePrompt.trim())
                      setDelegatePrompt('')
                      setShowDelegateMenu(false)
                    }}
                    className="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-[10px] font-medium transition hover:border-juno-500/40"
                    style={{ background: 'rgba(124,58,237,0.08)', border: '1px solid rgba(124,58,237,0.2)', color: '#c0bfd8' }}
                  >
                    <GitBranch className="h-2.5 w-2.5 text-purple-400" />
                    {child.name}
                    <span className="text-[8px] text-[#6b6a8a]">{child.model.split(':')[0]}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Input */}
      <form onSubmit={handleSubmit} className="px-4 py-4 md:px-8"
            style={{ borderTop: '1px solid rgba(255,255,255,0.05)' }}>
        <div className="flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder={isStreaming ? `${agent.name} is thinking…` : `Message ${agent.name}…`}
            disabled={isStreaming}
            className="flex-1 rounded-xl px-4 py-3 text-sm text-[#f0eff8] placeholder-[#6b6a8a] outline-none transition disabled:opacity-50 disabled:cursor-not-allowed"
            style={{
              background: '#0a0a18',
              border: '1px solid rgba(255,255,255,0.07)',
            }}
            onFocus={(e) => { if (!isStreaming) e.currentTarget.style.border = '1px solid rgba(255,107,74,0.3)' }}
            onBlur={(e)  => { e.currentTarget.style.border = '1px solid rgba(255,255,255,0.07)' }}
            autoFocus
          />
          <button
            type="submit"
            disabled={!input.trim() || isStreaming}
            className="flex items-center justify-center rounded-xl px-4 text-white transition-all hover:opacity-90 active:scale-95 disabled:cursor-not-allowed disabled:opacity-30"
            style={{ background: 'linear-gradient(135deg, #ff6b4a, #e84e2c)', minWidth: '48px' }}
          >
            <Send className="h-4 w-4" />
          </button>
        </div>
        <p className="mt-2 text-center text-[10px] text-[#6b6a8a]">
          {(tierFooterText[agent.default_tier] ?? tierFooterText.local) + footerWavs}
        </p>
      </form>
    </div>
  )
}

function MessageBubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === 'user'

  return (
    <div className={`mb-5 flex gap-3 animate-slide-up ${isUser ? 'flex-row-reverse' : ''}`}>
      <div
        className="flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-lg"
        style={isUser
          ? { background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.08)' }
          : { background: 'rgba(255,107,74,0.1)',   border: '1px solid rgba(255,107,74,0.15)' }
        }
      >
        {isUser
          ? <User className="h-3.5 w-3.5 text-[#6b6a8a]" />
          : <Bot  className="h-3.5 w-3.5 text-juno-400" />
        }
      </div>
      <div
        className="max-w-2xl rounded-xl px-4 py-3 text-sm leading-relaxed"
        style={isUser
          ? { background: 'rgba(255,107,74,0.07)', border: '1px solid rgba(255,107,74,0.12)', color: '#f0eff8' }
          : { background: '#0a0a18',               border: '1px solid rgba(255,255,255,0.06)', color: '#c0bfd8' }
        }
      >
        {message.content}
        <div className="mt-1 text-[10px] text-[#6b6a8a]">
          {new Date(message.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
        </div>
      </div>
    </div>
  )
}
