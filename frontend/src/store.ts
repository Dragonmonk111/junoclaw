import { create } from 'zustand'
import type { AgentInfo, ChatMessage, DaoInstance, DelegationRecord, SiloConfig, ToolCallRecord, ToolCategory, WsClientMessage, WsServerMessage } from './types'
import { DEFAULT_SILOS, getToolCategory } from './types'

// ── localStorage persistence (must be defined before store) ──

const AGENTS_KEY = 'junoclaw_agents'
const ACTIVE_KEY = 'junoclaw_active_agent'
const DAOS_KEY = 'junoclaw_daos'
const ACTIVE_DAO_KEY = 'junoclaw_active_dao'
const SILOS_KEY = 'junoclaw_silos'

function persistAgents(agents: AgentInfo[]) {
  try {
    localStorage.setItem(AGENTS_KEY, JSON.stringify(agents))
  } catch { /* quota exceeded — ignore */ }
}

function hydrateAgents(): { agents: AgentInfo[]; activeAgentId: string | null } {
  try {
    const raw = localStorage.getItem(AGENTS_KEY)
    if (raw) {
      const agents: AgentInfo[] = JSON.parse(raw)
      const savedActive = localStorage.getItem(ACTIVE_KEY)
      const activeAgentId = agents.find(a => a.id === savedActive) ? savedActive : agents[0]?.id ?? null
      return { agents, activeAgentId }
    }
  } catch { /* corrupt data — ignore */ }
  return { agents: [], activeAgentId: null }
}

function persistSilos(silos: Record<string, SiloConfig[]>) {
  try { localStorage.setItem(SILOS_KEY, JSON.stringify(silos)) } catch {}
}

function hydrateSilos(): Record<string, SiloConfig[]> {
  try {
    const raw = localStorage.getItem(SILOS_KEY)
    if (raw) return JSON.parse(raw)
  } catch {}
  return {}
}

const initialSilos = hydrateSilos()

function persistDaos(daos: DaoInstance[]) {
  try { localStorage.setItem(DAOS_KEY, JSON.stringify(daos)) } catch {}
}

function hydrateDaos(): { daos: DaoInstance[]; activeDaoId: string | null } {
  try {
    const raw = localStorage.getItem(DAOS_KEY)
    if (raw) {
      const daos: DaoInstance[] = JSON.parse(raw)
      const savedActive = localStorage.getItem(ACTIVE_DAO_KEY)
      const activeDaoId = daos.find(d => d.id === savedActive) ? savedActive : daos[0]?.id ?? null
      return { daos, activeDaoId }
    }
  } catch {}
  return { daos: [], activeDaoId: null }
}

const hydrated = hydrateAgents()
const initialAgents = hydrated.agents
const initialActiveId = hydrated.activeAgentId
const hydratedDaos = hydrateDaos()
const initialDaos = hydratedDaos.daos
const initialActiveDaoId = hydratedDaos.activeDaoId

interface AppState {
  // Connection
  connected: boolean
  daemonVersion: string | null
  ws: WebSocket | null

  // Agents
  agents: AgentInfo[]
  activeAgentId: string | null

  // Sessions (agent_id -> messages)
  sessions: Record<string, ChatMessage[]>

  // Agent delegation chain
  delegations: DelegationRecord[]

  // Streaming
  streamingToken: string
  isStreaming: boolean
  streamingAgentId: string | null
  lastError: string | null

  // Tool call approval
  pendingToolCall: { taskId: string; toolCallId: string; name: string; args: unknown; category: ToolCategory } | null

  // Tool call history (per-agent, for inline rendering)
  toolCallHistory: Record<string, ToolCallRecord[]>

  // Silo configs (per-agent)
  agentSilos: Record<string, SiloConfig[]>

  // DAOs (multi-DAO)
  daos: DaoInstance[]
  activeDaoId: string | null

