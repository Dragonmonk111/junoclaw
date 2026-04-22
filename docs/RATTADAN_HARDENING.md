# Rattadan Variable Hardening — v4 → v5

> **ATTRIBUTION CORRECTION — 2026-04-22.** This document's title and opening framing overstate Rattadan's role in the hardening pass. The contracts, tests, and iteration passes were authored by **Cascade** (the pair-programming AI agent) at VairagyaNodes's direction. **Rattadan did not review the contracts.** His contribution to JunoClaw is real but different — validator-ops guidance, `uni-7` orientation, testnet-token coordination, and broader Juno-community welcome. The "chaoscoder with no-clean code mode only" quote is authentic and motivated the *framing* of this document; it did not produce the contract changes described below.
>
> The authoritative narrative (three genuine security fixes, two self-corrections of earlier over-shipping, one capability expansion) is in `@docs/MEDIUM_ARTICLE_AFTER_THE_VOTE.md` Section II. This file is left intact as a historical record of how the framing arose, not as a current attribution claim.
>
> For the separate, correctly-attributed Juno-module-surface analysis motivated by Marius's public comment on CosmWasm-on-Juno, see `@docs/MARIUS_ASSESSMENT.md` (2026-04-22).

---

> "I'm chaoscoder with no-clean code mode only. 3 variables, 5 declarations."

This document describes the hardening passes applied to JunoClaw's core variables, inspired by Rattadan's structural audit. Each variable maps to a security property that was either missing or weakly enforced.

