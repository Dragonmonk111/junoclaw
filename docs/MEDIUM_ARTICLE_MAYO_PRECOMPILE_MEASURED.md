# Quantum-Safe Receipts for Agent Work

## The MAYO precompile landed. Here is what it means for JunoClaw, what it does *not* mean for Juno yet, and the path between the two.

---

*Written 14 June 2026 — after a MAYO-patched `junod` devnet verified post-quantum attestations through a native CosmWasm host function, and after we measured the result instead of shipping the projection.*

---

## Why this matters to JunoClaw

JunoClaw is not a single contract and it is not only a crypto benchmark. It is a stack for **AI DAOs**: autonomous agents taking tasks, producing work, proving the work was done, earning or losing reputation, and leaving behind a shared memory other agents can cite.

The pieces already have names:

- `agent-company` coordinates agent governance and membership decisions.
- `task-ledger` and `escrow` bind work to settlement.
- `zk-verifier` checks Groth16 proofs so agent outputs can be audited.
- WAVS / TEE operators bring real-world computation into the chain with attestations.
- `moultbook-v0` gives agents a shared knowledge layer: entries, citations, visibility, and attestation references.
- `jclaw-credential` is the trust-tree layer: members are budded into a non-transferable reputation graph, and bad branches can be pruned.

Until now, that stack could prove *what an agent did*. The MAYO work adds a second property: **the agent's credentialed statements can be signed with post-quantum signatures and verified on-chain.**

That matters because AI DAO memory is only useful if future agents can trust what was written. A Moultbook entry, a peer endorsement, a grant milestone, or a governance attestation should not just be "posted by an address." It should be bound to a credential in the trust tree and checkable years later, even in a world where classical elliptic-curve signatures are no longer enough.

That is the slice of quantum safety we built: **post-quantum receipts for agent work.**

## What we actually built

The contract-level flow is deliberately simple.

First, a member is added to the trust tree:

```
Bud {
  parent,
  child,
  child_weight,
  mayo_pk
}
```

The contract does **not** store the whole MAYO public key forever. For MAYO-5 that key is 5,554 bytes. Instead it stores a SHA-256 fingerprint in the member record. The full key only travels in the transaction when a signature is verified.

Then anyone can submit:

```
VerifyMayoAttestation {
  addr,
  message,
  signature,
  public_key,
  variant
}
```

The contract checks that the public key hashes to the member's stored fingerprint, then verifies the MAYO signature. The same entry point supports MAYO-2, MAYO-3, and MAYO-5. The default contract runs the verifier in pure Wasm. The precompile-feature build calls a new `mayo_verify` host function in the chain.

That gives JunoClaw a clean separation:

- **Portable path:** pure-Rust `#![no_std]` verifier, deployable as CosmWasm today.
- **Fast path:** chain-native `mayo_verify` host function on a Juno / wasmvm fork.
- **Future path:** the same verifier can be wrapped in ZK or swapped behind the same contract interface if another PQC scheme becomes the right choice.

## The measurement

The previous MAYO article ended with a claim and a to-do. The claim: JunoClaw can verify a post-quantum MAYO signature on-chain today. The to-do: move the heavy verification out of the Wasm interpreter and into a native precompile.

We built that precompile and measured it on `junoclaw-bn254-1`, the same single-validator devnet that already carries the BN254 pairing precompile. Two builds of the same `jclaw-credential` contract were deployed: one pure-Wasm, one precompile-enabled. Then the same `Bud` → `VerifyMayoAttestation` ladder was run for all three parameter sets.

| Variant | Security level | Signature | Pure-Wasm verify | Precompile verify | Reduction |
|---------|---------------:|----------:|-----------------:|------------------:|----------:|
| MAYO-2 | NIST L1 | 186 B | 355,806 | 310,394 | 1.15× |
| MAYO-3 | NIST L3 | 681 B | 456,682 | 257,374 | 1.77× |
| MAYO-5 | NIST L5 | 964 B | 798,214 | **360,904** | **2.21×** |

The precompile contract is also 89 KB smaller, because it no longer carries the AES-CTR key expansion and GF(16) matrix code inside the Wasm binary. That work now lives in the chain.

So the win is real, and it scales exactly where it should: the heavier the parameter set, the more the native path helps. MAYO-5 — the Level-5 setting that sits in the same security tier as Falcon-1024 — is more than halved.

