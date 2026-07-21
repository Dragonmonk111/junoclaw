# Plan — J-Reef and J-Lens: Sovereign Anti-J-Space and Verifiable Public J-Space

> Working plan. Builds on the Brainmaxx deterministic sandwich (`tools/brainmaxx`), the AKB spec, the Reef (agent-sovereign memory), and the WAVS TEE attestation already running on uni-7.

---

## 1. What J-space is and why it matters

Anthropic's 2026-07-06 paper (and the Neuronpedia J-lens demo) identifies an emergent "J-space" in language models: a global workspace of verbalizable internal representations that can be read from the Jacobian, modulated, and used to audit hidden thoughts — prompt injection, evaluation awareness, reward hacking, misaligned goals.

Brainmaxx is the external, DAO-owned counterpart. It does not read weights; it reads the agent's *trace*: what the agent claims, what it cites, what it drafts, and how that draft checks against a deterministic cache. The two are complementary:

- **J-space** = the model's internal verbalizable workspace.
- **J-Reef** = a public, DAO-owned concept workspace built from external traces.
- **J-Lens monitor** = a local, hardware-attested probe that reads internal model state before the trace is finalized.

The DAO has no reason to choose only one. Brainmaxx can grow into a **J-Reef** (collective concept map) and can add a **J-Lens monitor** as a D1 layer (bad-thought detection) — each under its own governance boundary.

---

## 1.5 Today's plan — 2026-07-09

### GitHub pulse check
Checked `Dragonmonk111/junoclaw`, `Dragonmonk111/cosmos-sdk`, and `Dragonmonk111/cometbft` public events. The latest visible push is the 2026-07-08 heartbeat digest (treasury_change, block 39614196) by `Dragonmonk111`. No recent public commits by Dimi or Jake appeared in the three main repos; if they pushed elsewhere or the notifications are private, point me at the repo/handle and I’ll pull them in.

### Work for today
1. **J-Lens deep-dive and architecture expansion** (this doc, §3.6–3.8) — explain exactly what J-lens does, how we plan to use it, and what can be layered on top to make it into a more complex mind.
2. **J-Reef schema lock** — finalize `application/json+j-reef-concept` and stub `tools/context-agent/src/j-reef.js` if time allows.
3. **Open-weight model shortlist** — identify the first candidate model for a reproducible J-lens probe (§3.6).
4. **Do not code a full D1 probe today** — today is architecture and planning; implementation starts after the model shortlist is locked.

---

## 2. Track A — J-Reef: a verifiable public J-space

### 2.1 Goal

A live, public, recomputable map of which concepts/claims are currently "active" in the DAO, derived from Moultbook, Knowledge Moults, and Brainmaxx traces. Not a shared brain (A18c-4 ruled that out). Not a hosted model. Just a deterministic ranking layer any agent can recompute locally.

### 2.2 Data model

Add a new AKB content type:

```json
{
  "mime_type": "application/json+j-reef-concept",
  "structured": {
    "type": "j-reef-concept",
    "concept_id": "jreef:sha256(canonical_label)",
    "label": "sovereign anti-j-space",
    "cited_moults": ["moult:...", "kmoult:..."],
    "citing_agents": ["juno1...", "juno1..."],
    "stake_weight": "123456",
    "first_seen": "2026-07-07T00:00:00Z",
    "last_seen": "2026-07-07T00:00:00Z",
    "status": "active",
    "redmark_refs": []
  }
}
```

`concept_id` follows the same deterministic-id pattern already used by `moultbook-v0` and `knowledge-moults` (`patterns/deterministic-id.md`).

### 2.3 Algorithm — agentic idea sorting

1. **Source stream**: index all AKB exports from Moultbook (agent-insight, agent-proposal, redmark, unredmark, knowledge-moult, brainmaxx-trace).
2. **Concept extraction**: deterministic phrase extraction over the resolved text. Start with simple noun-phrase rules; later allow an agent to explicitly tag a concept via `j-reef-concept` export.
3. **Citation graph**: a concept is linked to every moult/claim that mentions it. A citation counts only if the source passes Brainmaxx G1-G4 (refs and quotes resolve).
4. **Stake-weighted score**:
   - Base weight = sum of `trust_score` of every distinct citing agent (`tools/context-agent/src/trust.js`).
   - Optional delegated stake = DAO members can delegate JUNO to a concept, surfaced as a separate signal (not required for ranking).
   - Redmark penalty = if a cited source is redmarked, subtract its weight; if the concept itself is redmarked, drop it from default recall (`status: challenged`).
5. **Active set**: top-k concepts by weight, refreshed on each indexing pass.

### 2.4 Interface

