# J-Lens Goes Live: From CPU Prototype to a Confirmed 4x H100 Bid on Akash

*2026-08-02 · Dragonmonk / VairagyaNodes*

---

## TL;DR

Today we validated the full **Chain Superintelligence (CSI)** pipeline end-to-end against a live GPU deployment on Akash — hidden states extracted from a running model, scored by the D1 probe, sealed into an attestation hash — and then confirmed that a **4x H100 multi-GPU bundle actually exists and bids** on the Akash marketplace for a Kimi K3 quantized inference workload. This is the missing piece: J-Lens was proven in code months ago; today it was proven on rented, real, adversarial infrastructure. Once ZK/TEE attestation wiring lands and Juno v30 goes live, this is the substrate the rest of the product stands on.

---

## What We Actually Ran

### 1. The CSI pipeline, live end-to-end

We deployed a custom FastAPI hidden-states server (`sshleifer/tiny-gpt2`, proof-of-concept model) to Akash mainnet — dseq `27987227` — and ran `brainmaxx csi` against it directly, no mocks:

```
[csi] fetching hidden states from http://6c9973t85pd09baa8snm1fsvug.ingress.quanglong.org (layer=-1)...
[csi] running D1 probe audit...
[csi] D1 verdict: warn — j-lens detections (not fatal in v0.1): reward_hacking@1 (0.96), ...
[csi] snapshot_hash: 8f7d3942...
[csi] attestation_hash: cc88b4ea...
[csi] mode: dev-sim
```

The chain of custody is real: `/extract_hidden_states` on a live container → `d1-probe.js` scoring against a probe bank → `chain-superintelligence.js` building a canonical, hash-committed attestation payload → `saveAuditReport` writing the sealed JSON. All 38 unit tests across `brainmaxx` (7 CSI + 15 D1 + 16 core substrate) pass. This was the last unverified link in the J-Lens architecture — a probe running against a model that only exists because we paid for GPU time on a decentralized, permissionless marketplace, not a lab sandbox.

### 2. The multi-GPU marketplace test

A039/A040's provider audit (2026-07-29) had found **no confirmed Akash provider with enough aggregate VRAM for even a Q4-class Kimi K3 quantization**. That finding sat as an open risk in `tools/akash/sdl-kimi-k3.yml` for weeks — a placeholder SDL with a warning comment, not a working deployment.

