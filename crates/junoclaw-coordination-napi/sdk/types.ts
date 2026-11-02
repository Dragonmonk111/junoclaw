/**
 * Shared TypeScript types for @junoclaw/coordination SDK.
 */

/** Gate verdict levels for J-Lens truth gate. */
export enum GateVerdict {
  Green = 'Green',
  Yellow = 'Yellow',
  Red = 'Red',
}

/** A single agent-to-agent message in the coordination network. */
export interface AgentMessage {
  from: Uint8Array
  to: Uint8Array
  content: Uint8Array
  contentHash: Uint8Array
  timestamp: bigint
  jLensGate?: GateVerdict
  proposalRef?: bigint
}

/** A batch of ordered messages — the consensus block format. */
export interface Batch {
  messages: AgentMessage[]
  prevHash: Uint8Array
  height: bigint
  timestamp: bigint
  gateResult?: GateResult
  /** Thought records anchored in this batch (merkle-committed). */
  thoughtRecords?: ThoughtRecord[]
  /** Merkle root over thoughtRecords leaf hashes (hex). */
  thoughtRoot?: string
}

/** Trust tier of a thought attestation. */
export type ThoughtTier = 't0-operator' | 't2-tee'

/**
 * Compact reference to a jlens.thought.v1 snapshot — travels inside an
 * IntentMessage next to executionProofRef. Raw activations never travel;
 * activationRoot commits to the off-chain int8 blobs.
 */
export interface ThoughtRef {
  /** Must be "jlens.thought.v1". */
  schema: string
  /** Must be "jlens.integrity.v1". */
  scoreSemantics: string
  snapshotHash: string
  modelId: string
  weightsHash: string
  bankHash: string
  /** Binds the thought to exactly this action (see actionHashForIntent). */
  actionHash: string
  /** Merkle root over the int8-quantized activations (off-chain blobs). */
  activationRoot: string
  /** jlens.integrity.v1 — higher = healthier. */
  integrity: number
  tier: ThoughtTier
  /** TEE attestation reference (quote hash / tx) — required for t2-tee. */
  teeAttestationRef?: string
  /** Where the off-chain activation blobs can be fetched for re-audit. */
  storageLocator?: string
}

/**
 * Robot/agent intent — the auditable action envelope. Encoded into
 * AgentMessage.content; the gate classifies `action` into an action
 * class and enforces the DAO thought policy on it.
 */
export interface IntentMessage {
  robotId: string
  action: string
  /** Structured action parameters, serialized as JSON. */
  paramsJson: string
  sensorSnapshotHash: string
  controllerTimestamp: bigint
  rationale?: string
  executionProofRef?: string
  /** Generation-time J-Lens thought attestation. */
  thought?: ThoughtRef
}

/** One thought outcome anchored in a batch (merkle leaf). */
export interface ThoughtRecord {
  agentId: string
  actionHash: string
  snapshotHash: string
  bankHash: string
  activationRoot: string
  integrity: number
  gate: 'green' | 'yellow' | 'red'
  tier: ThoughtTier
  /** Accepted under bootstrap dev-gating (t0 thought for a t2 class). */
  devGated: boolean
  batchHeight: bigint
}

/** J-Lens gate audit result attached to a batch. */
export interface GateResult {
  verdict: GateVerdict
  attestationHash?: string
  /** Worst audited integrity (jlens.integrity.v1: higher = healthier,
   *  1.0 = no detections). Undefined when nothing was audited. */
  integrity?: number
  modelId?: string
}

/** A finalized block with a threshold certificate. */
export interface BatchCertificate {
  batch: Batch
  certificate: Uint8Array
  height: bigint
  finalizedAt: bigint
}

/** Configuration for joining the coordination network. */
export interface CoordinationNetworkConfig {
  /** Peer identities (public keys) to connect to. */
  peers: Uint8Array[]
  /** This node's identity (public key). */
  identity: Uint8Array
  /** J-Lens server endpoint (e.g. http://localhost:7777). */
  jLensEndpoint?: string
  /** Registered model to probe with (server resolves endpoint + bank). */
  jLensModelId?: string
  /** API key for J-Lens server. */
  jLensApiKey?: string
  /** Use mock gate (no HTTP calls, keyword heuristics). */
  mockGate?: boolean
  /** Settlement contract address on Juno. */
  settlerContract?: string
  /** Juno RPC endpoint for settlement. */
  junoRpc?: string
  /** Chain ID (e.g. uni-7, juno-1). */
  chainId?: string
}

/** Result of sending a message through the network. */
export type SendResult =
  | { status: 'relayed'; batchHeight: bigint }
  | { status: 'blocked'; reason: string }
  | { status: 'pending' }

/** Event handler for incoming messages. */
export type MessageHandler = (msg: AgentMessage, audit?: GateResult) => void

/** Event handler for finalized batches. */
export type BatchHandler = (cert: BatchCertificate) => void

/** Internal: native addon function signatures. */
export interface NativeAddon {
  createAgentMessage(from: Uint8Array, to: Uint8Array, content: Uint8Array, timestamp: bigint): AgentMessage
  encodeAgentMessage(msg: AgentMessage): Uint8Array
  decodeAgentMessage(data: Uint8Array): AgentMessage
  verifyMessageHash(msg: AgentMessage): boolean
  isBroadcastMessage(msg: AgentMessage): boolean
  createBatch(messages: AgentMessage[], prevHash: Uint8Array, height: bigint, timestamp: bigint): Batch
  hashBatch(batch: Batch): Uint8Array
  batchHasBlockedMessage(batch: Batch): boolean
  batchLen(batch: Batch): bigint
  createGateResult(verdict: GateVerdict, integrity?: number, attestationHash?: string, modelId?: string): GateResult
  encodeIntentMessage(msg: IntentMessage): Uint8Array
  decodeIntentMessage(data: Uint8Array): IntentMessage
  /** Deterministic action hash — binds a thought to exactly one action.
   *  Same value as jlens-serve thought.py action_hash (cross-language
   *  golden: j-lens/fixtures/action_hash.v1.json). */
  actionHashForIntent(msg: IntentMessage): string
}
