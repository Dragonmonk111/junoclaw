import { useState } from 'react'
import { useStore } from '../store'
import { Bot, Building2, Shield, Search } from 'lucide-react'
import type { AgentInfo, DaoInstance } from '../types'

type Tab = 'agents' | 'daos'

export function DiscoverScreen({ onSelectAgent, onSelectDao, onNavigateHome }: {
  onSelectAgent: (id: string) => void
  onSelectDao: (id: string) => void
  onNavigateHome: () => void
}) {
  const agents = useStore((s) => s.agents)
  const daos = useStore((s) => s.daos)
  const createAgent = useStore((s) => s.createAgent)
  const [tab, setTab] = useState<Tab>('agents')
  const [query, setQuery] = useState('')

  const handleCreateAgent = () => {
    createAgent('New Agent', 'A new JunoClaw agent', 'llama3.1:8b')
    // createAgent sets activeAgentId internally, navigate after a tick
    setTimeout(() => {
      const state = useStore.getState()
      const newAgent = state.agents.find((a) => a.name === 'New Agent' && a.description === 'A new JunoClaw agent')
      if (newAgent) onSelectAgent(newAgent.id)
    }, 100)
  }

  const filteredAgents = agents.filter((a) =>
    a.name.toLowerCase().includes(query.toLowerCase()) ||
    a.description.toLowerCase().includes(query.toLowerCase())
  )
  const filteredDaos = daos.filter((d) =>
    d.name.toLowerCase().includes(query.toLowerCase())
  )

  return (
    <div className="flex flex-col overflow-y-auto" style={{ height: '100%' }}>
      {/* Header */}
      <div className="px-8 pt-6 pb-4">
        <h1 className="text-xl font-bold text-white mb-4">Discover</h1>

        {/* Tab switcher */}
        <div className="flex items-center gap-1 mb-4">
          {(['agents', 'daos'] as Tab[]).map((t) => (
            <button key={t} onClick={() => setTab(t)}
                    className="rounded-lg px-4 py-2 text-sm font-medium capitalize transition"
                    style={tab === t ? {
                      background: 'rgba(255,107,74,0.08)',
                      color: '#ff6b4a',
                      border: '1px solid rgba(255,107,74,0.15)',
                    } : {
                      color: '#6b6a8a',
                      border: '1px solid transparent',
                    }}>
              {t}
              {t === 'agents' && agents.length > 0 && (
                <span className="ml-1.5 text-xs opacity-60">{agents.length}</span>
              )}
              {t === 'daos' && daos.length > 0 && (
                <span className="ml-1.5 text-xs opacity-60">{daos.length}</span>
              )}
            </button>
          ))}
        </div>

        {/* Search bar */}
        <div className="relative mb-4">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-[#3a3a5a]" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={`Search ${tab}...`}
              className="w-full rounded-xl py-2.5 pl-10 pr-4 text-sm text-white placeholder-[#3a3a5a]"
              style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}
            />
          </div>
      </div>

      {/* Content */}
      <div className="px-8 pb-8">
        {tab === 'agents' && (
          <div>
            {filteredAgents.length === 0 ? (
              <EmptyState
                icon={<Bot className="h-12 w-12 text-[#2a2a4a]" />}
                title="No agents found"
                subtitle="Create your first AI agent to get started"
                actionLabel="Create Agent"
                onAction={handleCreateAgent}
              />
            ) : (
              <div className="grid grid-cols-3 gap-4">
                {filteredAgents.map((agent) => (
                  <AgentCard key={agent.id} agent={agent} onClick={() => onSelectAgent(agent.id)} />
                ))}
              </div>
            )}
          </div>
        )}

        {tab === 'daos' && (
          <div>
            {filteredDaos.length === 0 ? (
              <EmptyState
                icon={<Building2 className="h-12 w-12 text-[#2a2a4a]" />}
                title="No DAOs found"
                subtitle="Deploy a DAO from the DAO Governance portal on Home"
                actionLabel="Back to Home"
                onAction={onNavigateHome}
              />
            ) : (
              <div className="grid grid-cols-3 gap-4">
                {filteredDaos.map((dao) => (
                  <DaoCard key={dao.id} dao={dao} onClick={() => onSelectDao(dao.id)} />
                ))}
              </div>
            )}
          </div>
        )}

      </div>
    </div>
  )
}

function AgentCard({ agent, onClick }: { agent: AgentInfo; onClick: () => void }) {
  return (
    <button onClick={onClick}
            className="flex flex-col gap-3 rounded-2xl p-5 text-left transition hover:bg-white/5"
            style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl"
             style={{ background: 'rgba(255,107,74,0.08)' }}>
          <Bot className="h-5 w-5 text-[#ff6b4a]" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm font-semibold text-white truncate">{agent.name}</span>
            {agent.wavs_verified && <Shield className="h-3.5 w-3.5 text-blue-400 flex-shrink-0" />}
          </div>
          <p className="text-xs text-[#6b6a8a] truncate">{agent.model}</p>
        </div>
      </div>
      <p className="text-xs text-[#6b6a8a] line-clamp-2">{agent.description || 'No description'}</p>
      <div className="flex items-center gap-2">
        <span className="rounded-md px-2 py-0.5 text-[10px] font-medium"
              style={{ background: 'rgba(255,107,74,0.08)', color: '#ff6b4a' }}>
          {agent.default_tier}
        </span>
        <span className="rounded-md px-2 py-0.5 text-[10px] font-medium"
              style={{ background: 'rgba(107,106,138,0.08)', color: '#6b6a8a' }}>
          {agent.personality}
        </span>
      </div>
    </button>
  )
}

function DaoCard({ dao, onClick }: { dao: DaoInstance; onClick: () => void }) {
  const color = dao.template_color || '#00d4aa'
  return (
    <button onClick={onClick}
            className="flex flex-col gap-3 rounded-2xl p-5 text-left transition hover:bg-white/5"
            style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.05)' }}>
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl"
             style={{ background: `${color}14` }}>
          <Building2 className="h-5 w-5" style={{ color }} />
        </div>
        <div className="flex-1 min-w-0">
          <span className="text-sm font-semibold text-white truncate block">{dao.name}</span>
          <p className="text-xs text-[#6b6a8a]">{dao.template_id}</p>
        </div>
      </div>
      <div className="flex items-center gap-3 text-xs text-[#6b6a8a]">
        <span>{dao.config.members.length} members</span>
        <span>·</span>
        <span>{dao.proposals.length} proposals</span>
        <span>·</span>
        <span style={{ color: dao.status === 'active' ? '#00d4aa' : '#fbbf24' }}>{dao.status}</span>
      </div>
    </button>
  )
}

function EmptyState({ icon, title, subtitle, actionLabel, onAction }: {
  icon: React.ReactNode; title: string; subtitle: string; actionLabel: string; onAction: () => void
}) {
  return (
    <div className="flex flex-col items-center justify-center py-20">
      {icon}
      <h3 className="mt-4 text-sm font-medium text-white">{title}</h3>
      <p className="mt-1 text-xs text-[#6b6a8a]">{subtitle}</p>
      <button onClick={onAction}
              className="mt-4 rounded-lg px-4 py-2 text-sm font-medium text-white"
              style={{ background: 'linear-gradient(135deg, #ff6b4a, #e84e2c)' }}>
        {actionLabel}
      </button>
    </div>
  )
}
