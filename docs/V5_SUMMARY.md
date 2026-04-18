# JunoClaw v5 — Cross-Contract Coherence Pass

**Date:** 2026-04-18
**Scope:** On-chain deterministic state machine across all 10 contracts
**Tests:** 137 / 137 passing across the workspace
**Status:** Deployment-ready (migration path documented in `RATTADAN_HARDENING.md`)

> TL;DR for the detailed writeup: `docs/RATTADAN_HARDENING.md`, "v5 Cross-Contract Coherence Pass".

---

## What v4 got wrong

v4 shipped the *wiring* for atomic cross-contract callbacks, but three of its central claims were **architectural theatre** — the state never reached the wiring. v5 closes the gap.

| # | Claim in v4 | Reality before v5 | v5 fix |
|---|-------------|-------------------|--------|
| C1 | "`ContractRegistry` enables atomic settlement" | `{ None, None, None }` hardcoded at instantiate; no `UpdateRegistry` exec; every `CompleteTask` callback silently skipped | Instantiate mirrors direct fields into registry + admin-only `UpdateRegistry` on all 3 ledgers |
| C2 | "Direct `agent_registry` / `task_ledger` fields are kept in sync with registry" | No mirroring — fields and registry drifted on every config change | `execute_update_config` and `execute_update_registry` mirror both directions |
| C3 | "`WavsPush` settles the escrow obligation via `CompleteTask`" | `WavsPush` authorized escrow with `task_id = proposal_id`; `task-ledger::SubmitTask` allocated a fresh autoincrement; `Escrow::Confirm { task_id }` → `NoObligationForTask` → whole completion reverts | `TaskRecord.proposal_id: Option<u64>` + `TASKS_BY_PROPOSAL` reverse index; escrow callback routed via `proposal_id.unwrap_or(task_id)` |
| C4 | "Constitutional proposals need 67 % supermajority" | Gate was `total_voted_weight ≥ 67 %` + `yes > no`, so a 51 % yes / 16 % abstain split would pass | True yes-ratio gate: `yes_weight × 100 ≥ total_weight × 67`. Abstain no longer counts toward passage. |
| H1 | "Only DAO members execute" | `execute_execute_proposal` was permissionless (mempool-ordering games) | Member-only gate + admin emergency dispatch |
| H2 | "Attestations are gated on task Completion" | `submit_attestation` queried `task-ledger::GetTask { task_id: proposal_id }`, which *always* errored on WavsPush tasks; the `if let Ok(_)` fall-through silently let attestations through unchecked | New `GetTaskByProposal` query; attestation now authoritatively verifies `Completed` state |
| H3 | — | `proposal_timelock_blocks` was required, emitted as an event, never stored, never read | Removed from `InstantiateMsg`; off-chain deploy scripts updated |
| H4 | — | `ConfigChange` validated addresses only at execute (after a full voting cycle) | `addr_validate` at proposal creation |

## What the integration test found

Adding the first real-contract-against-real-contract integration test (`agent-company/src/integration.rs`, 4 tests) surfaced a latent bug **born of the C1 fix**:

**`agent_id == 0` sentinel.** `WavsPush` hardcodes `agent_id: 0u64` because governance-initiated tasks aren't bound to any registered agent. Under v4 this was invisible — the registry callback was dead. Under v5 with the callback live, task-ledger would call `agent-registry::IncrementTasks { agent_id: 0 }` → `AgentNotFound` → `CompleteTask` reverts atomically. Fix: treat `agent_id == 0` as a reserved sentinel in `execute_complete` / `execute_fail` and skip the registry callback (escrow settlement unaffected, it runs on `proposal_id`).

## Medium / low polish

- **M3 — eager `msg_json` validation.** `CodeUpgradeAction::{InstantiateContract, MigrateContract, ExecuteContract}` payloads are now parsed via `cosmwasm_std::from_json::<serde::de::IgnoredAny>` at proposal creation. Malformed JSON fails fast instead of reverting the entire upgrade after a full voting cycle.
- **M4 — strict-`>` execute deadline.** `ExecuteProposal` now requires `env.block.height > voting_deadline_block` (was `>=`). `CastVote` still accepts at `height == deadline`, so voters get one full block to cast the decisive vote without competing with execution in the same block. Closes the single-block vote/execute race.
- **L5 — governance-percent bounds.** `quorum_percent` and `supermajority_quorum_percent` are bounds-checked at instantiate: must be `1..=100`, with `supermajority ≥ quorum`. Prevents 0 (auto-pass on zero votes), >100 (governance freeze), and incoherent configurations.

## Test delta

| Category | v4 | v5 | Delta |
|----------|----|----|-------|
| agent-company unit | 34 | 37 | +3 regression (M3, M4, L5) |
| agent-company integration | 0 | 4 | +4 (full WavsPush lifecycle, H2 negative, admin gate, instantiate snapshot) |
| **Workspace total** | **130** | **137** | **+7** |

## Deployment

v5 direct-field mirroring only runs at **instantiate**, so pre-v5 deployments need an explicit one-shot wire-up after migration:

```
# After migrating task-ledger, escrow, agent-registry, agent-company to v5 code-ids:
task-ledger.UpdateRegistry    { escrow: Some(<escrow>),   agent_registry: None,     task_ledger: None }
escrow.UpdateRegistry         { task_ledger: Some(<tl>),  agent_registry: None,     escrow: None }
agent-registry.UpdateRegistry { task_ledger: Some(<tl>),  agent_registry: None,     escrow: None }
```

After the `UpdateRegistry` calls, `CompleteTask → Confirm → IncrementTasks` fires atomically end-to-end.

## Key files touched

- `contracts/agent-company/src/{contract,msg,tests,lib,integration}.rs`
- `contracts/task-ledger/src/{contract,msg,state,tests}.rs`
- `contracts/escrow/src/{contract,msg,tests}.rs`
- `contracts/agent-registry/src/{contract,msg,tests}.rs`
- `contracts/junoclaw-common/src/lib.rs`
- `contracts/agent-company/Cargo.toml` (dev-deps for integration tests)
- `deploy/deploy.mjs`, `wavs/bridge/src/deploy-fresh.ts`, `wavs/bridge/src/parliament-demo.ts` (off-chain scripts)
- `docs/RATTADAN_HARDENING.md` (detailed writeup)
