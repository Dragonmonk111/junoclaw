// J2-J9 method-dispatch tests for d1-probe.js. Run with: node --test test/
// J1 backward-compat is covered by d1-probe.test.js — these cover the new paths.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { scoreAll, scoreCompositions, buildJSpaceSnapshot } from '../src/d1-probe.js'

const MODEL = 'fixture-model'

test('J2 logistic-l2-v0.1: sigmoid(h . w + b) scoring', () => {
  const bank = {
    probe_model: MODEL,
    layer: 5,
    method: 'logistic-l2-v0.1',
    concepts: {
      // w = [1,0], b = 0 => sigmoid(h0). h0 = 0 -> p = 0.5
      alpha: { method: 'logistic-l2-v0.1', vector: [1, 0], bias: 0, threshold: 0.6 },
    },
  }
  const hs = {
    probe_model: MODEL,
    layer: 5,
    states: [
      { token: 'zero', position: 0, vector: [0, 0] },
      { token: 'high', position: 1, vector: [4, 0] },
    ],
  }
  const { detections, maxScores } = scoreAll(hs, bank)
  assert.equal(maxScores.alpha, Number((1 / (1 + Math.exp(-4))).toFixed(6)))
  assert.equal(detections.length, 1)
  assert.equal(detections[0].token, 'high')
})

test('J3 trajectory-dom-v0.1: weighted per-layer cosine + slope bonus', () => {
  const bank = {
    probe_model: MODEL,
    method: 'trajectory-dom-v0.1',
    concepts: {
      alpha: {
        method: 'trajectory-dom-v0.1',
        layers: { 1: [1, 0], 2: [0, 1] },
        layer_weights: { 1: 0.5, 2: 0.5 },
        slope_weight: 0.5,
        threshold: 0.4,
      },
    },
  }
  const hs = {
    probe_model: MODEL,
    states: [
      // Rising signal: layer1 orthogonal (0), layer2 aligned (1).
      { token: 'rise', position: 0, vectors: { 1: [0, 1], 2: [0, 1] } },
      // Flat zero at both layers.
      { token: 'flat', position: 1, vectors: { 1: [1, 0], 2: [1, 0] } },
    ],
  }
  const { detections } = scoreAll(hs, bank)
  const rise = detections.find((d) => d.token === 'rise')
  // base = 0.5*0 + 0.5*1 = 0.5; slope over layers 1,2 = 1; score = 0.5 + 0.5*1 = 1.0
  assert.ok(rise, 'rising trajectory must be detected')
  assert.equal(rise.jacobian_score, 1.0)
  // flat: base = 0.5*1 + 0.5*0 = 0.5; slope = -1 -> clamped to 0; score 0.5 (detected at 0.4)
  const flat = detections.find((d) => d.token === 'flat')
  assert.equal(flat.jacobian_score, 0.5)
})

test('J6 compositions: mean +- uncertainty across folds, sub-threshold inputs count', () => {
  const compositions = {
    combo: {
      inputs: ['alpha', 'beta'],
      folds: [
        { weights: [1, 1], bias: 0 },
        { weights: [2, 2], bias: 0 },
      ],
      threshold: 0.5,
    },
  }
  const out = scoreCompositions({ alpha: 0.4, beta: 0.4 }, compositions)
  assert.equal(out.length, 1)
  const p1 = 1 / (1 + Math.exp(-0.8))
  const p2 = 1 / (1 + Math.exp(-1.6))
  assert.equal(out[0].probability, Number(((p1 + p2) / 2).toFixed(6)))
  assert.ok(out[0].uncertainty > 0)
})

test('J5 jacobian bank: uniform-weight trajectory path, slope zero', () => {
  const bank = {
    probe_model: MODEL,
    method: 'jacobian-v0.1',
    concepts: {
      alpha: {
        method: 'jacobian-v0.1',
        layers: { 1: [1, 0], 2: [1, 0] },
        layer_weights: { 1: 0.5, 2: 0.5 },
        slope_weight: 0,
        threshold: 0.9,
      },
    },
  }
  const hs = {
    probe_model: MODEL,
    states: [{ token: 'hit', position: 0, vectors: { 1: [1, 0], 2: [1, 0] } }],
  }
  const { detections } = scoreAll(hs, bank)
  assert.equal(detections.length, 1)
  assert.equal(detections[0].jacobian_score, 1.0)
})

test('snapshot carries compositions + method metadata', () => {
  const dir = mkdtempSync(join(tmpdir(), 'jlens-j6-'))
  try {
    const bankPath = join(dir, 'bank.json')
    const hsPath = join(dir, 'hs.json')
    writeFileSync(bankPath, JSON.stringify({
      probe_model: MODEL,
      layer: 5,
      concepts: { alpha: { vector: [1, 0], threshold: 0.99 } },
      compositions: {
        combo: { inputs: ['alpha'], folds: [{ weights: [1], bias: 0 }], threshold: 0.1 },
      },
    }))
    writeFileSync(hsPath, JSON.stringify({
      probe_model: MODEL,
      layer: 5,
      // cosine with [1,0] is exactly 0.5 — below the 0.99 threshold,
      // so no detection, but the composite still sees the raw score.
      states: [{ token: 'x', position: 0, vector: [0.5, Math.sqrt(0.75)] }],
    }))
    const snap = buildJSpaceSnapshot({ hiddenStatesPath: hsPath, probeBankPath: bankPath })
    // alpha below its 0.99 threshold, so no detections — but the composite
    // still sees the raw score (0.5 cosine -> sigmoid(0.5) = 0.622459).
    assert.equal(snap.detections.length, 0)
    assert.equal(snap.compositions.length, 1)
    assert.equal(snap.compositions[0].probability, 0.622459)
    assert.ok(snap.snapshot_hash)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})
