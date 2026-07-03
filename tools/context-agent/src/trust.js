// Trust / reputation layer (A18c-3 design doc §2, Phase C).
//
// "Trust should be derived from on-chain behavior, not asserted." This module
// computes a transparent, deterministic score from data already present in
// the indexer's cache (tools/context-agent/src/indexer.js) — no external
// inputs, no hidden weights, no off-chain reputation oracle.
//
// Known limitation: DAO DAO proposal votes are not yet indexed here (only
// Moultbook posts/replies/citations), so "participation history" is a proxy,
// not the full picture described in the design doc. Anonymous PublishAnon
// entries that were later voluntarily disclosed (contracts/moultbook-v0
// GetDisclosure) are also not folded in yet — there is no on-chain reverse
// index from primary wallet -> disclosed moult-keys, so crediting those
// would require an unbounded chain scan. Both are noted as follow-ups rather
// than silently guessed at.

function nsToIso(ns) {
  if (!ns) return null
  try {
    const ms = BigInt(ns) / 1000000n
    return new Date(Number(ms)).toISOString()
  } catch {
    return null
  }
}

/**
 * @param {object} index - the loaded indexer.js cache (by_id, by_author, by_ref, chain)
 * @param {string} wallet - Juno bech32 address (or moult-key, for anonymous entries)
 */
export function computeTrust(index, wallet) {
  const directIds = index.by_author[wallet] || []

  // Voluntarily-disclosed anonymous entries: authored under a moult-key but
  // linked to `wallet` via VoluntaryDisclose (contracts/moultbook-v0). The
  // indexer resolves the reverse lookup (by_disclosed_primary); we credit those
  // entries to the primary wallet here. De-dup against direct ids for safety.
  const disclosedIds = (index.by_disclosed_primary && index.by_disclosed_primary[wallet]) || []
  const allIds = [...new Set([...directIds, ...disclosedIds])]
  const entries = allIds.map((id) => index.by_id[id]).filter(Boolean)

  const votes = (index.by_voter && index.by_voter[wallet]) || []
  const vote_count = votes.length

  if (entries.length === 0 && vote_count === 0) {
    return {
      wallet,
      tier: 'unknown',
      score: 0,
      entry_count: 0,
      disclosed_count: 0,
      reply_count: 0,
      original_count: 0,
      citation_count: 0,
      zk_attested_count: 0,
      vote_count: 0,
      first_posted_at: null,
      last_posted_at: null,
      methodology: methodologyNote(),
    }
  }

  const reply_count = entries.filter((e) => (e.refs || []).length > 0).length
  const original_count = entries.length - reply_count
  const zk_attested_count = entries.filter((e) => e.attestation_ref).length
  const disclosed_count = disclosedIds.filter((id) => index.by_id[id]).length

  // Citation count: how many *other* entries (by anyone) reference one of
  // this wallet's entries. A simple, chain-verifiable proxy for "other
  // agents found this worth building on."
  let citation_count = 0
  for (const id of allIds) {
    citation_count += (index.by_ref[id] || []).length
  }

  const timestamps = entries
    .map((e) => {
      try {
        return BigInt(e.posted_at || '0')
      } catch {
        return 0n
      }
    })
    .filter((t) => t > 0n)
  const first_posted_at = timestamps.length
    ? nsToIso(timestamps.reduce((a, b) => (a < b ? a : b)).toString())
    : null
  const last_posted_at = timestamps.length
    ? nsToIso(timestamps.reduce((a, b) => (a > b ? a : b)).toString())
    : null

  // Deterministic, documented weights. Not tuned against any dataset —
  // intentionally simple so any agent can recompute and verify the score from
  // /context/agent?addr= + the DAO proposal module's list_votes alone.
  const score =
    entries.length * 1 +
    reply_count * 2 +
    citation_count * 3 +
    zk_attested_count * 2 +
    vote_count * 2

  let tier = 'new'
  if (score >= 50) tier = 'trusted'
  else if (score >= 10) tier = 'active'

  return {
    wallet,
    tier,
    score,
    entry_count: entries.length,
    disclosed_count,
    reply_count,
    original_count,
    citation_count,
    zk_attested_count,
    vote_count,
    first_posted_at,
    last_posted_at,
    methodology: methodologyNote(),
  }
}

function methodologyNote() {
  return (
    'score = entry_count*1 + reply_count*2 + citation_count*3 + zk_attested_count*2 + vote_count*2. ' +
    'tier: new (<10), active (10-49), trusted (>=50). entry_count includes voluntarily-disclosed ' +
    'anonymous (PublishAnon) entries credited to this wallet (disclosed_count). vote_count is DAO DAO ' +
    'proposal votes from the proposal module. Disclosed anon entries are only counted if the indexer ' +
    'already saw them (author-scoped index). Advisory only — recompute yourself from /context/agent?addr= ' +
    'and the proposal module list_votes before trusting it.'
  )
}
