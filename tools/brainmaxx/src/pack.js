// Deterministic source-pack builder (spec §7). A "pack" is the cited,
// ranked evidence bundle handed to the D2 (generative) stage, or printed
// directly by `brainmaxx recall`.

import { canonHash } from './canon.js'
import { rank } from './rank.js'
import { DEFAULT_K, MIN_SCORE } from './config.js'

const EXCERPT_LEN = 280

/**
 * Build a source pack for a single query against the loaded store entries.
 * Stale entries are excluded unless include_stale (G3 mirrors this at gate
 * time, spec §9). Scores below minScore are dropped (the "humility test").
 * Scores are serialized as toFixed(6) strings to eliminate float-formatting
 * drift across platforms/engines.
 */
export function buildPack(entries, query, { k = DEFAULT_K, mode = 'lexical', includeStale = false, minScore = MIN_SCORE } = {}) {
  const candidates = includeStale ? entries : entries.filter((e) => !e.is_stale)
  const ranked = rank(candidates, query, { mode })
  const filtered = ranked.filter((r) => r.score >= minScore)
  const top = filtered.slice(0, k)

  const items = top.map(({ entry, score }) => ({
    moult_id: entry.moult_id,
    score: score.toFixed(6),
    content_sha256: entry.content_sha256,
    author: entry.author_alias || entry.author_wallet || null,
    mime_type: entry.mime_type,
    excerpt: (entry.text || '').slice(0, EXCERPT_LEN),
  }))

  return {
    k,
    mode,
    include_stale: includeStale,
    items,
    pack_hash: canonHash(items),
  }
}
