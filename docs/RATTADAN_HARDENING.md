# Rattadan Variable Hardening — v4

> "I'm chaoscoder with no-clean code mode only. 3 variables, 5 declarations."

This document describes the three hardening passes applied to JunoClaw's core variables, inspired by Rattadan's structural audit. Each variable maps to a security property that was either missing or weakly enforced.

---

## The 3 Variables

| # | Variable | Role | Risk Before | Fix |
|---|----------|------|-------------|-----|
| 1 | `attestation_hash` | Cryptographic spine | Stored blindly — any hex string accepted | On-chain SHA-256 re-computation |
| 2 | `status` | State machine | Independent per-contract, eventual consistency via off-chain daemon | Atomic cross-contract callbacks |
| 3 | `weight` | Power variable | No cap, no cooldown, no floor — 51% coalition can zero out minorities | Delta cap + cooldown + floor |

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

## Test Results

All 109 tests pass across 9 contracts:

| Contract | Tests |
|----------|-------|
| agent-company | 34 |
| builder-grant | 14 |
| agent-registry | 13 |
| task-ledger | 12 |
| escrow | 11 |
| faucet | 12 |
| junoswap-pair | 7 |
| junoswap-factory | 6 |

---

## Deployment Notes

- No new contracts needed
- agent-company requires v4 migration (upload new code + migrate)
- task-ledger and agent-registry require config updates to wire ContractRegistry
- All changes are backward compatible until explicitly activated