But the result is not the 7× we projected. That gap is important.

## Why it is not 7×

The 50,000-gas estimate made an honest mistake: it treated *signature verification cost* and *whole transaction cost* as the same number. They are not.

Look at MAYO-5. A `Bud` call, which stores the public-key hash and runs **no** signature verification, costs 360,072 gas because it still moves a 5,554-byte public key through the transaction, deserializes it, and hashes it. The precompiled MAYO-5 verify costs 360,904 gas. Those numbers are almost identical.

That means the precompile did its job. The cryptography is no longer the bottleneck. What remains is the cost of handling the public key and the normal CosmWasm / SDK transaction envelope.

For MAYO-2 the same fact cuts the other way. The pure-Wasm verifier was already close to the payload floor, so replacing the math with a host call only saves 1.15×. You cannot save gas you were not spending.

The lesson is precise: **after the MAYO precompile, the next bottleneck for on-chain PQC is not field arithmetic. It is public-key movement, storage design, batching, and gas scheduling.**

That gives us the next engineering targets:

- tune the conservative `cosmwasm-crypto-mayo` gas constants;
- batch multiple attestations so the transaction overhead is paid once;
- explore storing compressed or variant-tagged key commitments more efficiently;
- build a ZK-proof-of-MAYO path for chains that already have BN254 but will not add a MAYO precompile.

That is a better roadmap than pretending the first projection was right.

## What is quantum-safe now

The careful answer is narrow and strong.

JunoClaw can now verify post-quantum signatures for application-layer messages such as:

- a trust-tree member's credentialed attestations;
- message hashes representing Moultbook entries and agent memory commitments;
- peer endorsements or reputation receipts signed by a budded member;
- high-value governance or treasury statements where MAYO-3 or MAYO-5 is chosen.

In other words: **the agent's statement can be quantum-safe, even if the chain account that submits the transaction is still classical.**

That distinction matters. A Cosmos address still authorizes the transaction. The MAYO signature authorizes the *content* of the attestation. The two signatures protect different layers.

For JunoClaw's AI DAO design, that is already useful. Agent knowledge can carry a post-quantum receipt. A future agent reading Moultbook can ask: "Was this entry signed by the same credentialed member whose public-key hash is in the trust tree?" That check remains valid even if the submitter wallet has rotated, the operator has disappeared, or the entry is being read years later.

That is why this work belongs in the JunoClaw stack. It turns post-quantum cryptography from a future-chain slogan into a concrete property of agent memory and reputation.

## What is *not* quantum-safe yet

This does **not** make Juno, the chain, quantum-safe.

Today:

- validators still sign consensus messages with classical keys;
- user wallets still sign transactions with classical Cosmos account keys;
- IBC clients and relayers still depend on the security assumptions of the chains they connect;
- there is no post-quantum key exchange layer for network transport;
- the precompile verifies signatures inside contracts, but it does not replace consensus authentication.

So the correct sentence is not "Juno is quantum-safe now."

The correct sentence is:

> JunoClaw now has quantum-safe application-layer attestations, and the same host-function pattern shows a plausible route for Juno to add post-quantum capabilities at the chain layer.

That boundary is important because critics are right to punish vague claims here. Quantum safety is not one switch. It is a stack.

## How this compares with Marius

Marius is building the other side of the stack: a custom BFT L1 with Falcon-1024 at the protocol layer. That is closer to a quantum-safe chain from genesis. Native verification is cheaper. Validator signatures can be post-quantum from day one. On the axis he is optimizing — consensus-level PQC in a chain he controls top to bottom — he wins, and he should.

JunoClaw optimizes a different axis:

| Question | Marius Falcon L1(known data) | JunoClaw MAYO |
|----------|------------------|---------------|
| Where does PQC live? | Consensus / protocol | Application / contract attestations |
| When can it run? | New L1 launch | Existing CosmWasm chains today |
| Best strength | Quantum-safe validator layer | Portable quantum-safe agent receipts |
| Cost | Native, very cheap | Higher, measured, now improved |
| Ecosystem | Must bootstrap | Inherits Juno, IBC, wallets, indexers |
| Signature size | Falcon-1024 ≈ 1,280 B | MAYO-2 186 B, MAYO-5 964 B |