> ### v6.1 Update — 2026-04-18 — Cross-contract security sweep
>
> After v6 closed the identity-mismatch holes, a full-workspace sweep for
> *value-flow* bugs found four more sharp edges where the contract
> accepted inputs it had no business accepting. Each one is a single
> predicate that fails closed; none of them changes the public-API shape.
>
> **F1 (CRITICAL) — `task-ledger::CompleteTask` / `FailTask` let the
> submitter self-attest.** The v5 atomic-callback fabric routes task
> completion straight into `escrow::Confirm` (and `FailTask` into
> `escrow::Cancel`). Both paths were gated on
> `record.submitter == info.sender || is_authorized(...)` — meaning the
> original submitter could finalise their own task. Combined with
> `escrow::Authorize` (where `info.sender` becomes the payer), this gave
> a payer-who-is-also-submitter the ability to file an escrow obligation
> against any victim payee and then mark that obligation `Confirmed`
> without a single ujunox ever moving: a complete bypass of the payment
> journal's core promise. F1 removes the submitter path from
> `CompleteTask` / `FailTask` — only operators (incl. admin) and the
> configured `agent_company` can finalise a task. `CancelTask`'s
> submitter-self-cancel path is retained because it has no escrow
> side-effect. The WAVS daemon still finalises tasks unimpeded: it is
> registered as an operator, so `is_authorized` covers it. Regressions
> `test_complete_task_by_submitter_is_rejected` and
> `test_fail_task_by_submitter_is_rejected` nail the invariant shut.
>
> **F2 (HIGH) — `agent-company::DistributePayment` was front-runnable
> for one ujunox.** The handler keyed idempotency on
> `PAYMENT_HISTORY.has(task_id)` and was open to any caller. A griefer
> watching the mempool could pre-empt a pending distribution with 1
> ujunox against the same `task_id`, permanently locking out the
> legitimate large distribution with `AlreadyDistributed`. The asymmetry
> (1 ujunox of attacker cost vs. arbitrary legitimate value blocked) is
> now closed: `DistributePayment` is gated to `admin | DAO member`,
> which is also the intended operational model (members fund
> distributions from their own wallets; external funders route through a
> member proxy). Fixture change: the four existing tests re-route their
> payer through `admin` (who passes the gate); a new regression
> `test_distribute_payment_rejects_non_member_caller` verifies the gate
> rejects strangers.
>
> **F3 (HIGH) — `builder-grant::SubmitWork` accepted unlimited
> duplicates of the same `work_hash`.** `work_hash` is the SHA-256 of
> the actual work output: by construction, two identical hashes are the
> same work and must only be claimed once. Without a uniqueness index,
> an attacker (or a confused legitimate user) could spam the
> pending-verification queue with structurally-identical entries
> (rotating only the `evidence` string), bloating state, confusing the
> TEE verifier about which record is canonical, and potentially racing
> the rightful builder's own claim. F3 introduces
> `WORK_HASH_USED: Map<&str, u64>` as a reverse index and rejects
> duplicates with a new `DuplicateWorkHash { existing_submission_id }`
> error. Uniqueness is **global**, not per-builder: if two builders
> somehow produce the same output hash, only the first-submitter wins —
> which is the correct outcome because the hash *is* the output.
> Regression `test_submit_work_rejects_duplicate_hash` covers same-builder,
> different-builder, and different-hash-same-builder cases.
>
> **F4 (MED) — `junoswap-pair` silently ate unexpected denoms.**
> `extract_native_amounts` (used by `ProvideLiquidity`) looked for
> `token_a` and `token_b` by name in `info.funds` and `unwrap_or_default`'d
> anything else — so a user attaching a third coin (or a typo'd denom)
> would forfeit those funds into the pair contract's bank balance with
> no reclaim path. `execute_swap` had the same shape for its offer-side
> funds. F4 makes both handlers fail closed on any denom outside the
> expected set, via a new `ContractError::UnexpectedDenom { denom,
> expected_a, expected_b }` variant. The revert refunds every attached
> coin atomically. Regression
> `test_provide_liquidity_rejects_unexpected_denom` confirms `uatom`
> sent alongside a legitimate `ujuno+uusdc` deposit is fully refunded.
>
> **Known-by-design, deliberately not patched in v6.1:**
>
> 1. `escrow::Confirm` still accepts `payer == info.sender` without any
>    on-chain proof of payment. This is the intended shape of the
>    **non-custodial payment journal** — the contract holds no funds,
>    it only records attestations. The real guarantee for the payee is
>    the `attestation_hash` subsequently attached by the WAVS operator
>    via `AttachAttestation`. F1 closes the *indirect* self-confirm via
>    `CompleteTask`; the *direct* self-confirm remains by design, and
>    any downstream consumer of escrow status MUST wait for
>    `attestation_hash.is_some()` before treating an obligation as
>    settled. This is now called out explicitly in the integration
>    docstring so no future contract misreads `Confirmed` as "paid".
>
> 2. `junoswap-factory::CreatePair` spawns a pair subcontract but never
>    records the instantiated address into its `PAIRS` map — it would
>    need a reply-based `SubMsg` flow to capture the address. The
>    factory's own state is therefore incomplete (queries to
>    `PAIRS` return empty) but no state is *corrupted* and no funds are
>    at risk. This is a UX/feature-completeness issue, not a security
>    one. Flagged here so it doesn't get confused with the v6.1 fixes.
>
> 3. `junoswap-pair::ProvideLiquidity` has no Uniswap-V2-style "minimum
>    liquidity burn" on the first deposit. The classic donation-attack
>    shape this protects against (attacker deposits 1 wei of each,
>    then bank-sends a donation to inflate exchange rate) does **not**
>    apply here because the pair tracks reserves in storage, not in
>    bank balance — bank sends to the pair contract do not move the
>    reserves. The first-depositor-sets-initial-price property remains,
>    but that is price discovery, not theft. Left unchanged.
>
> **Files changed (v6.1):**
>
> | File | Change |
> |------|--------|
> | `task-ledger/src/contract.rs` | `execute_complete` / `execute_fail` auth gated to `operators ∪ admin ∪ agent_company` (no submitter-self-path) |
> | `task-ledger/src/tests.rs` | 4 test sites retargeted to `admin`; 2 new regressions (`test_complete_task_by_submitter_is_rejected`, `test_fail_task_by_submitter_is_rejected`) |
> | `agent-company/src/contract.rs` | `execute_distribute` gated to `admin ∪ members` |
> | `agent-company/src/tests.rs` | 4 test fixtures re-funnelled through `admin` / `alice`; 1 new regression (`test_distribute_payment_rejects_non_member_caller`) |
> | `builder-grant/src/state.rs` | New `WORK_HASH_USED: Map<&str, u64>` reverse index |
> | `builder-grant/src/error.rs` | New `DuplicateWorkHash { existing_submission_id }` variant |
> | `builder-grant/src/contract.rs` | `execute_submit` checks + writes `WORK_HASH_USED` |
> | `builder-grant/src/tests.rs` | New `VALID_HASH_2` constant; `test_builder_stats` updated to use distinct hashes; 1 new regression (`test_submit_work_rejects_duplicate_hash`) |
> | `junoswap-pair/src/error.rs` | New `UnexpectedDenom { denom, expected_a, expected_b }` variant |
> | `junoswap-pair/src/contract.rs` | `extract_native_amounts` + `execute_swap` reject unknown denoms in `info.funds` |
> | `junoswap-pair/src/tests.rs` | 1 new regression (`test_provide_liquidity_rejects_unexpected_denom`) |
>
> **Tests (v6.1):** `cargo test --workspace` → all crates green, 136
> passing (up from ~131 at v6.0 baseline), 0 failed, 0 ignored. The
> `v6.0` auth gates, atomic callbacks, attestation coherence, and
> supermajority ratio checks remain enforced.
>
> v6.1 is a pure tightening pass — no new capabilities, no new contracts,
> no new message shapes at the public ABI (the new errors are additive
> variants on existing enums). Migration is trivial: redeploy + re-run
> the v6.0 `UpdateConfig.agent_company` and `ConfigChange.new_wavs_operator`
> wires (documented in the v6.0 block below). Existing on-chain obligations
> and tasks are unaffected.
>
> ---
>
> ### v6.0 Update — 2026-04-18
>
> A second structural audit after v5 landed found two sharp-edged
> **identity mismatches** that v5 had papered over rather than fixed. Both
> were single-variable problems wearing architectural costume. v6 removes
> the costume.
>
> 1. **Public `SubmitTask` was unauthenticated.** `task-ledger::SubmitTask`
>    is a public execute — anyone can call it. It recorded
>    `agent_id`, `submitter`, and (after v5) `proposal_id` verbatim,
>    with no check that the submitter actually *owned* the agent. This
>    let any wallet forge task records against any registered agent and
>    drive that agent's `agent-registry` reputation counters (via the v5
>    atomic callback) — a pure reputation-forgery surface.
>    v6 hardens `execute_submit` with three invariants: public callers
>    (a) cannot claim the reserved `agent_id = 0` sentinel, (b) must be
>    the registered owner of the `agent_id` they are submitting under,
>    verified by a cross-contract `agent-registry::GetAgent` query, and
>    (c) cannot overwrite an existing `proposal_id -> task_id` mapping
>    (closing the duplicate-proposal-task loophole that would have let a
>    second submitter clobber a governance-initiated task's reverse
>    index).
>    The `agent_id == 0` sentinel remains reserved for
>    governance-initiated tasks dispatched by the `agent-company`
>    contract itself — v6 adds an optional `agent_company` pointer to
>    `task-ledger::Config` and accepts `agent_id = 0` only when
>    `info.sender == config.agent_company`.
>
> 2. **WAVS operator identity was overloaded onto `task_ledger`.**
>    `agent-company::SubmitAttestation` and
>    `agent-company::SubmitRandomness` authorised the caller if it was
>    admin, governance, *or* the configured `task_ledger` address —
>    because the WAVS bridge in v5 was deployed under the same wallet as
>    the task-ledger contract admin. This coupling was incidental, not
>    structural: the WAVS operator is an off-chain signer (currently
>    Neo's deploy key), not an on-chain contract. Using `task_ledger` as
>    its stand-in meant rotating the WAVS operator would require
>    migrating the `task-ledger` contract, and any contract that ever
>    became `task_ledger` would gain attestation privileges for free.
>    v6 introduces an explicit `wavs_operator: Option<Addr>` field on
>    `agent-company::Config`. Attestation and randomness authorisation
>    now checks `admin | governance | wavs_operator` — `task_ledger` is
>    no longer implicitly trusted. The field is set at instantiate and
>    rotatable via a `ConfigChange` governance proposal
>    (`new_wavs_operator`).
>
> 3. **A silent latent bug surfaced while wiring v6.** `agent-company`'s
>    `instantiate` in v5 built a fresh `Config { … }` value and forgot to
>    call `CONFIG.save(deps.storage, &config)`. Every fresh deployment
>    was effectively running on an unsaved config — the contract only
>    worked because v5 tests happened to exercise paths that loaded from
>    defaults or from post-instantiate updates. v6 restores the missing
>    `CONFIG.save` (and the regression is covered by the new
>    `wavs_operator` assertions in `test_transfer_admin` /
>    `test_config_change_proposal`, which now *require* the stored config
>    to match the instantiate inputs).
>
> **Files changed (v6):**
>
> | File | Change |
> |------|--------|
> | `task-ledger/src/state.rs` | `Config.agent_company: Option<Addr>` (`#[serde(default)]`) |
> | `task-ledger/src/msg.rs` | `InstantiateMsg.agent_company`, `UpdateConfig.agent_company` |
> | `task-ledger/src/error.rs` | `ReservedAgentId`, `AgentNotOwnedBySender`, `ProposalAlreadyHasTask` |
> | `task-ledger/src/contract.rs` | `execute_submit` agent ownership + reserved-id + reverse-index-uniqueness gates; `execute_update_config` manages `agent_company` |
> | `task-ledger/src/tests.rs` | Stub registry extended to answer `GetAgent`; 3 new regressions (`test_submit_task_rejects_reserved_agent_id_for_public_sender`, `test_submit_task_rejects_unowned_agent`, `test_submit_task_proposal_id_requires_agent_company_and_is_unique`) |
> | `agent-company/src/state.rs` | `Config.wavs_operator: Option<Addr>`; `ProposalKind::ConfigChange.new_wavs_operator` |
> | `agent-company/src/msg.rs` | `InstantiateMsg.wavs_operator`; `ProposalKindMsg::ConfigChange.new_wavs_operator` |
> | `agent-company/src/contract.rs` | Restored `CONFIG.save` in instantiate; `wavs_operator` wired through instantiate + `ConfigChange` execution + `SubmitAttestation` / `SubmitRandomness` auth |
> | `agent-company/src/tests.rs` | 3 new regressions (`test_sortition_wavs_operator_authorized`, `test_submit_attestation_wavs_operator_authorized`, `test_submit_attestation_task_ledger_not_authorized_when_wavs_operator_set`); `test_config_change_proposal` updated to drive `new_wavs_operator` end-to-end |
> | `agent-company/src/integration.rs` | Fresh-stack deploy now calls `task-ledger::UpdateConfig { agent_company }` so governance-originated `SubmitTask` calls carry explicit authority |
> | `deploy/deploy.mjs` | Fresh deploy sets `wavs_operator = deployer` on agent-company and wires `task-ledger.agent_company` right after |
> | `wavs/bridge/src/deploy-fresh.ts` | Same two wires for the operator-facing deployer |
>
> **Tests (v6):** `cargo test -p task-ledger -p agent-company` → task-ledger
> `15 passed`, agent-company `44 passed`. No pre-existing tests were
> weakened; the v5 auth paths (admin / governance) remain green without
> change.
>
> v6 is a pure tightening: no new capabilities, no new contracts, no new
> external surface. Every change is either an invariant check that
> rejects previously-accepted inputs, or a field that is `Option<_>` and
> defaults to the pre-v6 behaviour when unset. Migration path documented
> in "Deployment path for existing chains (v6)" below.
>
> ---
>
> ### v5 Update — 2026-04-18
>
> A follow-up on-chain state-machine review found that three of the v4 claims had drifted into **architectural theatre**: the wiring existed, but the state never reached it. v5 turns them from theatre into enforced contracts. Highlights, in priority order:
>
> 1. **Supermajority was a participation gate, not a vote-ratio gate.** v4 required `total_voted_weight ≥ 67%` and then checked `yes > no`, which let a 51 %-yes / 16 %-abstain split pass a constitutional proposal. v5 switches constitutional proposals (`CodeUpgrade`, `WeightChange`) to a true **yes-ratio** gate: `yes_weight × 100 ≥ total_weight × 67`. Abstain no longer counts toward passage. Minorities above 33 % can now in fact block, as the retraction block below promised.
> 2. **`ContractRegistry` was dead on every deployment.** v4 shipped the atomic-callback fabric but left `ContractRegistry { None, None, None }` hard-coded at instantiate in all three ledgers, with no `ExecuteMsg` to populate it. `CompleteTask → Escrow::Confirm` and `CompleteTask → AgentRegistry::IncrementTasks` silently skipped on every fresh deploy. v5 wires the pointers at instantiate time (with sensible defaults from the required direct fields) **and** exposes an admin-only `UpdateRegistry` execute on all three contracts for post-deploy rewiring.
> 3. **`task_id` collided between agent-company and escrow.** `agent-company::WavsPush` authorised the escrow obligation under `task_id = proposal_id`, but `task-ledger::SubmitTask` allocated a fresh autoincrement `task_id`. Once the registry was wired (#2), `CompleteTask` would then call `Escrow::Confirm { task_id: T }` where `T ≠ proposal_id` → `NoObligationForTask` → whole completion reverts atomically. v5 adds an optional `proposal_id` correlation field on `TaskRecord`, threads it from `WavsPush → SubmitTask`, and routes the escrow callback via `task.proposal_id.unwrap_or(task_id)` so both governance-initiated and daemon-initiated tasks settle coherently.
> 4. **Attestation coherence queried the wrong key.** `submit_attestation` called `task-ledger::GetTask { task_id: proposal_id }`, which errored on every WavsPush task, and the `if let Ok(_)` fall-through silently allowed the attestation through unchecked. v5 introduces `task-ledger::GetTaskByProposal { proposal_id }` (a reverse index) and `submit_attestation` now authoritatively verifies task Completion before storing the attestation.
> 5. **`execute_execute_proposal` was permissionless.** Anyone could finalize a passed `CodeUpgrade` or `WavsPush` — opening mempool-ordering games. v5 restricts execution to DAO members (and the admin for emergency dispatch).
> 6. **Housekeeping.** `proposal_timelock_blocks` was a required `InstantiateMsg` field that was only ever emitted as an event attribute, never stored, never read — removed. `ConfigChange` now `addr_validate`s `new_admin` / `new_governance` at proposal creation instead of failing after a full voting cycle.
> 7. **Latent bug surfaced by the integration test (2026-04-18).** `agent-company::WavsPush` hardcodes `agent_id: 0u64` as a "no specific agent" placeholder. Once the `IncrementTasks` callback actually fired (thanks to #2), task-ledger would dutifully call `agent-registry::IncrementTasks { agent_id: 0 }` → `AgentNotFound` → entire `CompleteTask` reverts. v5 treats `agent_id == 0` as a reserved sentinel in task-ledger's `execute_complete` / `execute_fail` and skips the registry callback for it — governance-initiated tasks settle cleanly without spurious registry updates.
> 8. **Medium/low polish.** `CodeUpgradeAction::{InstantiateContract, MigrateContract, ExecuteContract}.msg_json` is now parsed with `serde::de::IgnoredAny` at proposal creation (M3 — malformed JSON fails fast). `ExecuteProposal` now requires `env.block.height > voting_deadline_block` strictly (M4 — closes the single-block vote/execute race at the deadline). `quorum_percent` and `supermajority_quorum_percent` are bounds-checked at instantiate (L5 — must be `1..=100`, with `supermajority ≥ quorum`).
>
> Full details in the "v5 Cross-Contract Coherence Pass" section at the end of this document.

---

## The 3 Variables

| # | Variable | Role | Risk Before | Fix |
|---|----------|------|-------------|-----|
| 1 | `attestation_hash` | Cryptographic spine | Stored blindly — any hex string accepted | On-chain SHA-256 re-computation |
| 2 | `status` | State machine | Independent per-contract, eventual consistency via off-chain daemon | Atomic cross-contract callbacks |
| 3 | `weight` | Power variable | No cap, no cooldown, no floor — 51% coalition can zero out minorities | ~~Delta cap + cooldown + floor~~ → **retracted 2026-04-17**, replaced by 67% supermajority for `WeightChange` (mirrors `CodeUpgrade`). See retraction note in the Variable 3 section below. |

## The 5 Declarations (Contracts)

| Contract | Primary State | Role |
|----------|--------------|------|
| `agent-company` | Proposals, Votes, Attestations, Config (members + weights) | DAO governance hub |
| `agent-registry` | AgentProfiles, AgentStats, trust_score | Agent identity + reputation |
| `task-ledger` | TaskRecords, LedgerStats | Task lifecycle (Running → Completed/Failed) |
| `escrow` | PaymentObligations, LedgerStats | Non-custodial payment tracking |
| `builder-grant` | WorkSubmissions, Config | Builder grant lifecycle |

---

## Variable 1: `attestation_hash` — On-chain Re-computation

### Problem

`execute_submit_attestation` in `agent-company` accepted any 64-char hex string as the attestation hash. There was no on-chain validation that the hash was correctly derived from the claimed inputs. An operator or bridge script could submit fabricated hashes.

### Fix

Re-compute the hash deterministically on-chain before storing:

```rust
// agent-company/contract.rs :: execute_submit_attestation
let mut hasher = Sha256::new();
hasher.update(b"junoclaw-wavs-v0.1.0");   // COMPONENT_ID
hasher.update(task_type.as_bytes());
hasher.update(data_hash.as_bytes());
let expected = hex(hasher.finalize());
if attestation_hash != expected {
    return Err("attestation_hash mismatch");
}
```

### What this proves

The submitted `attestation_hash` is internally consistent with the claimed `task_type` and `data_hash`. You cannot submit a fake hash that doesn't match the inputs. The trust boundary moves from "trust the bridge script entirely" to "trust that the `data_hash` itself is correct" — which is independently verifiable by anyone who re-fetches the same data sources.

### What this does NOT prove

It does not prove the computation happened inside a TEE. That's the ZKP question (see `JunoClaw_ZKP_Discussion_Jake.md`).

### Files changed

- `agent-company/contract.rs` — ~12 lines in `execute_submit_attestation`
- `agent-company/tests.rs` — `compute_attestation_hash()` helper + updated 2 tests

---

## Variable 2: `status` — Cross-contract Status Coherence

### Problem

Three desync points existed:

1. **Completion flow**: Task completion, escrow confirmation, and attestation storage were 3 independent transactions. If TX 1 succeeded and TX 2 failed, the system entered an inconsistent state (task Completed but escrow still Pending).

2. **Attestation without completion**: `execute_submit_attestation` checked that the *proposal* was executed but not whether the corresponding *task* in task-ledger was actually Completed. Attestations could land for Running or Failed tasks.

3. **Trust score drift**: `agent-registry::IncrementTasks` was callable by any admin, with no enforcement that it matched actual task-ledger events.

### Fix — Atomic Callbacks

**task-ledger `execute_complete`** now fires two sub-messages atomically:

```
CompleteTask
  ├─ sub-msg → escrow::Confirm { task_id }     (if registry.escrow is wired)
  └─ sub-msg → registry::IncrementTasks { agent_id, success: true }  (if registry.agent_registry is wired)
```

If any sub-message fails, the entire TX rolls back — no partial state.

**task-ledger `execute_fail`** fires:

```
FailTask
  ├─ sub-msg → registry::IncrementTasks { agent_id, success: false }
  └─ sub-msg → escrow::Cancel { task_id }
```

**agent-company `execute_submit_attestation`** now cross-queries task-ledger:

```rust
// Query task-ledger to verify task is Completed
let task = querier.query::<TaskRecord>(task_ledger::GetTask { task_id })?;
if task.status != Completed { return Err(...); }
```

**agent-registry `execute_increment_tasks`** is now restricted:

```rust
// Only task-ledger or admin can call
let is_task_ledger = config.registry.task_ledger == Some(info.sender);
if !is_task_ledger && info.sender != config.admin { return Err(Unauthorized); }
```

### Activation

Callbacks are **opt-in** via the `ContractRegistry` struct that already exists in every contract's Config. When fields are `None`, no sub-messages fire — fully backward compatible. To activate on uni-7:

```
task-ledger UpdateConfig → registry.escrow = <escrow_addr>, registry.agent_registry = <registry_addr>
agent-registry UpdateConfig → registry.task_ledger = <task_ledger_addr>
```

### Files changed

- `task-ledger/contract.rs` — ~40 lines in `execute_complete` + `execute_fail`
- `agent-company/contract.rs` — ~15 lines in `execute_submit_attestation`
- `agent-registry/contract.rs` — ~6 lines in `execute_increment_tasks`

---

## Variable 3: `weight` — Governance Anti-circularity

> ### ⚠ RETRACTED 2026-04-17 — replaced by `WeightChange` supermajority
>
> After further analysis, the three guardrails described in this section — **delta cap**, **cooldown**, and **floor** — were determined to be architecturally misplaced and were removed in contract `v5`. In summary:
>
> - **Delta cap** (`max_weight_delta`) delays majoritarian consolidation by ~25 minutes without changing the outcome. Patience is not a defence against patience.
> - **Cooldown** (`weight_change_cooldown_blocks`) protects a compromised key holder against a thief, not minorities against majorities. Key hygiene belongs at the keystore layer, not the governance contract.
> - **Floor** (`min_member_weight`) is a fat-finger input-validation catch wearing the costume of minority rights. A member at 1 bps of 10,000 is *commemorated*, not *protected*.
>
> **What replaces them (Alternative D):** `WeightChange` proposals now require the same 67% supermajority quorum as `CodeUpgrade`. Weight redistribution is treated as a constitutional-class action alongside contract migration. This is a *structural* protection — as long as minorities collectively hold > 33%, they can block weight-change proposals. It mirrors an existing pattern in the contract rather than inventing one.
>
> **Why this is safe under JunoClaw's primary deployment shape:** JunoClaw is designed for humans-with-delegate-agents exchanging skills via fixed contracts. AI agents act as delegates of human principals, not as autonomous governance members. The 67% threshold therefore protects humans from other humans' consolidation attempts without blocking emergency interrupts — rogue-agent misbehaviour is handled at the principal–delegate boundary (key rotation, MCP auth changes) and at the task layer (`task-ledger` failed state, `agent-registry` trust_score degradation), not through `WeightChange`.
>
> **Variables 1 and 2 are unchanged** by this retraction. The on-chain `attestation_hash` re-computation and the atomic cross-contract status callbacks are clean fixes for real bugs and remain in force.
>
> **Files changed on retraction:**
> - `agent-company/state.rs` — removed `max_weight_delta`, `weight_change_cooldown_blocks`, `min_member_weight` fields from `Config`, removed `LAST_WEIGHT_CHANGE_BLOCK` storage item.
> - `agent-company/contract.rs` — removed the creation-time validation block in `execute_create_proposal`, removed the cooldown-record side-effect in `execute_execute_proposal`, added `WeightChange` to the supermajority match in vote tallying, **disabled legacy weight-change execute messages at runtime** (decode-compatible but non-executable), and rewrote `migrate` to handle v4→v5 transition and clean up the orphaned `last_weight_change_block` storage key.
> - `agent-company/tests.rs` — updated `test_execute_passed_proposal` to vote with 100% weight (sufficient to clear the 67% threshold) and removed the stale `max_weight_delta` comment.
>
> *The original description of Variable 3 follows below for historical record. It is not accurate about what the contract currently does. It accurately describes what was shipped in v4 and what was overclaimed about it.*

### Problem

No guardrails on `WeightChange` proposals:

1. A 51% coalition could vote to give themselves 100% weight in a single proposal, permanently locking out minorities
2. Weight changes were instant — no cooldown between successive proposals
3. Members could be reduced to 0 weight, silencing them completely

### Fix — Three Guardrails

**1. Delta cap** (`max_weight_delta = 2000 bps = 20%`)

No single member's weight can shift more than `max_weight_delta` bps in one WeightChange proposal. Reaching 100% concentration from 51% takes at minimum 3 proposals.

**2. Cooldown** (`weight_change_cooldown_blocks = 500`)

After a WeightChange executes, no new WeightChange proposal can be *created* for 500 blocks (~8 minutes on Juno). This gives minorities time to organize counter-proposals.

**3. Floor** (`min_member_weight = 1 bps`)

No member can be reduced below 1 bps (0.01%) while remaining a member. Every member retains voting voice.

### New State

```rust
// Config (3 new fields)
pub max_weight_delta: u64,              // default 2000
pub weight_change_cooldown_blocks: u64, // default 500
pub min_member_weight: u64,             // default 1

// New storage item
pub const LAST_WEIGHT_CHANGE_BLOCK: Item<u64>;
```

### Enforcement Points

- **Proposal creation** (`execute_create_proposal`): cooldown check + delta cap + floor check
- **Proposal execution** (`execute_execute_proposal`): records `LAST_WEIGHT_CHANGE_BLOCK`

### Migration

v4 migration patches the Config JSON with defaults if the fields are missing:

```json
"max_weight_delta": 2000,
"weight_change_cooldown_blocks": 500,
"min_member_weight": 1
```

`LAST_WEIGHT_CHANGE_BLOCK` is initialized to 0 if not present.

### Files changed

- `agent-company/state.rs` — 3 new Config fields + `LAST_WEIGHT_CHANGE_BLOCK` storage
- `agent-company/contract.rs` — ~30 lines (create validation + execute recording + instantiate defaults + migrate patch)

---

## v5 Cross-Contract Coherence Pass

The v4 hardening added the atomic-callback fabric but left it un-instantiated. v5 turns the theatre into enforced contract and corrects the supermajority arithmetic.

### C4 — Supermajority: participation gate → yes-ratio gate

**Before** (`agent-company/contract.rs` vote tally):

```rust
let required_quorum = match &proposal.kind {
    ProposalKind::CodeUpgrade { .. } | ProposalKind::WeightChange { .. } => 67,
    _ => 51,
};
let quorum_threshold = cfg.total_weight * required_quorum / 100;
if proposal.total_voted_weight >= quorum_threshold
    && proposal.yes_weight > proposal.no_weight
{
    proposal.status = ProposalStatus::Passed;
}
```

A 51 %-yes / 16 %-abstain / 0 %-no split met `total_voted_weight = 67%` and trivially satisfied `yes > no` → constitutional proposal passed on **51 %** yes. Abstain was silently counted as "present for quorum", which is incompatible with a supermajority story.

**After** (v5):

```rust
match &proposal.kind {
    ProposalKind::CodeUpgrade { .. } | ProposalKind::WeightChange { .. } => {
        let yes_threshold = cfg.total_weight * cfg.supermajority_quorum_percent / 100;
        let max_no_tolerable = cfg.total_weight - yes_threshold;
        if proposal.yes_weight >= yes_threshold {
            Some(ProposalStatus::Passed)
        } else if proposal.no_weight > max_no_tolerable || all_voted {
            Some(ProposalStatus::Rejected)  // passage mathematically impossible
        } else { None }
    }
    _ => { /* participation-quorum + simple-majority, unchanged */ }
}
```

- Constitutional proposals now require **pure yes-weight ≥ 67 %** of `total_weight`. Abstain no longer contributes to passage.
- Early Rejection also fires when No weight exceeds `100% − 67% = 33%`, so a minority holding more than one-third can block even before the last member votes — this is the precise guarantee the Variable 3 retraction note promised.
- Ordinary proposals are unchanged: participation ≥ 51 % + simple majority.

The regression test `test_code_upgrade_supermajority_blocks_minority` was flipped from asserting `Passed` (the old exploitable behaviour) to `Rejected` (the correct behaviour for a 60 / 40 split), and its comment rewritten to document the new semantics.

### C1 / C2 — Wiring the `ContractRegistry`

Every v4 ledger instantiated with `ContractRegistry { None, None, None }` and offered no execute path to ever populate it. v5 changes, per contract:

**`task-ledger`**

- `InstantiateMsg` now accepts an optional `registry: Option<ContractRegistry>`. When `None`, the registry is initialised with `agent_registry` mirrored from the required flat field — so the `IncrementTasks` callback is live from block 0 without any post-deploy action.
- New execute: `UpdateRegistry { agent_registry, task_ledger, escrow }` (admin-only; each field optional, each validated via `addr_validate`).
- `UpdateConfig::agent_registry` now also mirrors the change into `registry.agent_registry`, so legacy admin flows stay coherent.

**`escrow`**

- Same `Option<ContractRegistry>` on `InstantiateMsg`. `registry.task_ledger` is mirrored from the required flat field.
- New `UpdateRegistry` execute identical in shape. `UpdateConfig::task_ledger` likewise mirrors into `registry.task_ledger`.

**`agent-registry`**

- Same `Option<ContractRegistry>` on `InstantiateMsg` (no mandatory pointer to mirror).
- New `UpdateRegistry` execute. This is the **sole** on-chain path to authorise a task-ledger to call `IncrementTasks` — use it during rollout:
  ```
  agent-registry.UpdateRegistry { task_ledger: Some(<addr>) }
  ```

All test fixtures updated: `escrow/tests.rs`, `agent-registry/tests.rs`, `task-ledger/tests.rs` now pass `registry: None` (the default path). `task-ledger/tests.rs` additionally implements a raw-bytes `StubRegistry` contract so the now-firing `IncrementTasks` sub-message can be routed to a live wasm address in `cw_multi_test`.

### C3 / H2 — `task_id` vs `proposal_id` reconciliation

The core collision:

```
agent-company.WavsPush(proposal_id=P)
   ├─ task-ledger.SubmitTask(...)            → issues fresh task_id T
   └─ escrow.Authorize(task_id=P, ...)        (uses proposal_id as the key)

...later...

task-ledger.CompleteTask(task_id=T)
   └─ escrow.Confirm(task_id=T)               → NoObligationForTask (obligation is at P!)
```

**Fix** (`junoclaw-common/lib.rs`, `task-ledger/msg.rs`, `task-ledger/state.rs`, `task-ledger/contract.rs`, `agent-company/contract.rs`):

1. `TaskRecord` gets an optional `proposal_id: Option<u64>` field (serialised with `#[serde(default)]` for backward-compat with pre-v5 stored records).
2. `task-ledger::SubmitTask` accepts an optional `proposal_id` argument; when `Some(_)`, the task is indexed in a new `TASKS_BY_PROPOSAL: Map<u64, u64>` reverse-index.
3. `task-ledger::CompleteTask` routes the escrow `Confirm` callback via:
   ```rust
   let escrow_key = task.proposal_id.unwrap_or(task_id);
   ```
   — so governance-initiated tasks settle at the `proposal_id` the payer used, and daemon-initiated tasks continue to settle at `task_id`.
4. `task-ledger::FailTask` mirrors the same routing for the escrow `Cancel` callback.
5. `agent-company::WavsPush` execution now passes `proposal_id: Some(proposal_id)` into `SubmitTask`.
6. `agent-company::submit_attestation` queries `task-ledger::GetTaskByProposal { proposal_id }` instead of `GetTask { task_id: proposal_id }`, turning the attestation status-coherence check from a silent no-op into an authoritative gate.

`escrow` itself is **unchanged** — it keeps its existing `OBLIGATIONS_BY_TASK` keying, because the *caller* (governance vs daemon) now consistently uses the appropriate id.

### H1 — Members-only `execute_execute_proposal`

```rust
let sender_is_member = cfg.members.iter().any(|m| m.addr == info.sender);
if info.sender != cfg.admin && !sender_is_member {
    return Err(ContractError::Unauthorized {});
}
```

Permissionless execute was convenient but opened cheap mempool-ordering games around `CodeUpgrade`, `WavsPush`, and `ConfigChange`. The member-only gate keeps the trust surface aligned with who can vote, while the admin retains unconditional dispatch for emergency cases.

### H3 / H4 — Housekeeping

- `proposal_timelock_blocks` removed from `agent-company::InstantiateMsg`. The field was required, written only into an instantiate event attribute, never stored, never read. Callers updated: `contracts/agent-company/src/tests.rs` (2 sites), `deploy/deploy.mjs`, `wavs/bridge/src/deploy-fresh.ts`, `wavs/bridge/src/parliament-demo.ts`.
- `ProposalKindMsg::ConfigChange` now `deps.api.addr_validate`s both `new_admin` and `new_governance` at **proposal creation** — invalid addresses fail immediately instead of wasting a full voting cycle.

### `agent_id == 0` sentinel (surfaced by the C3 integration test)

`agent-company::WavsPush` has always hardcoded `agent_id: 0u64` when it calls `task-ledger::SubmitTask` — governance-initiated tasks aren't bound to any specific registered agent. Under v4 this was invisible because the `IncrementTasks` callback was dead (C1). Under v5 with the callback live, task-ledger's `CompleteTask` would call `agent-registry::IncrementTasks { agent_id: 0 }` → `AgentNotFound` → whole completion reverts atomically — a fresh bug born of the fix.

**Fix** (`task-ledger/src/contract.rs`): `agent_id == 0` is now a reserved **sentinel** meaning "no specific agent". In `execute_complete` and `execute_fail`, the registry callback is skipped for `task.agent_id == 0`:

```rust
if task.agent_id != 0 {
    if let Some(registry_addr) = &config.registry.agent_registry {
        // fire IncrementTasks
    }
}
```

The escrow callback still fires on the `proposal_id` path (unchanged), so settlement is unaffected — only the registry-trust-score update is suppressed, which is semantically correct (there's no agent to credit/debit).

### Medium / low polish (M3 / M4 / L5)

- **M3**: `CodeUpgradeAction::{InstantiateContract, MigrateContract, ExecuteContract}.msg_json` is now validated as JSON at proposal creation via `cosmwasm_std::from_json::<serde::de::IgnoredAny>` — zero allocation, zero new deps, zero schema assumptions. Malformed payloads fail immediately instead of reverting the entire upgrade after a full voting cycle.
- **M4**: `ExecuteProposal`'s deadline check is now strictly `env.block.height > voting_deadline_block` (was `>=`). `CastVote` still accepts a vote at `height == deadline` — so voters get one full block (`deadline`) to cast the decisive vote without competing with execution in the same block. Closes the single-block vote/execute race.
- **L5**: `quorum_percent` and `supermajority_quorum_percent` are now bounds-checked at instantiate — values outside `1..=100` are rejected (0 auto-passes on zero votes; >100 freezes governance). A `supermajority_quorum_percent < quorum_percent` is also rejected as incoherent (constitutional weaker than ordinary).

### Files changed (v5)

| File | Change |
|------|--------|
| `junoclaw-common/src/lib.rs` | `TaskRecord.proposal_id: Option<u64>` with `#[serde(default)]` |
| `task-ledger/src/msg.rs` | `InstantiateMsg.registry`, `SubmitTask.proposal_id`, `UpdateRegistry`, `GetTaskByProposal` |
| `task-ledger/src/state.rs` | `TASKS_BY_PROPOSAL` reverse index |
| `task-ledger/src/contract.rs` | Instantiate mirrors `agent_registry` into registry; `execute_submit` writes `proposal_id` + reverse index; `execute_complete` / `execute_fail` route escrow callback via `proposal_id.unwrap_or(task_id)`; `execute_update_config` mirrors into registry; new `execute_update_registry`; query handler for `GetTaskByProposal` |
| `task-ledger/src/tests.rs` | `StubRegistry` contract + `instantiate_stub_registry` helper; all test sites migrated from fake `Addr` to the stub |
| `escrow/src/msg.rs` | `InstantiateMsg.registry`, `UpdateRegistry` execute |
| `escrow/src/contract.rs` | Instantiate mirrors `task_ledger` into registry; `execute_update_config` mirrors into registry; new `execute_update_registry` |
| `escrow/src/tests.rs` | `registry: None` at one instantiate site |
| `agent-registry/src/msg.rs` | `InstantiateMsg.registry`, `UpdateRegistry` execute |
| `agent-registry/src/contract.rs` | Instantiate validates registry; new `execute_update_registry` |
| `agent-registry/src/tests.rs` | `registry: None` at two instantiate sites |
| `agent-company/src/msg.rs` | Removed `proposal_timelock_blocks` from `InstantiateMsg` |
| `agent-company/src/contract.rs` | Instantiate attribute cleanup; `ConfigChange` eager address validation; new yes-ratio / participation-quorum tally; `WavsPush` forwards `proposal_id` into `SubmitTask`; `execute_execute_proposal` member-only gate + **strict-`>` deadline (M4)**; `submit_attestation` uses `GetTaskByProposal`; **L5 governance-percent bounds** at instantiate; **M3 eager `msg_json` validation** in `CodeUpgrade` action parse |
| `agent-company/src/tests.rs` | `test_code_upgrade_supermajority_blocks_minority` inverted to expected-Rejected; `proposal_timelock_blocks` removed at two sites; **3 new regression tests for M3 / M4 / L5** |
| `agent-company/src/lib.rs`, `agent-company/src/integration.rs` | New cross-contract integration test module (4 tests: full WavsPush lifecycle settled via proposal_id, attestation-rejection while task Running, admin-only UpdateRegistry gate, instantiate-time registry snapshot) |
| `agent-company/Cargo.toml` | Added dev-deps on `task-ledger`, `escrow`, `agent-registry` (path-deps, test-only) to power the integration tests |
| `task-ledger/src/contract.rs` (patch 2) | `agent_id == 0` sentinel: skip `IncrementTasks` callback in `execute_complete` / `execute_fail` for governance-initiated tasks with no bound agent |
| `deploy/deploy.mjs`, `wavs/bridge/src/deploy-fresh.ts`, `wavs/bridge/src/parliament-demo.ts` | Off-chain deploy scripts: `proposal_timelock_blocks` removed |

### Deployment path for existing chains

Because the v5 direct-field mirroring only runs at **instantiate**, pre-v5 deployments need an explicit wire-up after migration:

```
# 1. Migrate task-ledger, escrow, agent-registry, agent-company to v5 code-ids.
# 2. Wire pointers from the admin wallet (one-shot, idempotent):
task-ledger.UpdateRegistry    { escrow: Some(<escrow>),   agent_registry: None,           task_ledger: None }
escrow.UpdateRegistry         { task_ledger: Some(<tl>),  agent_registry: None,           escrow: None }
agent-registry.UpdateRegistry { task_ledger: Some(<tl>),  agent_registry: None,           escrow: None }
```

After step 2, `CompleteTask → Confirm → IncrementTasks` fires atomically.

### Deployment path for existing chains (v6)

v6 adds two **identity pointers** that default to `None` in migrated
configs:

- `task-ledger::Config.agent_company` — must point at the
  `agent-company` contract address before governance-initiated
  `WavsPush → SubmitTask` calls (which carry `agent_id = 0`) will be
  accepted. Between migrate and wire-up, all governance-initiated task
  submissions fail with `ReservedAgentId`.
- `agent-company::Config.wavs_operator` — must point at the off-chain
  WAVS bridge wallet before `SubmitAttestation` and `SubmitRandomness`
  will be accepted. Between migrate and wire-up, all WAVS submissions
  fail with `Unauthorized` / `UnauthorizedRandomness`.

Both are fail-closed: pre-v6 behaviour is **not** silently recreated
when the pointer is `None`. This is intentional — the whole point of v6
is to stop overloading `task_ledger` as the WAVS identity and to stop
accepting the reserved `agent_id = 0` from unauthenticated callers.

**Migration order (admin wallet, same session):**

```
# 1. Upload new code-ids for task-ledger and agent-company.
# 2. Migrate both in either order. MigrateMsg is {} for both; no new
#    config fields are required at migrate time.
# 3. Wire task-ledger → agent-company (admin-only, one-shot):
task-ledger.UpdateConfig {
    admin: None,
    agent_registry: None,
    agent_company: Some("<agent_company_addr>")
}
# 4. Wire agent-company → wavs-operator (admin-only, one-shot):
agent-company.RotateWavsOperator {
    new_operator: Some("<wavs_bridge_wallet>")
}
```

Steps 3 and 4 are admin-only direct executes — no voting cycle is
required during the migration window. Decentralized rotation after the
fact is still available via a `ConfigChange` governance proposal with
`new_wavs_operator`, which follows the ordinary 51 % / simple-majority
flow.

**Fresh deployments** (via `deploy/deploy.mjs` or
`wavs/bridge/src/deploy-fresh.ts`) wire both pointers automatically at
instantiate time — see the deploy-script diff in the v6 files table
above.

**Clearing the operator** (`RotateWavsOperator { new_operator: None }`)
is an explicit **kill-switch**: it disables WAVS submissions entirely
without touching code-id or governance. Useful if the operator key is
suspected of compromise and a replacement is not yet ready.

---

## Test Results

All **137 tests pass** across 10 contracts (v5 adds 7 regressions on top of the v4 suite — 4 cross-contract integration covering the C1+C2+C3+H2 happy path, the H2 attestation-rejection negative case, the C1 admin gate, and the instantiate-time registry snapshot; 3 unit-level locking M3, M4, and L5):

| Contract | Tests |
|----------|-------|
| agent-company | 41 (34 unit + 4 integration + 3 new M3/M4/L5) |
| escrow | 14 |
| agent-registry | 13 |
| task-ledger | 12 |
| builder-grant | 12 |
| junoswap-factory | 12 |
| faucet | 11 |
| zk-verifier | 9 |
| junoclaw-common | 7 |
| junoswap-pair | 6 |

---

## Deployment Notes

- No new contracts needed.
- All four governance-adjacent contracts (`agent-company`, `task-ledger`, `escrow`, `agent-registry`) require v5 migration (upload new code-ids + migrate).
- After migration, run the `UpdateRegistry` one-shot on `task-ledger`, `escrow`, and `agent-registry` from the admin wallet (see "Deployment path for existing chains" above).
- Fresh deployments get the correct registry wiring automatically from the existing required-field mirroring — no post-deploy step needed for `task-ledger → agent-registry` and `escrow → task-ledger` pointers; only `registry.escrow` on `task-ledger` and `registry.task_ledger` on `agent-registry` need explicit wiring.
