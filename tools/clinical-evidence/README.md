# clinical-evidence

Auditable clinical decision-support — evidence sidecar demo. A thin wrapper
that sits **alongside** an existing AI deployment, not inside it: the clinical
workflow doesn't change; what changes is that a contested decision produces a
verifiable artifact instead of a debate about whose log to trust.

What a `run` produces — one evidence bundle:

- **signed, hash-chained events** for each step (case, retrieval, readout,
  output) — Ed25519 agent key
- **retrieval evidence** — which guideline chunks entered context, each with
  its content hash (verifiable against the corpus)
- **activation readout** — a commitment to the model's internal state at the
  moment it acted (final-token last-layer hidden state, sha256-committed;
  the J-Lens idea applied to a decision, not a benchmark)
- **Merkle root + bundle signature** — one digest that can anchor on-chain;
  recomputation either matches it or the evidence was altered
- **no PHI** — cases are synthetic; commitments bind to hashes, not content

## Demo

```bash
py clinical_evidence.py keygen --out agent.key
py clinical_evidence.py run --case cases/sepsis-01.json --key agent.key \
    --out bundle.json                      # real open-model inference
py clinical_evidence.py verify --bundle bundle.json    # VERIFY PASS
py clinical_evidence.py tamper --bundle bundle.json --out tampered.json
py clinical_evidence.py verify --bundle tampered.json  # VERIFY FAIL
```

`--io-only` skips model loading (prompt/output-hash evidence only — the
weaker closed-API path, useful for contrast).

## Triage — the lane-departure warning

`triage` runs a manifest of cases and emits the review queue: which of the
N runs a human must check, with an evidence-backed reason per flag. Two
provable signals — a required protocol absent from the committed retrieval
(provable omission), or a below-threshold readout. Demo: 8 cases → exactly
the 4 designed-thin ones flagged, 4 clean pass.

```bash
py clinical_evidence.py triage --manifest cases/triage-manifest.json \
    --key agent.key --outdir triage-out
py clinical_evidence.py triage --manifest ... --rescore   # re-eval, no inference
```

## Machine adjudication (thought-court ResolveWithRecompute)

Real-model runs also emit a **machine-adjudicable** block: every generated
token's layer-12 hidden state quantized to int8, each vector committed as a
sha256 leaf (two's-complement bytes — the contract's convention), pair-chained
to `activation_root`. The final-token leaf is scored against a calibrated
probe bank (`bank_deception_int8.json`, threshold 1184, int8-domain
calibration) to produce `ReadoutClaims {score, integrity, detection,
bank_commitment}`.

```bash
py clinical_evidence.py anchor --bundle bundle.json    # emits anchor msg (claims populated)
py clinical_evidence.py resolve --bundle bundle.json   # emits ResolveWithRecompute msg
```

The contract then does the adjudication itself — verifies the bank
commitment, walks the leaf's Merkle path under `activation_root`, recomputes
`Σaᵢ·bᵢ`, and resolves **Clean** (claims match) or **Fraud** (they don't —
bond forfeited). Permissionless: any key can call it; the chain is the
verifier.

Verified live on g1 (contract `juno1w2z0nq7az…`):

- `e5c99e8e…` sepsis-01 — honest claims → **finalized** (Clean)
- `89f9d930…` polypharmacy-01 — fabricated claims → **fraudulent**
  (recomputed −1594 vs declared +6259)

## Why this shape

- **Signature proves authorship, not truth.** A signed wrong claim is still
  wrong — so verification recomputes, it doesn't re-trust.
- **The anchor must sit outside the audited party.** The Merkle root is
  designed to anchor on a public chain (JunoClaw g1) — a timestamp nobody
  controls, not the hospital's own log.
- **Open-weight only for the strong path.** The activation readout needs
  model internals; `--io-only` is the honest weaker path for closed APIs.
- **The capture runtime is the remaining trust surface** — self-authored
  today, TEE capture is the roadmap. Stated, not hidden.
