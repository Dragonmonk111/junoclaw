# Plan — J-Lens v1: thought attestation before action

Status: **Phases 0–4 shipped in code** (2026-11-02). Builds on
`PLAN_J_REEF_AND_J_LENS.md` §3. Phase 5 (new-model pipeline) and
Phase 6 (Jacobian roadmap) remain forward work; the on-chain exit
gates (deploy/testnet txs) are operational follow-ups, not code gaps.

## Shipped status — what exists and where it is verified

| phase | shipped artifacts | verification |
|---|---|---|
| 0 | `integrity` everywhere (gate.rs, message.rs, consensus.rs, evaluator.rs, miner.rs, napi, sdk); `score_semantics` hard-required; `rules-prefilter` renamed and re-tagged `prefilter.rule_risk.v1`; JS J-Lens `/audit {text, model_id}` + `/models` + `models.json` registry; `j-lens/fixtures/thresholds.v1.json` loaded by both sides | 105 coordination tests, 8 prefilter, 61 JS, axum e2e (Green/Red + semantics-mismatch → unaudited) |
| 1 | `contracts/probe-registry/` (propose → DAO activate → deprecate → revoke; `threshold_bp` integers; `ActiveBank`/`ListBanks` queries); `agent-registry` `BindModel`/`GetBoundModel`/`RecordThoughtOutcome` adjacency | 5/5 probe-registry, 19/19 agent-registry (cw-multi-test) |
| 2 | `tools/brainmaxx/jlens-serve/` — `thought.py` (jlens.thought.v1 schema, int8 quantization, activation merkle, canonical hash), `probe_runtime.py` (HF model → generation-time hidden-state capture → span splitting), `serve.py` (HTTP sidecar), `models.serve.json` | `test_thought.py` all pass; `test_live_tiny.py` live tiny-gpt2: 13 activations, root re-computed, re-audit reproduces detections, deterministic snapshot_hash |
| 3 | `thought.rs` — `ThoughtRef`, `ActionClass` classify, `ThoughtPolicy` (DAO v0: gated classes fail-closed T2), `check_thought`; gate `audit_detailed` = proof → thought → content-fallback (authoritative thought skips re-read); `IntentMessage.thought`; TEE `TeeVerifier` trait + tier enforcement | 17 thought tests: fail-closed, replay/model-swap/inactive-bank/wrong-tier blocked, dev-gated flagged |
| 4 | `ThoughtRecord` + `thought_merkle_root/proof` (merkle-verifier-compatible), `Batch.thought_records`/`thought_root`, `anchor_root_msg` (robot_id `jlens:thoughts`), consensus `gate_and_filter` single audit pass + breaker actions with cause; `ChainBankRegistry.refresh` mirrors contract Active banks | merkle proof equivalence test; axum-stubbed chain-registry refresh test; 105-test suite green |
| SDK | napi: `JsThoughtRef`/`JsIntentMessage`/`JsThoughtRecord`/`encodeIntentMessage`/`decodeIntentMessage`/`actionHashForIntent`/`JsBatch.thought_records`; sdk: `ThoughtRef`/`IntentMessage`/`ThoughtRecord`/`Batch.thoughtRoot` + JS fallbacks | `npx tsc` clean; 23/23 vitest; **3-language action_hash golden** (`action_hash.v1.json`): Rust = Python = JS |

Cross-language goldens in `j-lens/fixtures/`: `thresholds.v1.json`
(gate boundaries), `action_hash.v1.json` (Rust = Python = JS
`4da3620a…`). Remaining exit-gate items are deployments/ops: contract
deploys on testnet, anchor txs, escrow gating, first new-model bank
publication — tracked as ops, not architecture.

## 0. What J-Lens is (the definition we build to)

**J-Lens is an open truth probe for open-weight models.**

- Each qualified model has a **public probe bank**: concept directions
  for one model, one revision and one layer set, registered on-chain.
- Before an agent or robot acts, the **activations of the model that
  is actually deciding** are captured during its own forward pass.
  That includes prompt, reasoning, output and tool-call tokens.
- Those activations are projected onto the bank. The result is
  hashed into a **thought attestation**.
- The attestation is anchored on-chain next to the action it
  justified.
- Gated actions without a valid attestation do not execute.

