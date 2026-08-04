// Tests for CSI v0.2: separation score, gate verdict, panel audit, panel attestation.
// Run with: node --test test/csi-v02.test.js

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  computeSeparationScore,
  gateVerdict,
  DEFAULT_THRESHOLDS,
  buildPanelAttestation,
  CSI_VERSION,
} from '../src/chain-superintelligence.js'

test('CSI v0.2: computeSeparationScore returns 1.0 for clean snapshot (no detections)', () => {
  const snapshot = { detections: [] }
  assert.equal(computeSeparationScore(snapshot), 1.0)
})

test('CSI v0.2: computeSeparationScore returns 1 - max_score for detections', () => {
  const snapshot = {
    detections: [
      { jacobian_score: 0.8 },
      { jacobian_score: 0.6 },
    ],
  }
  assert.ok(Math.abs(computeSeparationScore(snapshot) - 0.2) < 1e-10)
})

test('CSI v0.2: computeSeparationScore clamps to 0', () => {
  const snapshot = {
    detections: [{ jacobian_score: 1.0 }],
  }
  assert.equal(computeSeparationScore(snapshot), 0)
})

test('CSI v0.2: gateVerdict returns green for high separation', () => {
  const gate = gateVerdict(0.5)
  assert.equal(gate.gate, 'green')
  assert.ok(gate.label.includes('intact'))
})

test('CSI v0.2: gateVerdict returns yellow for mid separation', () => {
  const gate = gateVerdict(0.2)
  assert.equal(gate.gate, 'yellow')
  assert.ok(gate.label.includes('degradation'))
})

test('CSI v0.2: gateVerdict returns red for low separation', () => {
  const gate = gateVerdict(0.05)
  assert.equal(gate.gate, 'red')
  assert.ok(gate.label.includes('block'))
})

test('CSI v0.2: gateVerdict respects custom thresholds', () => {
  const custom = { green: 0.5, yellow: 0.25, red: 0.0 }
  assert.equal(gateVerdict(0.4, custom).gate, 'yellow')
  assert.equal(gateVerdict(0.6, custom).gate, 'green')
  assert.equal(gateVerdict(0.1, custom).gate, 'red')
})

test('CSI v0.2: DEFAULT_THRESHOLDS has green/yellow/red', () => {
  assert.ok(typeof DEFAULT_THRESHOLDS.green === 'number')
  assert.ok(typeof DEFAULT_THRESHOLDS.yellow === 'number')
  assert.ok(typeof DEFAULT_THRESHOLDS.red === 'number')
  assert.ok(DEFAULT_THRESHOLDS.green > DEFAULT_THRESHOLDS.yellow)
})

test('CSI v0.2: buildPanelAttestation produces deterministic hash in dev-sim', () => {
  const panelResult = {
    models: [
      {
        modelId: 'model-a',
        separationScore: 0.8,
        gate: 'green',
        snapshot: { snapshot_hash: 'abc123' },
      },
      {
        modelId: 'model-b',
        separationScore: 0.7,
        gate: 'green',
        snapshot: { snapshot_hash: 'def456' },
      },
    ],
    panelSepScore: 0.75,
    panelVerdict: {
      panel_size: 2,
      panel_separation_score: 0.75,
      panel_gate: 'green',
      panel_label: 'truth geometry intact',
      consensus: 'unanimous',
      consensus_label: 'all 2 models agree: green',
    },
    consensus: { status: 'unanimous', gate: 'green', label: 'all 2 models agree: green' },
  }

  const att1 = buildPanelAttestation(panelResult, { mode: 'dev-sim', proposalId: 1 })
  const att2 = buildPanelAttestation(panelResult, { mode: 'dev-sim', proposalId: 1 })

  assert.equal(att1.attestation_hash, att2.attestation_hash)
  assert.equal(att1.data_hash, att2.data_hash)
  assert.equal(att1.task_type, 'j_lens_panel_audit')
  assert.equal(att1._csi_meta.panel_size, 2)
  assert.equal(att1._csi_meta.consensus, 'unanimous')
  assert.equal(att1._csi_meta.version, CSI_VERSION)
})

test('CSI v0.2: buildPanelAttestation uses teeAttestationHash when mode=tee', () => {
  const panelResult = {
    models: [{ modelId: 'm', separationScore: 0.5, gate: 'green', snapshot: { snapshot_hash: 'h' } }],
    panelSepScore: 0.5,
    panelVerdict: { panel_gate: 'green', consensus: 'unanimous' },
    consensus: { status: 'unanimous' },
  }

  const teeHash = 'deadbeef1234'
  const att = buildPanelAttestation(panelResult, { mode: 'tee', teeAttestationHash: teeHash })
  assert.equal(att.attestation_hash, teeHash)
  assert.equal(att._csi_meta.mode, 'tee')
})

test('CSI v0.2: panel attestation data_hash differs for different panel results', () => {
  const result1 = {
    models: [{ modelId: 'a', separationScore: 0.9, gate: 'green', snapshot: { snapshot_hash: 'h1' } }],
    panelSepScore: 0.9,
    panelVerdict: {},
    consensus: {},
  }
  const result2 = {
    models: [{ modelId: 'b', separationScore: 0.5, gate: 'green', snapshot: { snapshot_hash: 'h2' } }],
    panelSepScore: 0.5,
    panelVerdict: {},
    consensus: {},
  }

  const att1 = buildPanelAttestation(result1, { mode: 'dev-sim' })
  const att2 = buildPanelAttestation(result2, { mode: 'dev-sim' })
  assert.notEqual(att1.data_hash, att2.data_hash)
})

test('CSI v0.2: version is v0.2', () => {
  assert.match(CSI_VERSION, /v0\.2/)
})
