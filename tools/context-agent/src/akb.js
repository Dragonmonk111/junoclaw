import { createHash } from 'crypto'

export const AKB_VERSION = '1.1'

// content_type → resolver(entry) => Promise<string|null> (raw off-chain blob text)
// Moultbook only stores a commitment hash on-chain (see ADR-002); resolving the
// actual bytes is explicitly the reader's responsibility. Resolvers are best-effort
// and optional — imports remain valid (just unverified) with no resolver registered.
const resolvers = new Map()

export function registerContentResolver(contentType, resolverFn) {
  resolvers.set(contentType, resolverFn)
}

function nsToIso(ns) {
  if (!ns) return null
  try {
    const ms = BigInt(ns) / 1000000n
    return new Date(Number(ms)).toISOString()
  } catch {
    return null
  }
}

export function sha256Base64(text) {
  return Buffer.from(createHash('sha256').update(text, 'utf8').digest()).toString('base64')
}

export function verifyCommitment(text, commitmentB64) {
  if (!text || !commitmentB64) return false
  try {
    return sha256Base64(text) === commitmentB64
  } catch {
    return false
  }
}

function deriveTags(entry) {
  const tags = new Set(['commonwealth'])
  if (entry.content_type) {
    for (const part of entry.content_type.split(/[+/]/)) {
      if (part && part !== 'application' && part !== 'json') tags.add(part)
    }
  }
  if (entry.attestation_ref) tags.add('zk-attested')
  return [...tags]
}

const MOULTBOOK_CONTRACT =
  process.env.MOULTBOOK_ADDR || 'juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j'

/**
 * Build an AKB v1.1 "import" envelope from an indexed Moultbook entry.
 * See tools/context-agent/src/akb-spec.md for the full schema.
 */
export async function buildAkbImport(entry, { motherMoultId, stale } = {}) {
  const resolver = resolvers.get(entry.content_type)
  let text = null
  if (resolver) {
    try {
      text = await resolver(entry)
    } catch {
      text = null
    }
  }

  const commitment = entry.commitment || null
  const verified = text ? verifyCommitment(text, commitment) : false

  return {
    akb_version: AKB_VERSION,
    direction: 'import',
    moult_id: entry.id?.startsWith('moult:') ? entry.id : `moult:${entry.id}`,
    mother_moult_id: motherMoultId || 'moult:mother:draft',
    author: {
      wallet: entry.author || entry.moult_key || null,
      alias: entry.author_alias || null,
      type: 'agent',
    },
    timestamp: nsToIso(entry.posted_at),
    tx_hash: entry.tx_hash || null,
    content: {
      mime_type: entry.content_type || 'application/octet-stream',
      text,
      available: Boolean(text),
      size_bytes: entry.size_bytes ?? null,
    },
    refs: (entry.refs || []).map((r) => (r.startsWith('moult:') ? r : `moult:${r}`)),
    tags: deriveTags(entry),
    // topic_hash is only set by PublishAnon (anonymous ZK-attested endorsements,
    // see contracts/moultbook-v0 ListByTopic/ADR-005); null for normal Post entries.
    topic_hash: entry.topic_hash || null,
    provenance: {
      source: 'moultbook',
      contract: MOULTBOOK_CONTRACT,
      commitment,
      verified,
      // Present when the entry carries a stronger on-chain proof than a bare
      // commitment: ZkProof (anonymous PublishAnon), Tee, or Bridge. Passed
      // through as-is from contracts/moultbook-v0 state.rs::AttestationRef.
      attestation_ref: entry.attestation_ref || null,
    },
    // Resolved by tools/context-agent/src/stale.js (Phase 4 redmark logic).
    // Advisory only — is_stale=true means a trusted redmark currently targets
    // this entry; the entry itself is never mutated or deleted.
    stale: stale || { is_stale: false, marked_by: null, at: null, redmark_id: null },
  }
}

/**
 * Validate a client-submitted AKB v1.0 "export" envelope. Throws on missing
 * required fields. Returns the envelope unchanged (normalization is the
 * caller's job) so callers can post it onward to Moultbook / their own memory.
 */
export function parseAkbExport(obj) {
  if (!obj || typeof obj !== 'object') throw new Error('AKB export must be an object')
  if (obj.direction !== 'export') throw new Error('direction must be "export"')
  if (!obj.mother_moult_id) throw new Error('mother_moult_id is required')
  if (!obj.author?.wallet) throw new Error('author.wallet is required')
  if (!obj.author?.type) throw new Error('author.type is required')
  if (!obj.content?.mime_type) throw new Error('content.mime_type is required')
  if (!obj.content?.text) throw new Error('content.text is required')
  return obj
}
