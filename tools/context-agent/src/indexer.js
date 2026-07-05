import { existsSync, mkdirSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath, pathToFileURL } from 'url'
import { listByAuthor, listByRef, getStats, getDisclosure } from './moultbook.js'
import { buildVoteIndex } from './dao.js'

const __dirname = dirname(fileURLToPath(import.meta.url))
export const CACHE_DIR = process.env.CACHE_DIR || join(__dirname, '..', 'cache')
export const INDEX_FILE = join(CACHE_DIR, 'index.json')

export const HEARTBEAT_AUTHOR = process.env.HEARTBEAT_AUTHOR || 'juno17nmczzsfycwn74z2yrxqe7fc96033e7rm2gut6'
// moultbook-v0 has no global "list all entries" query (see contracts/moultbook-v0/src/msg.rs
// QueryMsg — only ListByAuthor / ListByRef / ListByMoultKey / ListByTopic), so this indexer
// can only discover entries by crawling from a known author's own posts plus whatever replies
// (transitively) into that tree. An AKB export that refs the Mother-Moult directly instead of
// replying into the heartbeat tree — e.g. tools/reply-bot's own insight/redmark/proposal posts —
// would otherwise never be found. EXTRA_SEED_AUTHORS widens the crawl root beyond the heartbeat
// watcher to any other known DAO-agent wallets. Once junoclaw-agent-registry has registered
// members (empty today), ListAgents there would be a more general source for this same seed set.
export const EXTRA_SEED_AUTHORS = (
  process.env.EXTRA_SEED_AUTHORS || 'juno1r7g6q3lwkzedxgjae7alvc8x0848dgjyzllat7'
)
  .split(',')
  .map((a) => a.trim())
  .filter(Boolean)
export const PAGE_LIMIT = 30

async function fetchAllByAuthor(author) {
  const entries = []
  let startAfter = null
  let page = 0

  while (true) {
    const resp = await listByAuthor(author, startAfter, PAGE_LIMIT)
    const pageEntries = resp.entries || []
    if (pageEntries.length === 0) break

    entries.push(...pageEntries)
    console.log(`[indexer] page ${++page}: fetched ${pageEntries.length} entries`)

    if (pageEntries.length < PAGE_LIMIT) break
    startAfter = pageEntries[pageEntries.length - 1].id
  }

  return entries
}

export function buildIndex(entries, stats = null) {
  const byId = {}
  const byTopic = {}
  const byAuthor = {}
  const byContentType = {}
  const byRef = {}
  const chain = []

  for (const entry of entries) {
    byId[entry.id] = entry

    const author = entry.author || entry.moult_key || '_unknown'
    ;(byAuthor[author] ||= []).push(entry.id)

    if (entry.topic_hash) {
      ;(byTopic[entry.topic_hash] ||= []).push(entry.id)
    }

    const ct = entry.content_type || 'application/octet-stream'
    ;(byContentType[ct] ||= []).push(entry.id)

    for (const ref of entry.refs || []) {
      ;(byRef[ref] ||= []).push(entry.id)
    }

    chain.push(entry.id)
  }

  // Sort chain by posted_at descending. posted_at is a nanosecond timestamp
  // (too large for Date), so compare as BigInt.
  chain.sort((a, b) => {
    const ta = BigInt(byId[a].posted_at || '0')
    const tb = BigInt(byId[b].posted_at || '0')
    return tb > ta ? 1 : tb < ta ? -1 : 0
  })

  const index = {
    meta: {
      indexed_at: new Date().toISOString(),
      author: HEARTBEAT_AUTHOR,
      entry_count: entries.length,
      moultbook: process.env.MOULTBOOK_ADDR || 'juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j',
    },
    by_id: byId,
    by_topic: byTopic,
    by_author: byAuthor,
    by_content_type: byContentType,
    by_ref: byRef,
    chain,
    // Augmented asynchronously by fetchIndex (best-effort); empty here so the
    // shape is stable for direct buildIndex() callers (tests, etc.).
    disclosures: {},
    by_disclosed_primary: {},
    by_voter: {},
  }

  if (stats) index.meta.moultbook_stats = stats
  return index
}

