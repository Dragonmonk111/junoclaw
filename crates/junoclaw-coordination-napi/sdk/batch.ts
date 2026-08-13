/**
 * Batch helpers — create, hash, inspect.
 */

import type { Batch, AgentMessage, GateResult } from './types.js'
import { getNative } from './native.js'

/**
 * Create a new batch of messages.
 */
export function createBatch(
  messages: AgentMessage[],
  prevHash: Uint8Array,
  height: bigint,
  timestamp?: bigint,
): Batch {
  const ts = timestamp ?? BigInt(Date.now())
  return getNative().createBatch(messages, prevHash, height, ts)
}

/**
 * Compute the SHA-256 hash of a batch.
 */
export function hashBatch(batch: Batch): Uint8Array {
  return getNative().hashBatch(batch)
}

/**
 * Check if any message in the batch has a red gate verdict.
 */
export function hasBlockedMessage(batch: Batch): boolean {
  return getNative().batchHasBlockedMessage(batch)
}

/**
 * Get the number of messages in a batch.
 */
export function batchLength(batch: Batch): bigint {
  return getNative().batchLen(batch)
}

/**
 * Attach a gate result to a batch.
 */
export function withGateResult(batch: Batch, result: GateResult): Batch {
  return { ...batch, gateResult: result }
}

/**
 * Filter out red-gated messages from a batch.
 */
export function filterBlocked(batch: Batch): Batch {
  return {
    ...batch,
    messages: batch.messages.filter((m) => m.jLensGate !== 'Red'),
  }
}
