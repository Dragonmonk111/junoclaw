export type Personality = 'professional' | 'creative' | 'analytical' | 'conversational' | 'custom'

export interface AgentInfo {
  id: string
  name: string
  description: string
  model: string
  capabilities: Capability[]
  default_tier: ExecutionTier
  wavs_verified: boolean
  personality: Personality
  system_prompt?: string
  created_at: string
  is_active: boolean
  chain_id?: string
  parent_id?: string | null
}

export interface DelegationRecord {
  id: string
  from_agent_id: string
  to_agent_id: string
  prompt: string
  status: 'pending' | 'in_progress' | 'completed' | 'failed'
  created_at: string
  completed_at?: string
  result?: string
}

export type ExecutionTier = 'local' | 'akash'
export type Capability = 'web_browsing' | 'file_read_write' | 'shell_execution' | 'code_execution' | 'image_generation' | 'data_analysis'
export type MessageRole = 'user' | 'assistant' | 'system' | 'tool'
export type TaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled' | 'budget_exceeded'

// ── Tool Silo System ──

export type ToolCategory = 'shell' | 'chain_query' | 'chain_tx' | 'file_io' | 'web_browse' | 'dex' | 'code' | 'image'

export type SiloApprovalMode = 'auto' | 'ask' | 'blocked'

export interface SiloConfig {
  category: ToolCategory
  approval: SiloApprovalMode
  /** Restrict scope (e.g., allowed directories, contract addresses) */
  scope?: string[]
  /** Blocklist patterns (e.g., "rm -rf", "sudo") */
  blocklist?: string[]
  /** Max spend per tx (for chain_tx silo) in micro-denom */
  max_spend?: number
}

export const DEFAULT_SILOS: SiloConfig[] = [
  { category: 'chain_query', approval: 'auto', scope: [] },
  { category: 'chain_tx',    approval: 'ask',  scope: [], max_spend: 1_000_000 },
  { category: 'shell',       approval: 'ask',  scope: [], blocklist: ['rm -rf', 'sudo', 'format', 'del /'] },
  { category: 'file_io',     approval: 'ask',  scope: [] },
  { category: 'web_browse',  approval: 'auto', scope: [] },
  { category: 'dex',         approval: 'ask',  scope: [] },
  { category: 'code',        approval: 'auto', scope: [] },
  { category: 'image',       approval: 'auto', scope: [] },
]

export const TOOL_CATEGORY_META: Record<ToolCategory, { label: string; icon: string; color: string; description: string }> = {
  shell:       { label: 'Shell',       icon: 'Terminal',    color: '#00d4aa', description: 'Execute shell commands on the host' },
  chain_query: { label: 'Chain Query', icon: 'Search',      color: '#60a5fa', description: 'Read-only queries to on-chain contracts' },
  chain_tx:    { label: 'Chain TX',    icon: 'Send',        color: '#f87171', description: 'Submit transactions to the chain (costs gas)' },
  file_io:     { label: 'File I/O',    icon: 'FileText',    color: '#a78bfa', description: 'Read and write files on the host filesystem' },
  web_browse:  { label: 'Web Browse',  icon: 'Globe',       color: '#fbbf24', description: 'Fetch URLs and search the web' },
  dex:         { label: 'DEX',         icon: 'ArrowLeftRight', color: '#ff6b4a', description: 'Junoswap v2 swap, liquidity, and pool operations' },
  code:        { label: 'Code',        icon: 'Code',        color: '#34d399', description: 'Execute code snippets in a sandbox' },
  image:       { label: 'Image',       icon: 'Image',       color: '#e879f9', description: 'Generate or analyze images' },
}

/** Map tool names to their silo category */
export function getToolCategory(toolName: string): ToolCategory {
  const lower = toolName.toLowerCase()
  if (lower.includes('shell') || lower.includes('exec_command') || lower.includes('run_command')) return 'shell'
  if (lower.includes('query') || lower.includes('get_balance') || lower.includes('get_pool') || lower.includes('simulate')) return 'chain_query'
  if (lower.includes('swap') || lower.includes('provide_liquidity') || lower.includes('withdraw_liquidity')) return 'dex'
  if (lower.includes('send_tx') || lower.includes('execute_contract') || lower.includes('submit')) return 'chain_tx'
  if (lower.includes('read_file') || lower.includes('write_file') || lower.includes('list_dir')) return 'file_io'
  if (lower.includes('browse') || lower.includes('fetch_url') || lower.includes('search_web')) return 'web_browse'
  if (lower.includes('run_code') || lower.includes('eval') || lower.includes('python')) return 'code'
  if (lower.includes('image') || lower.includes('generate_image')) return 'image'
  return 'shell' // fallback to most restrictive
}

// ── Enhanced Chat Messages ──

export type ToolCallStatus = 'pending' | 'approved' | 'denied' | 'running' | 'completed' | 'failed'

export interface ChatMessage {
  id: string
  role: MessageRole
  content: string
  tool_calls?: ToolCallRecord[]
  timestamp: string
}

export interface ToolCallRecord {
  id?: string
  tool_name: string
  category?: ToolCategory
  input: unknown
  output: unknown
  duration_ms: number
  approved: boolean
  status?: ToolCallStatus
  error?: string
}

export interface Task {
  id: string
  agent_id: string
  input: string
  tier: ExecutionTier
  status: TaskStatus
  created_at: string
  completed_at?: string
  result?: TaskResult
  cost?: TaskCost
  chain_tx?: string
}

