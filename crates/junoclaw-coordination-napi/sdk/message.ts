/**
 * AgentMessage helpers — create, encode, decode, verify.
 */

import type { AgentMessage } from './types.js'
import { getNative } from './native.js'

/**
 * Create a new agent message with a computed content hash.
 */
export function createMessage(
  from: Uint8Array,
  to: Uint8Array,
  content: Uint8Array | string,
  timestamp?: bigint,
): AgentMessage {
  const contentBytes = typeof content === 'string'
    ? new Uint8Array(Buffer.from(content, 'utf-8'))
    : content
  const ts = timestamp ?? BigInt(Date.now())
  return getNative().createAgentMessage(from, to, contentBytes, ts)
}

/**
 * Encode a message to a portable byte format.
 */
export function encodeMessage(msg: AgentMessage): Uint8Array {
  return getNative().encodeAgentMessage(msg)
}

/**
 * Decode a message from bytes.
 */
export function decodeMessage(data: Uint8Array): AgentMessage {
  return getNative().decodeAgentMessage(data)
}

/**
 * Verify that a message's content hash matches its content.
 */
export function verifyMessageHash(msg: AgentMessage): boolean {
  return getNative().verifyMessageHash(msg)
}

/**
 * Check if a message is a broadcast (no specific recipient).
 */
export function isBroadcastMessage(msg: AgentMessage): boolean {
  return getNative().isBroadcastMessage(msg)
}

/**
 * Attach a gate verdict to a message.
 */
export function withGateVerdict(msg: AgentMessage, verdict: AgentMessage['jLensGate']): AgentMessage {
  return { ...msg, jLensGate: verdict ?? undefined }
}
