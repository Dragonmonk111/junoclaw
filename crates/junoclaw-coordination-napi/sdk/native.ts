/**
 * Native addon loader with pure-JS fallback.
 *
 * Attempts to load the napi-rs native addon. If unavailable (e.g. during
 * development without a built addon), falls back to pure JavaScript
 * implementations of the core operations.
 */

import type { NativeAddon, AgentMessage, Batch, GateResult, GateVerdict } from './types.js'
import { GateVerdict as GateVerdictEnum } from './types.js'
import { createHash } from 'node:crypto'

// ─── Pure-JS fallback implementations ────────────────────────────────

function sha256(data: Uint8Array): Uint8Array {
  const h = createHash('sha256')
  h.update(Buffer.from(data))
  return new Uint8Array(h.digest())
}

function jsCreateAgentMessage(
  from: Uint8Array,
  to: Uint8Array,
  content: Uint8Array,
  timestamp: bigint,
): AgentMessage {
  const contentHash = sha256(content)
  return { from, to, content, contentHash, timestamp }
}

function jsEncodeAgentMessage(msg: AgentMessage): Uint8Array {
  const json = JSON.stringify({
    from: Buffer.from(msg.from).toString('hex'),
    to: Buffer.from(msg.to).toString('hex'),
    content: Buffer.from(msg.content).toString('hex'),
    content_hash: Buffer.from(msg.contentHash).toString('hex'),
    timestamp: Number(msg.timestamp),
    j_lens_gate: msg.jLensGate ?? null,
    proposal_ref: msg.proposalRef ? Number(msg.proposalRef) : null,
  })
  return new Uint8Array(Buffer.from(json, 'utf-8'))
}

function jsDecodeAgentMessage(data: Uint8Array): AgentMessage {
  try {
    const json = JSON.parse(Buffer.from(data).toString('utf-8'))
    return {
      from: new Uint8Array(Buffer.from(json.from, 'hex')),
      to: new Uint8Array(Buffer.from(json.to, 'hex')),
      content: new Uint8Array(Buffer.from(json.content, 'hex')),
      contentHash: new Uint8Array(Buffer.from(json.content_hash, 'hex')),
      timestamp: BigInt(json.timestamp),
      jLensGate: json.j_lens_gate ?? undefined,
      proposalRef: json.proposal_ref != null ? BigInt(json.proposal_ref) : undefined,
    }
  } catch {
    return jsCreateAgentMessage(new Uint8Array(0), new Uint8Array(0), new Uint8Array(0), 0n)
  }
}

function jsVerifyMessageHash(msg: AgentMessage): boolean {
  const expected = sha256(msg.content)
  return Buffer.from(msg.contentHash).equals(Buffer.from(expected))
}

function jsIsBroadcastMessage(msg: AgentMessage): boolean {
  return msg.to.length === 0 || msg.to.every((b) => b === 0)
}

function jsCreateBatch(
  messages: AgentMessage[],
  prevHash: Uint8Array,
  height: bigint,
  timestamp: bigint,
): Batch {
  return { messages, prevHash, height, timestamp }
}

function jsHashBatch(batch: Batch): Uint8Array {
  const parts: Buffer[] = [Buffer.from(batch.prevHash)]
  for (const msg of batch.messages) {
    parts.push(Buffer.from(msg.contentHash))
  }
  const heightBuf = Buffer.alloc(8)
  heightBuf.writeBigUInt64LE(batch.height)
  parts.push(heightBuf)
  const tsBuf = Buffer.alloc(8)
  tsBuf.writeBigUInt64LE(batch.timestamp)
  parts.push(tsBuf)
  const h = createHash('sha256')
  for (const p of parts) h.update(p)
  return new Uint8Array(h.digest())
}

function jsBatchHasBlockedMessage(batch: Batch): boolean {
  return batch.messages.some((m) => m.jLensGate === GateVerdictEnum.Red)
}

function jsBatchLen(batch: Batch): bigint {
  return BigInt(batch.messages.length)
}

function jsCreateGateResult(
  verdict: GateVerdict,
  separationScore: number,
  attestationHash?: string,
  modelId?: string,
): GateResult {
  return { verdict, separationScore, attestationHash, modelId }
}

// ─── Native loader ────────────────────────────────────────────────────

const fallback: NativeAddon = {
  createAgentMessage: jsCreateAgentMessage,
  encodeAgentMessage: jsEncodeAgentMessage,
  decodeAgentMessage: jsDecodeAgentMessage,
  verifyMessageHash: jsVerifyMessageHash,
  isBroadcastMessage: jsIsBroadcastMessage,
  createBatch: jsCreateBatch,
  hashBatch: jsHashBatch,
  batchHasBlockedMessage: jsBatchHasBlockedMessage,
  batchLen: jsBatchLen,
  createGateResult: jsCreateGateResult,
}

let _native: NativeAddon | null = null
let _tried = false

/**
 * Get the native addon if available, otherwise return the JS fallback.
 */
export function getNative(): NativeAddon {
  if (_native) return _native
  if (!_tried) {
    _tried = true
    try {
      // Try to load the native addon (built by napi-rs)
      const native = require('../index.js')
      _native = {
        createAgentMessage: native.createAgentMessage,
        encodeAgentMessage: native.encodeAgentMessage,
        decodeAgentMessage: native.decodeAgentMessage,
        verifyMessageHash: native.verifyMessageHash,
        isBroadcastMessage: native.isBroadcastMessage,
        createBatch: native.createBatch,
        hashBatch: native.hashBatch,
        batchHasBlockedMessage: native.batchHasBlockedMessage,
        batchLen: native.batchLen,
        createGateResult: native.createGateResult,
      }
    } catch {
      // Native addon not available, use fallback
      _native = fallback
    }
  }
  return _native ?? fallback
}

/**
 * Check if the native addon is loaded.
 */
export function isNativeLoaded(): boolean {
  return getNative() !== fallback
}
