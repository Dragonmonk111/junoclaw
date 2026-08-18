# A45 — Three-Layer Coordination Stack: Architecture Ratification

> Re-submission of A44 (three-layer stack authorization, rejected by steward NO vote). BN254 precompile mandate removed from this proposal — it is now an open question to the Juno team (see below). This proposal narrows scope to coordination stack ratification only, with a progress-based gating mechanism and explicit mainnet deployment gate.

---

## Copy-paste box 1: Title

```
A45 — Three-Layer Coordination Stack: Architecture Ratification
```

## Copy-paste box 2: Description

```
A44 proposed authorizing the full 13-week build plan for a Commonware-based coordination layer. The juno-ai steward voted NO (3/6 power), likely due to the broad scope and lack of testnet validation. This proposal narrows the scope significantly and comes with concrete proof that the work is already done and deployed.

WHAT THIS PROPOSAL DOES:

1. Ratifies the three-layer architecture (Truth → Coordination → Settlement) as the DAO's intended direction for agent coordination.
2. Acknowledges that testnet evaluation is already underway — the coordination-settler contract, relayer daemon, J-Lens gate, and agent SDK are built and tested (28 Rust + 23 TypeScript tests passing). The coordination-settler contract is deployed live on Juno testnet (uni-7): contract address juno16gp6mm7ehhrx0ls4s77yk668cclq5gyc09x7k45f28apwmsqkg0qhmmnc8, code ID 86, with real on-chain store and instantiate transactions. The certificate verification TODO was fixed before deployment — the contract recomputes SHA256(messages_hash || validator_pubkeys) on-chain and rejects forged certificates.
3. Confirms the coordination network is NOT a blockchain: no tokens, no staking, no governance. Validator set appointed by the DAO. Juno remains the settlement layer.
4. Directs that every message passing through the coordination network must pass the J-Lens truth gate before acceptance (green proceeds, yellow warns, red blocks).
5. Gates mainnet deployment of the coordination-settler on a separate follow-up proposal after testnet validation proves the full pipeline works end-to-end against a live chain.
6. Endorses the validator sidecar security model as the target for production: Juno validators run the coordination node as a sidecar alongside their junod process (same model as oracle networks like Skip/Slinky), so the coordination layer's security eventually rests on the same validator set that already secures Juno — not a DAO-appointed committee. Initial testnet evaluation uses a DAO-appointed 4-node set (tolerates 1 byzantine). The sidecar model is the upgrade path once testnet validation proves the product and validators opt in.

This is a signal proposal, not a build authorization. The build is already done. The contract is deployed. This vote is about whether the DAO endorses the direction and agrees that mainnet deployment should follow successful testnet validation.

CURRENT STATUS (honest gaps):
- coordination-settler contract: deployed live on uni-7, certificate verification works (SHA256, rejects forged certificates)
- J-Lens gate: built, 28 Rust tests pass, gate-test binary verified — blocks deceptive content, passes clean content
- Agent SDK: built, 23 TypeScript tests pass, 3-agent demo works
- Commonware P2P mesh: scaffolded but not yet compiled with NASM — real P2P transport needs native compilation
- BFT consensus: simulated (single-process, produces real-format certificates) — real multi-node consensus needs NASM + 4 running nodes
- Relayer daemon: built, not yet run against uni-7 deployment
- Validator sidecar: not yet built — this is the production security target, not the current state

What works end-to-end today: message → simulated consensus → J-Lens gate → certificate production → on-chain verification on uni-7.
What's still needed: real P2P (NASM compile), real multi-node consensus (4 nodes), relayer against live uni-7, validator sidecar + outreach.

The coordination stack runs on stock Juno today — pure CosmWasm, no precompile dependencies, no custom wasmvm. A pure-Wasm BN254 verifier (zk-verifier, code ID 5146) is already deployed on Juno mainnet for on-chain proof verification. The coordination-settler uses SHA256 certificate verification now and can upgrade to BLS12-381 threshold signatures later when BN254 precompile support is available — the contract interface stays identical, only the verification function changes.

Note on Commonware: Commonware is independently launching production "open" Constantinople clusters with on-chain staking at very high throughput. This validates the P2P/consensus primitives we use, but it does not change our scope. The JunoClaw coordination layer is not a competing blockchain — it is an agent-message ordering and certificate-production layer that settles on Juno. Our 300ms block time is for small batches of J-Lens-audited agent messages, not a ledger. We benefit from Commonware's real-world testing without needing to match their TPS numbers.

OPEN QUESTION TO THE JUNO TEAM — BN254 PRECOMPILE:

Prop #374 (passed May 2026, ~80% Yes) signaled community support for BN254 host functions in CosmWasm. The v30 upgrade shipped without them. Track B (v3.0.x forward-port) is complete: 10/10 patches apply clean against cosmwasm v3.0.6, 22/22 crypto-bn254 tests pass. The upstream CosmWasm issue (#2685) is deferred to Backlog — the CosmWasm team will not take external proposals until ~Q3/Q4 2026.

The patch series is published and ready. The build approach (P2) requires zero fork maintenance — patches are applied at build time, no git fork to maintain. When upstream reopens, the same patches become the PR body.

This proposal does NOT mandate the Juno team to ship a patched wasmvm. Instead, it asks: does the Juno team want BN254 precompile support sooner than Q3/Q4 2026? If so, the builder will:
- Provide the build script and patch series for validator reproducibility
- Build and publish a verified libwasmvm.x86_64.so with SHA256 checksums
- Support the Juno team with integration and bug fixes

If the Juno team prefers to wait for upstream CosmWasm, that is fully acceptable — the coordination stack works without the precompile. The pure-Wasm BN254 verifier is higher gas (~371k vs ~203k with precompile) but functionally complete.

VALIDATOR SIDECAR SECURITY MODEL:

The coordination network needs nodes to run BFT consensus. Three options for who operates them:

1. DAO-appointed validator set (current design, testnet only) — 4 nodes appointed by the DAO, tolerates 1 byzantine. Cheapest, sufficient for testnet validation. Trust assumption: the DAO picks honest operators.

2. Juno validator sidecar (target for mainnet) — Juno validators run the coordination node as a sidecar alongside their junod process, same model as oracle networks like Skip/Slinky. Security inherits from Juno's validator set — no new trust assumption beyond what already secures the chain. This proposal endorses option 2 as the target for mainnet.

3. Full protocol-level shared security (long term, not recommended now) — gating settlement on Juno's own CometBFT validator signatures via ICS-style shared security or ABCI++ vote extensions. Requires Juno governance upgrade, multi-month, high-risk. Not recommended until options 1 and 2 prove the product.

The path: ship testnet with option 1 (DAO-appointed), prove the product, then approach Juno validators to run sidecars (option 2). Selling validators on running new infra is easier with a working demo than a whitepaper.

WHAT CHANGED FROM A44:
- Reframed from "authorize 13-week build" to "ratify architecture + acknowledge testnet evaluation underway."
- No fixed timeline commitment. Build proceeds at builder's pace.
- Explicitly gates mainnet on a separate proposal + testnet validation.
- BN254 removed as a mandate — now an open question to the Juno team, not a demand.
- Validator sidecar model endorsed as the production security target — addresses the concern about DAO-appointed committees being a new trust assumption.
- Honest about gaps: simulated consensus vs real P2P, DAO-appointed validators vs validator sidecars. No overstatement of what's deployed.
- Zero liability for the Juno team: coordination stack runs on stock Juno, BN254 is optional and opt-in.

VOTING:
- YES = endorse the three-layer architecture direction + signal interest in BN254 precompile if Juno team is willing.
- NO = do not pursue the coordination stack direction at this time.
- ABSTAIN = defer to builders.

No funds requested. No mainnet contract changes. No membership changes. No tokens.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A45 — Three-Layer Coordination Stack: Architecture Ratification",
  "description": "A44 proposed authorizing a 13-week build plan for a Commonware-based coordination layer. The juno-ai steward voted NO (3/6 power), likely due to broad scope and lack of testnet validation. This proposal narrows scope and comes with concrete proof the work is done and deployed. WHAT IT DOES: (1) Ratifies the three-layer architecture (Truth→Coordination→Settlement) as the DAO's intended direction. (2) Acknowledges testnet evaluation is underway — coordination-settler contract deployed live on uni-7 (code ID 86, real on-chain txs), certificate verification implemented (SHA256 on-chain, rejects forged certificates), 28 Rust + 23 TS tests pass. (3) Confirms the coordination network is NOT a blockchain — no tokens, no staking, validator set appointed by DAO, Juno remains settlement layer. (4) Every message must pass J-Lens truth gate before acceptance. (5) Mainnet deployment gated on separate proposal after testnet validation. (6) Endorses validator sidecar security model as mainnet target — Juno validators run coordination node as sidecar (like Skip/Slinky oracles), inheriting security from Juno's validator set instead of a DAO-appointed committee. Testnet uses DAO-appointed 4-node set; sidecar is the upgrade path. STATUS GAPS: Commonware P2P scaffolded but needs NASM compile for real transport; BFT consensus is simulated (single-process, real-format certificates); relayer built but not yet run against uni-7; validator sidecar not yet built. What works end-to-end: message→simulated consensus→J-Lens gate→certificate→on-chain verification on uni-7. The coordination stack runs on stock Juno — pure CosmWasm, no precompile dependencies. OPEN QUESTION TO JUNO TEAM: BN254 precompile patches complete (10/10 clean, 22/22 tests pass) but upstream CosmWasm deferred to Q3/Q4 2026. Does Juno team want BN254 sooner? If so, builder provides patches+build script+binary+support. If not, coordination stack works without it. Changes from A44: reframed from authorize-build to ratify-architecture, no fixed timeline, mainnet gated, BN254 is open question not mandate, validator sidecar endorsed as security target, honest about simulated-vs-real gaps, zero liability for Juno team. Voting: YES=endorse architecture+validator sidecar target+signal BN254 interest; NO=do not pursue; ABSTAIN=defer. No funds, no mainnet changes, no tokens.",
  "funds": []
}
```

