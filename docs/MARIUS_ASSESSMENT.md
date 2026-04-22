# Marius Assessment — Juno Module Surface Relevant to JunoClaw

> *"Cosmos SDK is good. On CosmWasm — mediocre, but probably decent with some improvements to the modules on Juno."* — Marius, last core contracts dev on Juno (before Cosmos Labs + Dimi's security-patch cadence)
>
> This document acts on that critique. It surveys every module in Juno `v29.0.0`, flags relevance to JunoClaw's agent/task/attestation loop, and proposes specific module-level hardening candidates. Analysis only; no code changes in this commit.

**Date:** 2026-04-22
**Target:** `CosmosContracts/juno` @ `v29.0.0` (the tag JunoClaw's devnet builds against — see `@devnet/Dockerfile:10-12`)
**Ground truth:** `app/keepers/keepers.go` at that tag, read 2026-04-22
**Scope:** Analysis and prioritisation. Any module PR-shaped work that follows this document lives in its own commits, its own tests, and — where appropriate — its own governance proposal.

---

## 1. Inventory — every module Juno v29 carries

Grouped by origin. Each row cites the import path from `app/keepers/keepers.go` so a reviewer can verify every line against the source.

### 1.1 Juno-specific modules (`github.com/CosmosContracts/juno/v29/x/*`)

| Module | Purpose | Grounded import |
|--------|---------|-----------------|
| `x/burn` | Burner module account; sinks tokens routed to a dedicated address. | `junoburn "github.com/CosmosContracts/juno/v29/x/burn"` |
| `x/clock` | Recurring contract execution — contracts that register with `x/clock` are invoked every `BeginBlock` / `EndBlock`. | `clockkeeper, clocktypes` |
| `x/cw-hooks` | CosmWasm contract hooks into staking / distribution lifecycle — contracts receive callbacks on delegations, rewards, etc. | `cwhookskeeper, cwhookstypes` |
| `x/drip` | Token-distribution drip — schedules incremental payouts. | `dripkeeper, driptypes` |
| `x/feepay` | Contracts can sponsor transaction fees for their users — the contract pays gas, the user signs. | `feepaykeeper, feepaytypes` |
| `x/feeshare` | Revenue share between chain + contract devs: a percentage of gas collected on a contract's txs routes back to the contract's registered recipient. | `feesharekeeper, feesharetypes` |
| `x/globalfee` | Chain-wide minimum-fee floor; overrides per-denom min-gas-prices. | `globalfeekeeper, globalfeetypes` |
| `x/mint` | Juno's custom inflation schedule (decaying → target bond ratio). | `mintkeeper, minttypes` |
| `x/tokenfactory` | Native-denom factory (Osmosis-derived); creator pays a bond, gets a `factory/<creator>/<subdenom>` namespace. Capabilities extended on Juno to include `EnableBurnFrom`, `EnableForceTransfer`, `EnableSetMetadata`. | `tokenfactorykeeper, tokenfactorytypes` |
| `x/wrappers/gov` | Wrapper around the upstream `x/gov` keeper that modifies behaviour **without forking** — the lever Juno uses to carry custom governance logic with minimal upstream drift. | `wrappedgovkeeper "github.com/CosmosContracts/juno/v29/x/wrappers/gov/keeper"` |

### 1.2 `wasmd` + bindings

| Module | Purpose | Grounded import |
|--------|---------|-----------------|
| `x/wasm` | Standard `CosmWasm/wasmd` VM module. Capabilities at Juno v29: wasmd built-ins + `"token_factory"` (from `wasmCapabilities` in keepers.go). | `github.com/CosmWasm/wasmd/x/wasm` |
| `wasmbindings` | **Custom** wasm bindings layer that exposes `x/tokenfactory` messages to CosmWasm contracts — lets a contract create/mint/burn factory denoms from inside Wasm. | `bindings "github.com/CosmosContracts/juno/v29/wasmbindings"` |

### 1.3 IBC + IBC apps

| Module | Purpose |
|--------|---------|
| `ibc-core` (ibc-go v8) | IBC clients / connections / channels. |
| `ibc-transfer` (ICS-20) | Fungible-token transfer. |
| `ibc-fee` (ICS-29) | Incentivised relayers. |
| `ica-controller` + `ica-host` (ICS-27) | Interchain accounts. |
| `icq` (async ICQ from ibc-apps) | Interchain queries — one chain reads another's state. |
| `ibc-hooks` (from ibc-apps) | IBC packets can invoke a Wasm contract on arrival via a memo field. |
| `packet-forward-middleware` | Multi-hop IBC routing. |

### 1.4 Standard Cosmos SDK (reference only — none are Juno forks)

`auth`, `bank`, `staking`, `slashing`, `distribution`, `gov` (wrapped — see §1.1), `crisis`, `upgrade`, `params`, `consensus`, `evidence`, `feegrant`, `nft`, `authz`, `capability`.

### 1.5 Upgrade handlers present at v29.0.0

From `app/app.go`: `v28.Upgrade`, `v29.Upgrade` — meaning v29 ships with its own migration and can replay from the v28 snapshot. Any BN254 upgrade would be a `v30` handler in the same shape.

---

## 2. Relevance to JunoClaw — where each module sits in our loop

**Current posture.** Every JunoClaw contract is pure `cosmwasm-std` + `cw-storage-plus` + `cw2`. Verified in `@contracts/task-ledger/Cargo.toml:10-18`, `@contracts/agent-company/Cargo.toml:10-19`, and every sibling. **No JunoClaw contract calls a Juno-specific module today.** That is not a design virtue; it is a missed opportunity. The table below quantifies it.

| Module | JunoClaw today | Could plausibly use | Priority |
|--------|----------------|---------------------|----------|
| `x/wasm` | **Yes — everything runs here** | — | — |
| `wasmbindings` (tokenfactory) | No | **Yes** — issuing `factory/agent-company/claw-N` attestation-badge denoms per successful task would give reputation an on-chain, transferable-or-soulbound representation. F4 (`junoswap-pair` rogue-denom acceptance) already defends against this surface; using it offensively is the next step. | **Medium** — worth a design memo; not urgent. |
| `x/clock` | No | **Yes, high** — `TimeAfter` and `BlockHeightAtLeast` Tier-1.5 constraints today rely on off-chain operators to trigger re-evaluation. Registering `task-ledger` or `agent-company` with `x/clock` moves expiry and timeout handling **on-chain and deterministic**, eliminating a whole class of operator-timing subtleties. | **High** — clean integration, real operational win. |
| `x/cw-hooks` | No | **Yes** — `agent-registry` could receive delegation/reward events to bind agent reputation to stake-weighted signals. Probably post-mainnet; premature today. | Low. |
| `x/feeshare` | No | **Maybe** — if JunoClaw ever holds a foundation-treasury contract, routing a fee share to it turns every tx on `agent-company` into treasury funding. Aligns with #373's post-pass funding narrative. | Low-Medium. |
| `x/feepay` | No | **Yes** — lets agents sponsor user gas on attestation submissions. Removes a UX barrier for downstream agent integrations (the submitter doesn't need JUNO). | Medium, UX-driven. |
| `x/globalfee` | Indirect (pays fees) | No integration lever. | — |
| `x/mint` | Indirect (inflation) | No integration lever. | — |
| `x/drip` | No | **Maybe** — alternative to `builder-grant`'s bespoke distribution logic. Probably not worth the migration cost; our implementation is audited via regression tests. | Low. |
| `x/burn` | No | **Maybe** — failed tasks could burn a fraction of the escrow as anti-spam. Currently escrow just returns. | Low-Medium (design decision, not a module bug). |
| `x/wrappers/gov` | No (we run our own DAO in `agent-company`) | — | — (we intentionally do not depend on chain-level governance for in-system decisions). |
| `ibc-hooks` | No | **Yes** — an IBC transfer arriving with a well-formed memo could atomically trigger an `agent-company` task. Would open JunoClaw to cross-chain task submission. | Medium-High, post-mainnet. |
| `icq` | No | **Yes** — Tier-2 constraints (when we write them) could query foreign-chain state; e.g. *"balance on Osmosis is at least X before this task completes."* Noted in `@docs/NEUTRON_FORK_STRATEGY.md:105` as a gap. | Medium, design work. |
| `ica-controller` | No | **Yes** — an agent running on Juno could hold an ICA on Osmosis or Stargaze. Opens cross-chain agentic work. | Medium, post-mainnet. |
| `ibc-fee` (ICS-29) | No | No current use. | — |
| `authz` | No | **Yes** — `agent-company` could accept `authz`-granted permissions to execute on a user's behalf, letting users delegate narrow actions to agents without handing over keys. | High — unlocks agent-as-signer safely. |

---

## 3. Proposed module-level improvements — the "Marius hardening" candidates

These are where Marius's critique *("mediocre, but probably decent with some improvements")* has the most purchase. Each candidate includes a one-line statement, a rough surface-area estimate, and whether the right home is upstream `wasmd`, Juno-specific, or a contract-level pattern.

### 3.1 `x/wasm` precompile gas-metering precision *(highest priority, BN254-adjacent)*

**Problem.** `wasmGasRegister` translates host-function gas to SDK gas via a fixed multiplier. BN254's 34 000 gas/pairing (pegged to EIP-1108) gets rounded by that multiplier, so the effective on-chain charge drifts from the advertised number. Today the BN254 crate tests pass ground-truth EIP vectors, but the SDK-gas number a user sees may round up or down by ≤ 5 %.
**Fix.** Allow precompile-backed host functions to declare SDK gas directly, bypassing the multiplier, with a documented conversion at registration time. Either an upstream `wasmd` PR or a Juno-level wasm-option override via `wasmkeeper.Option`.
**Surface:** ~80 LoC in `wasmd` or ~40 LoC in Juno-local wasm options.
**Owner:** Upstream preferred; local fallback acceptable.
**Priority:** **High.** Directly affects the 187 000 headline number in prop #374 — without this, the post-upgrade measurement will be *approximately* 187 000, not exactly.

### 3.2 Sub-message error-propagation clarity

**Problem.** Our `agent-company` optional zk-sidecar already had to manually name `IncompleteZkProofBundle` and `ZkVerifierNotConfigured` because `wasmd` bubbles sub-message failures as an opaque `dispatch: submessages: …` prefix. Any contract dispatching sub-messages re-learns this pattern.
**Fix.** Upstream `wasmd` could emit a structured sub-message-failure event distinguishing *caller-side* (pre-flight checks) from *callee-side* (sub-call revert) errors, with the callee's named error exposed separately.
**Surface:** ~30 LoC event-emission change + docs.
**Owner:** Upstream `wasmd`.
**Priority:** Medium — quality-of-life, not correctness.

### 3.3 `x/clock`-as-deadline for Tier-1.5 constraints

**Problem.** `TimeAfter { .. }` and `BlockHeightAtLeast { .. }` are **checked at the moment a caller attempts `CompleteTask`**, not the moment the deadline is reached. If nobody polls, a task sits `Running` forever. An off-chain operator currently fills the gap, which is exactly the kind of centralisation we're supposed to be removing.
**Fix.** Register `task-ledger` (or a small `task-watcher` helper contract) with `x/clock` so expired tasks auto-transition to `Failed` at the block their deadline hits. Atomic revert still applies; the only change is who fires the transition.
**Surface:** ~40 LoC in a new `task-watcher` contract, ~5 LoC change to `task-ledger` to accept clock-triggered transitions.
**Owner:** JunoClaw (contract-level, not module-level), but depends on `x/clock` being available — which it is at v29.
**Priority:** **High** for operational soundness; can ship independently of BN254.

### 3.4 `x/tokenfactory` denom-metadata validation — push F4 upstream

**Problem.** v6.1 fix F4 tightened `junoswap-pair` to reject unexpected denoms. The attack surface (rogue tokenfactory mints showing up in AMM balances and silently orphaning) is a **chain-wide** pattern, not just ours. Every CosmWasm AMM / escrow / routing contract on Juno has the same shape.
**Fix.** Upstream `x/tokenfactory` could optionally enforce a denom-whitelist flag per-contract-keeper, or emit a structured event on every mint that downstream contracts can subscribe to via `x/cw-hooks`.
**Surface:** ~100 LoC Juno-local (since tokenfactory is Juno-forked).
**Owner:** Juno core — would require engagement with Cosmos Labs.
**Priority:** Medium — our F4 covers us in isolation; the benefit here is ecosystem-wide.

### 3.5 `authz`-for-agent-company: typed msg filters

**Problem.** To let users delegate narrow actions to an agent (e.g. *"agent can submit attestations on my behalf but cannot transfer my funds"*), `x/authz` grants are per-msg-type, not per-msg-body. That's often too coarse. An attacker who obtains an `authz` grant for `MsgExecuteContract` on `agent-company` can call *any* of the contract's execute variants.
**Fix.** A contract-level `GrantedAuthz { allowed_variants: Vec<ExecuteMsg::Discriminant> }` map, checked at the top of `execute`. Not really a module fix; a contract-level pattern that complements `x/authz`.
**Surface:** ~50 LoC in `agent-company`; ~10 LoC of tests.
**Owner:** JunoClaw (contract-level).
**Priority:** Medium — unlocks the *"agent-as-bonded-signer"* use case cleanly.

### 3.6 IBC-hooks-to-task-ledger bridge

**Problem.** Cross-chain task submission today requires the submitter to have a Juno wallet and gas. IBC-hooks would let an ICS-20 transfer from Osmosis (say) with a structured memo atomically create a JunoClaw task.
**Fix.** A small `ibc-task-bridge` contract that implements the IBC-hooks interface and dispatches a `task-ledger::SubmitTask` on valid packet receipt.
**Surface:** ~200 LoC new contract.
**Owner:** JunoClaw (contract-level).
**Priority:** Medium, post-mainnet.

---

## 4. What this means for prop #374

**Nothing from this document changes the BN254 proposal.** The one candidate that touches BN254 directly (§3.1 — gas-metering precision) is a *separate* upstream concern that can ride the same wasmvm release or go in its own PR. Prop #374 stays narrow: *"signal BN254 direction."*

**What this document *does* do is pre-empt a fair forum objection.** When a reviewer asks *"is BN254 the most important change JunoClaw would make to the wasm surface?"*, we can answer *"no — here are five other candidates on our list, ranked; BN254 is the one whose case is most complete today. The others are written down, open to review, and not this proposal."*

A one-line reference in `@docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md` would suffice: *"Broader module-level improvements (sub-message error events, `x/clock` integration for task deadlines, denom-whitelist events for tokenfactory) are surveyed in `docs/MARIUS_ASSESSMENT.md` and will be pursued in separate governance items."*

---

## 5. What is **not** in this document

- **No code changes.** This is analysis. Any implementation work follows in separate commits with tests.
- **No PRs opened.** Each §3 candidate that advances to implementation will open its own upstream issue/PR or Juno issue first, for comment.
- **No commitment to timeline.** These are candidates, ranked. JunoClaw's working capacity decides cadence.
- **No attribution claim against Marius.** Marius's Telegram note motivates the framing; the specific prioritisation above is written from the JunoClaw repo's own state (v29 keepers.go, our contract surface, the existing hardening pass log). Any disagreement with the ranking lands on this author (VairagyaNodes / Cascade), not Marius.

---

## Appendix — how this document was produced

1. Read `@devnet/Dockerfile:10-12` to pin the Juno tag (`v29.0.0`).
2. Fetched `https://raw.githubusercontent.com/CosmosContracts/juno/v29.0.0/app/app.go` and `app/keepers/keepers.go` to enumerate modules from ground truth.
3. Grep'd `@contracts/*/Cargo.toml` to establish JunoClaw's current module dependencies (answer: none Juno-specific).
4. Cross-referenced `@docs/NEUTRON_FORK_STRATEGY.md:103-109` for prior-art on Juno-module gaps.
5. Ranked against JunoClaw's contract surface and the Tier-1-slim constraint vocabulary (`@docs/MEDIUM_ARTICLE_CONSTRAINTS.md`).

Every reviewer can reproduce this survey with the same five inputs. Analysis is fallible; the grounding is not.

*— VairagyaNodes / Cascade, 2026-04-22*
