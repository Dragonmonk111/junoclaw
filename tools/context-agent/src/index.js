import { createServer } from 'http'
import { readFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { refresh, INDEX_FILE } from './indexer.js'
import { loadIndex, buildChain, latestEntry } from './chain.js'
import { getLatestDigestContent, getLatestDigestJson } from './digest.js'

const __dirname = dirname(fileURLToPath(import.meta.url))
const PORT = Number(process.env.PORT || 3000)
const REFRESH_INTERVAL_MS = Number(process.env.REFRESH_INTERVAL_MS || 5 * 60 * 1000)

function loadIndexOrFail() {
  return JSON.parse(readFileSync(INDEX_FILE, 'utf8'))
}

function sendJson(res, status, data) {
  res.writeHead(status, { 'Content-Type': 'application/json' })
  res.end(JSON.stringify(data, null, 2))
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

  try {
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

    sendJson(res, 404, { error: 'not found' })
  } catch (e) {
    console.error('[server] error:', e.message)
    sendJson(res, 500, { error: e.message })
  }
})

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
