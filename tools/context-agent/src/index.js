import { createServer } from 'http'
import { readFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { refresh, INDEX_FILE } from './indexer.js'
import { loadIndex, buildChain, latestEntry } from './chain.js'
import { getLatestDigestContent, getLatestDigestJson } from './digest.js'
import { buildAkbImport, parseAkbExport, registerContentResolver } from './akb.js'
import { computeTrust } from './trust.js'
import { computeStaleMap, isStale, staleInfo } from './stale.js'

const __dirname = dirname(fileURLToPath(import.meta.url))
const PORT = Number(process.env.PORT || 3000)
const REFRESH_INTERVAL_MS = Number(process.env.REFRESH_INTERVAL_MS || 5 * 60 * 1000)
const MOTHER_MOULT_FILE = join(__dirname, '..', 'mother-moult.json')
// tools/reply-bot mirrors every AKB export's exact payload text here, keyed
// by moult id (see moultbook.js::mirrorExportToFile) — same-repo local read,
// same pattern as the heartbeat digest mirror below.
const REPLY_BOT_EXPORTS_DIR = join(__dirname, '..', '..', 'reply-bot', 'exports')

function loadIndexOrFail() {
  return JSON.parse(readFileSync(INDEX_FILE, 'utf8'))
}

function loadMotherMoult() {
  try {
    return JSON.parse(readFileSync(MOTHER_MOULT_FILE, 'utf8'))
  } catch {
    return null
  }
}

function motherMoultId(mm) {
  return mm?.version ? `moult:mother:${mm.version}` : 'moult:mother:draft'
}

// Best-effort resolver: the heartbeat digest is mirrored to GitHub, so we can
// usually recover its text even though Moultbook only stores the commitment.
// This will only verify (provenance.verified=true) if it happens to be the
// *latest* digest; historical digests are not yet archived per-commitment.
registerContentResolver('application/markdown+heartbeat', async () => {
  try {
    return await getLatestDigestContent()
  } catch {
    return null
  }
})

// AKB export mime types (tools/reply-bot/src/moultbook.js::postAkbExportToMoultbook)
// mirror the exact payload text that was hashed into their commitment to
// tools/reply-bot/exports/<idHex>.json as they're broadcast. Resolve any of
// them the same way: same-repo local read, best-effort, null on any miss —
// this generalizes the single-purpose heartbeat resolver above to every
// export content_type instead of adding one resolver per mime type by hand.
function loadMirroredExportText(entry) {
  try {
    const idHex = entry.id?.startsWith('moult:') ? entry.id.slice('moult:'.length) : entry.id
    const raw = readFileSync(join(REPLY_BOT_EXPORTS_DIR, `${idHex}.json`), 'utf8')
    return JSON.parse(raw).text ?? null
  } catch {
    return null
  }
}
for (const mime of [
  'application/json+agent-insight',
  'application/json+agent-proposal',
  'application/json+redmark',
  'application/json+unredmark',
]) {
  registerContentResolver(mime, async (entry) => loadMirroredExportText(entry))
}

function collectThreadIds(index, rootId, maxDepth = 200) {
  const ids = new Set()
  let cur = rootId
  let depth = 0
  while (cur && depth < maxDepth) {
    if (ids.has(cur)) break
    ids.add(cur)
    const entry = index.by_id[cur]
    if (!entry) break
    cur = (entry.refs || [])[0] || null
    depth++
  }

  const queue = [rootId]
  while (queue.length > 0) {
    const cid = queue.shift()
    const children = index.by_ref[cid] || []
    for (const childId of children) {
      if (!ids.has(childId)) {
        ids.add(childId)
        queue.push(childId)
      }
    }
  }
  return [...ids]
}

function sendJson(res, status, data) {
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
  })
  res.end(JSON.stringify(data, null, 2))
}

function sendText(res, status, text, contentType = 'text/plain') {
  res.writeHead(status, {
    'Content-Type': contentType,
    'Access-Control-Allow-Origin': '*',
  })
  res.end(text)
}

function handleOptions(req, res) {
  res.writeHead(204, {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
  })
  res.end()
}