  // Actions
  connect: () => void
  disconnect: () => void
  send: (msg: WsClientMessage) => void
  setActiveAgent: (id: string | null) => void
  sendMessage: (content: string) => void
  createAgent: (name: string, description: string, model: string, parentId?: string, personality?: import('./types').Personality, systemPrompt?: string) => void
  setAgentTier: (agentId: string, tier: import('./types').ExecutionTier) => void
  setWavsVerified: (agentId: string, enabled: boolean) => void
  setAgentPersonality: (agentId: string, personality: import('./types').Personality, systemPrompt?: string) => void
  delegateToAgent: (fromAgentId: string, toAgentId: string, prompt: string) => void
  getAgentChain: (agentId: string) => AgentInfo[]
  approveToolCall: () => void
  denyToolCall: () => void
  getAgentSilos: (agentId: string) => SiloConfig[]
  setAgentSilo: (agentId: string, category: ToolCategory, update: Partial<SiloConfig>) => void
  resetAgentSilos: (agentId: string) => void
  deployDao: (data: {
    name: string
    members: { addr: string; weight: number; role: string }[]
    template_id: string
    template_color: string
    voting_period_blocks: number
    quorum_percent: number
    verification_model: string
  }) => void
  setActiveDao: (id: string | null) => void
  createLocalDao: (dao: DaoInstance) => void
  joinAgentToDao: (daoId: string, agentId: string) => void
  removeAgentFromDao: (daoId: string, agentId: string) => void
  archiveDao: (daoId: string) => void
}

