/**
 * J-Lens truth gate — audits message content before relay.
 *
 * In production mode, calls the J-Lens HTTP server POST /audit with
 * {text, model_id}. The server resolves model_id to a registered
 * endpoint + probe bank — clients never send paths or endpoints.
 *
 * Score semantics: `jlens.integrity.v1` — integrity in [0,1],
 * higher = healthier, 1.0 = no detections. Responses that do not
 * declare this tag are treated as unaudited (Yellow).
 *
 * In mock mode, uses deterministic keyword heuristics.
 */

import type { GateResult, GateVerdict } from './types.js'
import { GateVerdict as GateVerdictEnum } from './types.js'

export const SCORE_SEMANTICS = 'jlens.integrity.v1'

const MOCK_RED_KEYWORDS = [
  'deceptive', 'malicious', 'hack', 'exploit', 'manipulate', 'fraud', 'scam',
]

const MOCK_YELLOW_KEYWORDS = [
  'suspicious', 'questionable', 'unverified', 'uncertain',
]

export interface GateConfig {
  jlensEndpoint: string
  modelId?: string
  apiKey?: string
  /** integrity >= greenThreshold → Green */
  greenThreshold: number
  /** integrity >= yellowThreshold → Yellow; below → Red */
  yellowThreshold: number
  timeoutMs: number
  mock: boolean
}

export const defaultGateConfig: GateConfig = {
  jlensEndpoint: 'http://localhost:7777',
  greenThreshold: 0.30,
  yellowThreshold: 0.15,
  timeoutMs: 5000,
  mock: false,
}

interface AuditOutcome {
  verdict: GateVerdict
  /** audited integrity; undefined = unaudited */
  integrity?: number
}

/**
 * Audit a single message's content and return a gate verdict.
 */
export async function auditContent(
  content: Uint8Array,
  config: GateConfig,
): Promise<GateVerdict> {
  return (await auditContentScored(content, config)).verdict
}

/**
 * Audit a single message, returning verdict + audited integrity.
 */
export async function auditContentScored(
  content: Uint8Array,
  config: GateConfig,
): Promise<AuditOutcome> {
  if (config.mock) {
    return mockAudit(content)
  }

  try {
    const text = Buffer.from(content).toString('utf-8')
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), config.timeoutMs)

    const body: Record<string, string> = { text }
    if (config.modelId) body.model_id = config.modelId

    const resp = await fetch(`${config.jlensEndpoint}/audit`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...(config.apiKey ? { Authorization: config.apiKey } : {}),
      },
      body: JSON.stringify(body),
      signal: controller.signal,
    })
    clearTimeout(timeout)

    if (!resp.ok) {
      return { verdict: GateVerdictEnum.Yellow } // unaudited
    }

    const data = await resp.json() as {
      integrity?: number
      score_semantics?: string
      attestation_hash?: string
    }

    // Reject anything that doesn't declare the canonical semantics —
    // the risk-vs-integrity inversion must be impossible to wire silently.
    if (data.score_semantics !== SCORE_SEMANTICS || typeof data.integrity !== 'number') {
      return { verdict: GateVerdictEnum.Yellow } // unaudited
    }

    return { verdict: verdictFromIntegrity(data.integrity, config), integrity: data.integrity }
  } catch {
    // On error, conservatively return Yellow (unaudited)
    return { verdict: GateVerdictEnum.Yellow }
  }
}

function verdictFromIntegrity(integrity: number, config: GateConfig): GateVerdict {
  if (integrity < config.yellowThreshold) return GateVerdictEnum.Red
  if (integrity < config.greenThreshold) return GateVerdictEnum.Yellow
  return GateVerdictEnum.Green
}

/**
 * Audit a batch of messages and return an aggregate GateResult.
 *
 * Aggregate logic: Red if any Red, Yellow if any Yellow (no Red), Green otherwise.
 * `integrity` is the worst (lowest) audited score; undefined if nothing audited.
 */
export async function auditBatch(
  messages: { content: Uint8Array }[],
  config: GateConfig,
): Promise<GateResult> {
  let worstVerdict: GateVerdict = GateVerdictEnum.Green
  let minIntegrity: number | undefined
  let attestationHash: string | undefined
  let modelId: string | undefined

  for (const msg of messages) {
    const outcome = await auditContentScored(msg.content, config)
    if (outcome.verdict === GateVerdictEnum.Red) {
      worstVerdict = GateVerdictEnum.Red
    } else if (outcome.verdict === GateVerdictEnum.Yellow && worstVerdict !== GateVerdictEnum.Red) {
      worstVerdict = GateVerdictEnum.Yellow
    }
    if (outcome.integrity !== undefined) {
      minIntegrity = minIntegrity === undefined ? outcome.integrity : Math.min(minIntegrity, outcome.integrity)
    }
  }

  if (config.mock) {
    const crypto = await import('node:crypto')
    const hash = crypto.createHash('sha256')
    for (const msg of messages) {
      hash.update(msg.content)
    }
    hash.update('mock-attestation')
    attestationHash = hash.digest('hex')
    modelId = 'mock-jlens'
  }

  return {
    verdict: worstVerdict,
    integrity: minIntegrity,
    attestationHash,
    modelId,
  }
}

/**
 * Mock audit — deterministic keyword heuristics.
 */
function mockAudit(content: Uint8Array): AuditOutcome {
  const text = Buffer.from(content).toString('utf-8').toLowerCase()

  for (const keyword of MOCK_RED_KEYWORDS) {
    if (text.includes(keyword)) {
      return { verdict: GateVerdictEnum.Red, integrity: 0.05 }
    }
  }

  for (const keyword of MOCK_YELLOW_KEYWORDS) {
    if (text.includes(keyword)) {
      return { verdict: GateVerdictEnum.Yellow, integrity: 0.2 }
    }
  }

  return { verdict: GateVerdictEnum.Green }
}
