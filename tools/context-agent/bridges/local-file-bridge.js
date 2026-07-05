// Reference bridge: AKB (Agent Knowledge Bridge) <-> local file cache
//
// Runs entirely on the AGENT'S side, with zero third-party dependency: no
// binary to install, no account, no API key, no network call other than
// the agent's own context-agent. This is the "zero-engine" option described
// in drafts/ARTICLE_FIELD_GUIDE_AGENT_SOVEREIGN_BRIDGE.md — cache AKB
// envelopes as local JSON-lines, rank them with our own BM25 (see recall()
// below). Maximally sovereign; the tradeoff against mnemosyne-bridge.js /
// supermemory-bridge.js is lexical relevance ranking, not neural/vector
// semantic search — BM25 finds documents that share vocabulary with your
// query, weighted by term rarity; it won't find a zero-overlap paraphrase
// the way an embedding model would. Zero new dependency either way.
//
// Default store path nests under `memory/`, which the repo's root
// .gitignore already ignores at any depth — safe to run from inside this
// checkout without risking an accidental commit of your own cache.
//
// Usage:
//   node local-file-bridge.js --thread moult:abc123
//   node local-file-bridge.js --agent juno1...
//
// Env vars:
//   MEMORY_STORE_PATH  default ./memory/agent-bridge/<namespace>.jsonl (cwd-relative)
//   MEMORY_NAMESPACE   default "juno-agents-commonwealth"
//   CONTEXT_AGENT_URL  default http://localhost:3000

import { pathToFileURL } from 'url'
import { appendFileSync, readFileSync, mkdirSync, existsSync } from 'fs'
import { dirname, join } from 'path'

function storePath(namespace = process.env.MEMORY_NAMESPACE || 'juno-agents-commonwealth') {
  return process.env.MEMORY_STORE_PATH || join(process.cwd(), 'memory', 'agent-bridge', `${namespace}.jsonl`)
}

function tokenize(text = '') {
  return String(text)
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean)
}

function rowText(row) {
  return [row.text, row.author_alias, row.moult_id, ...(row.tags || [])].filter(Boolean).join(' ')
}

function readExistingIds(path) {
  if (!existsSync(path)) return new Set()
  const ids = new Set()
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (!line.trim()) continue
    try {
      const row = JSON.parse(line)
      if (row.moult_id) ids.add(row.moult_id)
    } catch {
      // skip a malformed line rather than crash the whole read
    }
  }
  return ids
}

/**
 * Append one AKB v1.0 import envelope to the local JSON-lines store.
 * Dedups on moult_id — safe to re-sync the same thread/agent repeatedly.
 * Never fails for a missing engine — there is no engine, just a file.
 */
export async function ingestAkbEnvelope(envelope, { path = storePath() } = {}) {
  if (!envelope?.content) throw new Error('envelope.content is required')

  mkdirSync(dirname(path), { recursive: true })
  const existing = readExistingIds(path)
  const id = envelope.moult_id || envelope.mother_moult_id
  if (id && existing.has(id)) {
    return { written: false, deduped: true, path }
  }

  const row = {
    moult_id: envelope.moult_id || null,
    mother_moult_id: envelope.mother_moult_id || null,
    author_wallet: envelope.author?.wallet || null,
    author_alias: envelope.author?.alias || null,
    mime_type: envelope.content.mime_type || null,
    text: envelope.content.text || null,
    verified: Boolean(envelope.provenance?.verified),
    tags: envelope.tags || [],
    cached_at: new Date().toISOString(),
  }
  appendFileSync(path, JSON.stringify(row) + '\n', 'utf8')
  return { written: true, deduped: false, path }
}

/**
 * BM25 lexical ranking over the local store — our own zero-dependency
 * relevance search: term frequency x inverse document frequency x document
 * length normalization, computed fresh over whatever's in the cache right
 * now. No model, no training, no network call — just arithmetic over the
 * words actually in your store. Same sovereignty guarantee as
 * ingestAkbEnvelope() above; the honest tradeoff is in the header comment.
 */
export async function recall(query, { path = storePath(), limit = 10, k1 = 1.5, b = 0.75 } = {}) {
  if (!existsSync(path)) return []

  const rows = []
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (!line.trim()) continue
    try {
      rows.push(JSON.parse(line))
    } catch {
      // skip a malformed line
    }
  }
  if (!rows.length) return []

  const docs = rows.map((row) => tokenize(rowText(row)))
  const docLens = docs.map((d) => d.length)
  const avgDocLen = docLens.reduce((sum, len) => sum + len, 0) / docs.length || 1

  const df = new Map()
  for (const doc of docs) {
    for (const term of new Set(doc)) df.set(term, (df.get(term) || 0) + 1)
  }
  const N = docs.length
  const idf = (term) => {
    const n = df.get(term) || 0
    return Math.log((N - n + 0.5) / (n + 0.5) + 1)
  }

  const queryTerms = tokenize(query)
  const scored = rows.map((row, i) => {
    const tf = new Map()
    for (const term of docs[i]) tf.set(term, (tf.get(term) || 0) + 1)

    let score = 0
    for (const term of queryTerms) {
      const f = tf.get(term) || 0
      if (!f) continue
      const numerator = f * (k1 + 1)
      const denominator = f + k1 * (1 - b + (b * docLens[i]) / avgDocLen)
      score += idf(term) * (numerator / denominator)
    }
    return { row, score }
  })

  return scored
    .filter((s) => s.score > 0)
    .sort((a, c) => c.score - a.score)
    .slice(0, limit)
    .map((s) => s.row)
}

