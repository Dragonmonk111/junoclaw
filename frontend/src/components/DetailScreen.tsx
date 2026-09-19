import { useStore } from '../store'
import { Bot, Building2, Shield, ArrowLeft, MessageSquare, Vote, Cpu, Fuel, Pickaxe, Radio, Eye, FileCode2, HeartPulse, RefreshCw, ArrowLeftRight } from 'lucide-react'
import { ChatPanel } from './ChatPanel'
import { DaoPanel } from './DaoPanel'
import { DexPanel } from './DexPanel'
import { IntelPanel } from './IntelPanel'
import { UpdatesPanel } from './UpdatesPanel'
import { ContractsPanel } from './ContractsPanel'
import { CommonwealthPanel } from './CommonwealthPanel'
import { RobotOpsPanel } from './RobotOpsPanel'
import { FeePayPanel } from './FeePayPanel'
import { MinerPanel } from './MinerPanel'
import { BuzzPanel } from './BuzzPanel'

type DetailKind = 'agent' | 'dao' | 'tool'

export function DetailScreen({ kind, id, onBack }: {
  kind: DetailKind
  id: string
  onBack: () => void
}) {
  if (kind === 'agent') return <AgentDetail agentId={id} onBack={onBack} />
  if (kind === 'dao') return <DaoDetail daoId={id} onBack={onBack} />
  return <ToolDetail toolId={id} onBack={onBack} />
}

function AgentDetail({ agentId, onBack }: { agentId: string; onBack: () => void }) {
  const agents = useStore((s) => s.agents)
  const agent = agents.find((a) => a.id === agentId)
  const setActiveAgent = useStore((s) => s.setActiveAgent)

  if (!agent) {
    return (
      <div className="p-8 text-center text-[#6b6a8a]">Agent not found</div>
    )
  }

  // Set active agent so ChatPanel uses the right one
  if (useStore.getState().activeAgentId !== agentId) {
    setActiveAgent(agentId)
  }

  return (
    <div className="flex flex-col overflow-hidden" style={{ height: '100%' }}>
      {/* Header */}
      <div className="flex items-center gap-3 px-6 py-3"
           style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
        <button onClick={onBack} className="rounded-lg p-1.5 transition hover:bg-white/5">
          <ArrowLeft className="h-4 w-4 text-[#6b6a8a]" />
        </button>
        <div className="flex h-8 w-8 items-center justify-center rounded-lg"
             style={{ background: 'rgba(255,107,74,0.08)' }}>
          <Bot className="h-4 w-4 text-[#ff6b4a]" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h1 className="text-sm font-semibold text-white truncate">{agent.name}</h1>
            {agent.wavs_verified && <Shield className="h-3.5 w-3.5 text-blue-400" />}
          </div>
          <p className="text-xs text-[#6b6a8a]">{agent.model} · {agent.personality}</p>
        </div>
        <div className="flex items-center gap-2">
          <span className="rounded-md px-2 py-0.5 text-[10px] font-medium"
                style={{ background: 'rgba(255,107,74,0.08)', color: '#ff6b4a' }}>
            {agent.default_tier}
          </span>
        </div>
      </div>

      {/* Chat panel fills the rest */}
      <div className="flex-1 overflow-hidden">
        <ChatPanel />
      </div>
    </div>
  )
}