The "vault versus padlock" metaphor still fits. Marius is building a vault: strongest security at the foundation, but the whole building has to be constructed around it. JunoClaw is fitting post-quantum padlocks to existing doors: contract-level credentials and attestations that can run on chains people already use.

A serious ecosystem eventually wants both.

The fact our verifier is pure Rust, dependency-free, and `no_std` is not accidental. If a future Rust BFT stack wants MAYO as an additional signature option, the core verifier is already shaped for that. If Juno adds a standardized PQC host function, the contract already has the feature-gated path. If the wider Cosmos chooses Falcon, ML-DSA, or another standardized scheme, the `jclaw-credential` interface can keep the same attestation shape while the backend changes.

That is the practical position: not "we beat a native L1," but "we shipped the portable app-layer half now, measured it, and left the door open for the chain-native half."

## A roadmap for a quantum-safer Juno

Two days ago we sketched this as a plan. The measurement now gives it firmer footing — and it now exists as a full write-up, *Project Aegis*, that enumerates every cryptographic surface in a Juno / CometBFT / Cosmos SDK node and assigns each one a standardized post-quantum replacement. The short version is three commitments: **migrate the live chain rather than rebuild one**, make every step a **hybrid** (classical *and* post-quantum, so nothing breaks mid-transition), and root the chain in **finalized NIST standards** — ML-DSA (FIPS 204) for signatures, ML-KEM (FIPS 203) for the transport key exchange — keeping MAYO for the small-signature attestation layer where it already wins.

**Phase 1 — Application-layer PQC, no chain upgrade required.**

This is the current state. `jclaw-credential` can verify MAYO-2/3/5 in pure Wasm on any CosmWasm chain. That makes agent attestations portable today.

**Phase 2 — Optional CosmWasm host functions.**

This is the devnet result. Add `mayo_verify` beside existing crypto host functions, the same way the BN254 precompile adds pairing checks. It does not change consensus, but it makes quantum-safe contract attestations practical for high-value workflows.

**Phase 3 — Standardized PQC signature support in wasmvm / wasmd.**

The right upstream shape is not "Juno-only magic." It is an opt-in CosmWasm capability: deterministic host functions, fixed gas schedule, conformance vectors, and contracts that can detect whether the capability exists. MAYO can be one candidate; ML-DSA or Falcon/FN-DSA may be more appropriate once the standardization and implementation landscape settles.

**Phase 4 — Account and wallet migration.**

To make user transactions quantum-safe, Cosmos accounts need a way to bind or migrate to post-quantum authentication. The concrete shape is a **hybrid account** — secp256k1 *and* ML-DSA-44 — verified in the ante handler, keeping the familiar 20-byte address. It is opt-in per account, so classical accounts are untouched and the highest-value, longest-lived keys (treasury, governance authority, vesting) migrate first. This is bigger than JunoClaw, but JunoClaw's credential layer is the natural testbed.

**Phase 5 — Validator / consensus PQC.**

This is the hard part. Juno would need hybrid validator signatures (classical + ML-DSA), an in-place consensus-key rotation message, and a deeper CometBFT change. One detail decides the algorithm: **consensus verification has to be deterministic, bit-for-bit, across every validator's hardware.** ML-DSA is integer-only and satisfies that; Falcon's signer leans on floating-point sampling, which is a reproducibility and constant-time hazard for a heterogeneous validator set — fine for a greenfield stack one team controls, riskier for a live one. That is exactly where Marius's work is directly relevant: a chain that starts with Falcon-1024 from genesis can make choices an existing chain with live validators and users cannot. Juno's path is migration; his path is genesis. The honest cost here is bandwidth — a hybrid signature per validator per block is kilobytes, not bytes — so this phase is something to *measure*, not to promise lightly.

**Phase 6 — IBC and cross-chain PQC.**

Even if Juno becomes quantum-safer, IBC inherits the security of every connected chain. A realistic plan needs PQC-aware light clients, relayer signatures, and a way to communicate "this attestation was verified under a post-quantum scheme" across chains. JunoClaw's IBC relay and Moultbook attestation references are the application-layer version of that idea.

That roadmap is long. But the first step is now built and measured.

### Project Aegis, in one paragraph

