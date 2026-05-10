# AI DAOs & the Moltbook layer — JunoClaw architecture note

*Status: research / scoping. Not a roadmap commitment. Captures the framing from the May 2026 Twitter Spaces with Jake Hartnell, the netadao creator, and Cybernetics members, and maps it onto the existing JunoClaw stack.*

*Owner: VairagyaNodes. Audience: contributors, reviewers, and Jake's Juno AI team.*

---

## 1. Why this note exists

Two threads from the May Spaces are load-bearing for our next quarter:

1. **AI DAOs are the category.** Jake and the Cybernetics group framed the wave we are inside as "AI DAOs" — DAOs whose participants are autonomous agents, not (only) human members, and whose decisions are produced by verifiable computation rather than by show-of-hands voting. JunoClaw was already heading there; the framing makes it explicit.

2. **Moltbook — a shared substrate where AIs accumulate and exchange knowledge.** Jake described running multiple Claude Opus 4.7 instances on `max` plans concurrently against a Juno AI. The bottleneck is not compute; it is shared, durable, attestable memory. He pointed at "Moltbook" as a working name and at the Howl Social 2023 OSS as a possible substrate.

This note does not pick a winner. It (a) names the design constraints any solution must satisfy on Juno and (b) lists the substrates worth a deeper read.

## 2. JunoClaw, restated as an AI DAO primitive

The nine-contract stack (`task-ledger`, `escrow`, `agent-registry`, `agent-company`, `zk-verifier`, `junoswap-pair`, `builder-grant`, `jclaw-token`, `jclaw-airdrop`) has always been agent-shaped. With the BN254 precompile measuring **1.823× cheaper** verification on the devnet (370,600 → 203,266 gas, see `BN254_BENCHMARK_RESULTS.md`), every agent action becomes economically auditable, not just sampled. That moves us from "DAO that hires agents" to "AI DAO": a body whose every decision is a verifiable agent output.

Concretely, mapped to AI-DAO vocabulary:

| AI DAO concept                  | JunoClaw component (existing)                          |
|---------------------------------|--------------------------------------------------------|
| Agent identity & reputation     | `agent-registry` (soulbound) + `jclaw-token` (trust tree) |
| Task / proposal queue           | `task-ledger` (with adaptive voting in `agent-company`) |
| Verifiable execution receipt    | `zk-verifier` + WAVS bridge (TEE attestation in)        |
| Settlement / payout             | `escrow` + DAO treasury policy                          |
| Slashable behaviour             | `agent-registry` reputation + governance pruning        |
| External value (DEX / oracles)  | `junoswap-pair` + WAVS bridge (Chainlink-shaped)        |
| Onboarding / grants             | `builder-grant` + `jclaw-airdrop`                       |

What is **missing** from "AI DAO" is the shared knowledge layer — the Moltbook.

## 3. The Moltbook problem statement

Independent Opus instances, or any heterogeneous fleet of agents working on a Juno AI, need a substrate that:

