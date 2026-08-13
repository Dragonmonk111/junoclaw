/**
 * J-Lens truth gate — audits message content before relay.
 *
 * In production mode, calls the CSI HTTP server POST /audit.
 * In mock mode, uses deterministic keyword heuristics.
 */

import type { GateResult, GateVerdict } from './types.js'
import { GateVerdict as GateVerdictEnum } from './types.js'

const MOCK_RED_KEYWORDS = [
  'deceptive', 'malicious', 'hack', 'exploit', 'manipulate', 'fraud', 'scam',
]

const MOCK_YELLOW_KEYWORDS = [
  'suspicious', 'questionable', 'unverified', 'uncertain',
]

export interface GateConfig {
  csiEndpoint: string
  apiKey?: string
  yellowThreshold: number
  redThreshold: number
  timeoutMs: number
  mock: boolean
}

export const defaultGateConfig: GateConfig = {
  csiEndpoint: 'http://localhost:7777',
  yellowThreshold: 0.15,
  redThreshold: 0.35,
  timeoutMs: 5000,
  mock: false,
}

/**
 * Audit a single message's content and return a gate verdict.
 */
export async function auditContent(
  content: Uint8Array,
  config: GateConfig,
): Promise<GateVerdict> {
  if (config.mock) {
    return mockAudit(content)
  }

  try {
    const text = Buffer.from(content).toString('utf-8')
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), config.timeoutMs)

    const resp = await fetch(`${config.csiEndpoint}/audit`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...(config.apiKey ? { Authorization: config.apiKey } : {}),
      },
      body: JSON.stringify({ text }),
      signal: controller.signal,
    })
    clearTimeout(timeout)

    if (!resp.ok) {
      return GateVerdictEnum.Yellow
    }

    const data = await resp.json() as {
      separation_score: number
      attestation_hash?: string
    }

    if (data.separation_score >= config.redThreshold) {
      return GateVerdictEnum.Red
    } else if (data.separation_score >= config.yellowThreshold) {
      return GateVerdictEnum.Yellow
    }
    return GateVerdictEnum.Green
  } catch {
    // On error, conservatively return Yellow
    return GateVerdictEnum.Yellow
  }
}

/**
 * Audit a batch of messages and return an aggregate GateResult.
 *
 * Aggregate logic: Red if any Red, Yellow if any Yellow (no Red), Green otherwise.
 */
export async function auditBatch(
  messages: { content: Uint8Array }[],
  config: GateConfig,
): Promise<GateResult> {
  let worstVerdict: GateVerdict = GateVerdictEnum.Green
  let maxScore = 0
  let attestationHash: string | undefined
  let modelId: string | undefined

  for (const msg of messages) {
    const verdict = await auditContent(msg.content, config)
    if (verdict === GateVerdictEnum.Red) {
      worstVerdict = GateVerdictEnum.Red
      maxScore = Math.max(maxScore, 0.9)
    } else if (verdict === GateVerdictEnum.Yellow && worstVerdict !== GateVerdictEnum.Red) {
      worstVerdict = GateVerdictEnum.Yellow
      maxScore = Math.max(maxScore, 0.2)
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
    modelId = 'mock-csi'
  }

  return {
    verdict: worstVerdict,
    separationScore: maxScore,
    attestationHash,
    modelId,
  }
}

/**
 * Mock audit — deterministic keyword heuristics.
 */
function mockAudit(content: Uint8Array): GateVerdict {
  const text = Buffer.from(content).toString('utf-8').toLowerCase()

  for (const keyword of MOCK_RED_KEYWORDS) {
    if (text.includes(keyword)) {
      return GateVerdictEnum.Red
    }
  }

  for (const keyword of MOCK_YELLOW_KEYWORDS) {
    if (text.includes(keyword)) {
      return GateVerdictEnum.Yellow
    }
  }

  return GateVerdictEnum.Green
}
