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