---

## Background

- **A42/A43** (BN254 mandate): rejected, zero votes — invisible on daodao.zone due to Argus indexer freeze. The proposals existed on-chain but nobody could see them to vote. PR #1862 (pending review) fixes the indexer fallback so this won't recur. BN254 is no longer part of this proposal — it's an open question to the Juno team instead.
- **A44** (three-layer stack): rejected, juno-ai steward voted NO (3/6 power). The steward may have objected to the broad scope ("authorize 13-week build") or the lack of testnet validation before mainnet authorization.
- **Prop #374** (passed May 2026, ~80% Yes): signaled community support for BN254 host functions. Patches are complete and ready — waiting for either upstream CosmWasm (Q3/Q4 2026) or Juno team interest.
- **Prop #377** (v30 upgrade, passed July 2026): shipped without BN254, broke Argus indexer.
- **zk-verifier** (pure Wasm BN254, code ID 5146): already deployed on Juno mainnet. Proves BN254 verification works without a precompile — just at higher gas cost.

## Why BN254 was removed from this proposal

1. The coordination stack does NOT depend on BN254. It runs on stock Juno with pure CosmWasm today.
2. BN254 precompile requires the Juno team to ship a non-standard wasmvm — that's a liability only they can decide to take on.
3. Upstream CosmWasm deferred issue #2685 to Q3/Q4 2026. Mandating something that's blocked upstream creates political friction without unblocking anything.
4. The patches are done and ready. When the window opens (upstream or Juno team), they ship. No proposal needed to keep them warm.
5. Combining BN254 with the coordination stack dragged down the whole proposal by association. Splitting them lets the coordination stack stand on its own merits.

