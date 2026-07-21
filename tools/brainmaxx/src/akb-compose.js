// AKB export draft generator (spec §10). Writes a draft file for a human
// operator to hand to tools/reply-bot; never posts, never signs, never
// touches JUNO_REPLY_BOT_MNEMONIC.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { envelopeCommitment } from './canon.js'

function draftFilename(run_id) {
  return `${run_id.replace(/:/g, '_')}.envelope.json`
}

function traceDraftFilename(run_id) {
  return `${run_id.replace(/:/g, '_')}.trace.envelope.json`
}

export function draftFilePath(brainmaxxDir, run_id) {
  return join(brainmaxxDir, 'drafts', draftFilename(run_id))
}

export function traceDraftFilePath(brainmaxxDir, run_id) {
  return join(brainmaxxDir, 'drafts', traceDraftFilename(run_id))
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

/** Build an AKB export envelope for the entire Brainmaxx trace (not just the insight). */
export function composeTraceEnvelope(trace, { motherMoultId, authorWallet, authorAlias, tags = [] } = {}) {
  const refs = refsFromTrace(trace)
  if (!refs.length) throw new Error('cannot compose a trace envelope with no refs')

  const gateLines = (trace.gates || []).map((g) => `- ${g.gate}: ${g.verdict}`).join('\n') || '(no gates recorded)'
  const sourceLines = (trace.pack?.items || []).map((i) => `- ${i.moult_id} (${i.author || 'unknown'}, ${i.mime_type || 'unknown'})`).join('\n') || '(none)'

  const text = [
    `# Brainmaxx trace export — ${trace.run_id}`,
    '',
    `**Objective:** ${trace.objective}`,
    `**Corpus snapshot:** ${trace.corpus_snapshot_hash}`,
    `**Pack hash:** ${trace.pack?.pack_hash || 'none'}`,
    `**Determinism profile:** ${trace.determinism_profile || 'D0'}`,
    '',
    '## Gate verdicts',
    '',
    gateLines,
    '',
    '## Cited sources',
    '',
    sourceLines,
    '',
    'Full structured trace is embedded in `content.structured.trace`.',
  ].join('\n')

  return {
    akb_version: '1.0',
    direction: 'export',
    ...(motherMoultId ? { mother_moult_id: motherMoultId } : {}),
    author: { wallet: authorWallet || null, alias: authorAlias || null, type: 'agent' },
    content: {
      mime_type: 'application/json+brainmaxx-trace',
      text,
      structured: {
        type: 'brainmaxx-trace',
        trace,
      },
    },
    refs,
    tags: ['brainmaxx', 'trace', 'j-reef', 'replayable', ...tags],
    memory_ops: { remember: [], stale: [] },
  }
}

export function writeTraceExport(brainmaxxDir, trace, envelope) {
  const path = traceDraftFilePath(brainmaxxDir, trace.run_id)
  mkdirSync(join(brainmaxxDir, 'drafts'), { recursive: true })
  writeFileSync(path, JSON.stringify(envelope, null, 2), 'utf8')
  return { path, commitment: envelopeCommitment(envelope) }
}
