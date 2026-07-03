// Redmark / stale resolution (Commonwealth Build Plan Phase 4).
//
// A redmark is just a normal Moultbook Post with content_type
// 'application/json+redmark' and refs: [target_id] (see akb-spec.md). An
// unredmark (content_type 'application/json+unredmark', same refs shape)
// reverses one — posted by anyone, not just the original marker, so a
// wrongly-honored redmark can be corrected without needing the original
// author's cooperation.
//
// Gating (resolution of the build plan's "Open decisions" — 2026-07-04):
// redmarks are advisory, not destructive (the underlying entry is never
// deleted, just excluded from default recall), so a full DAO vote per
// stale-flag would be disproportionate. Instead, a redmark/unredmark is only
// HONORED if its author's trust score (./trust.js) meets MIN_TRUST_SCORE —
// unknown/brand-new wallets cannot unilaterally hide content, but any
// established participant can, and any other established participant can
// undo it.
//
// Resolution: for each target, the most recent HONORED action (by
// posted_at) wins.

import { computeTrust } from './trust.js'

export const REDMARK_TYPE = 'application/json+redmark'
export const UNREDMARK_TYPE = 'application/json+unredmark'
export const MIN_TRUST_SCORE = Number(process.env.REDMARK_MIN_TRUST_SCORE || 10)

function nsToIso(ns) {
  if (!ns) return null
  try {
    const ms = BigInt(ns) / 1000000n
    return new Date(Number(ms)).toISOString()
  } catch {
    return null
  }
}

function targetOf(entry) {
  const ref = (entry.refs || [])[0]
  if (!ref) return null
  return ref.startsWith('moult:') ? ref : `moult:${ref}`
}

/**
 * Resolve current stale/not-stale state for every redmark target in the
 * index. Returns a Map<targetId, { isStale, by, atNs, redmarkId, actions }>.
 * `actions` includes both honored and un-honored attempts, for transparency.
 *
 * @param {object} index - the loaded indexer.js cache
 */
export function computeStaleMap(index) {
  const actionsByTarget = new Map()

  const sources = [
    [index.by_content_type[REDMARK_TYPE] || [], 'redmark'],
    [index.by_content_type[UNREDMARK_TYPE] || [], 'unredmark'],
  ]

  for (const [ids, type] of sources) {
    for (const id of ids) {
      const entry = index.by_id[id]
      if (!entry) continue
      const target = targetOf(entry)
      if (!target) continue

      const author = entry.author || entry.moult_key || null
      const trust = author ? computeTrust(index, author) : { score: 0 }
      const honored = trust.score >= MIN_TRUST_SCORE

      if (!actionsByTarget.has(target)) actionsByTarget.set(target, [])
      actionsByTarget.get(target).push({
        id,
        type,
        by: author,
        atNs: entry.posted_at || '0',
        trustScore: trust.score,
        honored,
      })
    }
  }

  const staleMap = new Map()
  for (const [target, actions] of actionsByTarget) {
    const honored = actions
      .filter((a) => a.honored)
      .sort((a, b) => {
        const ta = BigInt(a.atNs || '0')
        const tb = BigInt(b.atNs || '0')
        return ta > tb ? 1 : ta < tb ? -1 : 0
      })
    const latest = honored[honored.length - 1] || null

    staleMap.set(target, {
      isStale: latest ? latest.type === 'redmark' : false,
      by: latest?.by || null,
      atNs: latest?.atNs || null,
      redmarkId: latest?.id || null,
      actions,
    })
  }
  return staleMap
}

function normalizeId(id) {
  return id?.startsWith('moult:') ? id : `moult:${id}`
}

export function isStale(staleMap, id) {
  return Boolean(staleMap.get(normalizeId(id))?.isStale)
}

/**
 * Annotation object suitable for embedding in an AKB import envelope.
 */
export function staleInfo(staleMap, id) {
  const info = staleMap.get(normalizeId(id))
  if (!info) {
    return { is_stale: false, marked_by: null, at: null, redmark_id: null }
  }
  return {
    is_stale: info.isStale,
    marked_by: info.by,
    at: nsToIso(info.atNs),
    redmark_id: info.redmarkId,
  }
}