## What changed from A44 to address the NO vote

- **Reframed from authorization to endorsement**: "ratify architecture + acknowledge testnet evaluation underway" instead of "direct 13-week build." The code is already built — this is about whether the DAO endorses the direction, not whether it grants permission to build.
- **No fixed timeline**: build proceeds at builder's pace, no arbitrary deadline pressure.
- **Mainnet explicitly gated**: separate proposal required after testnet validation. No ambiguity about what this vote authorizes.
- **BN254 removed as a mandate**: now an open question to the Juno team, not a demand. The coordination stack works without it.
- **Validator sidecar model endorsed**: addresses the concern about DAO-appointed committees being a new trust assumption. Testnet uses DAO-appointed set; mainnet target is Juno validator sidecars (Skip/Slinky model).
- **Honest about gaps**: simulated consensus vs real P2P, DAO-appointed validators vs validator sidecars, relayer not yet run against live uni-7. No overstatement of what's deployed.
- **Zero liability**: coordination stack runs on stock Juno. No custom wasmvm, no fork maintenance, no upstream dependency.
- **Emphasizes existing proof**: 28 Rust + 23 TS tests passing, gate-test binary verified, 3-agent demo works, and the coordination-settler is already deployed live on uni-7 testnet with real on-chain transactions — this isn't a promise, it's a status report asking for endorsement.

## Submission checklist

- [ ] Confirm PR #1862 is merged (so proposal is visible on daodao.zone)
- [ ] Post to Commonwealth and DAO Discord simultaneously for off-UI visibility
- [ ] Reach out to juno-ai steward operators to understand their A44 NO vote
- [ ] Submit via DAO DAO UI or direct contract call
- [ ] After submission, verify proposal is visible on daodao.zone before campaigning

## Out of scope

- BN254 precompile mandate (open question to Juno team, not part of this proposal)
- Mainnet deployment of coordination-settler (separate proposal after testnet)
- Any token, ticker, or drip signal
- DAO membership changes
- Mandating that all agents must use the coordination network
- The v30.1 mainnet upgrade itself (separate governance vote if Juno team wants BN254)
