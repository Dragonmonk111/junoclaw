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

/**
 * Score every (hidden-state, concept) pair. Deterministic: iterates
 * states[] and concepts (sorted by name) in a fixed order, rounds scores to
 * 6 decimal places to avoid platform float-formatting drift across runs.
 */
export function scoreHiddenStates(hiddenStatesData, probeBank) {
  if (hiddenStatesData.probe_model !== probeBank.probe_model) {
    throw new Error(
      `probe/model mismatch: hidden states from "${hiddenStatesData.probe_model}", probe bank trained on "${probeBank.probe_model}" — J-lens probes do not transfer across models (spec §3.5)`
    )
  }
  if (hiddenStatesData.layer !== probeBank.layer) {
    throw new Error(`layer mismatch: hidden states at layer ${hiddenStatesData.layer}, probe bank built at layer ${probeBank.layer}`)
  }

  const conceptNames = Object.keys(probeBank.concepts).sort()
  const detections = []

  for (const state of hiddenStatesData.states) {
    for (const name of conceptNames) {
      const concept = probeBank.concepts[name]
      const score = Number(cosineScore(state.vector, concept.vector).toFixed(6))
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

  return detections
}

/**
 * Build a full j_space_snapshot for attachment to a Brainmaxx trace. Fails
 * safe per spec §3.5: an error here should block export (the caller treats
 * a thrown error as "cannot attach snapshot"), never silently skip the check.
 */
export function buildJSpaceSnapshot({ hiddenStatesPath, probeBankPath }) {
  const hiddenStatesData = loadHiddenStates(hiddenStatesPath)
  const probeBank = loadProbeBank(probeBankPath)
  const detections = scoreHiddenStates(hiddenStatesData, probeBank)

  const forbidden_concepts = Object.keys(probeBank.concepts).sort()
  const snapshot = {
    probe_model: probeBank.probe_model,
    probe_version: probeBank.probe_version || D1_PROBE_VERSION,
    layer: probeBank.layer,
    forbidden_concepts,
    detections,
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