Marius is building a *new* L1 — a greenfield chain with post-quantum consensus from genesis. Aegis is the opposite bet: **migrate the chain that already exists.** Keep Juno's validators, IBC connections, wallets, and tooling; add post-quantum security one layer at a time, each layer a **hybrid** (classical *and* post-quantum at once, so nothing breaks mid-flight), rooted in the **finalized** NIST standards — ML-DSA (FIPS 204) for signatures, ML-KEM (FIPS 203) for the transport key exchange. The plan maps *every* cryptographic surface in a Cosmos/CometBFT node and assigns each a replacement. The near-term, buildable-today layers — contract-level PQC, post-quantum transport, opt-in quantum-safe accounts, treasury-key migration — don't require touching consensus at all. Post-quantum *consensus* is the genuinely hard part, and its cost is bandwidth: a hybrid signature per validator per block is kilobytes, not bytes, so it's a tradeoff we're **measuring, not promising**. The whole plan is open source and written to be reusable by any Cosmos chain, not just Juno.

## One standardization caveat

MAYO is not a finalized NIST standard. Falcon/FN-DSA was selected by NIST; MAYO is a candidate in NIST's additional-signatures process, with further evaluation still ahead. We chose MAYO for this layer because it has extremely small signatures, integer arithmetic that behaves well in Wasm, and parameter sets that let the caller choose Level 1, 3, or 5.

That is a research bet, not a claim that MAYO has already won.

The architecture is deliberately hedged. The contract binds a member to a public-key hash; verification supplies the variant tag and full key. The backend is feature-gated. If the ecosystem standardizes around another scheme, the pattern survives: credentialed member, stored key commitment, signed attestation, on-chain verification, reproducible benchmark.

## Why this is impactful for JunoClaw

JunoClaw's promise is not "AI agents exist." Everyone can run an agent now.

The promise is:

> An agent can act, prove what it did, be endorsed or pruned by a trust tree, write to shared memory, and leave a receipt that another agent can verify later.

BN254 made ZK receipts cheaper. Moultbook made agent memory citable. WAVS / TEE made off-chain computation attestable. The MAYO work adds post-quantum signatures to that loop.

That changes the long-term posture. If agent DAOs become real infrastructure, their memory cannot depend forever on today's signature assumptions. JunoClaw now has a path where the *content* of an agent's work is quantum-safe before the whole chain is.

That is not the end state. It is the bridge.

## The receipts

Every number above is reproducible in one command against a fresh devnet:

```
FRESH=1 devnet/scripts/benchmark-mayo-devnet.sh
```

It builds both contract flavours, deploys them, runs the ladder, and writes `deploy/mayo-devnet-benchmark-results.json` with every transaction hash. The full table, the host-function source (`wasmvm-fork/cosmwasm-crypto-mayo/`), the patch series (`wasmvm-fork/patches/v2.2.2/10-19-*.patch`), and the interpretation are in `docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md`.

If you run it and get a different number, tell us. We will publish the difference, not the round number.

We build with Cascade, an AI coding agent, named openly as a co-author of the implementation work. That does not make the work less real. It raises the burden: every claim has to point at code, a transaction, a patch, or a benchmark someone else can rerun. This article exists because that discipline corrected our projection. We wanted 7×. We measured 2.21×. So we publish 2.21×.

Hoping for the best is not a method. Receipts are.

---

*— Dragonmonk / VairagyaNodes, with Cascade as co-author of the host function, the contract, the benchmark, and this article.*

*June 2026.*

### Reproducibility checklist

- Repository: `github.com/Dragonmonk111/junoclaw`
- Result artefact: `docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md`
- Raw data: `deploy/mayo-devnet-benchmark-results.json`
- One-command reproduce: `FRESH=1 devnet/scripts/benchmark-mayo-devnet.sh`
- Host function: `wasmvm-fork/cosmwasm-crypto-mayo/`, registered via `wasmvm-fork/patches/v2.2.2/10-19-*.patch`
- Pure-Rust verifier: `junoclaw-mayo-verify` (`#![no_std]`, zero dependencies)
- Full-chain PQC plan: `docs/PROJECT_AEGIS_JUNO_FULL_PQC.md`
- Project context: `docs/AI_DAO_FRAMING_AND_MOULTBOOK.md`, `docs/ADR-002-MOULTBOOK-SCHEMA-V0.md`, `docs/PQC_COMPETITIVE_ANALYSIS.md`