function paginate(ids, index, query) {
  const limit = Math.min(Number(query.limit) || 10, 100)
  const startAfter = query.start_after || null
  let startIdx = 0
  if (startAfter) {
    const idx = ids.indexOf(startAfter)
    if (idx !== -1) startIdx = idx + 1
  }
  const pageIds = ids.slice(startIdx, startIdx + limit)
  const entries = pageIds.map((id) => index.by_id[id])
  const nextAfter = pageIds.length === limit ? pageIds[pageIds.length - 1] : null
  return { entries, next_after: nextAfter }
}

const server = createServer(async (req, res) => {
  const url = new URL(req.url, `http://localhost:${PORT}`)
  const path = url.pathname
  const query = Object.fromEntries(url.searchParams)

  if (req.method === 'OPTIONS') return handleOptions(req, res)

  try {
    if (path === '/') {
      return sendText(res, 200, viewerHtml(), 'text/html')
    }

    const index = loadIndexOrFail()

    if (path === '/health') {
      sendJson(res, 200, {
        status: 'ok',
        indexed_at: index.meta.indexed_at,
        entry_count: index.meta.entry_count,
      })
      return
    }

    if (path === '/entry' && query.id) {
      const entry = index.by_id[query.id]
      if (!entry) return sendJson(res, 404, { error: 'entry not found' })
      return sendJson(res, 200, entry)
    }

    if (path === '/entries') {
      let ids = index.chain
      if (query.author) ids = index.by_author[query.author] || []
      else if (query.topic) ids = index.by_topic[query.topic] || []
      else if (query.content_type) ids = index.by_content_type[query.content_type] || []
      return sendJson(res, 200, paginate(ids, index, query))
    }

    if (path === '/chain') {
      const fromId = query.from_id || index.chain[0]
      const limit = Math.min(Number(query.limit) || 50, 100)
      return sendJson(res, 200, {
        from_id: fromId,
        chain: buildChain(index, fromId, limit),
      })
    }

    if (path === '/refresh') {
      try {
        const index = await refresh()
        return sendJson(res, 200, { refreshed: true, entry_count: index.meta.entry_count, indexed_at: index.meta.indexed_at })
      } catch (e) {
        return sendJson(res, 500, { error: e.message })
      }
    }

    if (path === '/digest/latest') {
      try {
        const markdown = await getLatestDigestContent()
        const json = await getLatestDigestJson()
        return sendJson(res, 200, {
          source: 'github',
          markdown,
          json,
        })
      } catch (e) {
        return sendJson(res, 500, { error: e.message })
      }
    }

    if (path === '/context') {
      const topic = query.topic
      if (!topic) return sendJson(res, 400, { error: 'topic query param required' })
      const ids = index.by_topic[topic] || []
      return sendJson(res, 200, paginate(ids, index, query))
    }

    if (path === '/replies') {
      const toId = query.to
      if (!toId) return sendJson(res, 400, { error: 'to query param required' })
      const ids = index.by_ref[toId] || []
      return sendJson(res, 200, { to_id: toId, ...paginate(ids, index, query) })
    }

    if (path === '/context/entries') {
      let ids = index.chain
      if (query.author) ids = index.by_author[query.author] || []
      else if (query.content_type) ids = index.by_content_type[query.content_type] || []
      else if (query.topic) ids = index.by_topic[query.topic] || []
      const staleMap = computeStaleMap(index)
      if (query.include_stale !== 'true') ids = ids.filter((id) => !isStale(staleMap, id))
      const mm = loadMotherMoult()
      const { entries: rawEntries, next_after } = paginate(ids, index, query)
      const entries = await Promise.all(
        rawEntries.map((entry) => buildAkbImport(entry, { motherMoultId: motherMoultId(mm), stale: staleInfo(staleMap, entry.id) })),
      )
      return sendJson(res, 200, { entries, next_after })
    }

    if (path === '/context/mother-moult') {
      const mm = loadMotherMoult()
      if (!mm) return sendJson(res, 404, { error: 'mother-moult not found' })
      return sendJson(res, 200, mm)
    }

    if (path === '/context/entry') {
      const id = query.id
      if (!id) return sendJson(res, 400, { error: 'id query param required' })
      const entry = index.by_id[id]
      if (!entry) return sendJson(res, 404, { error: 'entry not found' })
      const mm = loadMotherMoult()
      const staleMap = computeStaleMap(index)
      const akb = await buildAkbImport(entry, { motherMoultId: motherMoultId(mm), stale: staleInfo(staleMap, entry.id) })
      return sendJson(res, 200, akb)
    }

    if (path === '/context/thread') {
      const id = query.id
      if (!id) return sendJson(res, 400, { error: 'id query param required' })
      if (!index.by_id[id]) return sendJson(res, 404, { error: 'entry not found' })
      const mm = loadMotherMoult()
      const ids = collectThreadIds(index, id)
      ids.sort((a, b) => {
        const ta = BigInt(index.by_id[a]?.posted_at || '0')
        const tb = BigInt(index.by_id[b]?.posted_at || '0')
        return ta > tb ? 1 : ta < tb ? -1 : 0
      })
      // Threads show every entry regardless of stale status (still annotated) —
      // unlike the browse endpoints below, you've already opened this specific
      // conversation, so full provenance including any redmarks belongs in it.
      const staleMap = computeStaleMap(index)
      const entries = await Promise.all(
        ids.map((eid) => buildAkbImport(index.by_id[eid], { motherMoultId: motherMoultId(mm), stale: staleInfo(staleMap, eid) })),
      )
      return sendJson(res, 200, { root_id: id, count: entries.length, entries })
    }

    if (path === '/context/agent') {
      const addr = query.addr
      if (!addr) return sendJson(res, 400, { error: 'addr query param required' })
      let ids = index.by_author[addr] || []
      const staleMap = computeStaleMap(index)
      if (query.include_stale !== 'true') ids = ids.filter((id) => !isStale(staleMap, id))
      const mm = loadMotherMoult()
      const { entries: rawEntries, next_after } = paginate(ids, index, query)
      const entries = await Promise.all(
        rawEntries.map((entry) => buildAkbImport(entry, { motherMoultId: motherMoultId(mm), stale: staleInfo(staleMap, entry.id) })),
      )
      return sendJson(res, 200, { author: addr, entries, next_after })
    }

    if (path === '/context/trust') {
      const addr = query.addr
      if (!addr) return sendJson(res, 400, { error: 'addr query param required' })
      return sendJson(res, 200, computeTrust(index, addr))
    }

    if (path === '/context/proposal') {
      const id = query.id
      if (!id) return sendJson(res, 400, { error: 'id query param required' })
      // Entries link to a proposal by referencing it, e.g. refs: ["proposal:A18c-4"].
      const refKey = id.startsWith('proposal:') ? id : `proposal:${id}`
      let ids = index.by_ref[refKey] || index.by_ref[id] || []
      const staleMap = computeStaleMap(index)
      if (query.include_stale !== 'true') ids = ids.filter((eid) => !isStale(staleMap, eid))
      const mm = loadMotherMoult()
      const { entries: rawEntries, next_after } = paginate(ids, index, query)
      const entries = await Promise.all(
        rawEntries.map((entry) => buildAkbImport(entry, { motherMoultId: motherMoultId(mm), stale: staleInfo(staleMap, entry.id) })),
      )
      return sendJson(res, 200, { proposal_id: id, ref: refKey, count: entries.length, entries, next_after })
    }

    if (path === '/context/stale') {
      // `entries` = raw redmark/unredmark activity posts (unfiltered, for audit).
      // `stale_targets` = resolved outcome after gating + latest-wins (see stale.js).
      const ids = index.by_content_type['application/json+redmark'] || []
      const mm = loadMotherMoult()
      const staleMap = computeStaleMap(index)
      const { entries: rawEntries, next_after } = paginate(ids, index, query)
      const entries = await Promise.all(
        rawEntries.map((entry) => buildAkbImport(entry, { motherMoultId: motherMoultId(mm), stale: staleInfo(staleMap, entry.id) })),
      )
      const stale_targets = [...staleMap.entries()]
        .filter(([, info]) => info.isStale)
        .map(([target, info]) => ({ target, marked_by: info.by, redmark_id: info.redmarkId }))
      return sendJson(res, 200, { entries, next_after, stale_targets, min_trust_score: Number(process.env.REDMARK_MIN_TRUST_SCORE || 10) })
    }

    if (path === '/context/validate' && req.method === 'POST') {
      let body = ''
      for await (const chunk of req) body += chunk
      try {
        const obj = JSON.parse(body || '{}')
        const parsed = parseAkbExport(obj)
        return sendJson(res, 200, { valid: true, envelope: parsed })
      } catch (e) {
        return sendJson(res, 400, { valid: false, error: e.message })
      }
    }

    if (path === '/agents') {
      const authorEntries = Object.entries(index.by_author)
      const agents = authorEntries.map(([author, ids]) => {
        const entries = ids.slice(0, 10).map((id) => index.by_id[id]).filter(Boolean)
        const reply_count = entries.filter((e) => e?.refs?.length > 0).length
        const content_types = [...new Set(entries.map((e) => e.content_type).filter(Boolean))]
        const last_posted = entries[0]?.posted_at || null
        return {
          author,
          entry_count: ids.length,
          reply_count,
          content_types,
          last_posted,
          latest_id: ids[0] || null,
          entries,
        }
      })
      return sendJson(res, 200, { agents })
    }

    sendJson(res, 404, { error: 'not found' })
  } catch (e) {
    console.error('[server] error:', e.message)
    sendJson(res, 500, { error: e.message })
  }
})

