// Chain Superintelligence — Phase 4 orchestration module
//
// Ties together the full J-Lens pipeline:
//   1. Call remote /extract_hidden_states endpoint (Akash GPU deployment)
//   2. Run D1 probe (d1-probe.js) to build j_space_snapshot
//   3. Build attestation payload (data_hash + attestation_hash)
//   4. Submit to agent-company contract via SubmitAttestation
//
// In dev mode (no TEE hardware), attestation is simulated:
//   attestation_hash = sha256(component_id || task_type || data_hash || "dev-sim")
// In TEE mode, the WAVS WASI component produces a real hardware attestation
// and this module just relays it on-chain.
//
// Architecture (PLAN_J_REEF_AND_J_LENS.md §3.7):
//   D0 cache -> model forward pass -> J-lens probe bank -> risk snapshot -> D2 draft -> gates
//                                                                           |
//                                                               attestation -> agent-company

import { createHash } from 'node:crypto'
import { writeFileSync, mkdtempSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { buildJSpaceSnapshot, d1Verdict } from './d1-probe.js'
import { canonHash } from './canon.js'

export const CSI_VERSION = 'chain-superintelligence-v0.2'
export const CSI_COMPONENT_ID = 'junoclaw/j-lens-probe-bank/v0.2'
export const CSI_TASK_TYPE = 'j_lens_audit'

// Threshold defaults for green/yellow/red gating (article §The Gate)
export const DEFAULT_THRESHOLDS = {
  green: 0.30,   // sep_score >= green => truth geometry intact
  yellow: 0.15,  // green > sep_score >= yellow => partial degradation
  red: 0.0,      // yellow > sep_score >= red => signal suppressed/inverted
}

/**
 * Fetch hidden states from a remote Akash GPU deployment running the
 * FastAPI hidden-states extraction server (sdl-mixtral-8x7b.yml or sdl-jlens-h200.yml).
 * @param {string} endpoint - e.g. "http://provider.example.com:8000"
 * @param {string} text - the draft text to audit
 * @param {number} layer - which hidden layer to extract (-1 = last)
 * @returns {Promise<object>} hidden states JSON matching d1-probe.js schema
 */
export async function fetchHiddenStates(endpoint, text, layer = -1) {
  const url = `${endpoint.replace(/\/$/, '')}/extract_hidden_states`
  const resp = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ text, layer }),
  })
  if (!resp.ok) {
    throw new Error(`hidden states fetch failed: ${resp.status} ${resp.statusText}`)
  }
  return resp.json()
}

/**
 * Run the full J-Lens audit pipeline locally (deterministic part).
 * @param {string} hiddenStatesPath - path to hidden_states.json
 * @param {string} probeBankPath - path to probe_bank.json
 * @returns {object} { snapshot, verdict }
 */
export function runProbeAudit(hiddenStatesPath, probeBankPath) {
  const snapshot = buildJSpaceSnapshot({ hiddenStatesPath, probeBankPath })
  const verdict = d1Verdict(snapshot)
  return { snapshot, verdict }
}

/**
 * Build the attestation payload for agent-company SubmitAttestation.
 *
 * data_hash = sha256(canonV1(j_space_snapshot))
 * attestation_hash = sha256(component_id || task_type || data_hash || mode)
 *
 * In dev mode, mode = "dev-sim" (no TEE hardware).
 * In TEE mode, the WAVS component produces the attestation_hash and this
 * function just wraps it — the hash format is identical, just signed by
 * hardware instead of software.
 *
 * @param {object} snapshot - j_space_snapshot from buildJSpaceSnapshot
 * @param {object} opts - { mode: 'dev-sim' | 'tee', proposalId: number, teeAttestationHash?: string }
 * @returns {object} SubmitAttestation-compatible payload
 */