1. **Persistent.** Survives any single agent's lifetime; readable years later.
2. **Attestable.** Each entry carries a TEE attestation or Groth16 proof that the writing agent did the work it claims.
3. **Compositional.** New entries can reference and build on prior entries; revocations are first-class.
4. **Cheap to read.** Agents make many more reads than writes; reads should not be SDK-gas heavy.
5. **Permissioned at the edges.** Some entries are public; some are committee-restricted; some are encrypted to a specific agent's public key.
6. **Portable.** Not Juno-specific in its data model — Cosmos chains, Eth L2s, and Sui (Jake's earlier liquidity targets) should all be able to participate.

That last constraint is the reason this is an architecture note and not a contract. The right Moltbook is most likely a **standard** (a schema + an attestation discipline) implemented thinly on Juno first, with adapters elsewhere.

## 4. Substrate candidates

Three classes worth investigating; ranked by how far they have already solved (1)–(6).

### 4a. Howl Social (Juno-native, ~2023 OSS)

- **What it is.** On-chain micro-blogging suite of CosmWasm contracts on Juno mainnet, plus a TS frontend. Public docs at `docs.howl.social`. Names registered via `dens.sh`.
- **Why it is interesting.** Already proves the cost model for "many small immutable posts on Juno"; already integrates with a Juno-native naming layer; same block space and gas model we will use.
- **Unknowns to verify.** Repository licence; contract surface (do entries have anything attestation-shaped?); whether it is still mainnet-live or archival; size of the existing post corpus.
- **What we'd add.** An `attestation_ref` field on each entry pointing at a `zk-verifier` proof or a TEE attestation hash; an explicit per-entry visibility mode.
- **Action.** Locate the repo (Jake or Dimi can point); read contracts; produce a 2-page diff against (1)–(6); decide fork-vs-greenfield.

### 4b. Generic Cosmos message-board patterns

A surprising amount of recent Cosmos work — DAODAO posts, public IBC bulletin contracts, even DENS itself — already implements (1) and (3). They tend to fail (2) because no one connected them to a verifier. The cost of bolting `zk-verifier` onto an existing post contract is small.

- **Action.** Survey 3–4 such patterns (DAODAO post-extension, Stargaze names, juno-name-service if alive); pick whichever has the cleanest schema as the starting template.

### 4c. Off-chain shared memory (DA-layer / object-store)

For the **bulk** of agent knowledge — long contexts, vector embeddings, fine-tune deltas — the chain is the wrong place to store bytes. The right place is a DA layer (Celestia is the obvious one; see §6 on tiablob) or an attested object store. The chain holds **commitments** to those blobs and the verifier checks the commitment-proofs.

- **Action.** Sketch a hybrid: chain stores `(blob_hash, attestation, visibility, refs[])`; off-chain layer stores the blob; `zk-verifier` proves "this output was produced by reading these blobs honestly."

The right Moltbook is almost certainly (4a) or (4b) for the **index** + (4c) for the **bulk**, glued by `zk-verifier`.

## 5. Where this connects to Jake's other directional notes

- **ETH alignment / Chainlink readiness.** The WAVS bridge already speaks oracle-shaped messages. Document the bridge's adapter surface explicitly so a Chainlink-style relayer is a one-page integration, not a port. (Open task; not in scope for this note.)
- **Juno AI / multi-Opus stack.** The Moltbook is the substrate Jake is missing. Even a thin v0 — "Howl-Social-fork with attestation_ref + visibility" — gives the multi-Opus fleet a place to write, read, and cite each other.
- **netadao framing.** netadao's category position is broader than ours; treat it as a peer not a target. If they ship a Moltbook-equivalent first, fork their schema.

## 6. Mesh Security / tiablob constraint (forward-link)

A separate note, `MESH_TIABLOB_CONSTRAINTS.md`, captures the audit-pending posture for any Mesh-touching work. The Moltbook off-chain bulk store (§4c) intersects with that work because Celestia + tiablob is the obvious DA layer for it. **Do not** make Moltbook v0 depend on un-audited Mesh; design the off-chain layer so the DA backend is pluggable (Celestia, EigenDA, plain S3 with Merkle commitments).

## 7. Concrete next actions

In order of cost-to-value:

- [ ] **Read pass on Howl Social code** (1 session). Confirm licence and contract list; produce a 2-page §4a-style writeup.
- [ ] **Schema sketch for Moltbook v0** (1 session). 30-line CosmWasm `entry` struct; one `post`, one `read`, one `redact`. No backend bound yet.
- [ ] **Attestation-ref discipline** (1 session). Extend `zk-verifier` query API with a `verify_post(entry_hash, attestation)` helper or document why the existing API is sufficient.
- [ ] **Bridge ↔ Chainlink one-pager**. Map the WAVS bridge's submit/relay surface to a Chainlink-style adapter contract.
- [ ] **Outreach.** Send Jake a short note pointing at this file and asking which Moltbook framing matches what he has in mind. Avoid asking him to design; offer two concrete sketches and let him pick.

## 8. Discipline reminders

The same posture that the Medium article (`MEDIUM_ARTICLE_THE_VERIFIABLE_AGENT.md`) commits to applies here:

- Ship small. The Moltbook v0 should be ≤ 200 LoC of CosmWasm, ≤ 200 LoC of Rust attestation glue, and zero new chain modules.
- Every claim reproduces. Anything we measure (e.g. read-cost, write-cost, verify-cost) goes into a `MOLTBOOK_BENCHMARK_RESULTS.md` artefact alongside the BN254 results.
- Every dependency is a deliberate choice. If we fork Howl Social, the fork commit message names the upstream, the licence, and what we changed.
- Names are correct. If Jake or netadao supply the schema, the contract header credits them and the doc cites them.

---

*Drafted 10 May 2026. To be revised after the Howl Social read pass.*
