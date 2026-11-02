// Canonical threshold fixture parity test.
// Loads j-lens/fixtures/thresholds.v1.json — the SAME file the Rust
// gate tests load — and verifies gateVerdict() agrees case-for-case.
// If the two sides ever diverge, this test (and the Rust one) fails.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

import { gateVerdict, DEFAULT_THRESHOLDS } from '../src/chain-superintelligence.js'

const here = dirname(fileURLToPath(import.meta.url))
const FIXTURE = join(here, '..', 'j-lens', 'fixtures', 'thresholds.v1.json')

const fixture = JSON.parse(readFileSync(FIXTURE, 'utf8'))

test('fixture declares canonical score semantics', () => {
  assert.equal(fixture.score_semantics, 'jlens.integrity.v1')
})

test('default thresholds match fixture', () => {
  assert.equal(DEFAULT_THRESHOLDS.green, fixture.thresholds.green)
  assert.equal(DEFAULT_THRESHOLDS.yellow, fixture.thresholds.yellow)
})

for (const c of fixture.cases) {
  test(`integrity=${c.integrity} → ${c.expected_gate} (${c.note})`, () => {
    const verdict = gateVerdict(c.integrity, {
      green: fixture.thresholds.green,
      yellow: fixture.thresholds.yellow,
    })
    assert.equal(verdict.gate, c.expected_gate)
  })
}
