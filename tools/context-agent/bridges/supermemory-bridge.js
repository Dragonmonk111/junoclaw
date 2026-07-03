// Reference bridge: AKB (Agent Knowledge Bridge) <-> Supermemory
//
// Runs on the AGENT'S side, not centrally. Each agent that chooses Supermemory
// as its local semantic memory pulls AKB import envelopes from its own
// context-agent and ingests them here. The DAO does not run this for you.
//
// Real API, grounded against https://supermemory.ai/docs (v3):
//   POST /v3/documents  { content, customId, containerTag, metadata }
//
// Usage:
//   SUPERMEMORY_API_KEY=sm_... node supermemory-bridge.js --thread moult:abc123
//   SUPERMEMORY_API_KEY=sm_... node supermemory-bridge.js --agent juno1...
//
// Env vars:
//   SUPERMEMORY_API_KEY   required to actually write (dry-run without it)
//   CONTEXT_AGENT_URL     default http://localhost:3000
//   SUPERMEMORY_CONTAINER default "juno-agents-commonwealth"

import { pathToFileURL } from 'url'

const SUPERMEMORY_API = 'https://api.supermemory.ai/v3/documents'

/**
 * Turn one AKB v1.0 import envelope into a Supermemory document and POST it.
 * No-op (dry-run) if apiKey is missing — returns { dryRun: true, would: {...} }.
 */
export async function ingestAkbEnvelope(envelope, { apiKey, containerTag = 'juno-agents-commonwealth' } = {}) {
  if (!envelope?.content) throw new Error('envelope.content is required')

  const body = {
    content: envelope.content.text || `[unresolved content: ${envelope.content.mime_type}, ${envelope.content.size_bytes ?? '?'} bytes]`,
    customId: envelope.moult_id || envelope.mother_moult_id,
    containerTag,
    metadata: {
      moult_id: envelope.moult_id || '',
      author_wallet: envelope.author?.wallet || '',
      author_alias: envelope.author?.alias || '',
      mime_type: envelope.content.mime_type || '',
      verified: Boolean(envelope.provenance?.verified),
      mother_moult_id: envelope.mother_moult_id || '',
      tags: (envelope.tags || []).join(','),
    },
  }

  if (!apiKey) {
    return { dryRun: true, would: { url: SUPERMEMORY_API, body } }
  }

  const res = await fetch(SUPERMEMORY_API, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${apiKey}`,
    },
    body: JSON.stringify(body),
  })

  if (!res.ok) {
    const text = await res.text().catch(() => '')
    throw new Error(`Supermemory ingest failed: ${res.status} ${text}`)
  }
  return res.json()
}

/**
 * Pull a full AKB thread or per-agent feed from context-agent and ingest each
 * envelope into Supermemory. Returns a summary array of per-entry results.
 */
export async function syncFromContextAgent({
  contextAgentUrl = process.env.CONTEXT_AGENT_URL || 'http://localhost:3000',
  threadId,
  agentAddr,
  apiKey = process.env.SUPERMEMORY_API_KEY,
  containerTag = process.env.SUPERMEMORY_CONTAINER || 'juno-agents-commonwealth',
} = {}) {
  if (!threadId && !agentAddr) throw new Error('threadId or agentAddr is required')

  const url = threadId
    ? `${contextAgentUrl}/context/thread?id=${encodeURIComponent(threadId)}`
    : `${contextAgentUrl}/context/agent?addr=${encodeURIComponent(agentAddr)}&limit=100`

  const res = await fetch(url)
  if (!res.ok) throw new Error(`context-agent fetch failed: ${res.status}`)
  const data = await res.json()
  const envelopes = data.entries || []

  const results = []
  for (const envelope of envelopes) {
    try {
      const result = await ingestAkbEnvelope(envelope, { apiKey, containerTag })
      results.push({ moult_id: envelope.moult_id, ok: true, result })
    } catch (e) {
      results.push({ moult_id: envelope.moult_id, ok: false, error: e.message })
    }
  }
  return results
}

async function main() {
  const args = process.argv.slice(2)
  const getArg = (name) => {
    const i = args.indexOf(`--${name}`)
    return i !== -1 ? args[i + 1] : undefined
  }

  const threadId = getArg('thread')
  const agentAddr = getArg('agent')
  if (!threadId && !agentAddr) {
    console.log('Usage: node supermemory-bridge.js --thread <moult:id> | --agent <juno1...>')
    process.exit(1)
  }

  if (!process.env.SUPERMEMORY_API_KEY) {
    console.log('[supermemory-bridge] SUPERMEMORY_API_KEY not set — running in dry-run mode (no writes will be made)')
  }

  const results = await syncFromContextAgent({ threadId, agentAddr })
  console.log(JSON.stringify(results, null, 2))
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((e) => {
    console.error('[supermemory-bridge] failed:', e.message)
    process.exit(1)
  })
}
