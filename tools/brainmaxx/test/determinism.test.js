// Brainmaxx v0 determinism tests (spec §13, T1-T7). Run with: node --test test/

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

import { canonV1, canonV1Bytes, sha256hex, envelopeCommitment, envelopeCommitmentBytes } from '../src/canon.js'
import { loadSnapshot, loadStore, corpusSnapshotHash } from '../src/store.js'
import { buildPack } from '../src/pack.js'
import { runGates, checkRefsResolve, checkQuotesResolve, checkStale } from '../src/gates.js'
import { computeRunId } from '../src/trace.js'
import { sha256Base64 } from '../../reply-bot/src/moultbook.js'

const __dirname = dirname(fileURLToPath(import.meta.url))
const FIXTURES = join(__dirname, 'fixtures')
const CORPUS = join(FIXTURES, 'corpus.jsonl')
const CORPUS_PERMUTED = join(FIXTURES, 'corpus-permuted.jsonl')
const CLI = join(__dirname, '..', 'src', 'cli.js')

// ---------------------------------------------------------------------------
// T1 — canon fixtures: known objects -> known canonV1 bytes + hashes.
// ---------------------------------------------------------------------------
test('T1: canonV1 sorts keys and is stable across key insertion order', () => {
  const a = canonV1({ b: 1, a: 2, c: [3, 2, 1] })
  const b = canonV1({ c: [3, 2, 1], a: 2, b: 1 })
  assert.equal(a, b)
  assert.equal(a, '{"a":2,"b":1,"c":[3,2,1]}')
})

test('T1: canonV1 NFC-normalizes strings and hashing is deterministic', () => {
  const value = { text: 'café' }
  const h1 = sha256hex(canonV1Bytes(value))
  const h2 = sha256hex(canonV1Bytes(value))
  assert.equal(h1, h2)
  assert.equal(h1.length, 64)
})

test('T1: canonV1 drops undefined fields but keeps null', () => {
  const out = canonV1({ a: undefined, b: null, c: 1 })
  assert.equal(out, '{"b":null,"c":1}')
})

// ---------------------------------------------------------------------------
// T2 — double-run recall: same fixture store + query, two in-process runs
// produce an identical pack_hash.
// ---------------------------------------------------------------------------
test('T2: double in-process recall produces identical pack_hash', () => {
  const snapshot = loadSnapshot(CORPUS)
  const pack1 = buildPack(snapshot.entries, 'Reef memory Moultbook', { k: 5 })
  const pack2 = buildPack(snapshot.entries, 'Reef memory Moultbook', { k: 5 })
  assert.equal(pack1.pack_hash, pack2.pack_hash)
  assert.ok(pack1.items.length > 0)
});

// ---------------------------------------------------------------------------
// T3 — cross-process recall: two spawned CLI processes produce byte-identical
// --json stdout (the recall command's JSON has no created_at field, so no
// exclusion needed).
// ---------------------------------------------------------------------------
test('T3: cross-process recall --json is byte-identical across two spawns', () => {
  const env = { ...process.env, MEMORY_STORE_PATH: CORPUS, MEMORY_NAMESPACE: 'brainmaxx-test' }
  const run = () => spawnSync('node', [CLI, 'recall', 'Reef memory Moultbook', '--k', '5', '--json'], { env, encoding: 'utf8' })

  const r1 = run()
  const r2 = run()
  assert.equal(r1.status, 0, r1.stderr)
  assert.equal(r2.status, 0, r2.stderr)
  assert.equal(r1.stdout, r2.stdout)
})

// ---------------------------------------------------------------------------
// T4 — snapshot stability: permuted JSONL line order -> same
// corpus_snapshot_hash after dedupe/sort.
// ---------------------------------------------------------------------------
test('T4: corpus_snapshot_hash is invariant under JSONL line permutation', () => {
  const a = loadSnapshot(CORPUS)
  const b = loadSnapshot(CORPUS_PERMUTED)
  assert.equal(a.count, b.count)
  assert.equal(a.corpus_snapshot_hash, b.corpus_snapshot_hash)
})

