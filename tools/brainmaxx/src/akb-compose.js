// AKB export draft generator (spec §10). Writes a draft file for a human
// operator to hand to tools/reply-bot; never posts, never signs, never
// touches JUNO_REPLY_BOT_MNEMONIC.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { envelopeCommitment } from './canon.js'

function draftFilename(run_id) {
  return `${run_id.replace(/:/g, '_')}.envelope.json`
}

export function draftFilePath(brainmaxxDir, run_id) {
  return join(brainmaxxDir, 'drafts', draftFilename(run_id))
}

/** Read the text of the most recently attached llm-draft, if any. */
function attachedDraftText(trace) {
  const drafts = (trace.attachments || []).filter((a) => a.role === 'llm-draft')
  if (!drafts.length) return null
  const last = drafts[drafts.length - 1]
  if (!existsSync(last.path)) throw new Error(`attached draft file not found: ${last.path}`)
  return readFileSync(last.path, 'utf8')
}

/** Deterministic fallback insight text built from claims alone, when no D2 draft is attached. */
function claimsSummaryText(trace) {
  const lines = (trace.claims || []).map((c) => `- ${c.claim} (support: ${(c.support || []).join(', ') || 'none'})`)
  return [`Objective: ${trace.objective}`, '', 'Claims:', ...lines].join('\n')
}

function refsFromTrace(trace) {
  if (trace.claims?.length) {
    return [...new Set(trace.claims.flatMap((c) => c.support || []))]
  }
  return (trace.pack?.items || []).map((i) => i.moult_id)
}

export function composeEnvelope(trace, { motherMoultId, authorWallet, authorAlias, tags = [] } = {}) {
  const text = attachedDraftText(trace) ?? claimsSummaryText(trace)
  if (!text || !text.trim()) throw new Error('cannot compose an envelope with empty content.text — attach a draft or add claims first')

  const refs = refsFromTrace(trace)
  if (!refs.length) throw new Error('cannot compose an envelope with no refs — pack or claims must cite at least one moult')

  const envelope = {
    akb_version: '1.0',
    direction: 'export',
    ...(motherMoultId ? { mother_moult_id: motherMoultId } : {}),
    author: { wallet: authorWallet || null, alias: authorAlias || null, type: 'agent' },
    content: {
      mime_type: 'application/json+agent-insight',
      text,
      structured: {
        type: 'brainmaxx-insight',
        objective: trace.objective,
        claims: trace.claims || [],
        limitations: [],
        determinism_profile: trace.determinism_profile,
        run_id: trace.run_id,
      },
    },
    refs,
    tags: ['brainmaxx', 'reef', 'agent-cognition', ...tags],
    memory_ops: { remember: [], stale: [] },
  }

  return envelope
}

export function writeDraft(brainmaxxDir, trace, envelope) {
  const path = draftFilePath(brainmaxxDir, trace.run_id)
  mkdirSync(join(brainmaxxDir, 'drafts'), { recursive: true })
  writeFileSync(path, JSON.stringify(envelope, null, 2), 'utf8')
  return { path, commitment: envelopeCommitment(envelope) }
}