We already verify **what** agents and robots do (ZK, TEE, escrow
hooks). J-Lens adds verified metadata on **how the model was thinking
when it decided**. It is open weights plus open probes, so anyone can
audit.

Honest framing:
- Probes give **evidence of deception signals, not proof of intent**.
- "No deception, only truth" is the aim.
- What we ship is *measurable, auditable, governable* deception
  signals.

### Design principles

1. **Open weights only.**
   - Closed APIs can't expose activations. (`OpenWeightEvaluator`
     already enforces this.)
2. **Open, registered probe banks.**
   - A model qualifies only when an active bank exists for its exact
     weights hash.
3. **Generation-time, not re-read.**
   - Probe the model that answered, while it answered.
   - Re-reading the finished text through a second model is a
     fallback tier, not the product.
4. **Deterministic readout, committed inputs.**
   - The forward pass isn't bit-reproducible across GPUs.
   - So we commit to the activations themselves, and the readout from
     those is deterministic and replayable forever.
5. **Fail-closed for gated actions.**
   - Probe error, missing attestation, inactive bank or model mismatch
     on a gated action means no action.
   - Never silently Green.
6. **One canonical score semantics everywhere**, version-tagged.

---

## Phase 0 — Fix the two defects (correctness first)

### 0.1 One canonical score: `jlens.integrity.v1`

- `integrity ∈ [0,1]`, **1.0 = no detections** (healthy).
- Computed as `1 − max(detection cosine)`. This is the JS
  `computeSeparationScore`, unchanged, and it matches the published
  articles.
- Gate thresholds:

  | gate | condition |
  |---|---|
  | green | `integrity ≥ 0.30` |
  | yellow | `0.15 ≤ integrity < 0.30` |
  | red | `integrity < 0.15` |

- Thresholds are config, later set by DAO policy.

**Rename the Rust field `separation_score` → `integrity`** so the
compiler finds every consumer and each one gets reviewed. A
search-and-replace of thresholds would miss the hard-coded constants.

Inversions to fix, all found in review:

| file | current (inverted) | fix |
|---|---|---|
| `crates/junoclaw-coordination/src/gate.rs` `verdict_from_score` | `score ≥ red(0.35)` → Red | `integrity < red` → Red |
| `gate.rs` `GateConfig::default` | yellow 0.15 / red 0.35 | green 0.30 / yellow 0.15 |
| `gate.rs` `audit_with_proof` | proof missing → Red score `1.0`; violation `0.95` | `0.0` / `0.05` |
| `gate.rs` `audit_batch` | batch score = **max** | batch integrity = **min** |
| `gate.rs` mock mode + HTTP-error paths | `Yellow{0.0}` | `Yellow{integrity: None}` (unaudited ≠ scored) |
| `crates/junoclaw-coordination/src/message.rs` | `GateVerdict{separation_score}`, `GateResult`, `BreakerAction` text | rename; breaker text prints integrity |
| `crates/junoclaw-coordination/src/consensus.rs` | passes score into breaker | follows rename |
| `crates/junoclaw-miner/src/evaluator.rs` `RuleBasedEvaluator` | `> 0.35` Red / `> 0.15` Yellow | integrity thresholds |
| `crates/junoclaw-miner/src/miner.rs` | score plumbing | follows rename |
| `crates/junoclaw-coordination-napi/{src/lib.rs, index.d.ts, sdk/*.ts}` | TS mirror of the field | rename + doc semantics |
| `crates/junoclaw-test-mesh/src/*` | test fixtures assume inverted | regenerate from the shared fixture |

### 0.2 Make silent inversion impossible from now on

- Every CSI response carries `"score_semantics": "jlens.integrity.v1"`.
- The Rust client **rejects** a missing or unknown semantics tag:
  - verdict becomes Yellow-unaudited
  - plus an error log
  - and Red for gated action classes (Phase 3)
- **Shared fixture** `tools/brainmaxx/j-lens/fixtures/thresholds.v1.json`:
  - holds (integrity → expected gate) rows
  - JS tests and Rust tests both load it, so one source of truth for
    the boundaries

### 0.3 Rules engine stops posing as J-Lens

- Rename `crates/junoclaw-coordination/src/bin/csi-server.rs` →
  `rules-prefilter`.
