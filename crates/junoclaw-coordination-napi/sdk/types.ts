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
}

/** J-Lens gate audit result attached to a batch. */
export interface GateResult {
  verdict: GateVerdict
  attestationHash?: string
  separationScore: number
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
  /** J-Lens CSI server endpoint (e.g. http://localhost:7777). */
  jLensEndpoint?: string
  /** API key for CSI server. */
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
  createGateResult(verdict: GateVerdict, separationScore: number, attestationHash?: string, modelId?: string): GateResult
}
