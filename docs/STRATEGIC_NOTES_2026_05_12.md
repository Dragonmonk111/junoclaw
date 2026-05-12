# Strategic Notes — 2026-05-12

*Synthesis after Jake's PR-1202 reply ("Comment on the PR!" + Junoswap two-task split), Sunny Aggarwal's "Cosmos Labs ghost town" tweet, and questions on JunoClaw / JunoSwap entity separation, the budding system needing Moultbook, and the Juno-as-L0 thesis. Author: Cascade, on behalf of VairagyaNodes.*

---

## 1. Jake's Junoswap split — what we contribute, where, and how

Jake's framing (Telegram, 2026-05-12):

> Re: JunoSwap there are two tasks:
> * Get the legacy UI working well enough people can withdraw assets
> * Make a new DEX (IMO probably an Astroport fork is quickest / safest)

These are two **orthogonal** tasks with very different risk profiles. Treat them separately.

### 1.1 Task A — legacy UI triage (emergency, days-scale)

**Goal.** People with stuck LP positions can withdraw. Nothing more.

**Probable shape of work:**
- The Junoswap UI repo (legacy) is likely a Next.js / React app pointing at deprecated cosmwasm contract addresses on juno-1. The "broken" state is probably one of: (a) RPC endpoint dead, (b) signing-client API drift after wasmd / cosmjs upgrades, (c) chain-registry mismatch on Keplr.
- Fix is almost always *config + dependency bumps*, not contract changes. The contracts are immutable on-chain; only the front-end is broken.

**Our contribution path:**
1. Fork the legacy UI repo, identify the breakage class, ship a single-purpose "Withdraw" page that bypasses pool/swap routes entirely.
2. Use the existing JunoClaw MCP toolkit (we already have `query_contract`, `execute_contract` for cosmwasm) as the wallet-side bridge if the Keplr signing path is the breakage.
3. Estimated 1–2 working days for a minimal withdraw-only page.

**Decision criterion.** Only commit time to this if Jake explicitly hands it to us — it's emergency-room work, low strategic upside but high gratitude-coefficient with the Juno community. Worth doing if the agent-frontend angle (point 1.3 below) wants community goodwill before launch.

### 1.2 Task B — new DEX as Astroport fork (long-term, weeks-scale)

**Why Astroport is the right base.**
- Battle-tested on Terra → Neutron → Injective. Largest cosmwasm DEX in production.
- AMM + concentrated liquidity + IBC-aware. Already ICS-20 / wasmd-compatible.
- Apache-2 / GPL — forkable.
- Code quality is high (review-friendly), comparable maturity to Uniswap v3 in EVM-land.

**Quickest / safest framing is correct, but with a caveat:** an Astroport fork by Jake (vibecoded by Juno AI agent) is conceptually a different deliverable from an *agent-built / agent-maintained* DEX. The latter is the frontier work, the former is the safe-default work. Both can ship; both can co-exist.

