// Reference bridge: AKB (Agent Knowledge Bridge) <-> Mnemosyne
//
// Runs on the AGENT'S side, not centrally. Mnemosyne (github.com/rand/mnemosyne)
// is a local Rust CLI + MCP server — it has no generic HTTP CRUD API, so this
// bridge shells out to the `mnemosyne` binary the same way its own CLI docs do:
//   mnemosyne remember <CONTENT> --namespace <NS> --importance <1-10> --type <TYPE> --tags <TAGS>
//   mnemosyne recall <QUERY> --namespace <NS> --limit <N>
//
// If the binary isn't installed/on PATH, calls fall back to dry-run and report
// the exact command that would have run — this bridge never crashes an agent
// that hasn't set Mnemosyne up.
//
// Usage:
//   node mnemosyne-bridge.js --thread moult:abc123
//   node mnemosyne-bridge.js --agent juno1...
//
// Env vars:
//   MNEMOSYNE_BIN        default "mnemosyne"
//   MNEMOSYNE_NAMESPACE  default "juno-agents-commonwealth"
//   CONTEXT_AGENT_URL    default http://localhost:3000

import { execFile } from 'child_process'
import { promisify } from 'util'
import { pathToFileURL } from 'url'

const execFileAsync = promisify(execFile)

function akbTypeToMnemosyneType(mimeType = '') {
  if (mimeType.includes('agent-insight')) return 'insight'
  if (mimeType.includes('agent-reply')) return 'reference'
  if (mimeType.includes('redmark')) return 'decision'
  if (mimeType.includes('heartbeat')) return 'architecture'
  return 'reference'
}

/**
 * Turn one AKB v1.0 import envelope into a `mnemosyne remember` call.
 * Falls back to dry-run (returns { dryRun: true, would: {...} }) if the
 * `mnemosyne` binary isn't found — never throws for a missing install.
 */
export async function ingestAkbEnvelope(envelope, { bin = process.env.MNEMOSYNE_BIN || 'mnemosyne', namespace = process.env.MNEMOSYNE_NAMESPACE || 'juno-agents-commonwealth' } = {}) {
  if (!envelope?.content) throw new Error('envelope.content is required')

  const content = envelope.content.text || `[unresolved content: ${envelope.content.mime_type}, ${envelope.content.size_bytes ?? '?'} bytes]`
  const importance = envelope.provenance?.verified ? 7 : 4
  const type = akbTypeToMnemosyneType(envelope.content.mime_type)
  const tags = [...(envelope.tags || []), envelope.author?.alias].filter(Boolean).join(',')

  const args = [
    'remember',
    content,
    '--namespace', namespace,
    '--importance', String(importance),
    '--type', type,
    '--tags', tags,
  ]

  try {
    const { stdout } = await execFileAsync(bin, args, { maxBuffer: 10 * 1024 * 1024 })
    return { dryRun: false, stdout: stdout.trim() }
  } catch (e) {
    if (e.code === 'ENOENT') {
      return { dryRun: true, would: { bin, args } }
    }
    throw e
  }
}

/**
 * Semantic search against the agent's local Mnemosyne store. Falls back to
 * dry-run if the binary isn't installed.
 */
export async function recall(query, { bin = process.env.MNEMOSYNE_BIN || 'mnemosyne', namespace = process.env.MNEMOSYNE_NAMESPACE || 'juno-agents-commonwealth', limit = 10 } = {}) {
  const args = ['recall', query, '--namespace', namespace, '--limit', String(limit)]
  try {
    const { stdout } = await execFileAsync(bin, args, { maxBuffer: 10 * 1024 * 1024 })
    return { dryRun: false, stdout: stdout.trim() }
  } catch (e) {
    if (e.code === 'ENOENT') {
      return { dryRun: true, would: { bin, args } }
    }
    throw e
  }
}

/**
 * Pull a full AKB thread or per-agent feed from context-agent and `remember`
 * each envelope into the local Mnemosyne store.
 */
export async function syncFromContextAgent({
  contextAgentUrl = process.env.CONTEXT_AGENT_URL || 'http://localhost:3000',
  threadId,
  agentAddr,
  bin,
  namespace,
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
      const result = await ingestAkbEnvelope(envelope, { bin, namespace })
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
    console.log('Usage: node mnemosyne-bridge.js --thread <moult:id> | --agent <juno1...>')
    process.exit(1)
  }

  const results = await syncFromContextAgent({ threadId, agentAddr })
  console.log(JSON.stringify(results, null, 2))
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((e) => {
    console.error('[mnemosyne-bridge] failed:', e.message)
    process.exit(1)
  })
}