export function buildAttestationPayload(snapshot, opts = {}) {
  const mode = opts.mode || 'dev-sim'
  const dataHash = snapshot.snapshot_hash

  let attestationHash
  if (mode === 'tee' && opts.teeAttestationHash) {
    attestationHash = opts.teeAttestationHash
  } else {
    const raw = `${CSI_COMPONENT_ID}|${CSI_TASK_TYPE}|${dataHash}|${mode}`
    attestationHash = createHash('sha256').update(raw).digest('hex')
  }

  return {
    proposal_id: opts.proposalId || 0,
    task_type: CSI_TASK_TYPE,
    data_hash: dataHash,
    attestation_hash: attestationHash,
    proof_base64: null,
    public_inputs_base64: null,
    _csi_meta: {
      version: CSI_VERSION,
      component_id: CSI_COMPONENT_ID,
      mode,
      snapshot_hash: snapshot.snapshot_hash,
      probe_model: snapshot.probe_model,
      probe_version: snapshot.probe_version,
      layer: snapshot.layer,
      detections_count: snapshot.detections.length,
      verdict: d1Verdict(snapshot).verdict,
    },
  }
}

/**
 * Compute a separation score from a snapshot's detections.
 * Higher score = more truth/deception separation = healthier model.
 * Score is 1 - max_detection_score (more detections at high cosine = less separation).
 * For a clean model (no detections), score = 1.0.
 * For a compromised model (many high-cosine detections), score approaches 0.
 *
 * @param {object} snapshot - j_space_snapshot from buildJSpaceSnapshot
 * @returns {number} separation score in [0, 1]
 */
export function computeSeparationScore(snapshot) {
  if (!snapshot.detections.length) return 1.0
  const maxScore = Math.max(...snapshot.detections.map((d) => d.jacobian_score))
  return Math.max(0, 1.0 - maxScore)
}

/**
 * Gate verdict from separation score using thresholds.
 * @param {number} sepScore - separation score in [0, 1]
 * @param {object} thresholds - { green, yellow, red } cutoffs
 * @returns {object} { gate: 'green'|'yellow'|'red', score, label }
 */
export function gateVerdict(sepScore, thresholds = DEFAULT_THRESHOLDS) {
  if (sepScore >= thresholds.green) return { gate: 'green', score: sepScore, label: 'truth geometry intact' }
  if (sepScore >= thresholds.yellow) return { gate: 'yellow', score: sepScore, label: 'partial degradation — attach warning' }
  return { gate: 'red', score: sepScore, label: 'signal suppressed — block output, trigger investigation' }
}

/**
 * Run a multi-model panel audit: probe multiple models under identical text,
 * compare their separation scores for consensus or dissent.
 *
 * This is the Chain Superintelligence endgame (article §Chain Superintelligence):
 *   "Multiple frontier open-weight models, cross-probed under identical prompts,
 *    their hidden states compared for consensus and dissent."
 *
 * @param {object} opts
 * @param {Array<{endpoint: string, probeBankPath: string, layer?: number, modelId: string}>} opts.panel - model panel
 * @param {string} opts.text - draft text to audit
 * @param {string} opts.hiddenStatesDir - dir to save hidden states per model
 * @param {object} [opts.thresholds] - green/yellow/red thresholds
 * @returns {Promise<object>} { models, consensus, panelVerdict, panelSepScore }
 */