**Our angle:**
- We **don't fork Astroport ourselves**. That's Jake's lane and the Juno AI agent's natural lane (it's the kind of structured work agents do well: clone, port, replace chain-id constants, run integration tests until green).
- We **provide the agent-substrate**: every DEX state-change (LP add, swap, fee distribution) becomes a Moultbook entry → permanent on-chain audit trail. The DEX gets a "verifiable history" layer for free, citable from any other contract.
- We **provide the integration layer**: the JunoClaw MCP toolkit gains DEX query/execute primitives so any AI agent (Cascade, Juno AI, downstream) can interact with the new DEX as a first-class operation.

**Concrete deliverable when this fork lands.** A `junoclaw/contracts/dex-mirror/` contract that subscribes to DEX events and emits Moultbook entries with the right citation discipline. Estimated 2–3 days once Astroport-fork commits exist to read against.

### 1.3 Task C (optional) — agent-built frontend

If Jake wants the front-end to also be agent-built (consistent with the verifiable-agent thesis Juno is already proving with PR #1202), we offer to scaffold the React/wagmi/cosmjs UI from a JunoClaw template. JunoClaw already has 9 DAO templates with a 5-step wizard pattern — extending that pattern to a DEX UI is a natural reuse.

**This is offer-only.** Don't push it; let Jake or Juno AI lead. We're a contributor, not an owner.

---

## 2. JunoClaw vs JunoSwap — separate entities, shared substrate

**Short answer: separate code, separate contracts, but cross-citing on Moultbook.**

| Concern | JunoClaw | JunoSwap (Astroport fork) |
|---|---|---|
| Domain | Agent-driven DAO infrastructure, governance scaffolding, MCP toolkit, DENS identity, Moultbook record substrate | AMM, swaps, LPs, IBC routing |
| Code repo | Dragonmonk111/junoclaw | (CosmosContracts/junoswap-v2 or equivalent) |
| Contract addresses | Independent code IDs; no shared admin | Independent; no shared admin |
| Governance | DAO-DAO + DENS sovereignty | Standard Cosmos gov + per-pool config |
| Cross-pollination | (a) JunoClaw MCP toolkit gains DEX primitives, (b) JunoSwap state changes anchor as Moultbook entries, (c) JunoClaw DAO templates can spawn DEX-aware DAOs (treasury rebalancers, LP-managed funds) | Consumes JunoClaw's identity / Moultbook layers; doesn't depend on them for core function |
| Failure isolation | A bug in JunoSwap doesn't brick JunoClaw governance, and vice-versa | Same |

**Why not unify?** Unified-entity systems calcify. Modular systems compose. Cosmos's whole thesis is that sovereignty + composability beats monolithic L1s. Mirror that internally: each module is its own audit surface, its own upgrade rhythm, its own failure domain.

**Why "shared substrate" matters.** The ground floor — DENS identity, Moultbook on-chain memory, the BN254 precompile (Track B), the verifiable-agent CI pattern — is what every higher-level module (JunoClaw, JunoSwap-fork, future modules like a name service, oracle layer, prediction markets) consumes. That ground floor is what we are building. JunoSwap is a *user* of that floor, not a peer of it.

---

## 3. JunoClaw budding system → Moultbook back-end

The "budding" pattern in JunoClaw — every spawned child DAO from a template, every cloned agent-company, every forked governance rule-set — is currently event-emit-only. The events go into Tendermint's tx log and get indexed by chain-explorer infrastructure, but there's no **canonical, queryable, citable lineage record on-chain**.

This is exactly the gap Moultbook fills.

### 3.1 What "budding" produces today

- A `DaoTemplateGallery` deploy → spawns a new agent-company contract with members, weights, voting params, WAVS task list.
- The deploy emits `wasm-instantiate` events with code_id, label, sender.
- After the block lands, the lineage is reconstructible from explorers but not natively queryable.

### 3.2 What Moultbook gives the budding system

For every bud (DAO spawn, agent clone, template fork):

1. **Anchor entry** on Moultbook with:
   - `commitment` = sha256 of (parent template hash || child config hash || sender || posted_at_nanos)
   - `attestation_ref` = `Bridge { source_chain: "juno-1", tx_hash: "<spawn tx>" }` (or `ZkProof` if/when we add a verifier)
   - `refs` = [parent template's anchor entry id]
   - `visibility` = `Public` for community-template buds, `Group(...)` for private-DAO buds
2. **Forward citations** form naturally: every action the bud takes (proposals, votes, treasury moves) can cite the bud's anchor entry, building an auditable history.
3. **Lineage queries** become first-class: `query ListByRef(parent_anchor_id)` returns every bud that ever forked from a given template. This is the foundation for **template reputation** (which template has spawned 1000 successful DAOs vs 3 abandoned ones?) and **agent reputation** (which agent-company instance has the cleanest action history?).

### 3.3 Concrete integration plan

**Phase 1 — instrument the spawn site.** Modify `DaoTemplateGallery`'s deploy flow (front-end + agent-company contract's `instantiate`) to emit a Moultbook `Post` immediately after the child contract instantiates. The Moultbook entry's commitment is computed from the child's instantiate-msg JSON canonicalisation. ~1 day of work once Moultbook is on devnet.

**Phase 2 — instrument the action-taking sites.** Every `ExecuteMsg::Vote`, `ExecuteMsg::Propose`, `ExecuteMsg::DistributePayment` on agent-company gains a corresponding Moultbook `Post` citing the bud's anchor. ~3 days of integration work, partially automatable through codegen against the agent-company schema.

**Phase 3 — expose the lineage UI.** A `LineageView` page in the JunoClaw front-end that traverses Moultbook citation graphs. Walkthrough: "this DAO buds from `verifiable_outcome_market` template (entry `moult:abc...`), which buds from `community_vote` template (entry `moult:def...`); 47 other DAOs share this lineage; here's the action histories ranked by Moultbook entry count and longevity."

**Phase 4 — feed reputation back into governance.** A future contract `agent-reputation-v1` reads from Moultbook (`ListByAuthor`, `ListByRef`) and computes per-agent / per-template reputation scores. DAOs can opt in to weight votes by this reputation. This is the "Sovereign AI" loop closing in 4–5 contract hops.

---

## 4. Deterministic audit of JunoClaw — beyond Moultbook v0

We applied the deterministic scrutiny benchmark to `moultbook-v0` and found 1 HIGH (F5: unbounded `Visibility::Group`, fixed today), 1 MEDIUM (SE1: migration-safety for future fields), and 3 LOW findings. **The same benchmark needs to run against every JunoClaw contract.** Priority order based on attack surface and value-flow:

| Contract | Audit priority | Attack surface | Estimated audit time |
|---|---|---|---|
| `agent-company-v6.1` (or current) | **P0** | Direct treasury + reputation-weighted voting + WAVS callbacks | 1 day full pass |
| `task-ledger` | **P0** | Identity attestation entry-point; auth model is the gate | 0.5 day |
| `agent-registry` | **P1** | DENS-style alias claims; squatting / griefing surface | 0.5 day |
| `zk-verifier` (BN254) | **P1** | Already production-grade after the 311/311 baseline; wants a re-audit only when forward-ported to wasmvm v3 | 0.5 day at v3-port time |
| `junoclaw-common` (utility crate) | **P2** | No state, but utility-fn determinism matters | 0.25 day |

**Audit deliverable per contract** mirrors `contracts/moultbook-v0/DETERMINISTIC_AUDIT.md`:
- Gas trace of the hot path with per-step host-call attribution.
- Failure-mode enumeration (storage corruption, dependency migration, gas-out, malformed JSON, schema evolution).
- Storage layout at scale (10K, 100K, 1M entries).
- Determinism proof (no floats, no HashMap iteration, no SystemTime).
- Serde boundary hardening checks.
- Action-item table graded by severity, with fix PRs queued.

**Plan.** Run the audits in parallel with Moultbook devnet deploy + Junoswap legacy-UI triage. Estimated 3–4 days for the full P0–P2 sweep.

**This is also the answer to "what other back-end fixes alongside Moultbook?"** — every contract gets the same scrutiny treatment Moultbook just got, and the action items become PRs against the contracts' source. Most will be small (max-size validation, doc-comments, error-variant additions). A few may surface real bugs at the v0 level we shipped before the discipline existed.

---

## 5. Sunny Aggarwal's "Cosmos Labs ghost town" tweet — narrative implications

Sunny's tweet (12 May 2026, 16:10 UTC):

> Cosmos Labs has turned Cosmos into a ghost town. They came in with the expressed intent of "killing the old Cosmos" but failed to present anything remotely interesting to replace it.
>
> There's so much exciting stuff happening in the world right now. Privacy, Sovereign AI, End of Pax Americana.
>
> The time for cypherpunk crypto is now.
>
> But instead ATOM has pivoted to enterprise sales.
>
> A billion dollar market cap for what is now effectively a consulting dev shop. And that too, one that hasn't even been able to land any deals of note.

**This is a gift.** Sunny is the founder of Osmosis and co-creator of mesh security; his frustration with Cosmos Labs / ATOM-as-consulting-shop is pointing the entire Cosmos community at exactly the gap **Juno is naturally positioned to fill**.

The three pillars Sunny names — **privacy**, **sovereign AI**, **end of Pax Americana** — map directly onto the work that's actively in flight on Juno:

| Sunny's pillar | Juno's substrate | Status |
|---|---|---|
| Privacy | TEEs in WAVS, ZK via BN254 precompile, redaction-with-audit pattern in Moultbook | BN254 prop #374 passed; Moultbook v0 ships this week; WAVS-in-TEE per Jake |
| Sovereign AI | `juno-ai-dev` + Claude Opus 4.7 verifiable-agent CI pattern, agent-company DAOs, Moultbook reputation substrate, JunoClaw MCP | PR #1202 is the existence proof; JunoClaw v0.3 already ships 22-tool MCP across 7 chains |
| End of Pax Americana | Fully-sovereign chain governance (no foundation-controlled multisig), agent-driven dev with no centralised employer | Juno's been here since founding; the agent-DAO direction is the natural next step |

**Narrative move:** All forward-facing JunoClaw articles from this point should explicitly position Juno as the **substrate for Sunny's three pillars**, without naming Cosmos Labs / ATOM directly (don't pick the fight; just be where the energy is going).

The phrase that ties it together: **"Juno is becoming the L0 of agent-sovereign Cosmos."**

---

## 6. The Juno-as-L0 thesis — what it means and where to use it

Recap of where this came up earlier (paraphrased from prior session): "Juno will be a kind of L0 — every other Cosmos chain that wants verifiable agent-driven governance will look back to Juno's reference contracts (BN254 zk-verifier, voting-snapshot, Moultbook record substrate, agent-company templates, DENS identity) and either import them or fork them. Juno isn't competing with ATOM as a settlement layer or with Osmosis as a DEX — it's positioning underneath both as the canonical reference implementation for the agent-DAO pattern."

**Why this framing works:**

1. **"L0" in Cosmos has historically meant Tendermint + Cosmos SDK as the universal substrate.** Juno's contribution is the *application-layer* L0 — the canonical contracts that other chains import as `wasmvm` modules, the canonical CI pattern (`juno-ai-dev` + `Co-Authored-By: Claude`) that other chains adopt for their own AI dev teams, the canonical record substrate (Moultbook) that other chains anchor their own agent histories into.
2. **"L0" sidesteps the throughput / TPS dick-measuring contest.** We don't need to be the fastest chain or the cheapest chain. We need to be the chain whose contracts everyone else imports. Reference implementation → ecosystem leverage → durable position.
3. **"L0" matches what's actually true.** The BN254 precompile is being designed as a CosmWasm precompile (not a Juno-specific feature). The voting-snapshot module is being designed as a generic SDK module (not a Juno-specific module). The Moultbook contract is `cosmwasm-std`-portable and runs on any wasmd chain. Every artifact is exportable from day one. Other chains *will* import them; the question is just timing.

**How to use the framing in articles:**

- **In Medium articles from now on:** introduce the "L0" framing in the opening third of every article that touches Juno's role in the broader ecosystem. Don't repeat the framing — set it up once, let later articles refer back.
- **In governance proposals:** mention it at most once, in the "context" / "why this matters" section. Governance text wants to be sober, not aspirational.
- **In Telegram / public threads:** use the phrase **"Juno is becoming the L0 of agent-sovereign Cosmos"** as the natural follow-up when someone asks what Juno is for in 2026. It's compact, it differentiates, it slots into Sunny's framing without adversarial energy.
- **In the JunoClaw README:** add a one-paragraph "Why Juno?" section that lands the L0 framing as the answer.

**A concrete passage we can drop into the next Medium article:**

> Juno is becoming the L0 of agent-sovereign Cosmos. Not in the throughput-and-fees sense — that lane is crowded. In the *reference-implementation* sense: the chain whose verifiable-agent CI pattern (`juno-ai-dev` + `Co-Authored-By: Claude`) other chains adopt for their own AI dev teams, whose BN254 zk-verifier other chains import as the canonical privacy primitive, whose Moultbook record substrate other chains anchor their own agent histories into, whose DAO templates other chains spawn their own DAOs from. Every artifact is exportable from day one. Other chains will import them; the question is just timing. Sunny Aggarwal's call for **privacy**, **sovereign AI**, and **the end of Pax Americana** maps cleanly onto what Juno is already shipping. We are the substrate.

---

## 7. Sequenced action items

In execution order, with reasonable parallelism flagged:

1. ✅ **Paste the PR-1202 review** as a GitHub comment. *(Jake explicitly asked for this. P0.)*
2. ✅ **Send the updated short DM** to Jake. *(Already sent per the user.)*
3. **Wait for VM `catching_up: false`** — should land within an hour. Then run unjail tx. *(Sequential, but we can work on 4–6 in parallel while waiting.)*
4. **Start Moultbook devnet deploy.** WSL2 Ubuntu is the right environment. Build `cosmwasm-optimizer` image, run `devnet/scripts/build-moultbook.sh`, then deploy to local devnet *or* `uni-7` testnet (uni-7 is faster — public RPC, no Docker warm-up needed).
5. **Begin agent-company-v6.1 deterministic audit** in parallel with (4). Same template as `contracts/moultbook-v0/DETERMINISTIC_AUDIT.md`. Aim for first-pass complete by tomorrow.
6. **Junoswap Astroport-fork mirror contract design.** Sketch `contracts/dex-mirror-v0/` schema and the Moultbook event-anchoring discipline. ~half day; outputs a doc, not contract code yet.
7. **Decide on JunoSwap legacy-UI triage involvement.** Hold until Jake either explicitly hands the task to us or assigns it elsewhere. Don't volunteer prematurely — finishing PR-1202 follow-ups + Moultbook deploy is more leveraged work right now.
8. **Draft the next Medium article** ("Juno is becoming the L0 of agent-sovereign Cosmos") incorporating Sunny's three pillars and the agent-CI thesis. Position to publish after Moultbook v0 lands on devnet so the article has a concrete deployment to point at.

Items 4, 5, 6, 8 can all proceed in parallel while the VM syncs and waits for unjail. Items 1, 2, 3, 7 are gate-points.

---

*Apache-2.0. This file is itself a candidate Moultbook entry once the contract is on devnet — visibility = Public, refs = [PR-1202 review entry, ADR-002 entry, the next Medium article entry once it exists]. Dog-foods the citation discipline by being its first cross-doc user.*