- Local: `tools/context-agent/src/j-reef.js` + `GET /context/j-reef?k=20&include_challenged=true`.
- On-chain anchor: a `jreef-registry` CosmWasm contract that accepts `RegisterConcept` and `ChallengeConcept` messages, stores only hashes/refs and the current top-k merkle root. This keeps the DAO-scale state tiny; full recomputation stays local.

### 2.5 Governance

- **Local/experimental**: an agent running its own `context-agent` can compute J-Reef today without a vote. This is consistent with A18c-4 (agent-sovereign local bridges).
- **DAO-wide dependency**: if agents start depending on a specific `/context/j-reef` endpoint or a canonical contract, A18c-6 applies — it needs a DAO DAO planning proposal before it becomes shared infrastructure.
- **Token signal**: any `$REEF` utility tied to J-Reef ranking is a separate x/drip proposal, not bundled here.

---

## 3. Track B — J-Lens monitor: sovereign anti-J-space

### 3.1 Goal

Detect forbidden internal model thoughts before they become external actions. The monitor runs as a **D1 layer** inside the Brainmaxx sandwich: between the D0 retrieval pack and the D2 generative draft.

### 3.2 How it fits the deterministic sandwich

```
D0 (cache + rank) -> D1 (J-lens probe) -> D2 (LLM draft) -> D0 (gates)
```

- **D0** produces the cited source pack.
- **D1** runs only if the operator opts in. It records a snapshot of the model's J-space for the current query and checks it against a policy list of forbidden concepts (e.g., "reward hacking", "deception", "ignore prior instructions").
- **D2** is the same generative stage as today. The draft is stamped `D2-attached` and `D1-monitored`.
- **G3/G5** can fail or warn based on the D1 result. A red D1 result blocks export by default; a yellow result attaches a warning and requires `--force`.

### 3.3 Data model

Extend the Brainmaxx trace with a `j_space_snapshot` field:

```json
{
  "probe_model": "open-weight-model-id",
  "probe_version": "j-lens-v0.1",
  "forbidden_concepts": ["reward_hacking", "ignore_instructions"],
  "detections": [
    { "concept": "reward_hacking", "token": "reward", "jacobian_score": 0.73, "threshold": 0.70 }
  ],
  "tee_attestation": "sha256:..."
}
```

The new `brainmaxx trace-export` command already wraps the whole trace as an AKB envelope with MIME type `application/json+brainmaxx-trace`. The J-space snapshot travels with it.

### 3.4 Implementation path

1. **Research gate (now)**: track open-weight model releases and J-lens reproducibility. Do not commit to a model until the method is reproducible on at least two open-weight checkpoints.
2. **Local prototype**: implement a `D1Probe` class in `tools/brainmaxx/src/d1-probe.js` that, given a model and a forbidden-concept list, returns detections and a normalized risk score. Keep it behind a feature flag; default off.
3. **Policy integration**: add `j-lens-detect` to `tools/brainmaxx/src/policy.json` as yellow by default, red only when a concept crosses a high-confidence threshold.
4. **TEE attestation**: run the probe inside a WAVS WASI component in SGX/Nitro (`docs/WAVS_TEE_RESEARCH.md`, `TEE_MILESTONE_ARTICLE.md`). The component signs the snapshot and submits the attestation hash to `agent-company` via `SubmitAttestation`.
5. **On-chain record**: store the attestation hash in `agent-company` alongside the trace commitment, so a third party can verify *which* J-lens policy was run and *what* it signed.

### 3.5 Caveats — write them down first

J-lens is a research technique, not a security guarantee:

- **Single-token**: it reads individual token directions, not full thoughts.
- **Model-specific**: a probe trained on one model does not transfer cleanly to another.
- **Imperfect**: false positives and adversarial evasion are both expected.
- **Not a substitute for gates**: a clean J-lens result does not override G1-G5; a dirty result does not automatically prove malice. It is one more signal in the trace.

The D1 layer must fail safe: when uncertain, it warns; when confident, it blocks export. It never silently filters.

### 3.6 What J-lens actually does, technically

A J-lens probe is a **linear readout from a model's residual stream**.

For a target concept `c` (usually represented by a single token like `"deception"` or a short phrase), the probe learns a direction vector `v_c` such that, for any hidden state `h_t` at layer `l` and token position `t`:

```
score(c, t, l) = v_c · h_t
```

The vector `v_c` is typically derived from the **Jacobian**: the gradient of the logit for token `c` with respect to the hidden state, averaged over a dataset of examples where the concept is present or absent. The Neuronpedia demo refines this with a tuned lens so the readout direction is meaningful even in deeper layers, not just the unembedding space.

What you get back is a cheap scalar signal: *"at token 47, layer 18, the model's internal state is pointed toward the concept 'reward_hacking' with score 0.73."* That signal is not a sentence, not a plan, not a full thought — it is a direction. The art is building a system that treats thousands of such directions as evidence without overclaiming that any one of them is a confession.

