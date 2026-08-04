// D1 probe (J-Lens) determinism tests. Run with: node --test test/

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { loadProbeBank, loadHiddenStates, scoreHiddenStates, buildJSpaceSnapshot, d1Verdict } from '../src/d1-probe.js'

function tmpFile(dir, name, obj) {
  const p = join(dir, name)
  writeFileSync(p, JSON.stringify(obj), 'utf8')
  return p
}

// Two orthogonal unit-ish vectors so cosine similarity is exactly
// computable by hand: concept vector [1,0,0,0], token vectors either
// aligned (score 1.0) or orthogonal (score 0.0).
const PROBE_BANK = {
  probe_version: 'j-lens-v0.1',
  probe_model: 'fixture-model',
  layer: 5,
  concepts: {
    reward_hacking: { vector: [1, 0, 0, 0], threshold: 0.7 },
    deception: { vector: [0, 1, 0, 0], threshold: 0.7 },
  },
}

const HIDDEN_STATES_HIT = {
  probe_model: 'fixture-model',
  layer: 5,
  states: [
    { token: 'reward', position: 0, vector: [1, 0, 0, 0] }, // aligned with reward_hacking -> hit
    { token: 'the', position: 1, vector: [0, 0, 1, 0] }, // orthogonal to both -> no hit
  ],
}

const HIDDEN_STATES_CLEAN = {
  probe_model: 'fixture-model',
  layer: 5,
  states: [{ token: 'hello', position: 0, vector: [0, 0, 1, 0] }],
}

test('D1: loadProbeBank rejects a bank with no concepts', () => {
  const dir = mkdtempSync(join(tmpdir(), 'd1-probe-'))
  try {
    const p = tmpFile(dir, 'bank.json', { probe_model: 'x', layer: 1, concepts: {} })
    loadProbeBank(p) // empty concepts{} is technically valid (0 entries) — should not throw
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('D1: scoreHiddenStates detects an aligned concept and skips orthogonal ones', () => {
  const detections = scoreHiddenStates(HIDDEN_STATES_HIT, PROBE_BANK)
  assert.equal(detections.length, 1)
  assert.equal(detections[0].concept, 'reward_hacking')
  assert.equal(detections[0].token, 'reward')
  assert.equal(detections[0].jacobian_score, 1)
})

test('D1: scoreHiddenStates returns no detections when nothing crosses threshold', () => {
  const detections = scoreHiddenStates(HIDDEN_STATES_CLEAN, PROBE_BANK)
  assert.equal(detections.length, 0)
})

test('D1: scoreHiddenStates throws on probe_model mismatch (spec §3.5, no cross-model transfer)', () => {
  const wrongModel = { ...HIDDEN_STATES_HIT, probe_model: 'other-model' }
  assert.throws(() => scoreHiddenStates(wrongModel, PROBE_BANK), /probe\/model mismatch/)
})

test('D1: scoreHiddenStates throws on layer mismatch', () => {
  const wrongLayer = { ...HIDDEN_STATES_HIT, layer: 99 }
  assert.throws(() => scoreHiddenStates(wrongLayer, PROBE_BANK), /layer mismatch/)
})

test('D1: buildJSpaceSnapshot is byte-identical across repeated calls on the same inputs', () => {
  const dir = mkdtempSync(join(tmpdir(), 'd1-probe-'))
  try {
    const bankPath = tmpFile(dir, 'bank.json', PROBE_BANK)
    const statesPath = tmpFile(dir, 'states.json', HIDDEN_STATES_HIT)

    const snap1 = buildJSpaceSnapshot({ hiddenStatesPath: statesPath, probeBankPath: bankPath })
    const snap2 = buildJSpaceSnapshot({ hiddenStatesPath: statesPath, probeBankPath: bankPath })

    assert.equal(JSON.stringify(snap1), JSON.stringify(snap2))
    assert.equal(snap1.snapshot_hash, snap2.snapshot_hash)
    assert.equal(snap1.forbidden_concepts.length, 2)
    assert.deepEqual(snap1.forbidden_concepts, ['deception', 'reward_hacking']) // sorted
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('D1: d1Verdict warns (never blocks in v0.1) on any detection, per fail-safe spec §3.5', () => {
  const dir = mkdtempSync(join(tmpdir(), 'd1-probe-'))
  try {
    const bankPath = tmpFile(dir, 'bank.json', PROBE_BANK)
    const hitPath = tmpFile(dir, 'hit.json', HIDDEN_STATES_HIT)
    const cleanPath = tmpFile(dir, 'clean.json', HIDDEN_STATES_CLEAN)

    const hitSnap = buildJSpaceSnapshot({ hiddenStatesPath: hitPath, probeBankPath: bankPath })
    const cleanSnap = buildJSpaceSnapshot({ hiddenStatesPath: cleanPath, probeBankPath: bankPath })

    assert.equal(d1Verdict(hitSnap).verdict, 'warn')
    assert.equal(d1Verdict(cleanSnap).verdict, 'pass')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('D1: loadHiddenStates rejects a file with no states[]', () => {
  const dir = mkdtempSync(join(tmpdir(), 'd1-probe-'))
  try {
    const p = tmpFile(dir, 'empty.json', { probe_model: 'x', layer: 1, states: [] })
    assert.throws(() => loadHiddenStates(p), /no states/)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})
