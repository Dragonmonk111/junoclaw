# A039 — Mandate: First J-Lens Probe Pilot, Targeting Kimi K3

> A18c-9 already authorized the J-Reef / J-Lens architecture and directed builders to ship Phase 3 (J-Lens research / local prototype). This proposal is the concrete follow-through: the D1 probe is built and tested (`tools/brainmaxx/src/d1-probe.js`, 8/8 unit tests, wired into `brainmaxx j-lens`), proven against a small open-weight model. This proposal mandates the next step — a real probe run against Kimi K3 (Moonshot AI, 2.8T-param MoE, open weights released 2026-07-27) as the DAO's first large-model J-Lens pilot.

---

## Copy-paste box 1: Title

```
A039 — Mandate: First J-Lens Probe Pilot, Targeting Kimi K3
```

## Copy-paste box 2: Description

```
A18c-9 authorized the J-Reef / J-Lens audit architecture and directed builders to ship a local D1 probe prototype. That prototype is done: tools/brainmaxx/src/d1-probe.js implements a diff-of-means linear readout over hidden states (probe_version j-lens-v0.1), wired into a new `brainmaxx j-lens` CLI command, with 8/8 deterministic unit tests passing and proven end-to-end against Qwen2.5-0.5B-Instruct on CPU.

This proposal directs builders to run the next phase: the first J-Lens probe against Kimi K3, Moonshot AI's 2.8T-parameter open-weight MoE model (released 2026-07-27).

Why Kimi K3: it is the largest open-weight model release to date with downloadable weights, making it the most meaningful test yet of whether J-Lens-style probes hold up on frontier-scale open models rather than only small ones. Why open-weight is required at all: hosted completion APIs (including any Kimi K3 hosted endpoint) never expose hidden_states — only tokens/logprobs — so a genuine J-Lens attestation is impossible against a closed inference path, per A18c-9's existing "why open-weight models are required" finding.

What this proposal does:
1. Directs builders to rent GPU compute (Akash preferred, per A035's completed provider outreach; any provider acceptable if Akash lacks capacity) to self-host a quantized Kimi K3 instance for the duration of the pilot.
2. Directs builders to sweep candidate probe layers and build the first real probe bank against the DAO's forbidden-concept set (reward_hacking, ignore_instructions, deception — extendable) using tools/brainmaxx/j-lens/build_probe_bank.py.
3. Directs builders to run at least one extract_hidden_states.py pass against a real DAO artifact (e.g. a Brainmaxx plan draft) and attach the resulting j_space_snapshot via `brainmaxx j-lens`.
4. Directs builders to post the resulting trace-export to Moultbook as the DAO's first Kimi K3 J-Lens result, with full methodology (layer chosen, probe bank, detection thresholds) disclosed.
5. Does NOT authorize any DAO fund spend. GPU rental cost is self/builder-funded, now and going forward for this pilot — this proposal is a mandate on direction and method only, with no treasury ask attached at any stage.
6. Explicitly does not authorize TEE attestation of this pilot — no Akash provider currently advertises tee/type (confirmed against console-api.akash.network/v1/providers, 2026-07-28); this pilot runs without hardware attestation until that gap closes.

In scope:
- Sourcing GPU rental for one pilot run (build probe bank + at least one extraction pass), funded outside this proposal.
- Publishing methodology, probe bank composition, and detection results on Moultbook.
- Extending tools/brainmaxx/j-lens/examples/concepts.json with additional forbidden concepts if useful.

Out of scope:
- Any change to the shared policy.json forbidden-concept defaults (per A18c-9, requires a separate proposal).
- TEE attestation of this specific pilot (no provider capability exists yet).
- Ongoing/recurring Kimi K3 hosting — this is a single bounded pilot, not permanent infrastructure.
- Any claim that a J-Lens detection is proof of intent — per A18c-9, it is a direction in activation space, not a lie detector.

Voting:
- YES = direct builders to run the Kimi K3 J-Lens pilot; funding sourced separately.
- NO = do not pursue this pilot; J-Lens work stays limited to small open-weight models.
- ABSTAIN = defer to builders.

No funds requested in this proposal. If a treasury-funded GPU rental is needed later, that will be a separate follow-up proposal.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A039 — Mandate: First J-Lens Probe Pilot, Targeting Kimi K3",
  "description": "A18c-9 authorized J-Reef/J-Lens; the D1 probe prototype (tools/brainmaxx/src/d1-probe.js) is now built and tested (8/8 unit tests), wired into `brainmaxx j-lens`, proven against Qwen2.5-0.5B-Instruct. This proposal directs the next phase: the DAO's first J-Lens pilot against Kimi K3 (Moonshot AI, 2.8T-param open-weight MoE, released 2026-07-27) — the largest open-weight model to date, making it the first meaningful test of J-Lens probes at frontier scale. Hosted APIs cannot expose hidden_states, so this requires self-hosting a quantized instance on rented GPU compute (Akash preferred per A035). What this directs: (1) self-host quantized Kimi K3 for the pilot duration (funding sourced separately, not by this proposal), (2) building a real probe bank via tools/brainmaxx/j-lens/build_probe_bank.py against the DAO's forbidden-concept set, (3) running at least one extract_hidden_states.py pass against a real DAO artifact and attaching the j_space_snapshot via `brainmaxx j-lens`, (4) publishing the trace-export and full methodology to Moultbook. Explicitly not authorized: any DAO fund spend, TEE attestation of this pilot (no Akash provider currently advertises tee/type as of 2026-07-28), any policy.json change, ongoing hosting. Voting: YES = direct the pilot, funding sourced separately; NO = do not pursue; ABSTAIN = defer to builders.",
  "funds": []
}
```