export interface TaskResult {
  output: string
  output_hash: string
  tool_calls: ToolCallRecord[]
  tokens_used: TokenUsage
}

export interface TaskCost {
  amount: number
  denom: string
  usd_equivalent?: number
}

export interface TokenUsage {
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
}

// ── DAO Governance Types ──

export type MemberRole = 'agent' | 'human' | 'subdao'
export type VoteOption = 'yes' | 'no' | 'abstain'
export type ProposalStatus = 'open' | 'passed' | 'rejected' | 'executed' | 'expired'
export type VerificationModel = 'none' | 'witness' | 'wavs' | 'witness_and_wavs'

export interface DaoMember {
  addr: string
  weight: number
  role: MemberRole
}

export type ProposalKind =
  | { type: 'weight_change'; members: DaoMember[] }
  | { type: 'wavs_push'; task_description: string; execution_tier: ExecutionTier; escrow_amount: number }
  | { type: 'config_change'; new_admin?: string; new_governance?: string; new_wavs_operator?: string }
  | { type: 'free_text'; title: string; description: string }
  | { type: 'outcome_create'; question: string; resolution_criteria: string; deadline_block: number }
  | { type: 'outcome_resolve'; market_id: number; outcome: boolean; attestation_hash: string }

export interface DaoVote {
  voter: string
  option: VoteOption
  weight: number
  block_height: number
}

export interface DaoProposal {
  id: number
  proposer: string
  kind: ProposalKind
  votes: DaoVote[]
  yes_weight: number
  no_weight: number
  abstain_weight: number
  total_voted_weight: number
  status: ProposalStatus
  created_at_block: number
  voting_deadline_block: number
  min_deadline_block: number
  executed: boolean
}

export interface VerificationConfig {
  model: VerificationModel
  required_attestations: number
  total_witnesses: number
  attestation_timeout_blocks: number
  auto_release_on_verify: boolean
}

export interface DaoConfig {
  name: string
  admin: string
  governance?: string
  escrow_contract: string
  agent_registry: string
  task_ledger?: string
  members: DaoMember[]
  total_weight: number
  denom: string
  voting_period_blocks: number
  quorum_percent: number
  adaptive_threshold_blocks: number
  adaptive_min_blocks: number
  verification: VerificationConfig
}

// ── Multi-DAO Instance (local tracking) ──

export interface DaoInstance {
  id: string
  name: string
  template_id: string
  template_color: string
  chain_address?: string
  config: DaoConfig
  proposals: DaoProposal[]
  /** IDs of local agents that participate in this DAO */
  agent_ids: string[]
  created_at: string
  status: 'deploying' | 'active' | 'archived'
}

// WebSocket message types
export type WsClientMessage =
  | { type: 'send_message'; data: { agent_id: string; content: string } }
  | { type: 'create_agent'; data: AgentInfo }
  | { type: 'list_agents' }
  | { type: 'list_tasks'; data: { agent_id?: string } }
  | { type: 'cancel_task'; data: { task_id: string } }
  | { type: 'approve_tool_call'; data: { task_id: string; tool_call_id: string } }
  | { type: 'deny_tool_call'; data: { task_id: string; tool_call_id: string } }
  | { type: 'create_proposal'; data: { kind: ProposalKind } }
  | { type: 'cast_vote'; data: { proposal_id: number; vote: VoteOption } }
  | { type: 'execute_proposal'; data: { proposal_id: number } }
  | { type: 'expire_proposal'; data: { proposal_id: number } }
  | { type: 'query_proposals' }
  | { type: 'query_config' }

// ── Junoswap v2 DEX Types ──

export type DexAssetInfo = { native: string }

export interface DexPairInfo {
  pair_addr: string
  token_a: DexAssetInfo
  token_b: DexAssetInfo
  fee_bps: number
  factory: string
  junoclaw_contract?: string
}

export interface DexPoolState {
  pair_addr: string
  reserve_a: string
  reserve_b: string
  total_lp_shares: string
  total_swaps: number
  total_volume_a: string
  total_volume_b: string
  price_a_per_b: string
  price_b_per_a: string
  token_a: DexAssetInfo
  token_b: DexAssetInfo
}

export interface DexSimulation {
  return_amount: string
  spread_amount: string
  fee_amount: string
}

export interface DexSwapAttestation {
  pair: string
  offer_asset: string
  offer_amount: string
  return_amount: string
  effective_price: string
  price_impact_pct: string
  manipulation_flag: boolean
  verified: boolean
  attestation_hash: string
  timestamp: string
}

export interface DexLpPosition {
  pair_addr: string
  lp_shares: string
  share_of_pool_pct: string
  value_a: string
  value_b: string
}

export type WsServerMessage =
  | { type: 'stream_token'; data: { agent_id: string; token: string } }
  | { type: 'stream_complete'; data: { agent_id: string; message: ChatMessage } }
  | { type: 'tool_call_request'; data: { task_id: string; tool_call: { id: string; name: string; arguments: unknown } } }
  | { type: 'tool_call_result'; data: { task_id: string; record: ToolCallRecord } }
  | { type: 'task_status_update'; data: { task: Task } }
  | { type: 'agent_list'; data: AgentInfo[] }
  | { type: 'task_list'; data: Task[] }
  | { type: 'error'; data: { message: string } }
  | { type: 'connected'; data: { version: string } }
  | { type: 'dao_config'; data: DaoConfig }
  | { type: 'proposal_list'; data: DaoProposal[] }
  | { type: 'proposal_update'; data: DaoProposal }
