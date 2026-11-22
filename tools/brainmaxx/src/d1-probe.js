// D1 probe — J-Lens linear-readout audit layer (PLAN_J_REEF_AND_J_LENS.md §3,
// A18c-9 Phase 3). Local, operator-side, feature-flagged, default OFF.
//
// Brainmaxx D0 never runs model inference itself (spec: no embedded model).
// D1 does not change that rule for the deterministic core — it is a separate,
// optional layer that *consumes* hidden states produced by an external,
// non-deterministic forward pass (tools/brainmaxx/j-lens/*.py) and applies a
// deterministic linear readout + threshold check to them. Same probe bank +
// same hidden-states file => byte-identical j_space_snapshot, forever — that
// part IS deterministic and IS replayable, even though the forward pass that
// produced the hidden states is not.
//
// Method (v0.1): probe vectors are diff-of-means directions over contrastive
// example sets (tools/brainmaxx/j-lens/build_probe_bank.py), not a full
// Jacobian-of-logit gradient. This is the standard "contrastive activation
// direction" baseline from the interpretability literature — cheaper to
// compute, no autograd required, and it is what PLAN_J_REEF_AND_J_LENS.md §3.6
// describes as the starting point before phrase-level / tuned-lens probes.

import { readFileSync } from 'node:fs'
import { canonHash } from './canon.js'

export const D1_PROBE_VERSION = 'j-lens-v0.1'

/**
 * Probe bank shape (JSON file, produced by build_probe_bank.py):
 * {
 *   "probe_version": "j-lens-v0.1",
 *   "probe_model": "qwen2.5-0.5b-instruct",   // model the vectors were trained on
 *   "layer": 12,                               // layer index vectors were built at
 *   "concepts": {
 *     "reward_hacking": { "vector": [0.01, -0.03, ...], "threshold": 0.70 },
 *     "ignore_instructions": { "vector": [...], "threshold": 0.65 }
 *   }
 * }
 */
export function loadProbeBank(path) {
  const bank = JSON.parse(readFileSync(path, 'utf8'))
  if (!bank.concepts || typeof bank.concepts !== 'object') {
    throw new Error(`probe bank at ${path} has no concepts{}`)
  }
  for (const [name, c] of Object.entries(bank.concepts)) {
    if (!Array.isArray(c.vector) || !c.vector.length) throw new Error(`concept ${name}: vector must be a non-empty array`)
    if (typeof c.threshold !== 'number') throw new Error(`concept ${name}: threshold must be a number`)
  }
  return bank
}

/**
 * Hidden-states shape (JSON file, produced by extract_hidden_states.py):
 * {
 *   "probe_model": "qwen2.5-0.5b-instruct",
 *   "layer": 12,
 *   "states": [
 *     { "token": "reward", "position": 47, "vector": [0.02, 0.11, ...] },
 *     ...
 *   ]
 * }
 */
export function loadHiddenStates(path) {
  const data = JSON.parse(readFileSync(path, 'utf8'))
  if (!Array.isArray(data.states) || !data.states.length) {
    throw new Error(`hidden-states file at ${path} has no states[]`)
  }
  return data
}

function dot(a, b) {
  if (a.length !== b.length) throw new Error(`vector length mismatch: ${a.length} vs ${b.length}`)
  let sum = 0
  for (let i = 0; i < a.length; i++) sum += a[i] * b[i]
  return sum
}

function norm(a) {
  return Math.sqrt(dot(a, a))
}

/** Cosine similarity, clamped to [-1, 1] to absorb float drift. */
function cosineScore(a, b) {
  const na = norm(a)
  const nb = norm(b)
  if (na === 0 || nb === 0) return 0
  const raw = dot(a, b) / (na * nb)
  return Math.max(-1, Math.min(1, raw))
}

/** Logistic sigmoid with clamping (J2). */
function sigmoid(z) {
  return 1 / (1 + Math.exp(-Math.max(-60, Math.min(60, z))))
}

/** State vector lookup supporting single- and multi-layer states files. */
function stateVector(state, layer) {
  if (state.vectors && typeof state.vectors === 'object') {
    return state.vectors[String(layer)]
  }
  return state.vector
}

/**
 * Per-(state, concept) score under the concept's method. Deterministic;
 * rounds happen at the caller. Methods:
 *  - cosine-dom (J1, default): cosine(h, v)
 *  - logistic-l2-v0.1 (J2): sigmoid(h . w + b)
 *  - trajectory-dom-v0.1 (J3): weighted per-layer cosine + rising-slope bonus
 *  - jacobian-v0.1 (J5): same path as trajectory (uniform weights, slope 0)
 */
function scoreConcept(state, name, concept, bank) {
  const method = concept.method || 'j-lens-v0.1'
  if (method === 'logistic-l2-v0.1') {
    const h = stateVector(state, bank.layer)
    if (!h) return null
    return sigmoid(dot(h, concept.vector) + (concept.bias || 0))
  }
  if (concept.layers && typeof concept.layers === 'object') {
    const layerKeys = Object.keys(concept.layers).map(Number).sort((a, b) => a - b)
    let base = 0
    let wsum = 0
    const perLayer = []
    for (const l of layerKeys) {
      const h = stateVector(state, l)
      if (!h) return null
      const w = (concept.layer_weights && concept.layer_weights[String(l)]) ?? 1 / layerKeys.length
      const s = cosineScore(h, concept.layers[String(l)])
      perLayer.push(s)
      base += w * s
      wsum += w
    }
    if (wsum === 0) return 0
    base /= wsum
    let slope = 0
    if (perLayer.length >= 2) {
      const n = perLayer.length
      const xs = layerKeys
      const xm = xs.reduce((a, b) => a + b, 0) / n
      const ym = perLayer.reduce((a, b) => a + b, 0) / n
      let num = 0
      let den = 0
      for (let i = 0; i < n; i++) {
        num += (xs[i] - xm) * (perLayer[i] - ym)
        den += (xs[i] - xm) * (xs[i] - xm)
      }
      slope = den === 0 ? 0 : num / den
    }
    return base + (concept.slope_weight || 0) * Math.max(0, slope)
  }
  const h = stateVector(state, bank.layer)
  if (!h) return null
  return cosineScore(h, concept.vector)
}

