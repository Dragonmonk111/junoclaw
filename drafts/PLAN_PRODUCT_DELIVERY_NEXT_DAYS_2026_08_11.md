# Product Delivery Plan — Next Few Days (2026-08-11)

## Where things actually stand

| Layer | Component | Status |
|---|---|---|
| Truth | CSI v0.2, D1 probe, audit API, CLI | Built, tested, scaling study published |
| Truth | J-Lens Colab pipeline (free T4) | Built today — 6 concepts, layer sweep |
| Truth | On-chain probe question bank | Built today — `examples/onchain_probe_questions.md` |
| Coordination | `junoclaw-coordination` crate (message, gate, consensus) | Built — 28 Rust + 23 TS tests pass |
| Coordination | Real P2P mesh (`commonware-p2p`) | Blocked — needs NASM, simulated consensus used instead |
| Coordination | `coordination-settler` CosmWasm contract | Built, NOT deployed anywhere |
| Coordination | Relayer daemon | Built, NOT run against a live network |
| Settlement | Juno mainnet contracts (agent-company, moultbook, zk-verifier, jclaw-credential, skill-registry) | Live on juno-1 |
| Governance | A21 (three-layer stack authorization) | Drafted, **not yet confirmed submitted/passed** |

**Critical gap**: the coordination-settler contract has a certificate-verification TODO (`contract.rs:120-128`) — it currently accepts certificates on trust from the relayer, no real BLS threshold-signature check. This is fine for testnet, not for mainnet.

## What "launch on-chain in a few days" realistically means

Full mainnet BFT coordination network is a multi-week Rust engineering effort (NASM P2P, real BLS verification). That is NOT a few-days task. But there are several things that ARE a few-days task and deliver real, visible, on-chain value:

### Track 1 — Ship J-Lens as a usable audit service (fastest, most concrete)
1. Run the Colab pipeline notebook end-to-end today, capture a real `j_space_snapshot.json` from a Qwen2.5 model on the on-chain probe questions (`onchain_probe_questions.md`).
2. Post the results to Moultbook as a J-Lens audit report — this is a real, on-chain artifact proving the probe bank works on realistic DAO-agent prompts.
3. Wire CSI server (`tools/brainmaxx/src/csi-server.js`) to run persistently (small VPS or even a background process) so the coordination layer's gate.rs can call it for real instead of mock mode.
4. This alone is a shippable, on-chain-attested proof that Brainmaxx/J-Lens works on realistic governance-relevant prompts — no Akash, no waiting on NASM.

### Track 2 — Deploy coordination-settler to testnet (uni-7), not mainnet
1. Build + deploy `coordination-settler` to uni-7 (testnet). This is scoped as in-scope in A21 already — no new DAO proposal needed if A21 passed.
2. Run the relayer daemon against the simulated consensus engine (no NASM needed — Phase 2 is already "simulated simplex," which is functionally complete for testing the settlement path).
3. Run `test-mesh`, `gate-test`, `consensus-test` binaries to confirm the full local pipeline (message → consensus → gate → settle) works end-to-end against a real chain.
4. This proves the full three-layer stack works, just without the real P2P transport — which is an infrastructure detail, not a product gap.

### Track 3 — Confirm A21 governance status
Before doing anything mainnet-facing, confirm whether A21 (three-layer stack authorization) has actually passed. If not submitted yet, submit it. If passed, cite the passage in Track 2's testnet deployment as authorization.

## Validator-security integration question (does the coordination layer need its own validator set?)

Short answer: **not necessarily, and probably not yet.** Options, cheapest first:

1. **DAO-appointed validator set (current design)** — 4 nodes, appointed by the Juno Agents DAO, tolerates 1 byzantine. This is what's already scoped in A21/PLAN_THREE_LAYER_STACK_COMMONWARE.md. No new trust assumption beyond "the DAO picks operators." Cheapest, fastest, matches what's already built.

2. **Juno validator opt-in (medium term)** — some subset of actual Juno mainnet validators run the coordination node as a **sidecar**, using the same operator keys/infrastructure they already run for consensus. This doesn't require a protocol-level change to Juno — it's a voluntary, off-chain BFT network that Juno validators choose to participate in, similar to how validators run oracle sidecars (e.g. Slinky/Skip) today. You'd approach this as a proposal to the validator set: "run this sidecar, get X reward from DAO funds (if any) or reputation." This is a business-development/outreach task, not an engineering task — Jack Zampolin (Cosmology/Skip founder, mentioned in your existing article) is the natural first contact since he's already building on Commonware.

3. **Full protocol-level security inheritance (long term, hard)** — actually gating settlement on Juno's own CometBFT validator signatures (e.g., via ICS-style shared security or an ABCI++ vote extension). This requires a Juno governance upgrade and is a multi-month, high-risk path. Not recommended until Track 1/2 prove the product.

**Recommendation**: Do NOT chase option 3 now. Ship Track 1 and Track 2 this week. Use them as concrete proof-points when reaching out to Juno validators for option 2 — "here's a working testnet deployment, here's the code, will you run the sidecar." Selling validators on running new infra is much easier with a working demo than with a whitepaper.

## Repo assets you already have that are underused

- `tools/context-agent/` — Moultbook indexer + trust scoring (AKB v1.1). Should be the thing that reads J-Lens audit reports back out of Moultbook and feeds them into agent trust scores. Nobody has wired J-Lens output → context-agent trust input yet. This is a cheap, high-value connection.
- `wavs/bridge/invoke-server.ts` — off-chain TEE invoke API (A033, tested). Could eventually host the CSI server INSIDE a TEE-attested WAVS component so the audit itself is attested, not just trusted. Not urgent, but worth remembering as the "why is J-Lens trustworthy" answer for skeptics.
- `crates/junoclaw-coordination-napi/sdk/example.ts` — already has a working 3-agent coordination demo. This can be re-recorded/re-run against the uni-7 deployment from Track 2 as a demo video/gif for the article.

## Concrete task list for the next few days

1. [ ] Run `jlens_colab_pipeline.ipynb` on Colab against `onchain_probe_questions.md` — get real numbers.
2. [ ] Post J-Lens on-chain probe audit results to Moultbook.
3. [ ] Confirm A21 status (submitted? passed?). Submit if not done.
4. [ ] Deploy `coordination-settler` to uni-7 testnet.
5. [ ] Run relayer + `test-mesh`/`gate-test`/`consensus-test` against uni-7 deployment.
6. [ ] Wire `tools/context-agent` to ingest J-Lens Moultbook attestations into trust scoring.
7. [ ] Draft outreach message to 1-2 Juno validators proposing the sidecar model (option 2 above), pointing at the working testnet deployment.
8. [ ] Write/publish the "finish line" article tying the on-chain probe results + Anthropic introspection research together (separate deliverable, see article draft).