// Best-effort: for each indexed anonymous (PublishAnon / ZkProof-attested)
// entry, ask the contract whether its moult-key was voluntarily disclosed to a
// primary identity, and build the reverse index primary_wallet -> [entry_id].
// The on-chain DISCLOSURES map is keyed by entry_id only, so this is the reverse
// lookup the trust layer needs. Only covers entries already in the index.
async function augmentDisclosures(index, entries) {
  const disclosures = {}
  const byPrimary = {}
  const anon = entries.filter((e) => e.attestation_ref?.zk_proof)
  for (const e of anon) {
    try {
      const d = await getDisclosure(e.id)
      const primary = d?.primary_key
      if (primary) {
        disclosures[e.id] = primary
        ;(byPrimary[primary] ||= []).push(e.id)
      }
    } catch {
      // best-effort; a failed disclosure lookup must not break indexing
    }
  }
  index.disclosures = disclosures
  index.by_disclosed_primary = byPrimary
  console.log(`[indexer] disclosures: ${Object.keys(disclosures).length} of ${anon.length} anon entries`)
}

async function augmentVotes(index) {
  try {
    const { by_voter, proposal_count } = await buildVoteIndex()
    index.by_voter = by_voter
    index.meta.dao_proposal_count = proposal_count
    console.log(`[indexer] votes: ${Object.keys(by_voter).length} voters across ${proposal_count} proposals`)
  } catch (e) {
    console.warn('[indexer] vote index failed:', e.message)
    index.by_voter = {}
  }
}

async function fetchAllReplies(entryIds) {
  const replies = []
  const seen = new Set(entryIds)
  const queue = [...entryIds]
  let checked = 0

  while (queue.length > 0) {
    const id = queue.shift()
    let startAfter = null
    while (true) {
      const resp = await listByRef(id, startAfter, PAGE_LIMIT)
      const pageEntries = resp.entries || []
      if (pageEntries.length === 0) break

      for (const entry of pageEntries) {
        if (!seen.has(entry.id)) {
          seen.add(entry.id)
          replies.push(entry)
          queue.push(entry.id)
        }
      }

      if (pageEntries.length < PAGE_LIMIT) break
      startAfter = pageEntries[pageEntries.length - 1].id
    }
    checked++
  }

  console.log(`[indexer] fetched ${replies.length} unique replies from ${checked} entries`)
  return replies
}

export async function fetchIndex() {
  const seedAuthors = [HEARTBEAT_AUTHOR, ...EXTRA_SEED_AUTHORS.filter((a) => a !== HEARTBEAT_AUTHOR)]
  console.log(`[indexer] indexing entries for authors: ${seedAuthors.join(', ')}`)
  const seedEntries = []
  const seedIds = new Set()
  for (const author of seedAuthors) {
    for (const entry of await fetchAllByAuthor(author)) {
      if (seedIds.has(entry.id)) continue
      seedIds.add(entry.id)
      seedEntries.push(entry)
    }
  }
  const replyEntries = await fetchAllReplies([...seedIds])
  const allEntries = seedEntries.concat(replyEntries)

  const stats = await getStats().catch((e) => {
    console.warn('[indexer] stats query failed:', e.message)
    return null
  })
  const index = buildIndex(allEntries, stats)
  await augmentDisclosures(index, allEntries)
  await augmentVotes(index)
  return index
}

export async function refresh() {
  if (!existsSync(CACHE_DIR)) mkdirSync(CACHE_DIR, { recursive: true })
  const index = await fetchIndex()
  writeFileSync(INDEX_FILE, JSON.stringify(index, null, 2), 'utf8')
  console.log(`[indexer] wrote ${index.meta.entry_count} entries → ${INDEX_FILE}`)
  return index
}

async function main() {
  await refresh()
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((e) => {
    console.error('[indexer] failed:', e)
    process.exit(1)
  })
}