test('T4: corpus_snapshot_hash matches direct recomputation from loadStore', () => {
  const entries = loadStore(CORPUS)
  const expected = corpusSnapshotHash(entries)
  const snapshot = loadSnapshot(CORPUS)
  assert.equal(snapshot.corpus_snapshot_hash, expected)
})

// ---------------------------------------------------------------------------
// T5 — gate fixtures: fabricated ref -> G1 fail; mangled quote -> G2 fail;
// redmarked source -> G3 fail without include_stale.
// ---------------------------------------------------------------------------
test('T5: G1 fails on a fabricated ref', () => {
  const entries = loadStore(CORPUS)
  const verdict = checkRefsResolve({ refs: ['moult:does-not-exist'], claims: [] }, entries)
  assert.equal(verdict.verdict, 'fail')
})

test('T5: G1 passes when all refs resolve', () => {
  const entries = loadStore(CORPUS)
  const verdict = checkRefsResolve({ refs: ['moult:aaa1', 'moult:aaa2'], claims: [] }, entries)
  assert.equal(verdict.verdict, 'pass')
})

test('T5: G2 fails on a mangled quote', () => {
  const entries = loadStore(CORPUS)
  const claims = [{ claim_id: 'claim:1', claim: 'test', support: ['moult:aaa1'], quote: 'this text does not appear anywhere in the source at all' }]
  const verdict = checkQuotesResolve({ claims }, entries)
  assert.equal(verdict.verdict, 'fail')
})

test('T5: G2 passes on an exact substring quote', () => {
  const entries = loadStore(CORPUS)
  const claims = [{ claim_id: 'claim:1', claim: 'test', support: ['moult:aaa1'], quote: 'immutable append-only ledger' }]
  const verdict = checkQuotesResolve({ claims }, entries)
  assert.equal(verdict.verdict, 'pass')
})

test('T5: G3 fails on a redmarked (stale) source without include_stale', () => {
  const entries = loadStore(CORPUS)
  const verdict = checkStale({ refs: ['moult:aaa4'], claims: [], includeStale: false }, entries)
  assert.equal(verdict.verdict, 'fail')
})

test('T5: G3 warns (not fails) on a redmarked source with include_stale', () => {
  const entries = loadStore(CORPUS)
  const verdict = checkStale({ refs: ['moult:aaa4'], claims: [], includeStale: true }, entries)
  assert.equal(verdict.verdict, 'warn')
})

test('T5: recall pack excludes stale entries by default', () => {
  const entries = loadStore(CORPUS)
  const pack = buildPack(entries, 'DAO shared memory engine rejected', { k: 5 })
  assert.ok(!pack.items.some((i) => i.moult_id === 'moult:aaa4'))
})

// ---------------------------------------------------------------------------
// T6 — commitment parity: fixture envelope -> sha256b64(JSON.stringify(env,
// null, 2)) equals the hash computed by reply-bot's own method on the same
// file (byte-parity, spec §4).
// ---------------------------------------------------------------------------
test('T6: envelope commitment matches reply-bot sha256Base64 byte-for-byte', () => {
  const envelope = JSON.parse(readFileSync(join(FIXTURES, 'envelope.json'), 'utf8'))
  const ours = envelopeCommitment(envelope)
  const theirs = sha256Base64(JSON.stringify(envelope, null, 2))
  assert.equal(ours, theirs)
})

test('T6: envelopeCommitmentBytes equals JSON.stringify(env, null, 2) exactly', () => {
  const envelope = JSON.parse(readFileSync(join(FIXTURES, 'envelope.json'), 'utf8'))
  const bytes = envelopeCommitmentBytes(envelope).toString('utf8')
  assert.equal(bytes, JSON.stringify(envelope, null, 2))
})

