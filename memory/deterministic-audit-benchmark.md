# `memory/deterministic-audit-benchmark.md`

## Summary (3 lines)

The deterministic-scrutiny benchmark is a 4-axis methodology applied to every JunoClaw contract: (1) failure-mode enumeration, (2) gas trace, (3) storage-layout discipline, (4) determinism proof. Each contract gets a `DETERMINISTIC_AUDIT.md` next to its `Cargo.toml`. As of 2026-05-13 PM, **7 of 10** contracts are audited; the strongest cross-cutting findings are (A) "permission models are too lenient" — most contracts have at least one HIGH/MEDIUM where a public state-mutating handler lacks an auth or fee gate; and (B) "derivative protocols inherit surface without defenses" — `junoswap-pair` F1 demonstrates a Uniswap v2 inflation attack that the original protocol patched a decade ago.

## Key facts

| Item | Value |
|---|---|
| Method origin | Ffern / Lex review pattern; adapted to CosmWasm context |
| Anchor doc per contract | `contracts/<name>/DETERMINISTIC_AUDIT.md` |
| Audited (7) | `moultbook-v0`, `agent-company`, `agent-registry`, `task-ledger`, `escrow`, `zk-verifier`, `junoswap-pair` |
| Pending (3) | `junoswap-factory`, `builder-grant`, `faucet` |
| Cross-cutting HIGH findings | `agent-company` F1 (vote-weights), `zk-verifier` F1 (permissionless verify), `junoswap-pair` F1 (first-depositor inflation) |
| Cross-cutting pattern A | Permission/fee-gate gaps on public state-mutating handlers |
| Cross-cutting pattern B | Inheriting public-protocol surface without inheriting accumulated defenses (e.g. Uniswap v2's `MIN_LIQUIDITY` lockup) |
| CI enforcement | `.github/workflows/audit-bot.yml` requires audit-doc updates on contract source changes |

## The 4 axes

### 1. Failure mode enumeration

For every public handler, list every way it can fail:
- Authorization rejections.
- Input validation failures.
- State precondition mismatches.
- Resource exhaustion / DoS.
- Cross-contract callback failures.
- Migration / upgrade discontinuities.

Tag each by severity: HIGH / MEDIUM / LOW. Severity reflects exploit-realism, not just theoretical reachability.

### 2. Gas trace

For each hot-path handler, produce a per-step gas estimate breakdown. Match to measured numbers from devnet/uni-7 where possible. Surfaces:
- Storage writes that dominate cost.
- Cross-contract calls that don't yet exist but should be planned.
- Loop bounds that are unbounded (red flag).

### 3. Storage layout discipline

- Map keys: never collide, prefixes documented.
- Reverse indexes: tracked separately, gas cost called out.
- No raw `Item<Vec<T>>` for unbounded T.
- Versioned migrations: every state change has a clear migration story.

### 4. Determinism proof

- No `f32` / `f64`.
- No `HashMap` iteration (use `BTreeMap` if needed).
- No `std::time::SystemTime`; use `env.block.{height,time}`.
- Canonical serialization (cosmwasm-std `Binary`, arkworks `CanonicalSerialize`).
- No subgroup-check shortcuts in crypto (esp. BN254 G2).

## Per-contract finding index (2026-05-13 snapshot)

| Contract | Findings | Headline | Audit doc |
|---|---|---|---|
| `moultbook-v0` | 0 | Deterministic from day 0 | [`contracts/moultbook-v0/DETERMINISTIC_AUDIT.md`](../contracts/moultbook-v0/DETERMINISTIC_AUDIT.md) |
| `agent-company` | 1 HIGH + 4 LOW | F1: vote weights not snapshotted at proposal creation | [`contracts/agent-company/DETERMINISTIC_AUDIT.md`](../contracts/agent-company/DETERMINISTIC_AUDIT.md) |
| `agent-registry` | 1 MEDIUM + 7 LOW | F1: registration fees trapped (no withdraw path) | [`contracts/agent-registry/DETERMINISTIC_AUDIT.md`](../contracts/agent-registry/DETERMINISTIC_AUDIT.md) |
| `task-ledger` | 1 LOW-MED + 9 LOW | F1: CancelTask leaves orphaned escrow obligations | [`contracts/task-ledger/DETERMINISTIC_AUDIT.md`](../contracts/task-ledger/DETERMINISTIC_AUDIT.md) |
| `escrow` | 1 MEDIUM + 5 LOW | F1: timeout_blocks dead + unit mismatch with created_at | [`contracts/escrow/DETERMINISTIC_AUDIT.md`](../contracts/escrow/DETERMINISTIC_AUDIT.md) |
| `zk-verifier` | 1 HIGH + 8 LOW-MED | F1: VerifyProof permissionless + unmetered → gas-DoS + LAST_VERIFICATION spoofing | [`contracts/zk-verifier/DETERMINISTIC_AUDIT.md`](../contracts/zk-verifier/DETERMINISTIC_AUDIT.md) |
| `junoswap-pair` | 1 HIGH + 4 MED + 3 LOW | F1: first-depositor inflation attack (no `MIN_LIQUIDITY` lockup); F4: `f64` in `Pool` query (determinism violation) | [`contracts/junoswap-pair/DETERMINISTIC_AUDIT.md`](../contracts/junoswap-pair/DETERMINISTIC_AUDIT.md) |

## Cross-cutting pattern

**Every audited contract except `moultbook-v0` has at least one finding in the category "public state-mutating handler with insufficient permission or fee gate."** The fixes are layered defense, not single-line patches. Common shapes:

- `agent-registry F1`: registration fees collected without a withdraw path.
- `zk-verifier F1`: VerifyProof unmetered.
- `agent-company` (multiple): proposal creation/voting open without minimum-stake.
- `escrow F1`: timeout dead + unit drift.

This pattern is so consistent it should become an audit-bot lint: *"any public state-mutating handler must either (a) have an auth gate or (b) accept a fee."*

## CI enforcement

`.github/workflows/audit-bot.yml` requires that any PR changing a `contracts/<name>/src/**` file must also update the corresponding `contracts/<name>/DETERMINISTIC_AUDIT.md`. The gate posts a PR comment if the audit isn't updated, prompting the author to either re-audit or explain in the PR description.

## Cross-references

- [`docs/LESSONS_2026_05_13_MORNING.md`](../docs/LESSONS_2026_05_13_MORNING.md) §1 — origin of the method (Ffern/Lex pattern).
- [`.github/workflows/audit-bot.yml`](../.github/workflows/audit-bot.yml) — CI enforcement.
- Per-contract audit docs (linked above).
- [`memory/lessons-2026-05-13.md`](./lessons-2026-05-13.md) — contains the longer "audit cadence" reflection.

---

*Apache-2.0.*
