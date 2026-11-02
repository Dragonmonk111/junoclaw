/**
 * Native addon loader with pure-JS fallback.
 *
 * Attempts to load the napi-rs native addon. If unavailable (e.g. during
 * development without a built addon), falls back to pure JavaScript
 * implementations of the core operations.
 */

import type {
  NativeAddon,
  AgentMessage,
  Batch,
  GateResult,
  GateVerdict,
  IntentMessage,
  ThoughtRef,
} from './types.js'
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
  integrity?: number,
  attestationHash?: string,
  modelId?: string,
): GateResult {
  return { verdict, integrity, attestationHash, modelId }
}

// ─── Intent / thought fallbacks ──────────────────────────────────────

/** Canonical compact JSON — sorted keys at every level, matching
 *  serde_json's BTreeMap ordering used by Rust action_hash. */
function canonJson(v: unknown): string {
  if (v === null || typeof v !== 'object') return JSON.stringify(v)
  if (Array.isArray(v)) return `[${v.map(canonJson).join(',')}]`
  const keys = Object.keys(v as Record<string, unknown>).sort()
  const parts = keys.map((k) => `${JSON.stringify(k)}:${canonJson((v as Record<string, unknown>)[k])}`)
  return `{${parts.join(',')}}`
}

function thoughtRefToSnake(t: ThoughtRef): Record<string, unknown> {
  return {
    schema: t.schema,
    score_semantics: t.scoreSemantics,
    snapshot_hash: t.snapshotHash,
    model_id: t.modelId,
    weights_hash: t.weightsHash,
    bank_hash: t.bankHash,
    action_hash: t.actionHash,
    activation_root: t.activationRoot,
    integrity: t.integrity,
    tier: t.tier,
    tee_attestation_ref: t.teeAttestationRef ?? null,
    storage_locator: t.storageLocator ?? null,
  }
}

function thoughtRefFromSnake(t: Record<string, unknown>): ThoughtRef {
  return {
    schema: t.schema as string,
    scoreSemantics: t.score_semantics as string,
    snapshotHash: t.snapshot_hash as string,
    modelId: t.model_id as string,
    weightsHash: t.weights_hash as string,
    bankHash: t.bank_hash as string,
    actionHash: t.action_hash as string,
    activationRoot: t.activation_root as string,
    integrity: t.integrity as number,
    tier: t.tier as ThoughtRef['tier'],
    teeAttestationRef: (t.tee_attestation_ref as string) ?? undefined,
    storageLocator: (t.storage_locator as string) ?? undefined,
  }
}

function jsEncodeIntentMessage(msg: IntentMessage): Uint8Array {
  const json = JSON.stringify({
    robot_id: msg.robotId,
    action: msg.action,
    params: JSON.parse(msg.paramsJson || 'null'),
    sensor_snapshot_hash: msg.sensorSnapshotHash,
    controller_timestamp: Number(msg.controllerTimestamp),
    rationale: msg.rationale ?? null,
    execution_proof_ref: msg.executionProofRef ?? null,
    ...(msg.thought ? { thought: thoughtRefToSnake(msg.thought) } : {}),
  })
  return new Uint8Array(Buffer.from(json, 'utf-8'))
}

function jsDecodeIntentMessage(data: Uint8Array): IntentMessage {
  const empty: IntentMessage = {
    robotId: '',
    action: '',
    paramsJson: 'null',
    sensorSnapshotHash: '',
    controllerTimestamp: 0n,
  }
  try {
    const j = JSON.parse(Buffer.from(data).toString('utf-8'))
    return {
      robotId: j.robot_id ?? '',
      action: j.action ?? '',
      paramsJson: canonJson(j.params ?? null),
      sensorSnapshotHash: j.sensor_snapshot_hash ?? '',
      controllerTimestamp: BigInt(j.controller_timestamp ?? 0),
      rationale: j.rationale ?? undefined,
      executionProofRef: j.execution_proof_ref ?? undefined,
      thought: j.thought ? thoughtRefFromSnake(j.thought) : undefined,
    }
  } catch {
    return empty
  }
}

function jsActionHashForIntent(msg: IntentMessage): string {
  const v = {
    action: msg.action,
    controller_timestamp: Number(msg.controllerTimestamp),
    params: JSON.parse(msg.paramsJson || 'null'),
    robot_id: msg.robotId,
    sensor_snapshot_hash: msg.sensorSnapshotHash,
  }
  const h = createHash('sha256')
  h.update(Buffer.from(canonJson(v), 'utf-8'))
  return h.digest('hex')
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
  encodeIntentMessage: jsEncodeIntentMessage,
  decodeIntentMessage: jsDecodeIntentMessage,
  actionHashForIntent: jsActionHashForIntent,
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
        encodeIntentMessage: native.encodeIntentMessage,
        decodeIntentMessage: native.decodeIntentMessage,
        actionHashForIntent: native.actionHashForIntent,
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
