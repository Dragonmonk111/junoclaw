import { useState } from 'react'
import { useStore } from '../store'
import { Bot, Plus, Settings, ChevronRight, X, Shield, Wallet, GitBranch, MessageCircle, Brain } from 'lucide-react'
import type { Personality } from '../types'
import { CrabLogo } from './CrabLogo'

const SUB_AGENT_MODELS = [
  { value: 'llama3.2:3b',     label: 'Llama 3.2 3B',     tag: 'local' },
  { value: 'qwen2.5:1.5b',    label: 'Qwen 2.5 1.5B',    tag: 'local' },
  { value: 'llama3.1:8b',     label: 'Llama 3.1 8B',     tag: 'local' },
  { value: 'deepseek-r1:14b', label: 'DeepSeek R1 14B',  tag: 'local' },
  { value: 'claude-sonnet',   label: 'Claude Sonnet',     tag: 'cloud' },
  { value: 'gpt-4o',          label: 'GPT-4o',            tag: 'cloud' },
]

const PERSONALITIES: { id: Personality; label: string; emoji: string; desc: string }[] = [
  { id: 'professional',   label: 'Pro',       emoji: '💼', desc: 'Formal, concise, focused on deliverables' },
  { id: 'creative',       label: 'Creative',  emoji: '🎨', desc: 'Exploratory, generates ideas, uses metaphors' },
  { id: 'analytical',     label: 'Analyst',   emoji: '📊', desc: 'Data-driven, cautious, asks clarifying questions' },
  { id: 'conversational', label: 'Friendly',  emoji: '💬', desc: 'Casual, explains simply, good for non-technical users' },
  { id: 'custom',         label: 'Custom',    emoji: '⚙️', desc: 'Write your own system prompt' },
]

const MAIN_AGENT_WALLET = 'juno1tvpe...6hz5f4m'