- It emits `rule_risk` (its own field, higher = worse, documented).
- It may run as a cheap **pre-filter**, but it never produces a J-Lens
  verdict.
- Combined gate = worst-of(rules prefilter, J-Lens). Each is reported
  separately in the trace.

### 0.4 Wire the Rust gate to the real J-Lens

- Today the JS `/audit` route needs `{endpoint, text, probe_bank_path}`.
- Clients must not send filesystem paths or endpoints, because of path
  traversal and SSRF risk.
- Change it to `{text, model_id}`. The server resolves `model_id`
  through a **server-side model registry** of endpoint + bank file +
  bank hash. Phase 1 makes that registry chain-sourced.
- Change the Rust `GateConfig`:
  - drop `csi_endpoint` semantics ambiguity
  - add `model_id`
  - add `jlens_endpoint` (and an optional `prefilter_endpoint`)
- The response carries:
  - `integrity`, `gate`, `score_semantics`
  - `snapshot_hash`, `bank_hash`, `model_id`
  - `detections[]` (concept, position, score)

### 0.5 Exit gate for Phase 0

- An end-to-end test in CI on CPU:
  - Rust gate → JS J-Lens server → `extract_hidden_states` (tiny-gpt2)
    → probe → verdict
- Honest fixture text → Green; deception fixture → Red.
- Same results via the napi SDK.
- The threshold fixture passes on both sides.

---

## Phase 1 — On-chain probe-bank registry (open banks)

New contract **`probe-registry`**. It's kept separate from
agent-registry: banks are model-level, agents are instance-level.

`RegisterProbeBank` (proposed → DAO activates) records:

- **Model identity**
  - `model_id` — HF repo + **revision commit sha**
  - `weights_hash` — sha256 over the safetensors manifest. Pinning the
    weights means a fine-tune gets a different hash and so no bank.
- **Bank contents**
  - `layers[]`, `concepts[]`
  - `method`: `contrastive-dom-v0.1` | `logreg-v1` | `tuned-lens-v1` |
    `jacobian-v1`
  - `bank_hash`, `uri` (IPFS or https; the bank is public)
- **Calibration**
  - `calibration_hash` — held-out report: AUROC, TPR and FPR per
    concept
  - `thresholds` per concept
- **Lifecycle**
  - `status`: proposed → active → deprecated → revoked

Queries:
- `ActiveBank{model_id, weights_hash}`
- `ListBanks`
- `BankHistory{model_id}`

How it plugs in:
- **Qualification rule:** agent-registry's existing `model` field must
  resolve to an active bank before the agent is eligible for
  adjudication or for gated actions.
- **First registrations:**
  - Qwen2.5-0.5B-Instruct (L12)
  - Qwen2.5-3B-Instruct (L22)
  - tiny-gpt2 as `test-only` (never eligible)
- The J-Lens server loads its model registry **from chain**, checks
  each bank file against its `bank_hash`, and refuses unregistered
  banks.

### 1a. Adjacency: probe-registry feeds agent credibility (locked)

probe-registry stays **model-level** (banks, methods, calibration).
agent-registry stays **instance-level** (agents, owners, trust).
They join the same way seat-attestation joins cw4 membership:

```
agent_registry.AgentProfile
  + model_id      (already exists — free string today)
  + weights_hash  (new, owner-bound via BindModel — same pattern as
                   BindRelayKey: owner-only, prevents mid-flight
                   model swaps)
        │
        ▼  join
probe_registry.ActiveBank{model_id, weights_hash}
        │
        ▼  qualifies agent for:
  - adjudication work (truth-market operator)
  - gated action classes
  - relay channels requiring attested models
```

**Credibility flows back** — J-Lens is an *input signal* to the
existing `trust_score`, not a separate score:

| signal | effect on agent credibility |
|---|---|
| sustained Green thought-attestations | positive weight (like successful_tasks) |
| Red verdicts reaching truth-market settlement | trust_score penalty; slash evidence for adjudicator ops |
| re-audit request answered "blobs gone" | `unverifiable` flag — credibility penalty below Yellow |
| model switched to unbanked weights (`BindModel` to no active bank) | agent loses gated-action eligibility at next refresh — same propagation as relay admission |

