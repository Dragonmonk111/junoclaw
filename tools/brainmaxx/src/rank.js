// BM25 + PPMI ranking, pinned per spec §6 (rank_fn_version = tokv1+bm25v1+ppmiv1).
//
// Provenance: the BM25 term-scoring formula and the PPMI/cosine
// distributional-semantics algorithm are ported from
// tools/context-agent/bridges/local-file-bridge.js (recall / recallSemantic),
// which is the DAO's reference "zero-engine" bridge implementation — see that
// file's header comments for the full rationale (no model, no training run,
// same input always produces the same output). Brainmaxx's tokenizer (tokv1)
// additionally drops short tokens and stopwords, which the bridge's raw
// tokenize() does not — this is an intentional, pinned divergence for
// higher-precision recall in a cognition-prep context, not a reimplementation
// bug. Both remain deterministic; do not change either without bumping the
// relevant version string.

import { sha256hex } from './canon.js'
import { BM25_K1, BM25_B } from './config.js'

// Pinned stopword list (sorted, so the hash is order-independent of authoring).
export const STOPWORDS = [
  'a', 'about', 'after', 'again', 'all', 'also', 'an', 'and', 'any', 'are',
  'as', 'at', 'be', 'because', 'been', 'being', 'both', 'but', 'by', 'can',
  'could', 'did', 'do', 'does', 'for', 'from', 'had', 'has', 'have', 'he',
  'her', 'here', 'him', 'his', 'how', 'if', 'in', 'into', 'is', 'it', 'its',
  'may', 'more', 'most', 'no', 'nor', 'not', 'of', 'on', 'or', 'other',
  'our', 'over', 'own', 'per', 'she', 'should', 'so', 'some', 'such',
  'than', 'that', 'the', 'their', 'them', 'then', 'there', 'these', 'they',
  'this', 'those', 'through', 'to', 'too', 'under', 'until', 'up', 'very',
  'was', 'we', 'were', 'what', 'when', 'where', 'which', 'while', 'who',
  'will', 'with', 'would', 'you', 'your',
].sort()

const STOPWORD_SET = new Set(STOPWORDS)

export const STOPWORDS_HASH = sha256hex(Buffer.from(STOPWORDS.join('\n'), 'utf8'))

/** tokv1: lowercase -> split non-alphanumerics -> drop <2 chars -> drop stopwords. */
export function tokenize(text = '') {
  return String(text)
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((t) => t.length >= 2 && !STOPWORD_SET.has(t))
}

function entryText(entry) {
  return [entry.text, entry.author_alias, entry.moult_id, ...(entry.tags || [])].filter(Boolean).join(' ')
}

/**
 * BM25 over the full candidate entry list. Returns [{ entry, score }],
 * unsorted (caller applies tie-break). k1/b pinned in config.js.
 */
export function bm25Score(entries, query, { k1 = BM25_K1, b = BM25_B } = {}) {
  const docs = entries.map((e) => tokenize(entryText(e)))
  const docLens = docs.map((d) => d.length)
  const avgDocLen = docLens.reduce((sum, len) => sum + len, 0) / (docs.length || 1) || 1

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
  return entries.map((entry, i) => {
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
    return { entry, score }
  })
}

/**
 * PPMI + cosine over the full candidate entry list — fixed alphabetical
 * vocabulary and summation order (bit-identical across process runs, per
 * the bridge's recallSemantic contract). Returns [{ entry, score }].
 */
export function ppmiScore(entries, query) {
  const docTermSets = entries.map((e) => new Set(tokenize(entryText(e))))
  const vocab = Array.from(new Set(docTermSets.flatMap((s) => [...s]))).sort()
  const vocabIndex = new Map(vocab.map((term, i) => [term, i]))
  const N = entries.length

  const docFreq = new Map()
  for (const term of vocab) {
    let count = 0
    for (const set of docTermSets) if (set.has(term)) count++
    docFreq.set(term, count)
  }

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
  return entries.map((entry, i) => ({
    entry,
    score: queryVec.size ? cosine(queryVec, toVector([...docTermSets[i]])) : 0,
  }))
}

function normalizeScores(scored) {
  const max = scored.reduce((m, s) => Math.max(m, s.score), 0)
  if (!max) return scored.map((s) => ({ ...s, score: 0 }))
  return scored.map((s) => ({ ...s, score: s.score / max }))
}

/**
 * Rank entries for a query in the given mode. Returns [{ entry, score }]
 * sorted by score descending, then moult_id ascending (total order, no
 * float ambiguity, spec §6).
 */
export function rank(entries, query, { mode = 'lexical' } = {}) {
  let scored
  if (mode === 'semantic') {
    scored = ppmiScore(entries, query)
  } else if (mode === 'hybrid') {
    const bm25 = normalizeScores(bm25Score(entries, query))
    const ppmi = normalizeScores(ppmiScore(entries, query))
    scored = entries.map((entry, i) => ({
      entry,
      score: 0.5 * bm25[i].score + 0.5 * ppmi[i].score,
    }))
  } else {
    scored = bm25Score(entries, query)
  }

  return scored.sort((a, b) => {
    if (b.score !== a.score) return b.score - a.score
    return a.entry.moult_id < b.entry.moult_id ? -1 : a.entry.moult_id > b.entry.moult_id ? 1 : 0
  })
}
