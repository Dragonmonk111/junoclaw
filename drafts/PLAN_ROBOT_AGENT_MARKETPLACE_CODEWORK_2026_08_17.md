# Codework Plan — Robot-as-Agent / Marketplace / Operator-Independence

**Date:** 2026-08-17
**Gates:** `drafts/ARTICLE_ROBOT_IS_THE_AGENT_2026_08_17.md` (unfinished, resume after this lands)
**Priority order:** wasm rebuild + testnet deploy (unblocks everything else) → operator-count fix → skill-registry cross-check → plugin-ros2 stub.

---

## 1. Truth Market minimum-operator-count + diversity guard (HIGH — closes the "def win" gap)

**Problem:** `MultiOperatorGate::audit_with_attestations` (`crates/junoclaw-coordination/src/gate.rs:384`)
computes `verdict_counts[i] / total` with no floor on `total`. `truth-market::execute_finalize_epoch`
(`contracts/truth-market/src/contract.rs:208`) accepts any non-empty verdict set. A single operator
trivially "wins" every epoch.

**Fix (minimal, two places):**
- `gate.rs`: add `min_operators: usize` to `MultiOperatorConfig`; `audit_with_attestations` returns a
  distinguishable "insufficient operators" result (not a Green/Red verdict) when `tasks.len() < min_operators`,
  instead of computing a ratio.
- `contracts/truth-market/src/contract.rs::execute_finalize_epoch`: add `min_operators` to `Config`
  (instantiate + `UpdateConfig`); reject finalization with a new `ContractError::InsufficientOperators
  { required, got }` when `verdicts.len() < config.min_operators`. Mirrors the existing `NoVerdicts` pattern.

**Helper code summary:**
```rust
// state.rs
pub struct Config {
    ...
    pub min_operators: u64, // e.g. 3, matches ARTICLE_TRUTH_MARKETS_2026_08_14 "2/3 majority" framing
}

// error.rs
#[error("Insufficient operators for batch {batch_height}: required {required}, got {got}")]
InsufficientOperators { batch_height: u64, required: u64, got: u64 },

// contract.rs::execute_finalize_epoch, right after the NoVerdicts check:
ensure!(
    verdicts.len() as u64 >= config.min_operators,
    ContractError::InsufficientOperators {
        batch_height,
        required: config.min_operators,
        got: verdicts.len() as u64,
    }
);
```
**Tests to add:** `test_finalize_epoch_below_min_operators` (1 operator, min_operators=3 → error),
`test_finalize_epoch_at_min_operators` (exactly 3 → succeeds). Do not weaken/remove
`test_finalize_epoch_no_verdicts`.

**Diversity guard (stretch, lower priority):** operator registration could carry a
`fingerprint: String` (model+host hash) set at `Register`; `execute_finalize_epoch` could warn/flag
(not block — no on-chain way to *prove* diversity) when all matching operators share a fingerprint.
Likely off-chain (relayer-side alert) rather than a contract-level block, since it's a soft signal.

---

## 2. Marketplace ↔ skill-registry cross-check (MEDIUM)

**Problem:** `marketplace::Config.skill_registry` is optional and unused — `skill_ref` on `ListService`
is never checked against a real `skill-registry` entry.

**Fix:** in `execute_list_service` (`contracts/marketplace/src/contract.rs:115`), if
`config.skill_registry.is_some()`, issue a `QueryMsg::GetSkill { dapp_name: skill_ref }` to the
skill-registry contract and reject the listing if it 404s. Requires either a raw `deps.querier.query_wasm_smart`
call or a cross-contract dependency on `skill-registry`'s query msg type (prefer the former to avoid a
compile-time coupling between the two contract crates).

**Helper code summary:**
```rust
// contract.rs, inside execute_list_service, only if config.skill_registry.is_some():
if let Some(registry) = &config.skill_registry {
    let _: SkillEntry = deps.querier.query_wasm_smart(
        registry,
        &serde_json::json!({ "get_skill": { "dapp_name": skill_ref } }),
    ).map_err(|_| ContractError::SkillNotRegistered { skill_ref: skill_ref.clone() })?;
}
```
Keep `skill_registry: None` as the default (opt-in enforcement) so existing testnet deployments aren't
broken by this change.

---

## 3. `plugin-ros2` stub (LOW — proof-of-concept only, do not over-build)

**Goal:** one more concrete instance of the `Plugin` trait pattern (`crates/junoclaw-core/src/plugin.rs`)
proving the robotics claim, mirroring `plugin-rsynth`'s shape exactly (optional, config-gated, returns
`Err(Plugin{...})` stub messages describing the wiring point rather than a real ROS2 client — same style
as the existing peaq/rsynth plugins at this stage).

**Helper code summary:** `plugins/plugin-ros2/src/lib.rs` — `PluginCapability::ExecutionProof` (shared
with rsynth) or a new `PluginCapability::RoboticsControl`; config schema `{ ros2_bridge_url, node_namespace }`;
actions `fetch_sensor_snapshot` / `submit_intent_for_audit` stubbed the same way rsynth's
`fetch_execution_proof` / `verify_execution` are stubbed today. Do not implement a real `rclrs`/DDS
client in this pass — that's a separate, much larger undertaking.

---

## Explicitly out of scope for this plan

- Actually implementing operator "diversity" cryptographic proof (unsolved research problem, flagged
  in the article as a known gap, not something to code around today).
- Rewriting `MultiOperatorGate`'s consensus math beyond the min-count floor.
- Building a real ROS2/rclrs bridge.
- Any change to `skill-registry`'s own contract — it's correct as-is; the gap is purely on the
  marketplace side.

## Sequencing

1. **Now:** wasm rebuild + copy to `C:\Temp\junoclaw-wasm-target\wasm32-unknown-unknown\release`, deploy
   marketplace + truth-market + emergency-compute-escrow to uni-7 (unblocks real data for Robot Ops UI).
2. **Next session:** item 1 above (min-operator-count fix) — small, high-value, directly closes a real
   security gap identified today.
3. **Then:** item 2 (marketplace/skill-registry cross-check).
4. **Then:** resume `ARTICLE_ROBOT_IS_THE_AGENT_2026_08_17.md` with the before/after from item 1.
5. **Later, optional:** item 3 (plugin-ros2 stub) once the article ships and there's a concrete robotics
   integration ask.
