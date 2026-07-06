// Deterministic gate verdicts, fixed order G1 -> G5 (spec §9). Every gate
// always runs and always produces a verdict object, even when there is
// nothing to check (verdict "pass", details noting why) — so gate output
// shape and order is stable across every trace, replayable byte-for-byte.

import { QUOTE_TRIGRAM_THRESHOLD } from './config.js'

function normalizeQuote(text = '') {
  return String(text).toLowerCase().replace(/\s+/g, ' ').trim()
}

function trigrams(text) {
  const s = normalizeQuote(text)
  if (s.length < 3) return new Set([s])
  const grams = new Set()
  for (let i = 0; i <= s.length - 3; i++) grams.add(s.slice(i, i + 3))
  return grams
}

function trigramJaccard(a, b) {
  const ga = trigrams(a)
  const gb = trigrams(b)
  if (!ga.size && !gb.size) return 1
  let intersection = 0
  for (const g of ga) if (gb.has(g)) intersection++
  const union = ga.size + gb.size - intersection
  return union ? intersection / union : 0
}

/**
 * G1 refsResolve: every refs[] and claims[].support[] entry: moult:/kmoult:
 * must exist in the local cache; proposal:/tx: -> warn (non-fatal in v0).
 */
export function checkRefsResolve({ refs = [], claims = [] }, entries) {
  const known = new Set(entries.map((e) => e.moult_id))
  const allRefs = [...refs, ...claims.flatMap((c) => c.support || [])]
  const details = []
  let verdict = 'pass'

  if (!allRefs.length) {
    details.push('no refs to check')
    return { gate: 'G1', verdict: 'pass', details }
  }

  for (const ref of allRefs) {
    if (ref.startsWith('moult:') || ref.startsWith('kmoult:')) {
      if (!known.has(ref)) {
        details.push(`unresolvable: ${ref}`)
        verdict = 'fail'
      }
    } else if (ref.startsWith('proposal:') || ref.startsWith('tx:')) {
      details.push(`unresolved-external: ${ref}`)
      if (verdict === 'pass') verdict = 'warn'
    } else {
      details.push(`unrecognized ref format: ${ref}`)
      verdict = 'fail'
    }
  }

  return { gate: 'G1', verdict, details }
}

/**
 * G2 quotesResolve: for each claim with a quote, normalized substring match
 * against each cited source's text; fallback trigram Jaccard >= 0.8.
 */
export function checkQuotesResolve({ claims = [] }, entries) {
  const byId = new Map(entries.map((e) => [e.moult_id, e]))
  const details = []
  let verdict = 'pass'

  const withQuotes = claims.filter((c) => c.quote)
  if (!withQuotes.length) {
    details.push('no quotes to check')
    return { gate: 'G2', verdict: 'pass', details }
  }

  for (const claim of withQuotes) {
    const normQuote = normalizeQuote(claim.quote)
    let found = false
    for (const ref of claim.support || []) {
      const source = byId.get(ref)
      if (!source) continue
      const normText = normalizeQuote(source.text || '')
      if (normText.includes(normQuote) || trigramJaccard(normQuote, normText) >= QUOTE_TRIGRAM_THRESHOLD) {
        found = true
        break
      }
    }
    if (!found) {
      details.push(`quote not found in any cited source: ${claim.claim_id || claim.claim}`)
      verdict = 'fail'
    }
  }

  return { gate: 'G2', verdict, details }
}

/**
 * G3 staleCheck: any cited source with stale.is_stale = true in cache fails
 * unless the run had include_stale.
 */
export function checkStale({ refs = [], claims = [], includeStale = false }, entries) {
  const byId = new Map(entries.map((e) => [e.moult_id, e]))
  const allRefs = new Set([...refs, ...claims.flatMap((c) => c.support || [])])
  const details = []

  if (!allRefs.size) {
    details.push('no cited sources to check')
    return { gate: 'G3', verdict: 'pass', details }
  }

  let verdict = 'pass'
  for (const ref of allRefs) {
    const source = byId.get(ref)
    if (source?.is_stale) {
      details.push(`stale source cited: ${ref}`)
      verdict = includeStale ? 'warn' : 'fail'
    }
  }

  return { gate: 'G3', verdict, details }
}

const ALLOWED_MIME_TYPES = new Set([
  'application/json+agent-reply',
  'application/json+agent-insight',
  'application/json+agent-proposal',
  'application/json+redmark',
  'application/json+unredmark',
])

/**
 * G4 schemaCheck: AKB export draft — akb_version "1.0", direction "export",
 * content.mime_type in the allowlist, non-empty refs[] and tags[].
 */
export function checkSchema(envelope) {
  if (!envelope) return { gate: 'G4', verdict: 'pass', details: ['no envelope to check'] }

  const details = []
  if (envelope.akb_version !== '1.0') details.push(`akb_version must be "1.0", got ${envelope.akb_version}`)
  if (envelope.direction !== 'export') details.push(`direction must be "export", got ${envelope.direction}`)
  if (!ALLOWED_MIME_TYPES.has(envelope.content?.mime_type)) details.push(`invalid mime_type: ${envelope.content?.mime_type}`)
  if (!envelope.refs?.length) details.push('refs[] must be non-empty')
  if (!envelope.tags?.length) details.push('tags[] must be non-empty')

  return { gate: 'G4', verdict: details.length ? 'fail' : 'pass', details }
}

/**
 * G5 policyCheck: requested action looked up in policy.json (green/yellow/
 * red); red -> fail. No executor exists for any action in this tool anyway.
 */
export function checkPolicy(action, policy) {
  if (!action) return { gate: 'G5', verdict: 'pass', details: ['no action requested'] }

  const level = policy?.[action]
  if (!level) return { gate: 'G5', verdict: 'fail', details: [`unknown action: ${action}`] }
  if (level === 'red') return { gate: 'G5', verdict: 'fail', details: [`red action requires explicit human approval: ${action}`] }

  return { gate: 'G5', verdict: 'pass', details: [`${action}: ${level}`] }
}

/**
 * Run all five gates in fixed order against a context. Any subset of
 * { refs, claims, includeStale, envelope, action, policy } may be supplied;
 * missing context yields a "pass" verdict with an explanatory detail so the
 * shape and order of the gates array is always identical.
 */
export function runGates({ refs = [], claims = [], includeStale = false, entries = [], envelope = null, action = null, policy = null }) {
  return [
    checkRefsResolve({ refs, claims }, entries),
    checkQuotesResolve({ claims }, entries),
    checkStale({ refs, claims, includeStale }, entries),
    checkSchema(envelope),
    checkPolicy(action, policy),
  ]
}
