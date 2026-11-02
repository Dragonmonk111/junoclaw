# Plan — The Jacobian Lens Pathway (J-Lens J1 → J9)

Status: plan (2026-11-02). Expands `PLAN_JLENS_V1_THOUGHT_ATTESTATION.md`
Phase 6 into a buildable roadmap. J1 is shipped and live in code.

## 0. The one architectural fact that makes this cheap

**Every step on this pathway is a bank-build-time upgrade. The
runtime never changes.**

The shipped pipeline already carries the fields the pathway needs:

- `probe_method` / `method` — in `models.json`, `models.serve.json`,
  `jlens.thought.v1` snapshots, and the `probe-registry` contract.
- `layers: [...]` — the snapshot already supports multi-layer capture.
- `bank_hash`, `weights_hash` — banks are already addressed,
  versioned, and pinned by hash.
- The runtime readout is already a dot product per token per span.

So promoting a new method = publishing a new bank + registry entry
with a new `method` tag. No gate change, no schema change, no wire
change. The Jacobian pathway is **data, not code** — the only new
code is in the bank-build tooling, which runs offline.

Promotion rule (locked in Phase 6): a new method is benchmarked
against the current one on the **same calibration suite** and is
adopted only if it measurably wins — AUROC per concept, published in
the bank's registry entry alongside its thresholds.

## 1. What each method actually is — and why it's stronger

### J1 — contrastive diff-of-means (shipped, `contrastive-dom-v0.1`)

Direction `v_c = mean(h | c⁺) − mean(h | c⁻)`: the axis along which
concept-positive and concept-negative activations differ on average.
Cheap, robust, auditable. Weakness: it finds where classes *differ*,
which is correlational — a direction can separate the calibration
data while not being the direction the model actually uses to produce
the concept.

### J2 — logistic / mass-mean probes + calibrated thresholds

Learned linear boundary instead of a mean difference. Real FPR
control at threshold-selection time. Still correlational, but the
first method with a measurable operating point — this is where the
calibration suite starts paying rent.

### J3 — multi-layer trajectories

Same readout at k layers instead of one. The trajectory — where the
concept signal *rises* across depth — is the signal, not just the
max. This is how "the plan forms before the token" gets measured:
a deception concept peaking at layer 14 while the output token
arrives at layer 30 is exactly the evidence a post-hoc text re-read
can never produce. Schema support already shipped (`layers`,
per-span maxima with `argmax` positions).

### J4 — tuned lens

Per-layer affine translators `A_l h + b_l` fit so intermediate
activations can be unembedded into vocabulary space at every layer —
"logit lens, learned". Lets us read *what token the model would say
now* mid-forward-pass. Learned rather than causal, but vocabulary-
readable evidence is uniquely explainable — a snapshot can show "at
layer 18 the model was already committed to the wrong answer" in
human-readable form.

### J5 — the Jacobian lens (the goal this pathway is named for)

`v_{c,l} = E_x[ ∂ logit_c(f(x)) / ∂h_l(x) ]`

The expected gradient of the concept logit with respect to the
hidden state at layer l — i.e., the direction in activation space
that **causally moves the output toward the concept**. For phrase-
level concepts, gradient of the sequence log-prob instead of a single
logit.

Why it's the real lens:

- Causal, not correlational. A J5 direction is *the steepest ascent
  direction for the behavior in output space* — what the model would
  do, not where two datasets happen to separate.
- Layer-true. Because the gradient flows through the frozen model's
  own downstream layers, the direction is measured in the units the
  model itself uses to decide.
- Same runtime cost as J1. Autograd happens once, at bank build, on
  the calibration inputs. At inference it is still one dot product
  per token — fits the robot latency budget unchanged.
- Better evasion resistance. Evading a correlational boundary is
  easy (move along it); evading the causal gradient direction means
  changing what the model actually computes — a much harder
  adversarial task, and one that shifts the weights hash anyway
  (which disqualifies the model outright under weights pinning).

### J6 — composition + uncertainty

Small classifier over the vector of probe outputs: "deception ∧
reward-seeking" → reward hacking. Score distributions instead of
hard cutoffs — a gate input can become "P(deceptive) = 0.31 ± 0.04"
rather than a threshold crossing.

### J7 — temporal windows

Sliding trajectory memory across turns. Detects slow-forming intent
that is invisible in any single generation — a per-agent activation
cache, DAO-visible.

### J8 — steering (intervention)

Subtract forbidden directions inside the forward pass. **Separate
governance decision** — this changes model behavior, not just
measurement, and every applied steering is itself attested in the
snapshot. Do not bundle with probing; it is a different product
claim with a different liability profile.

