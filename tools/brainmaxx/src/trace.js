// Trace read/write + run_id / claim_id computation (spec §8).

import { existsSync, mkdirSync, readFileSync, writeFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { canonHash, CANON_VERSION } from './canon.js'
import { RANK_FN_VERSION } from './config.js'

export const TRACE_VERSION = 1

/**
 * run_id = "brainrun:" + sha256hex(canonV1({ mode, objective,
 * corpus_snapshot_hash, config_hash, rank_fn_version, canon_version,
 * query_set })) — created_at and attachments excluded so identical inputs
 * collide intentionally (dedupe/replay, spec U2).
 */
export function computeRunId({ mode, objective, corpus_snapshot_hash, config_hash, query_set }) {
  const hash = canonHash({
    mode,
    objective,
    corpus_snapshot_hash,
    config_hash,
    rank_fn_version: RANK_FN_VERSION,
    canon_version: CANON_VERSION,
    query_set,
  })
  return `brainrun:${hash}`
}

/** claim_id = "claim:" + sha256hex(canonV1({ claim, support, run_id })) (spec U7). */
export function computeClaimId({ claim, support, run_id }) {
  return `claim:${canonHash({ claim, support, run_id })}`
}

/** run_id contains a colon, which is not a valid Windows filename character. */
function runIdFilename(run_id) {
  return `${run_id.replace(/:/g, '_')}.json`
}

export function traceFilePath(brainmaxxDir, run_id) {
  return join(brainmaxxDir, 'traces', runIdFilename(run_id))
}

export function createTrace({ mode, objective, corpus_snapshot_hash, config_hash, query_set, pack, gates = [], claims = [] }) {
  const run_id = computeRunId({ mode, objective, corpus_snapshot_hash, config_hash, query_set })
  return {
    trace_version: TRACE_VERSION,
    run_id,
    mode,
    objective,
    created_at: new Date().toISOString(),
    corpus_snapshot_hash,
    config_hash,
    rank_fn_version: RANK_FN_VERSION,
    canon_version: CANON_VERSION,
    query_set,
    pack,
    gates,
    claims,
    determinism_profile: 'D0',
    attachments: [],
  }
}

export function writeTrace(brainmaxxDir, trace) {
  const path = traceFilePath(brainmaxxDir, trace.run_id)
  mkdirSync(join(brainmaxxDir, 'traces'), { recursive: true })
  writeFileSync(path, JSON.stringify(trace, null, 2), 'utf8')
  return path
}

export function readTrace(brainmaxxDir, run_id) {
  const path = traceFilePath(brainmaxxDir, run_id)
  if (!existsSync(path)) throw new Error(`no trace found for ${run_id} at ${path}`)
  return JSON.parse(readFileSync(path, 'utf8'))
}

/** Resolve a possibly-partial run_id (with or without the "brainrun:" prefix) against stored traces. */
export function resolveRunId(brainmaxxDir, partial) {
  const full = partial.startsWith('brainrun:') ? partial : `brainrun:${partial}`
  const tracesDir = join(brainmaxxDir, 'traces')
  if (existsSync(traceFilePath(brainmaxxDir, full))) return full
  if (!existsSync(tracesDir)) throw new Error(`no traces directory at ${tracesDir}`)
  const match = readdirSync(tracesDir).find((f) => f.startsWith(full.replace(/:/g, '_')))
  if (!match) throw new Error(`no trace matching ${partial}`)
  return match.replace(/\.json$/, '').replace(/_/g, ':')
}

/**
 * Attach a D2 (generative) draft to a trace. Flips determinism_profile to
 * D2-attached — the trace now records that a non-deterministic stage
 * touched the run (spec §8).
 */
export function attachDraft(trace, { path, sha256, role = 'llm-draft', model = 'operator-declared' }) {
  trace.attachments.push({ path, sha256, role, model })
  trace.determinism_profile = 'D2-attached'
  return trace
}