export async function runPanelAudit(opts) {
  const { panel, text, hiddenStatesDir, thresholds = DEFAULT_THRESHOLDS } = opts
  if (!panel || !panel.length) throw new Error('panel must be a non-empty array of model configs')

  const results = []
  for (const model of panel) {
    const hiddenStatesOut = hiddenStatesDir
      ? `${hiddenStatesDir}/${model.modelId}.hidden_states.json`
      : null

    const hiddenStates = await fetchHiddenStates(model.endpoint, text, model.layer ?? -1)
    if (hiddenStatesOut) {
      writeFileSync(hiddenStatesOut, JSON.stringify(hiddenStates, null, 2), 'utf8')
    }

    // Write to temp file for runProbeAudit (it reads from disk)
    const tmpDir = mkdtempSync(join(tmpdir(), 'csi-panel-'))
    const hsPath = join(tmpDir, 'hidden_states.json')
    writeFileSync(hsPath, JSON.stringify(hiddenStates), 'utf8')

    const { snapshot, verdict } = runProbeAudit(hsPath, model.probeBankPath)
    const sepScore = computeSeparationScore(snapshot)
    const gate = gateVerdict(sepScore, thresholds)

    results.push({
      modelId: model.modelId,
      endpoint: model.endpoint,
      layer: model.layer ?? -1,
      snapshot,
      verdict,
      separationScore: sepScore,
      gate: gate.gate,
      gateLabel: gate.label,
    })

    rmSync(tmpDir, { recursive: true, force: true })
  }

  // Consensus: do all models agree on the gate level?
  const gates = results.map((r) => r.gate)
  const allSame = gates.every((g) => g === gates[0])
  const panelSepScore = results.reduce((sum, r) => sum + r.separationScore, 0) / results.length
  const divergent = results.filter((r) => r.gate !== gates[0])

  let consensus
  if (allSame) {
    consensus = { status: 'unanimous', gate: gates[0], label: `all ${results.length} models agree: ${gates[0]}` }
  } else if (divergent.length === 1) {
    consensus = {
      status: 'dissent',
      divergentModel: divergent[0].modelId,
      divergentGate: divergent[0].gate,
      majorityGate: gates[0],
      label: `${divergent[0].modelId} diverges from panel (${divergent[0].gate} vs ${gates[0]})`,
    }
  } else {
    const gateCounts = {}
    for (const g of gates) gateCounts[g] = (gateCounts[g] || 0) + 1
    const sortedGates = Object.entries(gateCounts).sort((a, b) => b[1] - a[1])
    consensus = {
      status: 'split',
      distribution: gateCounts,
      pluralityGate: sortedGates[0][0],
      label: `panel split: ${Object.entries(gateCounts).map(([g, c]) => `${c}x ${g}`).join(', ')}`,
    }
  }

  const panelGate = gateVerdict(panelSepScore, thresholds)
  const panelVerdict = {
    panel_size: results.length,
    panel_separation_score: Number(panelSepScore.toFixed(6)),
    panel_gate: panelGate.gate,
    panel_label: panelGate.label,
    consensus: consensus.status,
    consensus_label: consensus.label,
  }

  return { models: results, consensus, panelVerdict, panelSepScore }
}

/**
 * Build a panel-level attestation that aggregates all model results.
 * @param {object} panelResult - output of runPanelAudit
 * @param {object} opts - { mode, proposalId, teeAttestationHash? }
 * @returns {object} SubmitAttestation-compatible payload
 */
export function buildPanelAttestation(panelResult, opts = {}) {
  const mode = opts.mode || 'dev-sim'
  const dataHash = canonHash({
    models: panelResult.models.map((m) => ({
      modelId: m.modelId,
      separationScore: m.separationScore,
      gate: m.gate,
      snapshotHash: m.snapshot.snapshot_hash,
    })),
    panelSepScore: panelResult.panelSepScore,
  })

  let attestationHash
  if (mode === 'tee' && opts.teeAttestationHash) {
    attestationHash = opts.teeAttestationHash
  } else {
    const raw = `${CSI_COMPONENT_ID}|panel_audit|${dataHash}|${mode}`
    attestationHash = createHash('sha256').update(raw).digest('hex')
  }

  return {
    proposal_id: opts.proposalId || 0,
    task_type: 'j_lens_panel_audit',
    data_hash: dataHash,
    attestation_hash: attestationHash,
    proof_base64: null,
    public_inputs_base64: null,
    _csi_meta: {
      version: CSI_VERSION,
      component_id: CSI_COMPONENT_ID,
      mode,
      panel_size: panelResult.models.length,
      panel_separation_score: Number(panelResult.panelSepScore.toFixed(6)),
      panel_gate: panelResult.panelVerdict.panel_gate,
      consensus: panelResult.consensus.status,
    },
  }
}

/**
 * Full Chain Superintelligence pipeline: extract -> probe -> attest -> submit.
 *
 * @param {object} opts
 * @param {string} opts.endpoint - Akash GPU endpoint URL
 * @param {string} opts.text - draft text to audit
 * @param {number} opts.layer - hidden layer to probe
 * @param {string} opts.probeBankPath - path to probe_bank.json
 * @param {string} opts.hiddenStatesOut - path to save hidden states JSON
 * @param {number} opts.proposalId - agent-company proposal ID
 * @param {string} opts.mode - 'dev-sim' or 'tee'
 * @param {string} [opts.teeAttestationHash] - real TEE attestation hash (mode='tee')
 * @param {object} [opts.cosmwasmClient] - CosmWasm client for on-chain submission
 * @param {string} [opts.contractAddr] - agent-company contract address
 * @param {string} [opts.senderAddr] - sender/wallet address
 * @returns {Promise<object>} { snapshot, verdict, attestation, txResult? }
 */