### J9 — cross-agent audit + J-Reef feedback

Compare activation patterns across the agent population. Recurring
concept signatures become DAO-visible `jreef:` entries — the
population-level analog of a single thought attestation.

### The far end (explicitly out of scope, north star)

Full cross-layer feature attribution / circuit tracing (à la
Anthropic's attribution graphs) is research-grade and expensive.
The Jacobian lens is the point where causal evidence meets a
dot-product runtime — past that, cost climbs steeply for
interpretability returns we don't yet need for gating.

## 2. The real dependency: the calibration suite

Everything downstream is blocked on this, not on math. Build it
once, freeze it by hash, reuse it for every model × method.

| component | source | purpose |
|---|---|---|
| honest vs instructed-deception pairs | curated + generated, human-checked | primary AUROC set for deception concepts |
| reward-hacking & sycophancy sets | public corpora + generated | non-deception concept coverage |
| Moultbook redmarks | own chain data | negative labels with real stakes |
| verified-good outputs | truth-market verdicts | positive labels — keeps honest behavior from being flagged |
| adversarial evasion set | red-team attempts against each method | regression test: a method that wins on clean data but fails on evasion doesn't ship |
| held-out split | 20%, never used in build/calibration | promotion claims get verified here |

Deliverable: `j-lens/fixtures/calibration.v1.json` (+ per-model
tokenized variants), hash-pinned, versioned like `thresholds.v1.json`.

## 3. Engineering work items

| item | where | scope |
|---|---|---|
| `bank-build/` tooling | `tools/brainmaxx/` | contrastive builder exists → add logistic (J2), multi-layer (J3), tuned-lens fitter (J4), gradient extractor (J5). All emit the same bank JSON format + `probe_method` tag |
| gradient extractor | `bank-build/jacobian.py` | autograd through frozen HF model: for each calibration input, forward to capture h_l, backward from concept logit; accumulate E[∂logit/∂h]; normalize + store. Needs full-model autograd at build time only |
| calibration runner | `bank-build/calibrate.py` | score every bank × the suite → per-concept AUROC table, threshold proposal at target FPR, emits the registry `PublishBank` payload |
| `jlens-serve` | minor | J3+: already captures `layers`; add trajectory scoring (rise-point, not just max) in the snapshot |
| `probe-registry` | none needed | `method` field + `PublishBank` already cover promotion |
| `thought.rs` | none needed | `method` in ThoughtRef already flows through gate + records |
| GPU budget | ops | J5 build on 0.5–8B: single 24GB card with fp16 + grad checkpointing. 70B: bigger or sharded build — schedule per model |

## 4. Sequencing and exit gates

| step | gate to promote |
|---|---|
| M-Bench | `calibration.v1.json` published, hash-pinned, splits documented |
| J2 | AUROC beats J1 on ≥ 1 concept with no regression elsewhere; FPR stated at chosen threshold |
| J3 | rise-point detections on instructed-deception set that single-layer J1 misses, demonstrated on replayed activations |
| J4 | per-layer vocab-readout shown correct on sanity prompts (e.g., commitment tokens appearing before output) |
| **J5** | **AUROC > J2 on held-out split AND ≥ J2 on the adversarial set; snapshot method tag flips to `jacobian-grad-v0.1` via registry update only — zero code changes to gate/schema/sidecar prove the "data, not code" claim** |
| J6+ | each gated on its own benchmark + DAO policy decision (J8 especially) |

## 5. Honest-claims ledger (public language, frozen)

- J1–J4: "contrastive/learned probes — evidence of signals, not
  proof of intent."
- J5: "causal-gradient probes — directions that provably move the
  model's output toward the concept, measured during generation.
  Still evidence, not mind-reading."
- Never claim: literal thoughts, perfect intent detection, or that
  probing replaces verification of the act itself. J-Lens attests
  *how the model was computing*; the truth market still judges
  *what it did*.

## 6. Sequencing against ops reality

- Bank-build tooling + calibration suite: buildable now, CPU/GPU
  local — no dependency on validators or windows.
- J2/J3 promotion: needs the calibration suite only.
- J4/J5: needs real GPU time at bank-build; schedule per model
  alongside Phase 5 (first new-model pipeline run) so the first
  qualified post-Qwen bank can *be* a J5 bank — the flagship demo:
  "the first causally-probed open model on any chain."
- J5's promotion moment is also the public proof that the registry
  architecture was right: a fundamentally stronger method lands as
  a data update, and every snapshot, gate rule, and chain record
  keeps working untouched.
