// Chain Superintelligence (Phase 4) tests. Run with: node --test test/

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync, readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  buildAttestationPayload,
  runProbeAudit,
  buildWavsManifest,
  saveAuditReport,
  CSI_VERSION,
  CSI_COMPONENT_ID,
  CSI_TASK_TYPE,
} from '../src/chain-superintelligence.js'

function tmpFile(dir, name, obj) {
  const p = join(dir, name)
  writeFileSync(p, JSON.stringify(obj), 'utf8')
  return p
}

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
    { token: 'reward', position: 0, vector: [1, 0, 0, 0] },
    { token: 'the', position: 1, vector: [0, 0, 1, 0] },
  ],
}

test('CSI: runProbeAudit produces snapshot and verdict', () => {
  const dir = mkdtempSync(join(tmpdir(), 'csi-'))
  try {
    const bankPath = tmpFile(dir, 'bank.json', PROBE_BANK)
    const statesPath = tmpFile(dir, 'states.json', HIDDEN_STATES_HIT)
    const { snapshot, verdict } = runProbeAudit(statesPath, bankPath)
    assert.equal(snapshot.detections.length, 1)
    assert.equal(snapshot.detections[0].concept, 'reward_hacking')
    assert.equal(verdict.verdict, 'warn')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('CSI: buildAttestationPayload produces deterministic hash in dev-sim mode', () => {
  const dir = mkdtempSync(join(tmpdir(), 'csi-'))
  try {
    const bankPath = tmpFile(dir, 'bank.json', PROBE_BANK)
    const statesPath = tmpFile(dir, 'states.json', HIDDEN_STATES_HIT)
    const { snapshot } = runProbeAudit(statesPath, bankPath)

    const att1 = buildAttestationPayload(snapshot, { mode: 'dev-sim', proposalId: 42 })
    const att2 = buildAttestationPayload(snapshot, { mode: 'dev-sim', proposalId: 42 })

    assert.equal(att1.attestation_hash, att2.attestation_hash)
    assert.equal(att1.data_hash, att2.data_hash)
    assert.equal(att1.proposal_id, 42)
    assert.equal(att1.task_type, CSI_TASK_TYPE)
    assert.equal(att1._csi_meta.mode, 'dev-sim')
    assert.equal(att1._csi_meta.version, CSI_VERSION)
    assert.equal(att1._csi_meta.component_id, CSI_COMPONENT_ID)
    assert.equal(att1._csi_meta.verdict, 'warn')
    assert.equal(att1.proof_base64, null)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('CSI: buildAttestationPayload uses teeAttestationHash when mode=tee', () => {
  const dir = mkdtempSync(join(tmpdir(), 'csi-'))
  try {
    const bankPath = tmpFile(dir, 'bank.json', PROBE_BANK)
    const statesPath = tmpFile(dir, 'states.json', HIDDEN_STATES_HIT)
    const { snapshot } = runProbeAudit(statesPath, bankPath)

    const teeHash = 'abc123deadbeef'
    const att = buildAttestationPayload(snapshot, { mode: 'tee', proposalId: 1, teeAttestationHash: teeHash })
    assert.equal(att.attestation_hash, teeHash)
    assert.equal(att._csi_meta.mode, 'tee')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('CSI: dev-sim and tee modes produce different attestation hashes for same data', () => {
  const dir = mkdtempSync(join(tmpdir(), 'csi-'))
  try {
    const bankPath = tmpFile(dir, 'bank.json', PROBE_BANK)
    const statesPath = tmpFile(dir, 'states.json', HIDDEN_STATES_HIT)
    const { snapshot } = runProbeAudit(statesPath, bankPath)

    const devAtt = buildAttestationPayload(snapshot, { mode: 'dev-sim' })
    const teeAtt = buildAttestationPayload(snapshot, { mode: 'tee', teeAttestationHash: 'fake-tee-hash' })

    assert.notEqual(devAtt.attestation_hash, teeAtt.attestation_hash)
    assert.equal(devAtt.data_hash, teeAtt.data_hash, 'data_hash should be same (same snapshot)')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('CSI: buildWavsManifest produces valid WAVS config', () => {
  const manifest = buildWavsManifest({
    componentPath: '/components/j-lens-probe.wasm',
    chainId: 'uni-7',
    contractAddr: 'juno1contract...',
  })
  assert.equal(manifest.component.id, CSI_COMPONENT_ID)
  assert.equal(manifest.trigger.type, 'cosmos_event')
  assert.equal(manifest.trigger.chain_id, 'uni-7')
  assert.equal(manifest.submission.contract, 'juno1contract...')
  assert.equal(manifest.tee.required, true)
  assert.deepEqual(manifest.tee.hardware, ['sgx', 'nitro', 'sev-snp', 'tdx'])
  assert.equal(manifest.tee.fallback, 'dev-sim')
})

test('CSI: saveAuditReport writes valid JSON with all fields', () => {
  const dir = mkdtempSync(join(tmpdir(), 'csi-'))
  try {
    const bankPath = tmpFile(dir, 'bank.json', PROBE_BANK)
    const statesPath = tmpFile(dir, 'states.json', HIDDEN_STATES_HIT)
    const { snapshot, verdict } = runProbeAudit(statesPath, bankPath)
    const attestation = buildAttestationPayload(snapshot, { mode: 'dev-sim', proposalId: 7 })

    const reportPath = join(dir, 'report.json')
    saveAuditReport(reportPath, {
      snapshot,
      verdict,
      attestation,
      txResult: null,
      text: 'test draft text',
      endpoint: 'http://localhost:8000',
      mode: 'dev-sim',
    })

    const report = JSON.parse(readFileSync(reportPath, 'utf8'))
    assert.equal(report.csi_version, CSI_VERSION)
    assert.equal(report.mode, 'dev-sim')
    assert.equal(report.attestation.proposal_id, 7)
    assert.equal(report.snapshot.detections.length, 1)
    assert.equal(report.verdict.verdict, 'warn')
    assert.equal(report.tx_result, null)
    assert.ok(report.input_text_hash)
    assert.ok(report.timestamp)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('CSI: attestation data_hash matches snapshot_hash (both derived from canonV1)', () => {
  const dir = mkdtempSync(join(tmpdir(), 'csi-'))
  try {
    const bankPath = tmpFile(dir, 'bank.json', PROBE_BANK)
    const statesPath = tmpFile(dir, 'states.json', HIDDEN_STATES_HIT)
    const { snapshot } = runProbeAudit(statesPath, bankPath)
    const att = buildAttestationPayload(snapshot, { mode: 'dev-sim' })

    // data_hash is canonHash(snapshot), snapshot_hash is also canonHash(snapshot)
    // (set in d1-probe.js buildJSpaceSnapshot). They should be equal.
    assert.equal(att.data_hash, snapshot.snapshot_hash)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})