export const useStore = create<AppState>((set, get) => ({
  connected: false,
  daemonVersion: null,
  ws: null,
  agents: initialAgents,
  activeAgentId: initialActiveId,
  sessions: {},
  delegations: [],
  streamingToken: '',
  isStreaming: false,
  streamingAgentId: null,
  lastError: null,
  pendingToolCall: null,
  toolCallHistory: {},
  agentSilos: initialSilos,
  daos: initialDaos,
  activeDaoId: initialActiveDaoId,

  connect: () => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const host = window.location.hostname || 'localhost'
    const wsUrl = `${protocol}//${host}:7777/ws`

    const ws = new WebSocket(wsUrl)

    ws.onopen = () => {
      set({ connected: true, ws })
      // Request agent list on connect
      ws.send(JSON.stringify({ type: 'list_agents' }))
    }

    ws.onclose = () => {
      set({ connected: false, ws: null, daemonVersion: null })
      // Auto-reconnect after 3 seconds
      setTimeout(() => get().connect(), 3000)
    }

    ws.onerror = () => {
      ws.close()
    }

    ws.onmessage = (event) => {
      try {
        const msg: WsServerMessage = JSON.parse(event.data)
        handleServerMessage(msg, set, get)
      } catch (e) {
        console.error('Failed to parse WS message:', e)
      }
    }
  },

  disconnect: () => {
    const { ws } = get()
    if (ws) {
      ws.close()
    }
  },

  send: (msg) => {
    const { ws } = get()
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(msg))
    }
  },

  setActiveAgent: (id) => {
    set({ activeAgentId: id, streamingToken: '' })
  },

  sendMessage: (content) => {
    const { activeAgentId, send, sessions } = get()
    if (!activeAgentId) return

    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content,
      timestamp: new Date().toISOString(),
    }

    const agentMessages = sessions[activeAgentId] || []
    set({
      sessions: {
        ...sessions,
        [activeAgentId]: [...agentMessages, userMsg],
      },
      streamingToken: '',
    })

    const { connected } = get()
    set({ isStreaming: true, streamingAgentId: activeAgentId, lastError: null })

    if (!connected) {
      // Daemon not connected — show error after brief delay so user sees the attempt
      setTimeout(() => {
        const s = get()
        if (s.isStreaming && s.streamingAgentId === activeAgentId && !s.streamingToken) {
          set({
            isStreaming: false,
            streamingAgentId: null,
            lastError: 'Daemon offline — start the JunoClaw daemon (port 7777) or switch to Akash compute.',
          })
        }
      }, 2500)
      return
    }

    send({ type: 'send_message', data: { agent_id: activeAgentId, content } })
  },

  approveToolCall: () => {
    const { pendingToolCall, send, activeAgentId, toolCallHistory } = get()
    if (!pendingToolCall) return
    send({ type: 'approve_tool_call', data: { task_id: pendingToolCall.taskId, tool_call_id: pendingToolCall.toolCallId } })
    // Record approved tool call in history
    if (activeAgentId) {
      const agentHistory = toolCallHistory[activeAgentId] || []
      const record: ToolCallRecord = {
        id: pendingToolCall.toolCallId,
        tool_name: pendingToolCall.name,
        category: pendingToolCall.category,
        input: pendingToolCall.args,
        output: null,
        duration_ms: 0,
        approved: true,
        status: 'running',
      }
      set({
        pendingToolCall: null,
        isStreaming: true,
        toolCallHistory: { ...toolCallHistory, [activeAgentId]: [...agentHistory, record] },
      })
    } else {
      set({ pendingToolCall: null, isStreaming: true })
    }
  },

  denyToolCall: () => {
    const { pendingToolCall, send, activeAgentId, toolCallHistory } = get()
    if (!pendingToolCall) return
    send({ type: 'deny_tool_call', data: { task_id: pendingToolCall.taskId, tool_call_id: pendingToolCall.toolCallId } })
    // Record denied tool call in history
    if (activeAgentId) {
      const agentHistory = toolCallHistory[activeAgentId] || []
      const record: ToolCallRecord = {
        id: pendingToolCall.toolCallId,
        tool_name: pendingToolCall.name,
        category: pendingToolCall.category,
        input: pendingToolCall.args,
        output: null,
        duration_ms: 0,
        approved: false,
        status: 'denied',
      }
      set({
        pendingToolCall: null,
        isStreaming: true,
        toolCallHistory: { ...toolCallHistory, [activeAgentId]: [...agentHistory, record] },
      })
    } else {
      set({ pendingToolCall: null, isStreaming: true })
    }
  },

  getAgentSilos: (agentId) => {
    const { agentSilos } = get()
    return agentSilos[agentId] || [...DEFAULT_SILOS]
  },

  setAgentSilo: (agentId, category, update) => {
    const { agentSilos } = get()
    const current = agentSilos[agentId] || [...DEFAULT_SILOS.map(s => ({ ...s }))]
    const updated = current.map(s => s.category === category ? { ...s, ...update } : s)
    const newSilos = { ...agentSilos, [agentId]: updated }
    set({ agentSilos: newSilos })
    persistSilos(newSilos)
  },

  resetAgentSilos: (agentId) => {
    const { agentSilos } = get()
    const newSilos = { ...agentSilos }
    delete newSilos[agentId]
    set({ agentSilos: newSilos })
    persistSilos(newSilos)
  },

  setAgentTier: (agentId, tier) => {
    const { agents } = get()
    const updated = agents.map(a => a.id === agentId ? { ...a, default_tier: tier } : a)
    set({ agents: updated })
    persistAgents(updated)
  },

  setWavsVerified: (agentId, enabled) => {
    const { agents } = get()
    const updated = agents.map(a => a.id === agentId ? { ...a, wavs_verified: enabled } : a)
    set({ agents: updated })
    persistAgents(updated)
  },

  setAgentPersonality: (agentId, personality, systemPrompt) => {
    const { agents } = get()
    const updated = agents.map(a => a.id === agentId ? { ...a, personality, system_prompt: systemPrompt } : a)
    set({ agents: updated })
    persistAgents(updated)
  },

  createAgent: (name, description, model, parentId, personality, systemPrompt) => {
    const { agents, send } = get()
    const agent: AgentInfo = {
      id: crypto.randomUUID(),
      name,
      description,
      model,
      capabilities: [],
      default_tier: 'local',
      wavs_verified: false,
      personality: personality ?? 'professional',
      system_prompt: systemPrompt,
      created_at: new Date().toISOString(),
      is_active: true,
      parent_id: parentId ?? null,
    }
    const updated = [...agents, agent]
    set({ agents: updated, activeAgentId: agent.id })
    persistAgents(updated)
    // Best-effort: also notify daemon if connected
    send({ type: 'create_agent', data: agent })
  },

  delegateToAgent: (fromAgentId, toAgentId, prompt) => {
    const { sessions, delegations, send, connected } = get()

    // Create delegation record
    const delegation: DelegationRecord = {
      id: crypto.randomUUID(),
      from_agent_id: fromAgentId,
      to_agent_id: toAgentId,
      prompt,
      status: 'pending',
      created_at: new Date().toISOString(),
    }

    // Add a system message to the sender's session showing delegation
    const fromAgent = get().agents.find(a => a.id === fromAgentId)
    const toAgent = get().agents.find(a => a.id === toAgentId)
    const delegateMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'system',
      content: `↪ Delegated to **${toAgent?.name ?? toAgentId}**: "${prompt}"`,
      timestamp: new Date().toISOString(),
    }
    const fromMessages = sessions[fromAgentId] || []

    // Add the prompt as a user message in the target agent's session
    const incomingMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: `[From ${fromAgent?.name ?? 'Main Agent'}] ${prompt}`,
      timestamp: new Date().toISOString(),
    }
    const toMessages = sessions[toAgentId] || []

    set({
      delegations: [...delegations, delegation],
      sessions: {
        ...sessions,
        [fromAgentId]: [...fromMessages, delegateMsg],
        [toAgentId]: [...toMessages, incomingMsg],
      },
      activeAgentId: toAgentId,
      streamingToken: '',
    })

    // Actually send the prompt to the target agent
    if (connected) {
      set({ isStreaming: true, streamingAgentId: toAgentId, lastError: null })
      send({ type: 'send_message', data: { agent_id: toAgentId, content: prompt } })
    } else {
      set({ isStreaming: true, streamingAgentId: toAgentId, lastError: null })
      setTimeout(() => {
        const s = get()
        if (s.isStreaming && s.streamingAgentId === toAgentId && !s.streamingToken) {
          set({
            isStreaming: false,
            streamingAgentId: null,
            lastError: 'Daemon offline — delegation queued. Start daemon to process.',
          })
        }
      }, 2500)
    }
  },

  getAgentChain: (agentId) => {
    const { agents } = get()
    const chain: AgentInfo[] = []
    let current = agents.find(a => a.id === agentId)
    while (current) {
      chain.unshift(current)
      current = current.parent_id ? agents.find(a => a.id === current!.parent_id) : undefined
    }
    return chain
  },

  deployDao: (data) => {
    const { send, daos, activeAgentId } = get()
    // Create a local DAO instance
    const dao: import('./types').DaoInstance = {
      id: crypto.randomUUID(),
      name: data.name,
      template_id: data.template_id,
      template_color: data.template_color,
      config: {
        name: data.name,
        admin: 'pending',
        escrow_contract: 'pending',
        agent_registry: 'pending',
        members: data.members.map(m => ({ addr: m.addr, weight: m.weight, role: m.role as import('./types').MemberRole })),
        total_weight: 10000,
        denom: 'ujunox',
        voting_period_blocks: data.voting_period_blocks,
        quorum_percent: data.quorum_percent,
        adaptive_threshold_blocks: 10,
        adaptive_min_blocks: 13,
        verification: {
          model: data.verification_model as import('./types').VerificationModel,
          required_attestations: 2,
          total_witnesses: 3,
          attestation_timeout_blocks: 200,
          auto_release_on_verify: true,
        },
      },
      proposals: [],
      agent_ids: activeAgentId ? [activeAgentId] : [],
      created_at: new Date().toISOString(),
      status: 'deploying',
    }
    const updated = [...daos, dao]
    set({ daos: updated, activeDaoId: dao.id })
    persistDaos(updated)
    // Best-effort: notify daemon
    send({
      type: 'deploy_dao' as any,
      data: {
        dao_id: dao.id,
        name: data.name,
        members: data.members,
        template_id: data.template_id,
        voting_period_blocks: data.voting_period_blocks,
        quorum_percent: data.quorum_percent,
        verification_model: data.verification_model,
      },
    } as any)
    // Simulate deploy completion after 2s
    setTimeout(() => {
      const current = get().daos
      const upd = current.map(d => d.id === dao.id ? { ...d, status: 'active' as const } : d)
      set({ daos: upd })
      persistDaos(upd)
    }, 2000)
  },

  setActiveDao: (id) => {
    set({ activeDaoId: id })
    if (id) try { localStorage.setItem(ACTIVE_DAO_KEY, id) } catch {}
  },

  createLocalDao: (dao) => {
    const { daos } = get()
    const updated = [...daos, dao]
    set({ daos: updated, activeDaoId: dao.id })
    persistDaos(updated)
  },

  joinAgentToDao: (daoId, agentId) => {
    const { daos } = get()
    const updated = daos.map(d => {
      if (d.id !== daoId) return d
      if (d.agent_ids.includes(agentId)) return d
      return { ...d, agent_ids: [...d.agent_ids, agentId] }
    })
    set({ daos: updated })
    persistDaos(updated)
  },

  removeAgentFromDao: (daoId, agentId) => {
    const { daos } = get()
    const updated = daos.map(d => {
      if (d.id !== daoId) return d
      return { ...d, agent_ids: d.agent_ids.filter(id => id !== agentId) }
    })
    set({ daos: updated })
    persistDaos(updated)
  },

  archiveDao: (daoId) => {
    const { daos, activeDaoId } = get()
    const updated = daos.map(d => d.id === daoId ? { ...d, status: 'archived' as const } : d)
    const newActive = activeDaoId === daoId ? (updated.find(d => d.status !== 'archived')?.id ?? null) : activeDaoId
    set({ daos: updated, activeDaoId: newActive })
    persistDaos(updated)
  },
}))