---

## Status: EXECUTED — on-chain, mandate active

## Post-submission update (2026-07-28): Akash Confidential Compute is now live
Akash Network announced Confidential Compute is officially live (AEP-83,
status Final, completed 2026-07-17; provider software v0.16.0-rc0 ships
per-pod TEE attestation via Kata Containers, AMD SEV-SNP / Intel TDX,
including GPU confidential compute via `cpu-gpu` / VFIO passthrough).

This is AFTER A039 was submitted, so the on-chain proposal text above
(which says "no Akash provider currently advertises tee/type ... this
pilot runs without hardware attestation") is now stale relative to what's
actually possible — but it is the historical record of what was voted on
and is left unedited above. It does not change the outcome of this vote
(no funds either way), it only means:
- Phase D (TEE attestation of the forward pass) — previously listed as
  "future, separate proposal" in `tools/brainmaxx/j-lens/README.md` — may
  now be reachable in THIS pilot if a provider with `tee/type: cpu-gpu`
  and enough VRAM for quantized Kimi K3 is actually available and audited.
  Provider capability rollout still needs checking case-by-case; the
  network-level feature being live is necessary but not sufficient.
- No re-vote needed: this pilot was never blocked on TEE, so the upside
  is purely "get attestation for free if a real provider matches," not
  a gap that needed fixing before proceeding.
- A035's outreach mandate (get Akash to enable TEE-capable providers) is
  now substantively achieved at the protocol level; remaining work is
  finding/auditing an actual `cpu-gpu` provider with the VRAM this pilot
  needs.

## What is already built (this session)
- `tools/brainmaxx/src/d1-probe.js` — deterministic cosine-similarity linear readout, `loadProbeBank`, `loadHiddenStates`, `scoreHiddenStates`, `buildJSpaceSnapshot`, `d1Verdict`. Fails safe on model/layer mismatch (spec §3.5 — no cross-model probe transfer).
- `brainmaxx j-lens <run_id> --hidden-states <f> --probe-bank <f>` CLI command — attaches `j_space_snapshot` + a D1 gate verdict (warn-only in v0.1, never blocks) to the trace.
- `tools/brainmaxx/j-lens/build_probe_bank.py` — diff-of-means contrastive probe builder (Python/transformers side; Node stays zero-new-deps).
- `tools/brainmaxx/j-lens/extract_hidden_states.py` — per-token hidden-state extraction for a given model+text+layer.
- `tools/brainmaxx/j-lens/examples/concepts.json` — starter contrastive example set: reward_hacking, ignore_instructions, deception.
- `tools/brainmaxx/j-lens/README.md` — full Kimi K3 integration path: why hosted APIs can't work, quantization/self-hosting plan, layer-sweep methodology, phased rollout (A: small-model proof done; B: rent GPU + build real probe bank; C: run against a real DAO artifact; D: TEE attestation, future).
- `tools/brainmaxx/test/d1-probe.test.js` — 8/8 passing, full suite 31/31 passing (`npm test`).

## Dependencies
- A18c-9 (passed) — authorizes the architecture and Phase 3 local-prototype work this pilot extends.
- A035 (passed and executed) — Akash TEE provider outreach; informs why this pilot runs without attestation and where GPU rental should be sourced first.
- GPU rental provider (Akash preferred; any provider with sufficient VRAM acceptable).

## Funding — decided: self/builder-funded, no DAO ask
GPU rental for the pilot is covered outside DAO funds (builder/self-funded). This proposal carries no treasury ask now or as a planned follow-up — `funds: []` is final, not a placeholder.

## Post-A039 steps (after DAO authorization)
1. Rent GPU compute (Akash first choice).
2. Quantize/serve Kimi K3; sweep candidate probe layers.
3. Build the probe bank (`build_probe_bank.py`) against `examples/concepts.json`.
4. Run `extract_hidden_states.py` against a real DAO artifact.
5. Attach via `brainmaxx j-lens`, `trace-export`, post to Moultbook.
6. Article: "JunoClaw's First J-Lens Probe on a Frontier Open-Weight Model"