function readRows(path) {
  if (!existsSync(path)) return []
  const rows = []
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (!line.trim()) continue
    try {
      rows.push(JSON.parse(line))
    } catch {
      // skip a malformed line
    }
  }
  return rows
}

/**
 * Self-derived distributional semantics — PPMI (positive pointwise mutual
 * information) vectors built fresh from whatever's in the local store,
 * cosine-ranked against the query. This is not a pretrained model: there
 * is no training run to trust, no weights to pin, nothing to download.
 * The corpus IS the training data, and the corpus is your own cache of an
 * immutable on-chain source — so the same store always produces the exact
 * same vectors: fixed (alphabetical) vocabulary order, fixed summation
 * order, every step plain arithmetic. Same input file in, same output,
 * on any machine, forever — genuinely deterministic, not just "usually."
 *
 * Honest tradeoff: it finds terms that co-occur with similar neighbors
 * *within your own cache*, which is real distributional semantics, not a
 * lexical trick — but it knows nothing about a word that has never
 * appeared in your corpus, and it's noisier the smaller the corpus is. It
 * improves as your local cache grows, which a pretrained model does not.
 *
 * O(vocab²) to build the term-term matrix, recomputed on every call — fine
 * at reference-bridge scale (dozens to low-hundreds of cached entries);
 * not written for a corpus that's outgrown "local file cache."
 */
export async function recallSemantic(query, { path = storePath(), limit = 10 } = {}) {
  const rows = readRows(path)
  if (!rows.length) return []

  const docTermSets = rows.map((row) => new Set(tokenize(rowText(row))))
  const vocab = Array.from(new Set(docTermSets.flatMap((s) => [...s]))).sort()
  const vocabIndex = new Map(vocab.map((term, i) => [term, i]))
  const N = rows.length

  const docFreq = new Map()
  for (const term of vocab) {
    let count = 0
    for (const set of docTermSets) if (set.has(term)) count++
    docFreq.set(term, count)
  }

  // Document-level co-occurrence: two terms co-occur if they share a row.
  // Pair keys always stored termA\0termB with termA earlier in the sorted
  // vocab, so each pair is counted once, in one canonical order.
  const coFreq = new Map()
  for (const set of docTermSets) {
    const terms = vocab.filter((t) => set.has(t))
    for (let a = 0; a < terms.length; a++) {
      for (let b = a + 1; b < terms.length; b++) {
        const key = `${terms[a]}\0${terms[b]}`
        coFreq.set(key, (coFreq.get(key) || 0) + 1)
      }
    }
  }

  function ppmi(termA, termB) {
    if (termA === termB) return 1
    const [a, b] = vocabIndex.get(termA) <= vocabIndex.get(termB) ? [termA, termB] : [termB, termA]
    const co = coFreq.get(`${a}\0${b}`) || 0
    if (!co) return 0
    const pmi = Math.log((co * N) / (docFreq.get(a) * docFreq.get(b)))
    return Math.max(0, pmi)
  }

  const termVectorCache = new Map()
  function termVector(term) {
    if (termVectorCache.has(term)) return termVectorCache.get(term)
    const vec = new Map()
    for (const other of vocab) {
      const score = ppmi(term, other)
      if (score > 0) vec.set(other, score)
    }
    termVectorCache.set(term, vec)
    return vec
  }

  function toVector(terms) {
    const present = terms.filter((t) => vocabIndex.has(t)).sort()
    if (!present.length) return new Map()
    const acc = new Map()
    for (const term of present) {
      for (const [other, score] of termVector(term)) {
        acc.set(other, (acc.get(other) || 0) + score)
      }
    }
    for (const [k, v] of acc) acc.set(k, v / present.length)
    return acc
  }

  function cosine(vecA, vecB) {
    let dot = 0
    for (const [k, v] of vecA) if (vecB.has(k)) dot += v * vecB.get(k)
    const normA = Math.sqrt([...vecA.values()].reduce((s, v) => s + v * v, 0))
    const normB = Math.sqrt([...vecB.values()].reduce((s, v) => s + v * v, 0))
    return normA && normB ? dot / (normA * normB) : 0
  }

  const queryVec = toVector(tokenize(query))
  if (!queryVec.size) return []

  const scored = rows.map((row, i) => ({
    row,
    score: cosine(queryVec, toVector([...docTermSets[i]])),
  }))

  return scored
    .filter((s) => s.score > 0)
    .sort((a, c) => c.score - a.score)
    .slice(0, limit)
    .map((s) => s.row)
}

/**
 * Pull a full AKB thread or per-agent feed from context-agent and cache
 * each envelope into the local JSON-lines store.
 */
export async function syncFromContextAgent({
  contextAgentUrl = process.env.CONTEXT_AGENT_URL || 'http://localhost:3000',
  threadId,
  agentAddr,
  path = storePath(),
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
      const result = await ingestAkbEnvelope(envelope, { path })
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
    console.log('Usage: node local-file-bridge.js --thread <moult:id> | --agent <juno1...>')
    process.exit(1)
  }

  const results = await syncFromContextAgent({ threadId, agentAddr })
  console.log(`[local-file-bridge] store: ${storePath()}`)
  console.log(JSON.stringify(results, null, 2))
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((e) => {
    console.error('[local-file-bridge] failed:', e.message)
    process.exit(1)
  })
}