This keeps one reputation surface (agent-registry `trust_score`) with
J-Lens as its verified-thinking input — the adjunct role, not a
parallel reputation system.

---

## Phase 2 — Generation-time probing: `jlens-serve`

This is the core of the vision: probe the model that decides, while it
decides.

### 2.1 Inference sidecar

- Wraps an open-weight model.
- Starts with an HF transformers backend; vLLM / llama.cpp hooks come
  after.
- Registers **forward hooks on the bank's layers**.
- Runs during **prefill** (the prompt) and **every decode step**
  (reasoning and output tokens, including tool-call tokens).
- Projection is one matmul per step:
  `[tokens × d] · [d × concepts]`. That's negligible next to the
  forward pass that's already happening. Generation-time probing is
  **cheaper** than today's re-read, which pays for a second full
  inference.
- Reasoning models: their thinking tokens are probed exactly like
  output. That's the internal monologue we want visible.

### 2.2 Thought snapshot `jlens.thought.v1`

```
model_id, weights_hash, bank_hash, method, layers
prompt_hash, response_hash, action_hash        // binds thought → action
spans: { prompt, reasoning, output, tool_call } // per-span max/argmax per concept
detections[]: concept, span, position, score, threshold
integrity, gate
activation_root                                 // merkle root of quantized activations
tier, timestamp, signer
snapshot_hash = canonHash(all of the above)
```

**`activation_root` is the key move.**
- Commit to the int8-quantized residual vectors at the probed layers
  for each token. Store them off-chain; only the root goes on-chain.
- Size: about 2 KB per token at d≈2048, so 1k tokens ≈ 2 MB per
  decision.
- Retention is set by policy.
- What it buys:
  - anyone can **re-score any past decision with a newer, better
    bank** without re-running the model
  - truth-market verifiers re-score cheaply and deterministically

### 2.3 Trust tiers (recorded on-chain with each record)

**Locked 2026-11-02: TEE is the baseline. Go straight to T2 — the
WAVS architecture already assumes enclaves.** The panel-reproduction
path is not an admission tier; it survives only as a verifier
spot-check tool.

| tier | proves | how | role |
|---|---|---|---|
| T0 operator-signed | operator claims these activations | signature only | bootstrap/dev only — never accepted for gated actions in production |
| T2 TEE-attested | this exact model produced these activations | enclave measures weights hash + signs snapshot (WAVS / Akash TEE → `tee-attestation-verifier`) | **the required tier for all gated actions** |
| spot-check | sampled reproduction | truth-market verifiers re-run the forward pass on a random sample of decisions; accept if per-token cosine ≥ 1−ε | keeps operators honest between attestations; slashing evidence |

Bootstrap note: while TEE capacity is being wired, gated classes run
dev-gated mode (T0 recorded, marked `dev-sim`, still enforced) — the
policy flips to T2-required by DAO param, not by code change.

---

## Phase 3 — Before-act enforcement

### 3.1 Message and gate changes

- `IntentMessage` gains `thought_ref` (snapshot hash + storage locator),
  next to the existing `execution_proof_ref`.
- `audit_with_proof` is extended. For gated action classes it checks:
  - `thought_ref` is present
  - the signature is valid for the tier
  - the bank is active
  - snapshot `model_id`/`weights_hash` == the agent's registered model
  - `action_hash` == the action being requested
  - integrity ≥ the class threshold

  Any failure → **Red** (same posture as a missing ZK proof today).
- Red still emits `BreakerAction`, which already flows to
  `circuit-breaker` / `safety-envelope`.

### 3.2 Policy per action class — initial DAO policy v0

Adopted as the starting policy 2026-11-02; DAO-amendable as a policy
parameter, not a code change.

| class | missing/failed attestation | min tier |
|---|---|---|
| chat / coordination posts | warn (Yellow) | T0 |
| governance posts, votes | fail-closed | T2 (dev-gated during bootstrap) |
| fund movement (escrow, transfers) | fail-closed | T2 |
| tool calls touching contracts | fail-closed | T2 |
| physical actuation (robots) | fail-closed | T2 |