export function Sidebar() {
  const agents        = useStore((s) => s.agents)
  const activeAgentId = useStore((s) => s.activeAgentId)
  const setActiveAgent = useStore((s) => s.setActiveAgent)
  const createAgent   = useStore((s) => s.createAgent)
  const sessions      = useStore((s) => s.sessions)

  const [showCreate, setShowCreate] = useState(false)
  const [name,  setName]  = useState('')
  const [desc,  setDesc]  = useState('')
  const [model, setModel] = useState('llama3.1:8b')
  const [personality, setPersonality] = useState<Personality>('professional')

  const handleCreate = () => {
    if (!name.trim()) return
    const parentId = agents.length > 0 ? agents[0].id : undefined
    createAgent(name, desc, model, parentId, personality)
    setName('')
    setDesc('')
    setPersonality('professional')
    setShowCreate(false)
  }

  const mainAgent = agents.length > 0 ? agents[0] : null
  const subAgents = agents.slice(1)

  return (
    <aside className="flex w-64 flex-col bg-[#0a0a18]"
           style={{ borderRight: '1px solid rgba(255,107,74,0.1)' }}>

      {/* Crab logo header */}
      <div className="flex flex-col items-center gap-2 px-4 pt-4 pb-3"
           style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
        <div className="relative">
          <div className="rounded-xl p-1"
               style={{ boxShadow: '0 0 24px rgba(255,107,74,0.25)', background: 'rgba(255,107,74,0.04)' }}>
            <CrabLogo size={80} />
          </div>
          <span className="absolute -bottom-0.5 -right-0.5 h-3 w-3 rounded-full bg-teal-500 ring-2 ring-[#0a0a18]" />
        </div>
        <div className="text-center">
          <div className="text-gradient-juno text-base font-bold tracking-wide">JunoClaw</div>
          <div className="text-[10px] text-[#6b6a8a] uppercase tracking-widest mt-0.5">Agentic AI · Juno Network</div>
        </div>
      </div>

      {/* Main Agent — always one, bound to wallet */}
      <div className="px-2 pt-3 pb-1">
        <div className="mb-1.5 flex items-center gap-1.5 px-2">
          <Shield className="h-3 w-3 text-juno-400" />
          <span className="text-[10px] font-semibold uppercase tracking-widest text-[#6b6a8a]">
            Main Agent
          </span>
        </div>

        {!mainAgent ? (
          <div className="rounded-xl p-3 text-center"
               style={{ background: '#0f0f20', border: '1px solid rgba(255,107,74,0.12)' }}>
            <Bot className="mx-auto mb-2 h-7 w-7 text-[#2a2a4a]" />
            <p className="mb-2 text-[11px] text-[#6b6a8a]">Bound to your wallet</p>
            <div className="mb-2 flex items-center justify-center gap-1 rounded-lg px-2 py-1"
                 style={{ background: 'rgba(0,212,170,0.06)', border: '1px solid rgba(0,212,170,0.15)' }}>
              <Wallet className="h-3 w-3 text-teal-400" />
              <span className="text-[9px] font-mono text-teal-400">{MAIN_AGENT_WALLET}</span>
            </div>
            <button
              onClick={() => {
                createAgent('JunoClaw Agent', 'Main agent bound to wallet', 'llama3.1:8b')
              }}
              className="w-full rounded-lg py-2 text-xs font-semibold text-white transition hover:opacity-90 active:scale-95"
              style={{ background: 'linear-gradient(135deg, #ff6b4a, #e84e2c)' }}
            >
              Initialize Main Agent
            </button>
          </div>
        ) : (
          <button
            onClick={() => setActiveAgent(mainAgent.id)}
            className="flex w-full items-center gap-2.5 rounded-xl px-3 py-2.5 text-left text-sm transition-all"
            style={activeAgentId === mainAgent.id ? {
              background: 'rgba(255,107,74,0.1)',
              border: '1px solid rgba(255,107,74,0.25)',
              boxShadow: '0 0 16px rgba(255,107,74,0.1)',
            } : {
              background: 'rgba(255,107,74,0.03)',
              border: '1px solid rgba(255,107,74,0.08)',
            }}
          >
            <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-lg bg-juno-500/20">
              <CrabLogo size={22} />
            </div>
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-semibold text-juno-300">{mainAgent.name}</div>
              <div className="flex items-center gap-1 mt-0.5">
                <Wallet className="h-2.5 w-2.5 text-teal-500" />
                <span className="text-[9px] font-mono text-teal-500/80">{MAIN_AGENT_WALLET}</span>
              </div>
            </div>
            {activeAgentId === mainAgent.id && <ChevronRight className="h-3.5 w-3.5 flex-shrink-0 text-juno-500" />}
          </button>
        )}
      </div>

      {/* Sub-Agents section */}
      <div className="flex-1 overflow-y-auto px-2 py-2">
        <div className="mb-1.5 flex items-center justify-between px-2">
          <div className="flex items-center gap-1.5">
            <GitBranch className="h-3 w-3 text-[#6b6a8a]" />
            <span className="text-[10px] font-semibold uppercase tracking-widest text-[#6b6a8a]">
              Sub-Agents
            </span>
          </div>
          {mainAgent && (
            <button
              onClick={() => setShowCreate(!showCreate)}
              className="rounded-md p-1 text-[#6b6a8a] transition hover:bg-[#16162b] hover:text-juno-400"
              title="New sub-agent"
            >
              {showCreate ? <X className="h-3.5 w-3.5" /> : <Plus className="h-3.5 w-3.5" />}
            </button>
          )}
        </div>

        {showCreate && (
          <div className="mb-3 animate-slide-up rounded-xl p-3"
               style={{ background: '#0f0f20', border: '1px solid rgba(255,107,74,0.15)' }}>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Sub-agent name"
              className="mb-2 w-full rounded-lg px-3 py-2 text-sm text-white placeholder-[#6b6a8a] outline-none transition"
              style={{ background: '#16162b', border: '1px solid rgba(255,255,255,0.06)' }}
              onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
              autoFocus
            />
            <input
              type="text"
              value={desc}
              onChange={(e) => setDesc(e.target.value)}
              placeholder="Role / description"
              className="mb-2 w-full rounded-lg px-3 py-2 text-sm text-white placeholder-[#6b6a8a] outline-none transition"
              style={{ background: '#16162b', border: '1px solid rgba(255,255,255,0.06)' }}
            />
            <select
              value={model}
              onChange={(e) => setModel(e.target.value)}
              className="mb-2 w-full rounded-lg px-3 py-2 text-sm text-white outline-none"
              style={{ background: '#16162b', border: '1px solid rgba(255,255,255,0.06)' }}
            >
              {SUB_AGENT_MODELS.map((m) => (
                <option key={m.value} value={m.value}>
                  {m.label} ({m.tag})
                </option>
              ))}
            </select>

            {/* Personality selector */}
            <div className="mb-3">
              <div className="mb-1.5 flex items-center gap-1 px-0.5">
                <Brain className="h-2.5 w-2.5 text-[#6b6a8a]" />
                <span className="text-[9px] font-semibold uppercase tracking-widest text-[#6b6a8a]">Personality</span>
              </div>
              <div className="flex flex-wrap gap-1">
                {PERSONALITIES.map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    onClick={() => setPersonality(p.id)}
                    className="rounded-md px-2 py-1 text-[10px] font-medium transition-all"
                    style={personality === p.id ? {
                      color: '#ff6b4a',
                      background: 'rgba(255,107,74,0.12)',
                      border: '1px solid rgba(255,107,74,0.25)',
                    } : {
                      color: '#6b6a8a',
                      background: 'rgba(255,255,255,0.02)',
                      border: '1px solid rgba(255,255,255,0.06)',
                    }}
                    title={p.desc}
                  >
                    {p.emoji} {p.label}
                  </button>
                ))}
              </div>
            </div>
            <button
              onClick={handleCreate}
              className="w-full rounded-lg py-2 text-sm font-semibold text-white transition hover:opacity-90 active:scale-95"
              style={{ background: 'linear-gradient(135deg, #ff6b4a, #e84e2c)' }}
            >
              Create Sub-Agent
            </button>
          </div>
        )}

        {mainAgent && subAgents.length === 0 && !showCreate && (
          <div className="mt-3 flex flex-col items-center gap-1.5 px-3 text-center">
            <GitBranch className="h-6 w-6 text-[#2a2a4a]" />
            <p className="text-[11px] text-[#6b6a8a]">No sub-agents yet</p>
            <p className="text-[10px] text-[#4a4a6a]">Sub-agents report to the main agent</p>
          </div>
        )}

        {!mainAgent && (
          <div className="mt-3 px-3 text-center">
            <p className="text-[10px] text-[#4a4a6a]">Initialize main agent first</p>
          </div>
        )}

        {subAgents.map((agent) => {
          const isActive = activeAgentId === agent.id
          const msgCount = (sessions[agent.id] || []).length
          return (
            <button
              key={agent.id}
              onClick={() => setActiveAgent(agent.id)}
              className="mb-1.5 flex w-full items-center gap-2.5 rounded-xl px-3 py-2.5 text-left text-sm transition-all group"
              style={isActive ? {
                background: 'rgba(255,107,74,0.08)',
                border: '1px solid rgba(255,107,74,0.2)',
                boxShadow: '0 0 12px rgba(255,107,74,0.08)',
              } : {
                background: 'rgba(255,255,255,0.01)',
                border: '1px solid rgba(255,255,255,0.04)',
              }}
            >
              <div className="flex items-center gap-1.5">
                <div className="h-4 w-px bg-[#2a2a4a]" />
                <div className={`relative flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-lg ${
                  isActive ? 'bg-juno-500/20' : 'bg-[#16162b] group-hover:bg-[#1e1e38]'
                }`}>
                  <Bot className={`h-3.5 w-3.5 ${isActive ? 'text-juno-400' : 'text-[#6b6a8a] group-hover:text-juno-400'}`} />
                  {msgCount > 0 && (
                    <span className="absolute -top-1 -right-1 flex h-3.5 min-w-[14px] items-center justify-center rounded-full px-0.5 text-[8px] font-bold text-white"
                          style={{ background: '#ff6b4a' }}>
                      {msgCount}
                    </span>
                  )}
                </div>
              </div>
              <div className="min-w-0 flex-1">
                <div className={`truncate text-[13px] font-medium ${isActive ? 'text-juno-300' : 'text-[#c0bfd8] group-hover:text-[#f0eff8]'}`}>
                  {agent.name}
                </div>
                {agent.description ? (
                  <div className="truncate text-[10px] text-[#6b6a8a]">{agent.description}</div>
                ) : (
                  <div className="truncate text-[10px] text-[#6b6a8a]">{agent.model}</div>
                )}
              </div>
              <div className="flex flex-col items-end gap-0.5 flex-shrink-0">
                {isActive ? (
                  <ChevronRight className="h-3 w-3 text-juno-500" />
                ) : (
                  <MessageCircle className="h-3 w-3 text-[#3a3a5a] group-hover:text-juno-400 transition" />
                )}
                <span className="rounded px-1 py-0.5 text-[8px] font-mono"
                      style={{ background: 'rgba(255,255,255,0.03)', color: '#6b6a8a' }}>
                  {agent.model.split(':')[0]}
                </span>
              </div>
            </button>
          )
        })}
      </div>

      {/* Footer */}
      <div className="px-2 pb-3" style={{ borderTop: '1px solid rgba(255,255,255,0.05)' }}>
        <button className="mt-2 flex w-full items-center gap-2 rounded-xl px-3 py-2 text-sm text-[#6b6a8a] transition hover:bg-[#16162b] hover:text-[#c0bfd8]">
          <Settings className="h-4 w-4" />
          Settings
        </button>
      </div>
    </aside>
  )
}
