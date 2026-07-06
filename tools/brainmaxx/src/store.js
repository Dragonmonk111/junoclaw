// Reads the existing local-file-bridge JSONL cache (spec §5). Brainmaxx does
// not invent a new sync mechanism or store format — it is a read-only
// consumer of memory/agent-bridge/<namespace>.jsonl, the same file
// tools/context-agent/bridges/local-file-bridge.js writes.

import { existsSync, readFileSync } from 'node:fs'
import { sha256hex } from './canon.js'

/**
 * Read raw JSONL rows, skipping malformed lines (same tolerance as the
 * bridge itself).
 */
function readRawRows(path) {
  if (!existsSync(path)) return []
  const rows = []
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (!line.trim()) continue
    try {
      rows.push(JSON.parse(line))
    } catch {
      // skip a malformed line rather than crash the whole read
    }
  }
  return rows
}

/** Extract entry text regardless of whether the row nests it under content. */
function entryText(row) {
  return row.text ?? row.content?.text ?? ''
}

function entryMoultId(row) {
  return row.moult_id || row.mother_moult_id || null
}

function entryIsStale(row) {
  return Boolean(row.stale?.is_stale)
}

/**
 * Load the store: dedupe by moult_id keeping the LAST occurrence (matches
 * bridge re-sync behavior — a later line for the same id supersedes an
 * earlier one), drop rows with no moult_id (can't be cited or deduped).
 * Returns entries sorted by moult_id ascending for a stable iteration order.
 */
export function loadStore(path) {
  const rows = readRawRows(path)
  const byId = new Map()
  for (const row of rows) {
    const id = entryMoultId(row)
    if (!id) continue
    byId.set(id, row)
  }

  const entries = [...byId.values()]
    .map((row) => {
      const text = entryText(row)
      return {
        moult_id: entryMoultId(row),
        author_wallet: row.author_wallet || row.author?.wallet || null,
        author_alias: row.author_alias || row.author?.alias || null,
        mime_type: row.mime_type || row.content?.mime_type || null,
        text,
        tags: row.tags || [],
        verified: Boolean(row.verified),
        is_stale: entryIsStale(row),
        content_sha256: sha256hex(Buffer.from(text ?? '', 'utf8')),
      }
    })
    .sort((a, b) => (a.moult_id < b.moult_id ? -1 : a.moult_id > b.moult_id ? 1 : 0))

  return entries
}

/**
 * corpus_snapshot_hash = sha256hex(utf8(lines.join('\n'))) where lines =
 * sorted ascending "${moult_id} ${content_sha256}" (spec §5). Stable under
 * JSONL line-order permutation and re-sync dedupe (T4).
 */
export function corpusSnapshotHash(entries) {
  const lines = entries
    .map((e) => `${e.moult_id} ${e.content_sha256}`)
    .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))
  return sha256hex(Buffer.from(lines.join('\n'), 'utf8'))
}

export function loadSnapshot(path) {
  const entries = loadStore(path)
  return { entries, corpus_snapshot_hash: corpusSnapshotHash(entries), count: entries.length }
}