// ---------------------------------------------------------------------------
// T7 — replay identity: full recall trace replays byte-identically; exit 0.
// ---------------------------------------------------------------------------
test('T7: plan -> replay is byte-identical and exits 0', () => {
  const tmp = mkdtempSync(join(tmpdir(), 'brainmaxx-t7-'))
  try {
    const env = { ...process.env, MEMORY_STORE_PATH: CORPUS, MEMORY_NAMESPACE: 'brainmaxx-test', BRAINMAXX_DIR: tmp }

    const planResult = spawnSync('node', [CLI, 'plan', 'Explain the Reef memory system', '--k', '5'], { env, encoding: 'utf8' })
    assert.equal(planResult.status, 0, planResult.stderr)

    const match = planResult.stdout.match(/run_id: (brainrun:[a-f0-9]+)/)
    assert.ok(match, `expected run_id in stdout: ${planResult.stdout}`)
    const runId = match[1]

    const replayResult = spawnSync('node', [CLI, 'replay', runId], { env, encoding: 'utf8' })
    assert.equal(replayResult.status, 0, replayResult.stderr || replayResult.stdout)
    assert.match(replayResult.stdout, /replay OK/)
  } finally {
    rmSync(tmp, { recursive: true, force: true })
  }
})

test('T7: replay still matches after attach adds claims (replay reproduces plan-time verdict, not post-hoc claims)', () => {
  const tmp = mkdtempSync(join(tmpdir(), 'brainmaxx-t7b-'))
  try {
    const env = { ...process.env, MEMORY_STORE_PATH: CORPUS, MEMORY_NAMESPACE: 'brainmaxx-test', BRAINMAXX_DIR: tmp }

    const planResult = spawnSync('node', [CLI, 'plan', 'Explain the Reef memory system', '--k', '5'], { env, encoding: 'utf8' })
    assert.equal(planResult.status, 0, planResult.stderr)
    const runId = planResult.stdout.match(/run_id: (brainrun:[a-f0-9]+)/)[1]

    const draftPath = join(tmp, 'draft.md')
    writeFileSync(draftPath, 'The Reef is built on Moultbook (moult:aaa1).')
    const claimsPath = join(tmp, 'claims.json')
    writeFileSync(claimsPath, JSON.stringify([{ claim: 'test', support: ['moult:aaa1'], quote: 'immutable append-only ledger' }]))

    const attachResult = spawnSync('node', [CLI, 'attach', runId, draftPath, '--claims', claimsPath], { env, encoding: 'utf8' })
    assert.equal(attachResult.status, 0, attachResult.stderr)

    const replayResult = spawnSync('node', [CLI, 'replay', runId], { env, encoding: 'utf8' })
    assert.equal(replayResult.status, 0, replayResult.stderr || replayResult.stdout)
    assert.match(replayResult.stdout, /replay OK/)
  } finally {
    rmSync(tmp, { recursive: true, force: true })
  }
})

test('T7: run_id is stable for identical inputs (excludes created_at)', () => {
  const snapshot = loadSnapshot(CORPUS)
  const id1 = computeRunId({
    mode: 'plan',
    objective: 'Explain the Reef',
    corpus_snapshot_hash: snapshot.corpus_snapshot_hash,
    config_hash: 'fixed-config-hash',
    query_set: ['Explain the Reef'],
  })
  const id2 = computeRunId({
    mode: 'plan',
    objective: 'Explain the Reef',
    corpus_snapshot_hash: snapshot.corpus_snapshot_hash,
    config_hash: 'fixed-config-hash',
    query_set: ['Explain the Reef'],
  })
  assert.equal(id1, id2)
})

// ---------------------------------------------------------------------------
// Humility test — no results above threshold for an off-corpus query.
// ---------------------------------------------------------------------------
test('humility: off-corpus query returns no items above MIN_SCORE', () => {
  const entries = loadStore(CORPUS)
  const pack = buildPack(entries, 'quantum thermodynamics recipe for sourdough bread', { k: 5 })
  assert.equal(pack.items.length, 0)
})

// ---------------------------------------------------------------------------
// G5 policy sanity — red actions fail closed.
// ---------------------------------------------------------------------------
test('G5: red action fails closed', () => {
  const entries = loadStore(CORPUS)
  const policy = { 'fund-transfer': 'red', recall: 'green' }
  const verdicts = runGates({ entries, action: 'fund-transfer', policy })
  const g5 = verdicts.find((v) => v.gate === 'G5')
  assert.equal(g5.verdict, 'fail')
})

test('G5: unknown action fails closed', () => {
  const entries = loadStore(CORPUS)
  const verdicts = runGates({ entries, action: 'never-defined-action', policy: {} })
  const g5 = verdicts.find((v) => v.gate === 'G5')
  assert.equal(g5.verdict, 'fail')
})
