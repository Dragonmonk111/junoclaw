import { createServer } from 'http'
import { URL } from 'url'
import { postReplyToMoultbook, buildReplyPost } from './moultbook.js'

const PORT = process.env.PORT || 3001
const ADMIN_TOKEN = process.env.REPLY_BOT_ADMIN_TOKEN

const pending = new Map()

function setCors(res) {
  res.setHeader('Access-Control-Allow-Origin', '*')
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization')
}

function sendJson(res, status, body) {
  setCors(res)
  res.writeHead(status, { 'Content-Type': 'application/json' })
  res.end(JSON.stringify(body, null, 2))
}

function requireAuth(req) {
  if (!ADMIN_TOKEN) return true
  const auth = req.headers['authorization']
  return auth && auth.startsWith('Bearer ') && auth.slice(7) === ADMIN_TOKEN
}

function generateId() {
  return `draft-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

const server = createServer(async (req, res) => {
  if (req.method === 'OPTIONS') {
    setCors(res)
    res.writeHead(204)
    res.end()
    return
  }

  const url = new URL(req.url, `http://${req.headers.host}`)
  const path = url.pathname

  try {
    if (path === '/api/health') {
      return sendJson(res, 200, { status: 'ok', pending: pending.size })
    }

    if (path === '/api/reply' && req.method === 'POST') {
      const body = await readBody(req)
      const { reply_to, text, agent = 'dragonmonk111-bot', approve = false } = body

      if (!reply_to) return sendJson(res, 400, { error: 'reply_to is required' })
      if (!text) return sendJson(res, 400, { error: 'text is required' })

      if (!approve) {
        const draft = {
          id: generateId(),
          reply_to,
          text,
          agent,
          preview: buildReplyPost(text, reply_to, agent),
          created_at: new Date().toISOString(),
        }
        pending.set(draft.id, draft)
        return sendJson(res, 200, {
          draft,
          note: 'Reply is pending. POST again with approve=true and the draft id to sign+broadcast.',
        })
      }

      if (!requireAuth(req)) {
        return sendJson(res, 401, { error: 'Unauthorized' })
      }

      const draftId = body.draft_id
      const draft = draftId ? pending.get(draftId) : null
      if (draftId && !draft) {
        return sendJson(res, 404, { error: 'Draft not found' })
      }

      const result = await postReplyToMoultbook(
        draft ? draft.text : text,
        draft ? draft.reply_to : reply_to,
        draft ? draft.agent : agent,
      )
      if (draftId) pending.delete(draftId)
      return sendJson(res, 200, result)
    }

    if (path === '/api/pending' && req.method === 'GET') {
      return sendJson(res, 200, { pending: Array.from(pending.values()) })
    }

    sendJson(res, 404, { error: 'not found' })
  } catch (e) {
    console.error('[reply-bot-server] error:', e)
    sendJson(res, 500, { error: e.message })
  }
})

function readBody(req) {
  return new Promise((resolve, reject) => {
    let data = ''
    req.on('data', (chunk) => (data += chunk))
    req.on('end', () => {
      try {
        resolve(data ? JSON.parse(data) : {})
      } catch (e) {
        reject(e)
      }
    })
  })
}

server.listen(PORT, () => {
  console.log(`[reply-bot] server listening on http://localhost:${PORT}`)
  console.log(`[reply-bot] admin token required: ${ADMIN_TOKEN ? 'yes' : 'no (open endpoint)'}`)
})