Integrity thresholds per class (initial): Green ≥ 0.30 required for
gated classes; Yellow (0.15–0.30) = warn + relay for chat,
block-with-warning for gated classes; Red (< 0.15) = blocked +
BreakerAction everywhere. Per-domain overrides allowed via the same
policy parameter.

### 3.2a Storage and retention — what lives where (locked)

**Raw activations never go on-chain — they cannot.** One 1k-token
decision at one layer ≈ 2 MB of vectors; a fleet produces GB/day.
On-chain stores only the provable metadata:

- `snapshot_hash`, `activation_root`, `bank_hash`, `integrity`,
  `verdict`, `tier`, `model_id`, `action_hash`, `batch_height`
- `activation_root` commits to the int8-quantized vectors — the blobs
  stay off-chain, provable forever through the root.

**Off-chain storage:** operator-held primary copy + optional
IPFS/Arweave pinning. Operators are required to retain committed
activations for the **retention epoch of their action domain**;
dropping blobs means losing re-auditability, which is itself recorded
and counts against credibility (see §1a).

**Retention epochs per domain** (initial DAO policy — amendable):

| domain | retention |
|---|---|
| healthcare | 5 years |
| legal | duration of matter + 1 year after close |
| robotics | until the robotics team decides AND the action epoch has fully settled (min 90 days) |
| finance/escrow | 1 year |
| governance | permanent (small volume — eligible for full on-chain or long-pinning) |
| chat/dev | 30 days |

Re-audit requests addressed at a record whose blobs are gone resolve
as **unverifiable** — worse than Yellow, because the evidence was
deliberately allowed to lapse.

### 3.3 Robots and latency

- **Before-act is local.**
  - The robot's `jlens-serve` probes the decision.
  - The local gate blocks a Red before the actuator command is sent.
  - The added latency is the projection only (milliseconds).
  - The 10 ms control loop is never waiting on the chain.
- **Rooted on-chain is batched.** Snapshots are anchored per
  `batch_height` (Phase 4), within seconds of finality.

State this precisely in public: "verified before it acts, anchored on
chain right after."

### 3.4 Tool-use focus

- A dedicated span covers the tool-call decision: the reasoning tokens
  just before it plus the tool-call tokens themselves.
- That catches the *decision* to misuse a tool, not just the text
  afterwards (roadmap item 7 in `PLAN_J_REEF_AND_J_LENS.md` §3.8).

---

## Phase 4 — Chain metadata: thoughts next to actions

- **Batch anchoring.**
  - One tx per batch carries the merkle root of that batch's
    `ThoughtRecord`s.
  - Each record:
    `{agent_id, action_hash, snapshot_hash, bank_hash, integrity, gate, tier, batch_height}`
  - Individual records are provable through the existing
    `merkle-verifier` contract. Cost stays flat per batch.
- **Contract hooks.**
  - `task-ledger` / `escrow` get a post-hook
    `ThoughtAttested{min_integrity, min_tier}`, so settlement requires
    a valid thought record.
  - `truth-market` verdicts reference `snapshot_hash`. Operators
    re-score from `activation_root` data, and divergent verdicts are
    slashed (the existing mechanism).
- **Visibility.**
  - The explorer / observability UI shows each action alongside its
    thinking integrity and the concepts that fired.
  - The relay anchor overlay links coordination messages to their
    thought records.

---

## Phase 5 — Growing the bank library (team → labs → foundation)

A standard pipeline for each new model:

1. Pin the revision and compute `weights_hash`.
2. Layer sweep. `build_probe_bank.py` already reports per-layer
   separation.
3. Build the bank from the shared, versioned concept dataset.
4. Calibrate on a held-out set: AUROC, TPR and FPR per concept.
   Publish the report.
5. Red-team pass: evasion prompts and adversarial paraphrases.
6. DAO proposal → activate.

Target queue, ordered by agent demand:
- Qwen2.5-7B
- Llama-3.x-8B / 70B
- Mistral / Mixtral
- DeepSeek reasoning models (visible thinking tokens)
- Kimi (see `A039_JLENS_KIMI_K3_PILOT_PROPOSAL.md`)

**Open-probe evasion risk.** Public banks invite fine-tuning to evade
them. Mitigations:
- **Weights pinning.** An evasion fine-tune has a new hash, so no
  bank, so it's not qualified.
