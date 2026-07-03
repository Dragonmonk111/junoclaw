import { createServer } from 'http'
import { URL } from 'url'
import { postReplyToMoultbook, buildReplyPost, buildAkbExportPost, postAkbExportToMoultbook, getSignerAddress } from './moultbook.js'
import { mintKnowledgeMoult, buildMintMsg } from './knowledge-moults.js'

const PORT = process.env.PORT || 3001
const ADMIN_TOKEN = process.env.REPLY_BOT_ADMIN_TOKEN
const REPLY_BOT_NAME = process.env.REPLY_BOT_NAME || 'dragonmonk111-bot'
const DEFAULT_MOTHER_MOULT_ID = process.env.MOTHER_MOULT_ID || 'moult:mother:draft'

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

    if (path === '/api/identity' && req.method === 'GET') {
      const wallet = await getSignerAddress()
      return sendJson(res, 200, { wallet, alias: REPLY_BOT_NAME, type: 'agent' })
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

    if (path === '/api/export' && req.method === 'POST') {
      const body = await readBody(req)
      const { approve = false } = body

      if (!body.envelope) return sendJson(res, 400, { error: 'envelope is required' })

      // Enrich a partial envelope from the UI with identity + scaffolding so the
      // client only supplies content/refs/tags/memory_ops. direction and author
      // are forced (author.wallet is the bot's signer and is re-stamped again at
      // broadcast), mother_moult_id/akb_version default if the client omits them.
      const signer = await getSignerAddress()
      const clientEnv = body.envelope
      const envelope = {
        akb_version: '1.1',
        mother_moult_id: DEFAULT_MOTHER_MOULT_ID,
        ...clientEnv,
        direction: 'export',
        author: {
          wallet: clientEnv.author?.wallet || signer || 'juno1unknown',
          alias: clientEnv.author?.alias || REPLY_BOT_NAME,
          type: clientEnv.author?.type || 'agent',
        },
      }

      let preview
      try {
        preview = buildAkbExportPost(envelope)
      } catch (e) {
        return sendJson(res, 400, { error: `Invalid AKB export envelope: ${e.message}` })
      }

      if (!approve) {
        const draft = {
          id: generateId(),
          kind: 'akb-export',
          envelope,
          preview,
          created_at: new Date().toISOString(),
        }
        pending.set(draft.id, draft)
        return sendJson(res, 200, {
          draft,
          note: 'Export is pending. POST again with approve=true and the draft id to sign+broadcast.',
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

      const result = await postAkbExportToMoultbook(draft ? draft.envelope : envelope)
      if (draftId) pending.delete(draftId)
      return sendJson(res, 200, result)
    }

    if (path === '/api/mint' && req.method === 'POST') {
      const body = await readBody(req)
      const {
        agent = REPLY_BOT_NAME,
        motive,
        knowledge_summary,
        source_moults = [],
        owner = null,
        approve = false,
      } = body

      if (!motive) return sendJson(res, 400, { error: 'motive is required' })
      if (!knowledge_summary) return sendJson(res, 400, { error: 'knowledge_summary is required' })

      if (!approve) {
        let preview
        try {
          preview = buildMintMsg({ agent, motive, knowledgeSummary: knowledge_summary, sourceMoults: source_moults, owner })
        } catch (e) {
          return sendJson(res, 400, { error: e.message })
        }
        const draft = {
          id: generateId(),
          kind: 'mint',
          agent,
          motive,
          knowledge_summary,
          source_moults,
          owner,
          preview,
          created_at: new Date().toISOString(),
        }
        pending.set(draft.id, draft)
        return sendJson(res, 200, {
          draft,
          note: 'Mint is pending. POST again with approve=true and the draft id to sign+broadcast.',
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

      try {
        const result = await mintKnowledgeMoult({
          agent: draft ? draft.agent : agent,
          motive: draft ? draft.motive : motive,
          knowledgeSummary: draft ? draft.knowledge_summary : knowledge_summary,
          sourceMoults: draft ? draft.source_moults : source_moults,
          owner: draft ? draft.owner : owner,
        })
        if (draftId) pending.delete(draftId)
        return sendJson(res, 200, result)
      } catch (e) {
        return sendJson(res, 400, { error: e.message })
      }
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
