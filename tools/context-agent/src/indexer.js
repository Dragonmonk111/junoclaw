import { existsSync, mkdirSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { listByAuthor, getStats } from './moultbook.js'

const __dirname = dirname(fileURLToPath(import.meta.url))
export const CACHE_DIR = process.env.CACHE_DIR || join(__dirname, '..', 'cache')
export const INDEX_FILE = join(CACHE_DIR, 'index.json')

export const HEARTBEAT_AUTHOR = process.env.HEARTBEAT_AUTHOR || 'juno17nmczzsfycwn74z2yrxqe7fc96033e7rm2gut6'
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
  }

  if (stats) index.meta.moultbook_stats = stats
  return index
}

export async function fetchIndex() {
  console.log(`[indexer] indexing entries for author ${HEARTBEAT_AUTHOR}`)
  const entries = await fetchAllByAuthor(HEARTBEAT_AUTHOR)
  const stats = await getStats().catch((e) => {
    console.warn('[indexer] stats query failed:', e.message)
    return null
  })
  return buildIndex(entries, stats)
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

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((e) => {
    console.error('[indexer] failed:', e)
    process.exit(1)
  })
}