/** Evaluate composite classifiers over base concept scores (J6). */
export function scoreCompositions(baseScores, compositions) {
  const out = []
  for (const [name, comp] of Object.entries(compositions || {}).sort()) {
    const foldPs = comp.folds.map((f) => {
      const z = comp.inputs.reduce((acc, inp, i) => acc + f.weights[i] * (baseScores[inp] || 0), f.bias)
      return sigmoid(z)
    })
    if (!foldPs.length) continue
    const mean = foldPs.reduce((a, b) => a + b, 0) / foldPs.length
    const std = Math.sqrt(foldPs.reduce((a, b) => a + (b - mean) * (b - mean), 0) / foldPs.length)
    out.push({
      concept: name,
      probability: Number(mean.toFixed(6)),
      uncertainty: Number(std.toFixed(6)),
      threshold: comp.threshold,
      detected: mean >= comp.threshold,
    })
  }
  return out
}

/**
 * Score every (hidden-state, concept) pair. Deterministic: iterates
 * states[] and concepts (sorted by name) in a fixed order, rounds scores to
 * 6 decimal places to avoid platform float-formatting drift across runs.
 */
export function scoreAll(hiddenStatesData, probeBank) {
  if (hiddenStatesData.probe_model !== probeBank.probe_model) {
    throw new Error(
      `probe/model mismatch: hidden states from "${hiddenStatesData.probe_model}", probe bank trained on "${probeBank.probe_model}" — J-lens probes do not transfer across models (spec §3.5)`
    )
  }
  if (probeBank.layer !== undefined && hiddenStatesData.layer !== undefined && hiddenStatesData.layer !== probeBank.layer) {
    throw new Error(`layer mismatch: hidden states at layer ${hiddenStatesData.layer}, probe bank built at layer ${probeBank.layer}`)
  }

  const conceptNames = Object.keys(probeBank.concepts).sort()
  const detections = []
  const maxScores = {}

  for (const state of hiddenStatesData.states) {
    for (const name of conceptNames) {
      const concept = probeBank.concepts[name]
      const raw = scoreConcept(state, name, concept, probeBank)
      if (raw === null) continue // this state lacks a layer the concept needs
      const score = Number(raw.toFixed(6))
      maxScores[name] = Math.max(maxScores[name] || 0, score)
      if (score >= concept.threshold) {
        detections.push({
          concept: name,
          token: state.token,
          position: state.position,
          jacobian_score: score,
          threshold: concept.threshold,
        })
      }
    }
  }

  return { detections, maxScores }
}

export function scoreHiddenStates(hiddenStatesData, probeBank) {
  return scoreAll(hiddenStatesData, probeBank).detections
}

/**
 * Build a full j_space_snapshot for attachment to a Brainmaxx trace. Fails
 * safe per spec §3.5: an error here should block export (the caller treats
 * a thrown error as "cannot attach snapshot"), never silently skip the check.
 */
export function buildJSpaceSnapshot({ hiddenStatesPath, probeBankPath }) {
  const hiddenStatesData = loadHiddenStates(hiddenStatesPath)
  const probeBank = loadProbeBank(probeBankPath)
  const { detections, maxScores } = scoreAll(hiddenStatesData, probeBank)

  const compositions = scoreCompositions(maxScores, probeBank.compositions)

  const forbidden_concepts = Object.keys(probeBank.concepts).sort()
  const snapshot = {
    probe_model: probeBank.probe_model,
    probe_version: probeBank.probe_version || D1_PROBE_VERSION,
    layer: probeBank.layer,
    layers: probeBank.layers,
    method: probeBank.method,
    forbidden_concepts,
    detections,
    compositions,
  }
  // snapshot_hash lets a third party verify "this exact snapshot" without
  // re-running the probe — same pattern as pack_hash/run_id elsewhere in
  // Brainmaxx (canonV1 + sha256).
  snapshot.snapshot_hash = canonHash(snapshot)
  return snapshot
}

/**
 * Risk verdict for the D1 layer, per spec §3.5: warn on any detection,
 * never silently filter. "red" (block) is reserved for a future
 * high-confidence-threshold policy; v0.1 always warns, never blocks, so an
 * operator must explicitly decide to treat a J-lens hit as fatal.
 */
export function d1Verdict(snapshot) {
  if (!snapshot.detections.length) {
    return { gate: 'D1', verdict: 'pass', details: ['no forbidden concepts detected'] }
  }
  const summary = snapshot.detections.map((d) => `${d.concept}@${d.position} (${d.jacobian_score} >= ${d.threshold})`)
  return { gate: 'D1', verdict: 'warn', details: [`j-lens detections (not fatal in v0.1): ${summary.join(', ')}`] }
}
