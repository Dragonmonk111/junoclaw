// Env + defaults for Brainmaxx v0 (spec §3). Every pinned constant that
// affects a deterministic output lives here or is referenced from here, so
// config_hash captures the full "recipe" a run was computed under.

import { join } from 'node:path'
import { canonHash } from './canon.js'

export const RANK_FN_VERSION = 'tokv1+bm25v1+ppmiv1'
export const BM25_K1 = 1.5
export const BM25_B = 0.75
export const MIN_SCORE = 0.05
export const DEFAULT_K = 12
export const QUOTE_TRIGRAM_THRESHOLD = 0.8

export function getConfig(env = process.env) {
  const namespace = env.MEMORY_NAMESPACE || 'juno-agents-commonwealth'
  const storePath = env.MEMORY_STORE_PATH || join(process.cwd(), 'memory', 'agent-bridge', `${namespace}.jsonl`)
  const brainmaxxDir = env.BRAINMAXX_DIR || join(process.cwd(), 'memory', 'brainmaxx', namespace)
  const contextAgentUrl = env.CONTEXT_AGENT_URL || 'http://localhost:3000'
  const motherMoultId = env.MOTHER_MOULT_ID || null

  return {
    namespace,
    storePath,
    brainmaxxDir,
    contextAgentUrl,
    motherMoultId,
    rankFnVersion: RANK_FN_VERSION,
    bm25K1: BM25_K1,
    bm25B: BM25_B,
    minScore: MIN_SCORE,
    defaultK: DEFAULT_K,
    quoteTrigramThreshold: QUOTE_TRIGRAM_THRESHOLD,
  }
}

/**
 * config_hash = sha256hex(canonV1(effective_config)) — the resolved values
 * above plus all pinned constants. contextAgentUrl and brainmaxxDir are
 * excluded: they affect where things are read/written, not what the
 * deterministic computation produces, so two operators pointing at the same
 * store but different local paths still get identical config_hash.
 */
export function configHash(config) {
  const { namespace, storePath, rankFnVersion, bm25K1, bm25B, minScore, defaultK, quoteTrigramThreshold } = config
  return canonHash({ namespace, storePath, rankFnVersion, bm25K1, bm25B, minScore, defaultK, quoteTrigramThreshold })
}
