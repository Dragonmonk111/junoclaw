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