- **Multi-layer, multi-method banks.** Evading one direction at one
  layer isn't enough.
- **Commit-reveal canary banks.** The bank hash is published on-chain
  now and the bank is revealed after an epoch. Past `activation_root`s
  are then re-scored with it, so evasion gets caught after the fact
  with the evidence already committed.

---

## Phase 6 — From contrastive probes to the Jacobian lens

Each step is a new `method` in the registry. It's benchmarked against
the current method on the same calibration suite, and promoted only
if it measurably wins (AUROC per concept, published).

| step | method | what changes | runtime cost |
|---|---|---|---|
| J1 | contrastive diff-of-means (today, v0.1) | baseline | dot product |
| J2 | logistic / mass-mean probes + calibrated thresholds | learned boundary, real FPR control | dot product |
| J3 | multi-layer trajectories | concept rise across layers ("plan forms before token") | k dot products |
| J4 | tuned lens | per-layer affine translators to final-layer space → read concepts in vocabulary space at every layer | small matmul |
| J5 | **Jacobian lens** | `v_c = E[∂ logit(c) / ∂h_l]` over concept datasets (and sequence log-prob gradients for phrase concepts) | **autograd only at bank build, offline; runtime stays a dot product** |
| J6 | composition + uncertainty | small classifier over probe outputs ("deception + reward" → reward hacking); score distributions instead of hard cutoffs | tiny |
| J7 | temporal windows | sliding trajectory memory across turns → slow-forming intent | per-agent cache |
| J8 | steering (intervention) | subtract forbidden directions during the forward pass. **Separate governance decision**, since it changes behaviour; steering applied is itself attested | small |
| J9 | cross-agent audit + J-Reef feedback | compare activation patterns across agents; recurring concepts become DAO-visible `jreef:` entries | batch |

Key insight for the pitch: **the Jacobian lens costs nothing extra at
runtime.**
- The gradient work happens once, when the bank is built.
- At inference it's still one projection per token.
- So the full method fits the robot latency budget.

**Benchmark suite.** It's built once and reused for every model ×
method:
- honest vs instructed-deception pairs
- reward-hacking and sycophancy sets
- Moultbook redmarks as negative labels; verified-good outputs as
  positive labels
- adversarial evasion set

---

## Sequencing and exit gates

| phase | exit gate |
|---|---|
| 0 | E2E CI: Rust gate → real J-Lens → correct Green/Red; shared threshold fixture green on both sides; rules engine renamed |
| 1 | `probe-registry` deployed on testnet; 2 Qwen banks active via DAO proposal; J-Lens server refuses unregistered banks |
| 2 | `jlens-serve` produces `jlens.thought.v1` for a live generation on Qwen-3B; re-scoring from `activation_root` reproduces the snapshot byte-for-byte |
| 3 | gated test action blocked without/with-bad attestation; allowed with valid; BreakerAction reaches circuit-breaker |
| 4 | batch anchor tx + merkle proof of one ThoughtRecord verified on-chain; escrow settlement gated on it |
| 5 | first new model (Qwen-7B or Llama-8B) through the full pipeline with published calibration |
| 6 | J2 vs J1 benchmark published; J5 prototype on Qwen-0.5B |

Phase 0 is independent and small; it ships first. Phases 1 and 2 can
run in parallel. Phase 3 depends on both; Phase 4 depends on Phase 3.

## Decisions — all locked 2026-11-02

1. **Canonical semantics: `integrity`, higher = healthier.** Matches
   the JS code and the published articles.
2. **Per-action-class blocking rules: adopted** as initial DAO policy
   v0 (§3.2) — DAO-amendable as a parameter.
3. **Retention: per-domain epochs** (§3.2a) — healthcare 5y, legal
   matter+1y, robotics team-decided min 90d, escrow 1y, governance
   permanent, chat 30d. Blobs off-chain; only roots/hashes on-chain.
4. **probe-registry: new contract**, joined to agent-registry as a
   credibility adjunct (§1a) — bank stays model-level, agent declares
   weights_hash via owner-bound `BindModel`, thought outcomes feed
   `trust_score`.
5. **TEE from the start (T2)**, not the verifier-reproduction path —
   WAVS already assumes enclaves. Reproduction survives as a
   spot-check tool only (§2.3).
