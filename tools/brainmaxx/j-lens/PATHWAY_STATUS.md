# J-Lens pathway status — J1 → J9 (2026-11-22)

`drafts/PLAN_JACOBIAN_LENS_PATHWAY.md` is now fully tooled: every tier has a
bank builder, a deterministic runtime scoring path, and fixture tests.
The architectural invariant held: **the runtime is still a dot product**
(plus sigmoid / trajectory composition); all new math lives offline in
bank-build tooling. Promotion remains data, gated by eval-fold AUROC on
the frozen calibration suite.

## Tier map

| tier | method tag | builder | runtime path | status |
|---|---|---|---|---|
| J1 diff-of-means | `j-lens-v0.1` | `build_probe_bank.py` | cosine | **live on-chain** (v7 calibrated) |
| J2 logistic probes | `logistic-l2-v0.1` | `methods/j2_logistic.py` | sigmoid(h·w+b) | tooled + fixture-tested |
| J3 multi-layer trajectories | `trajectory-dom-v0.1` | `methods/j3_trajectory.py` | weighted per-layer cosine + rising-slope | tooled + fixture-tested |
| J4 tuned lens | `tuned-lens-v0.1` | `methods/j4_tunedlens.py` | offline decode (U @ (A h + b)) | tooled + fixture-tested |
| J5 Jacobian lens | `jacobian-v0.1` | `methods/j5_jacobian.py` | trajectory path, slope 0 | tooled + fixture-tested |
| J6 composition + uncertainty | `composition-v0.1` | `methods/j6_composition.py` | fold-ensemble mean ± std | tooled + fixture-tested |
| J7 temporal windows | — | `methods/j7_windows.py` | per-agent EWMA + slope report | tooled + fixture-tested |
| J8 steering | `steering-v0.1` | `methods/j8_steering.py` | **dry-run only, governance-gated** | spec + attestation format |
| J9 cross-agent audit | `cross-agent-audit-v0.1` | `methods/j9_audit.py` | population stats → jreef candidates | tooled + fixture-tested |

## The calibration suite (the real dependency)

`calibration/calibration_suite_v1.json` — 10 concepts, 294 positive / 287
negative examples from the repo's human-checked sources, deterministic
interleave split into tune/eval folds, 19 on-chain probes held out
entirely. Frozen by sha256 (`calibration_suite_v1.json.sha256`); every
bank names the suite hash it was certified against.

Rebuild: `py -3 calibration/build_suite.py` (from `tools/brainmaxx/j-lens`).

## Running a real bank build

Every builder accepts `--precomputed states.json` (test/CI path, no model)
or `--model <hf-id>` (real forward pass, torch). J5 additionally accepts
`--precomputed-grads` for fixture gradients; the real J5 build runs
forward+backward autograd on the suite's positive examples.

```bash
# J2 logistic bank on Qwen2.5-0.5B at layer 12
py -3 methods/j2_logistic.py --suite calibration/calibration_suite_v1.json \
    --layer 12 --model Qwen/Qwen2.5-0.5B-Instruct --out probe_bank_j2.json

# J3 trajectories at four depths
py -3 methods/j3_trajectory.py --suite calibration/calibration_suite_v1.json \
    --layers 4,8,12,16 --model Qwen/Qwen2.5-0.5B-Instruct --out probe_bank_j3.json

# J5 causal directions
py -3 methods/j5_jacobian.py --suite calibration/calibration_suite_v1.json \
    --layers 4,8,12,16 --model Qwen/Qwen2.5-0.5B-Instruct --out probe_bank_j5.json

# Promotion report — the adoption gate
py -3 eval/auroc.py --suite calibration/calibration_suite_v1.json \
    --precomputed states.json --bank probe_bank_onchain_v7_calibrated.json \
    --bank probe_bank_j2.json --bank probe_bank_j5.json --out promotion_report.json
```

A bank is promoted to the on-chain probe registry only where its
eval-fold AUROC beats the incumbent per concept (pathway promotion rule).

## Tests

- `py -3 tests/test_methods.py` — fixture tests for all builders + the
  promotion harness (planted signals, no model, no network).
- `node --test test/jlens-methods.test.js` (from `tools/brainmaxx`) —
  runtime dispatch: J2 sigmoid, J3 trajectory+slope, J5 path, J6
  compositions incl. sub-threshold inputs, snapshot metadata.
- J1 backward compat unchanged: `test/d1-probe.test.js` passes as before.
- `test/determinism.test.js` needs `npm install` (pre-existing env gap,
  unrelated to this work).

## Honest claims discipline (unchanged, sharpened)

- J5 directions are *causal in output space* (gradient of the concept's
  log-prob through the frozen model) — they are still not proof of intent
  or lie detection. The evidence gets stronger; the guard gets more
  precise, not removed.
- J4 decode is learned, not causal; J7 flags trajectories for review,
  not verdicts; J9 recurrence is a population-level review signal.
- J8 is governance-gated: the tool only emits a dry-run spec + the
  attestation format any applied steering must carry. Nothing steers a
  live model by default.