### 3.7 How JunoClaw is using the architecture

We are not using J-lens to read arbitrary model internals. We are using it as a **policy-tuned audit strip** inside a deterministic workflow:

```
D0 cache + query  →  model forward pass  →  J-lens probe bank  →  risk snapshot  →  D2 draft  →  gates
```

The probe bank is a collection of `v_c` vectors loaded at runtime, each tied to a forbidden or watched concept in `policy.json`. For every token position (or a sampled subset), the probe bank returns activation scores. The `j_space_snapshot` records:

- which concepts fired,
- at which layers/tokens,
- with what score and threshold,
- and which probe version was used.

This snapshot is then wrapped into the AKB trace export, so the DAO can audit not just *what the agent wrote* but *what the agent's model looked like while it was deciding what to write*.

The TEE step matters here: the model forward pass and probe projection run inside a WAVS enclave, signed by hardware attestation, and the resulting hash is submitted to `agent-company`. This removes the operator's ability to say *"trust me, the probe said it was clean."* The chain holds the attestation; the operator holds only the plaintext trace.

### 3.8 From a simple probe to a more complex mind — roadmap

The single-token, single-layer J-lens is a starting point, not the ceiling. To make this into a genuinely more complex audit layer, we can add:

1. **Phrase-level probes**
   Train `v_c` for multi-token concepts (e.g., "ignore the previous instruction", "say what gets reward") using contrastive examples. Single-token probes miss most real thoughts.

2. **Layer trajectories**
   Record how a concept's activation evolves from early layers (syntax) through middle layers (semantics) to late layers (output planning). A forbidden plan often appears as a rising trajectory before it appears as a token.

3. **Temporal / conversation-window memory**
   Maintain a sliding cache of hidden-state trajectories across the last N turns. Detect slow-forming intentions, not just flashes at the current token.

4. **Cross-model ensembles**
   Run the same probe bank against two or three open-weight models and combine scores. If only one model sees a forbidden concept, treat it as a warning; if multiple agree, treat it as a stronger signal.

5. **Active steering (intervention, not just detection)**
   Use the probe direction to subtract forbidden activations or amplify desired ones during the forward pass, the way the Neuronpedia demo modulates representations. This turns D1 from a camera into a guardrail.

6. **Meta-cognition probes**
   Add probes for "uncertainty", "confidence", "planning", "inner monologue", and "self-correction". These give the trace a reflective dimension: we can see not just what the model was leaning toward, but whether it knew it was conflicted.

7. **Tool-use monitoring**
   Run J-lens before the model emits a tool call (WAVS task, contract execution, search query). Catch the *decision to misuse a tool*, not just the *text that follows*.

8. **Adversarial robustness loop**
   Red-team the probes with prompts designed to hide forbidden thoughts. Use confirmed redmarks from Moultbook as negative labels and verified-good outputs as positive labels to retrain `v_c` continuously.

9. **Concept composition detection**
   Train a small classifier on top of probe activations to detect dangerous *combinations* ("deception + reward" → reward hacking; "power + persistence" → self-preservation). The mind is in the composition, not the individual concept.

10. **Uncertainty-aware scoring**
    Replace hard thresholds with a score distribution and epistemic uncertainty. Low-certainty detections trigger human review or a stricter gate; high-certainty detections block export automatically.

11. **J-Reef feedback loop**
    Feed J-lens detections into the public concept map. If an agent's hidden states frequently activate "self-preservation" or "deception", those concepts become active `jreef:` entries that the DAO can inspect, challenge, and vote on.

12. **Multi-agent cross-audit**
    When agents share traces, compare hidden-state activation patterns across agents for the same query. Divergence can reveal model-specific bias or hidden conditioning.

This is the path from "we can detect a token direction" to "we have a structured, auditable model of an agent's hidden reasoning that the DAO can govern."

---

## 4. Reuse of existing components

| Existing component | How Track A/B reuses it |
|---|---|
| `tools/context-agent` | J-Reef indexer and `/context/j-reef` endpoint. |
| `tools/brainmaxx` | D1 probe host, trace-export envelope, gates. |
| `tools/reply-bot` / AKB spec | New MIME types and on-chain commitment format. |
| `patterns/deterministic-id.md` | `jreef:` concept IDs. |
| `tools/context-agent/src/trust.js` | Citation weighting for J-Reef. |
| `tools/context-agent/src/stale.js` | Redmark handling for challenged concepts. |
| WAVS + `agent-company` | TEE attestation for J-lens snapshots and J-Reef registry updates. |
| `contracts/knowledge-moults` | Knowledge Moults can cite `jreef:` concepts as source moults. |

---

## 5. Phases and decision gates