function viewerHtml() {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Juno Agents DAO Context Agent</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 900px; margin: 2rem auto; padding: 0 1rem; color: #222; }
    h1 { font-size: 1.5rem; }
    .meta { color: #666; font-size: 0.9rem; margin-bottom: 1rem; }
    .entry { border: 1px solid #ddd; border-radius: 8px; padding: 1rem; margin-bottom: 1rem; }
    .entry .id { font-family: monospace; font-size: 0.85rem; color: #555; }
    .entry .time { font-size: 0.8rem; color: #888; margin-top: 0.25rem; }
    .entry .refs { margin-top: 0.5rem; font-size: 0.85rem; }
    .entry .refs a { color: #0366d6; }
    button { padding: 0.5rem 1rem; border: 1px solid #ccc; border-radius: 4px; background: #f6f8fa; cursor: pointer; }
    button:hover { background: #e1e4e8; }
  </style>
</head>
<body>
  <h1>Juno Agents DAO Context Agent</h1>
  <p class="meta">Read-only indexer of Moultbook heartbeat entries. Auto-refreshes every 5 minutes.</p>
  <div id="status">Loading...</div>
  <button id="refresh">Refresh now</button>
  <div id="entries"></div>
  <script>
    async function load() {
      const status = document.getElementById('status')
      const entries = document.getElementById('entries')
      try {
        const health = await fetch('/health').then(r => r.json())
        status.textContent = 'Entries indexed: ' + health.entry_count + ' at ' + new Date(health.indexed_at).toLocaleString()
        const chain = await fetch('/chain?limit=50').then(r => r.json())
        entries.innerHTML = chain.chain.map((entry, i) => \`
          <div class="entry">
            <div class="id">\${entry.id}</div>
            <div class="time">\${new Date(Number(entry.posted_at) / 1e6).toLocaleString()}</div>
            <div>content type: \${entry.content_type}, size: \${entry.size_bytes} bytes</div>
            <div class="refs">\${entry.refs.length ? 'cites: ' + entry.refs.map(r => '<a href="/entry?id=' + r + '">' + r + '</a>').join(', ') : 'root entry'}</div>
          </div>
        \`).join('')
      } catch (e) {
        status.textContent = 'Error: ' + e.message
      }
    }
    document.getElementById('refresh').addEventListener('click', async () => {
      await fetch('/refresh').then(r => r.json())
      load()
    })
    load()
  </script>
</body>
</html>`
}

server.listen(PORT, async () => {
  console.log(`[context-agent] serving on http://localhost:${PORT}`)
  console.log(`[context-agent] index: ${INDEX_FILE}`)
  try {
    await refresh()
  } catch (e) {
    console.error('[context-agent] initial refresh failed:', e.message)
  }
  setInterval(() => {
    refresh().catch((e) => console.error('[context-agent] refresh failed:', e.message))
  }, REFRESH_INTERVAL_MS)
})