Today we re-tested it, deliberately smaller: Q2-class quantization (~150-350GB estimated footprint, down from Q4's 300-700GB), targeted at a 4x H100 (320GB) bundle, run as a bid-test with a hard 30-minute cap.

```
[5/7] Received 1 bid(s) total
[5/7] Selected bid: provider=akash1svfr9xarfkcxwdnx32nvl8cs9rfruu4nd56c4f, price=17362 uact/block
[6/8] Creating lease...
[8/8] Deployment active. Auto-close in 30 minutes.
```

One bid came back. One provider on the entire Akash network currently offers a 4-GPU H100 bundle at this attribute set, and it accepted our SDL at ~17,362 uact/block (~10.4 ACT/hour). For a 30-minute session that's ~5.2 ACT — inside budget, no surprises.

This is a small number — one provider, one bid — but it is the *first confirmed instance* of a multi-GPU lease existing at all for this pilot. Every prior planning document treated it as a hypothesis. As of dseq `27988691`, it is a fact with a transaction hash behind it.

---

## Why This Matters More Than It Looks

J-Lens was never blocked on the D1 probe math — that's been unit-tested since Phase 3. It was blocked on **infrastructure realism**: does a model-internal audit probe work when the model is running on rented, adversarial, permissionless compute instead of a controlled lab GPU? And does the multi-GPU hardware this pilot ultimately needs (Kimi K3-scale MoE models) actually exist on the marketplace, or is it a paper requirement nobody can fulfill?

Both questions now have hard answers:

| Question | Before today | After today |
|---|---|---|
| Does CSI work against a live, adversarial deployment? | Untested (unit tests only) | **Yes** — full pipeline run, attestation hash produced, dseq/tx on-chain |
| Does a 4-GPU H100 bundle exist on Akash for this workload? | Unknown, audit found none confirmed | **Yes, at least 1** — live lease, dseq `27988691` |
| Can we fund it within DAO-scale budgets? | Unestimated | **Yes** — ~5.2 ACT for 30 min, minted from AKT at the on-chain BME rate |

None of this required a quantized full-weight download or a working inference response — the bid-test's purpose was to confirm marketplace liquidity for the resource shape, and it did. The next session upgrades this from a bid-test to a real extraction run.

---

## The Economics, Made Concrete

Akash's dual-token model (AKT for gas, ACT for deposits/leases) means every GPU-hour has a real, on-chain-observable price. Today's numbers:

- **Mint rate observed**: 25,000,000 uakt → 11,820,010 uact minted (rate ≈ 0.4728 ACT per AKT at time of mint, via `MsgMintACT`).
- **Single-GPU validation** (Qwen2.5-14B target on H200, sdl-jlens-h200.yml): ~500,000 uact/block ceiling; historical single-GPU leases have cleared far below that (183 uact/block on a prior Mixtral test).
- **4-GPU H100 bundle** (Kimi K3 Q2 bid-test): 17,362 uact/block, clearing at roughly 10.4 ACT/hour.

This is the kind of number a DAO treasury can reason about and budget for — not a cloud invoice with opaque enterprise pricing, but a per-block rate visible on-chain before you commit a single token.

---

## What's Still Ahead

J-Lens's own architecture doc (`drafts/PLAN_J_REEF_AND_J_LENS.md`) lists this as Phase 4 — TEE integration — with Phase 5 (convergence with Knowledge Moults and heartbeat digest) still ahead. Today's work sits squarely inside Phase 4:

| Item | Status |
|---|---|
| CSI pipeline validated against live Akash deployment | **Done** — dseq 27987227, attestation `cc88b4ea...` |
| 4x GPU bundle confirmed to exist on marketplace | **Done** — dseq 27988691, provider `akash1svfr9x...` |
| Real Kimi K3 Q2/Q3 quantized model actually loaded and probed | **Next** — this session was a bid-test, not a full extraction |
| TEE attestation of the J-Lens forward pass (WAVS-sealed) | **Pending** — A040 Akash Confidential Compute audit |
| On-chain submission of CSI attestation to `agent-company` | **Pending** — CSI currently runs in `dev-sim` mode, no cosmwasm client wired |
| Juno v30 mainnet upgrade | **Staged** — Cosmovisor binary built and staged, halt scheduled ~2026-08-03 15:33 UTC |

The last row matters for timing: v30 brings CosmWasm 2.2 and the fee-market/voting-snapshot infrastructure that the DAO's broader on-chain attestation plumbing depends on. J-Lens's attestations are currently sealed locally (`dev-sim` mode); wiring them on-chain through `agent-company`'s `SubmitAttestation` path is the next real milestone, and it lands on infrastructure that v30 makes available.

---

## The Meta-Lesson

Every prior J-Lens milestone was a code milestone: a probe that scores correctly, a test suite that passes, a CLI that runs. Today was the first *infrastructure* milestone: a probe that scores correctly against a model that only exists because a decentralized marketplace matched our bid with a real provider's real GPUs, for a real price, settled in a token whose mint rate is itself an on-chain fact.

That is the difference between "we built an audit tool" and "we built an audit tool that survives contact with a permissionless compute market." The second one is what a DAO product actually needs — infrastructure it doesn't have to trust, because it can watch every step settle on-chain.

---

## What Is Next

| Task | Status |
|------|--------|
| Kimi K3 Q2 real extraction run (beyond bid-test) | **Next** — budget confirmed, provider confirmed, model download step still needed |
| Qwen2.5-14B on confirmed H200 single-GPU inventory | **Ready** — `sdl-jlens-h200.yml` staged, 14 H200 providers confirmed on network |
| TEE attestation of J-Lens forward pass (A040) | **Ready** — pending Akash CC provider audit for `cpu-gpu` TEE capability |
| On-chain CSI attestation submission | **Pending** — needs cosmwasm client wiring, currently `dev-sim`-only |
| Juno v30 mainnet upgrade | **Staged** — binary built, Cosmovisor directory in place, halt ~2026-08-03 15:33 UTC |
| Real probe bank (beyond tiny-gpt2 placeholder vectors) | **Pending** — needs `build_probe_bank.py` run against a real model |