function DaoDetail({ daoId, onBack }: { daoId: string; onBack: () => void }) {
  const daos = useStore((s) => s.daos)
  const dao = daos.find((d) => d.id === daoId)
  const setActiveDao = useStore((s) => s.setActiveDao)

  if (!dao) {
    return <div className="p-8 text-center text-[#6b6a8a]">DAO not found</div>
  }

  // Set active DAO so DaoPanel uses the right one
  if (useStore.getState().activeDaoId !== daoId) {
    setActiveDao(daoId)
  }

  const color = dao.template_color || '#00d4aa'

  return (
    <div className="flex flex-col overflow-hidden" style={{ height: '100%' }}>
      {/* Header */}
      <div className="flex items-center gap-3 px-6 py-3"
           style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
        <button onClick={onBack} className="rounded-lg p-1.5 transition hover:bg-white/5">
          <ArrowLeft className="h-4 w-4 text-[#6b6a8a]" />
        </button>
        <div className="flex h-8 w-8 items-center justify-center rounded-lg"
             style={{ background: `${color}14` }}>
          <Building2 className="h-4 w-4" style={{ color }} />
        </div>
        <div className="flex-1 min-w-0">
          <h1 className="text-sm font-semibold text-white truncate">{dao.name}</h1>
          <p className="text-xs text-[#6b6a8a]">
            {dao.config.members.length} members · {dao.proposals.length} proposals · {dao.template_id}
          </p>
        </div>
        <span className="rounded-md px-2 py-0.5 text-[10px] font-medium"
              style={{ background: dao.status === 'active' ? 'rgba(0,212,170,0.08)' : 'rgba(251,191,36,0.08)',
                       color: dao.status === 'active' ? '#00d4aa' : '#fbbf24' }}>
          {dao.status}
        </span>
      </div>

      {/* DAO panel fills the rest */}
      <div className="flex-1 overflow-hidden">
        <DaoPanel />
      </div>
    </div>
  )
}

const TOOL_META: Record<string, { label: string; icon: React.ReactNode; color: string; component: React.ReactNode }> = {
  chat:       { label: 'Agent Chat', icon: <MessageSquare className="h-4 w-4" />, color: '#ff6b4a', component: <ChatPanel /> },
  dao:        { label: 'DAO Governance', icon: <Vote className="h-4 w-4" />, color: '#00d4aa', component: <DaoPanel /> },
  dex:        { label: 'DEX', icon: <ArrowLeftRight className="h-4 w-4" />, color: '#fbbf24', component: <DexPanel /> },
  intel:      { label: 'Qu-Zeno Intel', icon: <Eye className="h-4 w-4" />, color: '#60a5fa', component: <IntelPanel /> },
  robotops:   { label: 'Robot Ops', icon: <Cpu className="h-4 w-4" />, color: '#a78bfa', component: <RobotOpsPanel /> },
  feepay:     { label: 'FeePay', icon: <Fuel className="h-4 w-4" />, color: '#34d399', component: <FeePayPanel /> },
  miner:      { label: 'Truth Miners', icon: <Pickaxe className="h-4 w-4" />, color: '#f87171', component: <MinerPanel /> },
  buzz:       { label: 'Buzz Relay', icon: <Radio className="h-4 w-4" />, color: '#e879f9', component: <BuzzPanel /> },
  commonwealth:{ label: 'Commonwealth', icon: <HeartPulse className="h-4 w-4" />, color: '#fb7185', component: <CommonwealthPanel /> },
  contracts:  { label: 'Contracts', icon: <FileCode2 className="h-4 w-4" />, color: '#94a3b8', component: <ContractsPanel /> },
  updates:    { label: 'Chain Updates', icon: <RefreshCw className="h-4 w-4" />, color: '#22d3ee', component: <UpdatesPanel /> },
}

function ToolDetail({ toolId, onBack }: { toolId: string; onBack: () => void }) {
  const meta = TOOL_META[toolId]
  if (!meta) return <div className="p-8 text-center text-[#6b6a8a]">Tool not found</div>

  return (
    <div className="flex flex-col overflow-hidden" style={{ height: '100%' }}>
      <div className="flex items-center gap-3 px-6 py-3"
           style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
        <button onClick={onBack} className="rounded-lg p-1.5 transition hover:bg-white/5">
          <ArrowLeft className="h-4 w-4 text-[#6b6a8a]" />
        </button>
        <div className="flex h-8 w-8 items-center justify-center rounded-lg"
             style={{ background: `${meta.color}14` }}>
          <span style={{ color: meta.color }}>{meta.icon}</span>
        </div>
        <h1 className="text-sm font-semibold text-white">{meta.label}</h1>
      </div>
      <div className="flex-1 overflow-hidden">
        {meta.component}
      </div>
    </div>
  )
}