// Persist activeAgentId changes
useStore.subscribe((state, prev) => {
  if (state.activeAgentId !== prev.activeAgentId && state.activeAgentId) {
    try { localStorage.setItem(ACTIVE_KEY, state.activeAgentId) } catch {}
  }
})

function handleServerMessage(
  msg: WsServerMessage,
  set: (partial: Partial<AppState> | ((state: AppState) => Partial<AppState>)) => void,
  get: () => AppState,
) {
  switch (msg.type) {
    case 'connected':
      set({ daemonVersion: msg.data.version })
      break

    case 'agent_list': {
      // Merge: daemon agents take precedence, but keep locally-created agents not on daemon
      const daemonIds = new Set(msg.data.map((a: AgentInfo) => a.id))
      const localOnly = get().agents.filter(a => !daemonIds.has(a.id))
      const merged = [...msg.data, ...localOnly]
      set({ agents: merged })
      persistAgents(merged)
      break
    }

    case 'stream_token':
      set((state) => ({
        streamingToken: state.streamingToken + msg.data.token,
        isStreaming: true,
      }))
      break

    case 'stream_complete': {
      const { sessions } = get()
      const agentId = msg.data.agent_id
      const agentMessages = sessions[agentId] || []
      set({
        sessions: {
          ...sessions,
          [agentId]: [...agentMessages, msg.data.message],
        },
        streamingToken: '',
        isStreaming: false,
        streamingAgentId: null,
      })
      break
    }

    case 'tool_call_request':
      set({
        isStreaming: false,
        pendingToolCall: {
          taskId: msg.data.task_id,
          toolCallId: msg.data.tool_call.id,
          name: msg.data.tool_call.name,
          args: msg.data.tool_call.arguments,
          category: getToolCategory(msg.data.tool_call.name),
        },
      })
      break

    case 'error':
      console.error('Server error:', msg.data.message)
      set({ isStreaming: false, streamingAgentId: null, lastError: msg.data.message })
      break

    default:
      break
  }
}