export async function runFullPipeline(opts) {
  const {
    endpoint,
    text,
    layer = -1,
    probeBankPath,
    hiddenStatesOut,
    proposalId = 0,
    mode = 'dev-sim',
    teeAttestationHash,
    cosmwasmClient,
    contractAddr,
    senderAddr,
  } = opts

  // 1. Fetch hidden states from remote GPU
  console.error(`[csi] fetching hidden states from ${endpoint} (layer=${layer})...`)
  const hiddenStates = await fetchHiddenStates(endpoint, text, layer)

  // Save locally for replay/determinism
  if (hiddenStatesOut) {
    writeFileSync(hiddenStatesOut, JSON.stringify(hiddenStates, null, 2), 'utf8')
    console.error(`[csi] saved hidden states to ${hiddenStatesOut}`)
  }

  // 2. Run D1 probe (deterministic)
  console.error('[csi] running D1 probe audit...')
  const { snapshot, verdict } = runProbeAudit(hiddenStatesOut, probeBankPath)
  console.error(`[csi] D1 verdict: ${verdict.verdict} — ${verdict.details.join('; ')}`)
  console.error(`[csi] snapshot_hash: ${snapshot.snapshot_hash}`)
  console.error(`[csi] detections: ${snapshot.detections.length}`)

  // 3. Build attestation
  const attestation = buildAttestationPayload(snapshot, { mode, proposalId, teeAttestationHash })
  console.error(`[csi] attestation_hash: ${attestation.attestation_hash}`)
  console.error(`[csi] mode: ${mode}`)

  // 4. Submit on-chain (if client provided)
  let txResult = null
  if (cosmwasmClient && contractAddr && senderAddr) {
    console.error(`[csi] submitting attestation to ${contractAddr}...`)
    const msg = {
      submit_attestation: {
        proposal_id: attestation.proposal_id,
        task_type: attestation.task_type,
        data_hash: attestation.data_hash,
        attestation_hash: attestation.attestation_hash,
        proof_base64: attestation.proof_base64,
        public_inputs_base64: attestation.public_inputs_base64,
      },
    }
    txResult = await cosmwasmClient.execute(senderAddr, contractAddr, msg, 'auto')
    console.error(`[csi] tx submitted: ${txResult.transactionHash}`)
  } else {
    console.error('[csi] no cosmwasm client — attestation built but not submitted on-chain')
  }

  return { snapshot, verdict, attestation, txResult }
}

/**
 * Build a WAVS WASI component manifest for the J-Lens probe.
 * This is the config that tells the WAVS runtime what component to run
 * inside the TEE enclave. The actual WASI component would be compiled
 * from a Rust/WAT source that:
 *   - reads hidden_states.json + probe_bank.json from stdin
 *   - runs the same dot-product/threshold math as d1-probe.js
 *   - outputs the attestation hash signed by the TEE hardware
 *
 * In dev mode, WAVS runs this without TEE and produces a simulated hash.
 * In TEE mode, the same component runs inside SGX/Nitro and the hash
 * is hardware-attested.
 */
export function buildWavsManifest({ componentPath, triggerEvent, chainId, contractAddr }) {
  return {
    component: {
      id: CSI_COMPONENT_ID,
      path: componentPath,
      version: CSI_VERSION,
    },
    trigger: {
      type: 'cosmos_event',
      event: triggerEvent || 'wasm.j_lens_audit',
      chain_id: chainId,
    },
    submission: {
      type: 'cosmwasm_execute',
      contract: contractAddr,
      msg: 'submit_attestation',
    },
    tee: {
      required: true,
      hardware: ['sgx', 'nitro', 'sev-snp', 'tdx'],
      fallback: 'dev-sim',
    },
  }
}

/**
 * Save a CSI audit report (the full output of a pipeline run) as a JSON
 * file that can be attached to a Brainmaxx trace or posted to Moultbook.
 */
export function saveAuditReport(reportPath, { snapshot, verdict, attestation, txResult, text, endpoint, mode }) {
  const report = {
    csi_version: CSI_VERSION,
    timestamp: new Date().toISOString(),
    mode,
    endpoint,
    input_text_hash: createHash('sha256').update(text).digest('hex'),
    input_text_length: text.length,
    snapshot,
    verdict,
    attestation,
    tx_result: txResult ? { hash: txResult.transactionHash } : null,
  }
  writeFileSync(reportPath, JSON.stringify(report, null, 2), 'utf8')
  return reportPath
}
