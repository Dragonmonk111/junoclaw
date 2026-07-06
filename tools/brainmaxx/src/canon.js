// Canonical serialization for Brainmaxx v0.
//
// Two profiles, both versioned (spec §4):
//
//  - canonV1(x): internal hashing profile. Recursively sort object keys,
//    NFC-normalize strings, JSON.stringify with no whitespace, UTF-8 bytes.
//    Used for run_id, pack_hash, claim_id, config_hash, corpus_snapshot_hash.
//
//  - Envelope-commitment profile: JSON.stringify(envelope, null, 2), exact
//    bytes, unsorted keys, insertion order — byte-parity with
//    buildAkbExportPost / mirrorExportToFile in
//    tools/reply-bot/src/moultbook.js, so the commitment previewed by
//    `brainmaxx moult-draft` equals what reply-bot will commit on-chain for
//    the same file (locked by test T6).

import { createHash } from 'node:crypto'

export const CANON_VERSION = 1

/**
 * Recursively normalize a value for canonical hashing:
 *  - arrays: map each element, preserve order (order is semantic)
 *  - objects: sort keys ascending, recurse into values
 *  - strings: NFC-normalize
 *  - other primitives: pass through unchanged
 */
function normalize(value) {
  if (value === null || value === undefined) return null
  if (typeof value === 'string') return value.normalize('NFC')
  if (typeof value === 'number' || typeof value === 'boolean') return value
  if (Array.isArray(value)) return value.map(normalize)
  if (typeof value === 'object') {
    const sortedKeys = Object.keys(value).sort()
    const out = {}
    for (const key of sortedKeys) {
      const v = value[key]
      if (v === undefined) continue
      out[key] = normalize(v)
    }
    return out
  }
  return value
}

/**
 * canonV1(x) -> UTF-8 Buffer of the canonical JSON bytes for x.
 * Deterministic: same logical value always produces the same bytes,
 * regardless of original key insertion order or platform.
 */
export function canonV1Bytes(value) {
  const normalized = normalize(value)
  return Buffer.from(JSON.stringify(normalized), 'utf8')
}

/** canonV1(x) -> canonical JSON string (no whitespace, sorted keys). */
export function canonV1(value) {
  return canonV1Bytes(value).toString('utf8')
}

export function sha256hex(bytesOrString) {
  const buf = Buffer.isBuffer(bytesOrString) ? bytesOrString : Buffer.from(String(bytesOrString), 'utf8')
  return createHash('sha256').update(buf).digest('hex')
}

export function sha256b64(bytesOrString) {
  const buf = Buffer.isBuffer(bytesOrString) ? bytesOrString : Buffer.from(String(bytesOrString), 'utf8')
  return createHash('sha256').update(buf).digest('base64')
}

/** sha256hex(canonV1(value)) — the common composition used throughout. */
export function canonHash(value) {
  return sha256hex(canonV1Bytes(value))
}

/**
 * Envelope-commitment profile: byte-parity with reply-bot's
 * buildAkbExportPost, which does sha256Base64(JSON.stringify(envelope, null, 2)).
 * Unsorted, insertion-order, pretty-printed with 2-space indent.
 */
export function envelopeCommitmentBytes(envelope) {
  return Buffer.from(JSON.stringify(envelope, null, 2), 'utf8')
}

export function envelopeCommitment(envelope) {
  return sha256b64(envelopeCommitmentBytes(envelope))
}