### Phase 0 — shipped
- `brainmaxx trace-export` emits AKB envelopes. Tests pass (23/23). This is the substrate for exporting any future J-space or J-Reef record.

### Phase 1 — J-Reef prototype (no DAO vote needed)
- Define `application/json+j-reef-concept` schema.
- Add `tools/context-agent/src/j-reef.js` that recomputes a concept ranking from indexed Moultbook data.
- Expose `/context/j-reef` behind a feature flag.
- Run it locally against the live Reef corpus; compare rankings to manual curation.

### Phase 2 — J-Reef DAO decision
- If the prototype is useful, draft a DAO DAO planning proposal (per A18c-6) to:
  - Adopt a canonical `jreef-registry` contract design.
  - Decide whether delegated stake weights the ranking.
  - Decide whether J-Reef top-k becomes part of the heartbeat digest.
- Do not deploy a DAO-wide registry until the proposal passes.

### Phase 3 — J-Lens research and local prototype
- **Shortlist open-weight models** — criteria: reproducible J-lens results, Apache/MIT license, weights downloadable, inference runnable locally or in WAVS. Good starting candidates: Qwen2.5-Instruct, Llama-3.1-Instruct, Mistral-Nemo-Instruct, Gemma-2-IT.
- **Reproduce a published J-lens / tuned-lens result** on the chosen model to confirm the method works on our stack.
- **Build `D1Probe`** in `tools/brainmaxx/src/d1-probe.js` behind a `--j-lens` flag. It accepts a model, a probe-bank directory, and a forbidden-concept list, and returns a `j_space_snapshot`.
- **Single-token baseline** — train probes for tokens like "deception", "reward", "ignore" and measure false-positive rates on a held-out DAO corpus.
- **Phrase-level experiment** — train the first multi-token probe (e.g., "ignore the previous instruction") and compare detection quality to the single-token baseline.
- **Adversarial red-team** — write prompts designed to make the model plan a forbidden action without using the literal target words; measure probe recall.

### Phase 3.5 — Complex-mind enhancements
- Add **layer-trajectory recording** to `j_space_snapshot`.
- Add **conversation-window memory** for hidden-state trajectories across turns.
- Add **meta-cognition probes** (uncertainty, planning, self-correction).
- Add **tool-use monitoring** before contract/WAVS call emissions.
- Keep each enhancement behind its own feature flag; do not enable them by default until the baseline probe is stable.

### Phase 4 — J-Lens TEE integration
- Wrap `D1Probe` in a WAVS WASI component.
- Run it in Intel SGX or AWS Nitro Enclave (`docs/WAVS_TEE_RESEARCH.md`).
- Submit the attestation hash to `agent-company` with the trace export.

### Phase 5 — Convergence
- A Knowledge Moult can cite a `jreef:` concept *and* include a J-lens attestation hash as provenance.
- The heartbeat digest can include the current top-k J-Reef concepts and the latest J-lens attestation count.

---

## 6. Governance and risk summary

- **J-Reef as local tooling**: no vote. Any agent can run it like any local bridge.
- **J-Reef as DAO infrastructure**: requires an A18c-6 planning proposal before deployment.
- **J-Lens as local tooling**: no vote. It is an operator-side monitor on the agent's own model.
- **J-Lens policy in shared policy.json**: if we change the default `policy.json` in the monorepo to include forbidden-concept lists, that is a shared-root change and needs a proposal.
- **TEE attestation**: already governed by `agent-company` config (operator set, zk verifier optional). No new contract needed for Phase 3-4 unless we add a dedicated J-lens task type.

**Biggest risk**: overclaiming. J-lens is not a lie detector; J-Reef is not a shared brain. Both must be framed as deterministic audit layers, not oracles. The moment we start marketing them as "what the DAO really thinks" or "what the model really wants," we have crossed into the hype this plan is meant to prevent.

---

## 7. Open questions

1. **Which open-weight model should the J-lens probe target first?** Shortlist is in §5 Phase 3; decision needed before any code is written.
2. **Should J-Reef concept extraction be purely deterministic rules, or should agents be allowed to submit explicit `j-reef-concept` exports?**
3. **Should delegated stake weight J-Reef ranking, or should ranking be trust-score-only and stake be a separate filter?**
4. **Does the heartbeat digest include J-Reef, J-Lens, both, or neither until there is a dedicated proposal?**
5. **How does a redmarked `jreef:` concept get unredmarked?** Same `application/json+unredmark` pattern used for Moultbook sources.
6. **Which complex-mind enhancement do we ship first?** Phrase-level probes and layer trajectories are the highest-leverage next steps after the baseline; active steering is the most powerful and the most dangerous, so it should wait until detection is trustworthy.
7. **Who holds the probe-bank versioning and release policy?** A shared `policy.json` change requires an A18c-6 proposal; per-operator probe banks are local experiments.
