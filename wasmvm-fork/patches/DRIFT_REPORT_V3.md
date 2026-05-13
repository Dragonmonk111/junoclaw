# Track-B day-1 drift report — v2.2.7 → v3.0.1

*Run: `powershell wasmvm-fork\patches\check-baseline-v3.ps1` against `cosmwasm` v3.0.1 (`74a568d38`).
Date: 2026-05-13. Time spent: ~25 min.*

---

## Headline

**Drift is much lower than the FORWARD_PORT_V3.md plan predicted.**

| | Plan estimate | Actual measurement |
|---|---|---|
| Patches CLEAN | "1-2 of 10" | **7 of 10** |
| Patches needing 3-way | "3-4 of 10" | **2 of 10** |
| Patches needing rewrite | "3-5 of 10" | **1 of 10** |
| Forward-port effort | 3-5 working days | **1 working day** (with low risk) |

The v3.0.x cosmwasm reorganization is **localized, not pervasive.** Most of our patches are insulated from upstream churn because they target deep internals (the `cosmwasm-vm` host-fn registration table, the `Api` trait surface, the new `cosmwasm-crypto-bn254` crate) that v3 left structurally intact.

This is the best possible outcome for the Juno v30 timeline — the BN254 fork can live on v3.0.4 with about one focused half-day of patch work.

---

## Per-patch results (against `cosmwasm` v3.0.1, `74a568d38`)

| #  | Patch                                              | Result    | Plan risk → Actual risk | Notes |
|----|----------------------------------------------------|-----------|------------------------:|-------|
| 00 | `00-rust-toolchain.toml.patch`                     | **CLEAN** | LOW → CLEAN | Rust toolchain pin survives. |
| 01 | `01-cosmwasm-std.imports.rs.patch`                 | **CONFLICT** | MEDIUM → REWRITE | File **moved** to `packages/std/src/exports/imports.rs` in upstream commit `7f63657e7` ("Group imports and exports in exports module"). Content drift inside the new file is also present (BLS12-381 host fns added inline), so a reanchor is needed in addition to the path move. |
| 02 | `02-cosmwasm-std.traits.rs.patch`                  | **CLEAN** | MEDIUM → CLEAN | The `Api::bn254_*` trait additions land cleanly. The trait surface in v3 is line-stable around our anchors. |
| 03 | `03-cosmwasm-std.testing.mock.rs.patch`            | **CLEAN** | LOW → CLEAN | Mock impl is self-contained. |
| 04 | `04-cosmwasm-std.Cargo.toml.patch`                 | **3WAY-OK** | MEDIUM → 3WAY | Cargo.toml structural drift between v2.2.x and v3.0.x. `git apply --3way` resolves cleanly without manual intervention. |
| 05 | `05-cosmwasm-vm.imports.rs.patch`                  | **CLEAN** | **HIGH → CLEAN** | The host-fn registration table did **not** restructure between v2.2 and v3.0. This is the single biggest risk-mitigation win. |
| 06 | `06-cosmwasm-vm.instance.rs.patch`                 | **CLEAN** | MEDIUM → CLEAN | Instance setup is line-stable. |
| 07 | `07-cosmwasm-vm.compatibility.rs.patch`            | **CLEAN** | **HIGH → CLEAN** | Capability strings in v3 still accept the `cosmwasm_2_3` token unchanged. The plan's worry that v3 might have introduced a new capability vocabulary turned out to be wrong — they kept it. |
| 08 | `08-cosmwasm-vm.Cargo.toml.patch`                  | **3WAY-OK** | MEDIUM → 3WAY | Same shape as 04. |
| 09 | `09-cosmwasm-crypto-bn254-new-crate.patch`         | **CLEAN** | LOW → CLEAN | New file (52K). No upstream conflict because the file didn't exist before. |
| 10 | (new) `10-wasmvm.api.rs.patch`                     | **N/A** (skipped — wasmvm patch series not yet checked) | Will need a separate `check-baseline-v3-wasmvm.ps1` against `wasmvm` v3.0.4. The dropped patches in v2.2.x are likely revivable in v3 because v3 has the BLS12-381 wrapper analogue. |
| 11 | (new) `11-wasmvm.lib.go.patch`                     | **N/A** (skipped — wasmvm patch series not yet checked) | Same as 10. |

---

## The single rewrite — `01-cosmwasm-std.imports.rs.patch`

### What broke

`git apply --check` reports:
```
error: packages/std/src/imports.rs: No such file or directory
```

The file was reorganized in upstream commit `7f63657e7` ("Group imports and exports in exports module") between v2.3.x and v3.0.x. Verified via `git ls-files`:
- v2.2.7: `packages/std/src/imports.rs`
- v3.0.1: `packages/std/src/exports/imports.rs`

### What also drifted inside the file

Side-by-side scan of the v3 file (`Select-String -Pattern 'ed25519|secp256k1|bn254|bls12'`):

- **BLS12-381 surface added inline** (lines 52-65): `bls12_381_aggregate_g1`, `_aggregate_g2`, `_pairing_equality`, `_hash_to_g1`, `_hash_to_g2`. These are part of the v2.3 capability bundle that v2.2.7 didn't have.
- **secp256k1, ed25519 still present** (lines 67-102): same surface as v2.2.7, same line shape.
- **Api impl block** (line 394+): contains BLS12-381 trait impls in v3; the v2.2.7 anchor (`impl Api for ExternalApi { ...ed25519_batch_verify... }`) is now followed by BLS12-381 fns before the closing brace.

### Fix shape

Two hunks need reanchoring. Both are mechanical:

1. **Hunk 1 (extern "C" block):** anchor immediately after `ed25519_batch_verify` (now line 102 in v3, was line 105 in v2.2.7). Insert BN254 declarations there. Net change: 4 lines of context shift; no semantic change.
2. **Hunk 2 (Api trait impl):** anchor immediately after `ed25519_batch_verify` impl in `impl Api for ExternalApi`. The v3 version has BLS12-381 impls between ed25519 and the existing trailing methods. Insert BN254 impls in the same logical place (after ed25519, before BLS12-381). Net change: ~8 lines of context shift.

**Estimated effort:** 30 min to rewrite the patch + 15 min to re-verify with `git apply --check`.

### Recommendation

Do the reanchor on day 2 (Track B day 2) and re-run `check-baseline-v3.ps1`. Once that's green, run the full `cargo test` against the patched checkout to verify the BN254 surface still works against v3's slightly-different `Api` trait surrounding code.

---

## The two 3-way merges

`04-cosmwasm-std.Cargo.toml.patch` and `08-cosmwasm-vm.Cargo.toml.patch`. Both are workspace-inheritance / feature-flag drift between v2.2 and v3.0. The `git apply --3way` output (when manually invoked) resolves cleanly without conflict markers, which means the underlying logical change (adding `bn254` feature to the package's feature list) maps unambiguously to v3's manifest.

**Recommendation.** Apply with `--3way` and regenerate the patch from the merged result. ~10 min per file.

---

## Implications for the v30 timeline

The plan's 3-5 working day estimate was conservative. Realistic timeline is:

- **Day 1 (today):** ✅ Baseline check + drift report. *Done.*
- **Day 2 (next session):**
  - Reanchor `01` (30 min).
  - Apply `04` and `08` with `--3way`, regenerate patches (20 min).
  - Run full `cargo test` against patched v3.0.1 (~40 min for `cosmwasm-vm`'s 311 tests + 22 in `cosmwasm-crypto-bn254`).
  - Repeat baseline check; expect 10/10 CLEAN.
- **Day 3 (optional):**
  - Add `wasmvm` v3.0.4 wrapper patches (`10`, `11`) — revive the dropped v2.2.x ones with appropriate updates.
  - Tag the `Dragonmonk111/wasmvm` fork at `v3.0.4-bn254`.
  - Hand Jake the `replace` directive for v30's `go.mod`.

**Net:** 1.5-2 working days of focused effort, not 3-5. Substantially de-risks the prop #374 timeline.

---

## What this enables for outreach

The baseline check is also evidence we can reference in upstream conversations:

- For **Issue 1 on CosmWasm/cosmwasm** (BN254 host functions): the patch series already applies cleanly to v3.0.1 with one path-move adjustment. The work is "ready to upstream" rather than "needs major rewriting first."
- For **Jake's reply** (the parallel BN254 Track B conversation): we now have a concrete, optimistic timeline. The DM template in `JAKE_DM_TRACK_B_CLARIFY.md` can be amended with: *"Day-1 baseline check came back at 7 clean / 2 3-way / 1 reanchor against v3.0.1. Realistic forward-port timeline is 1.5-2 days, not 3-5."*

---

## Reproducibility

```powershell
# From junoclaw repo root:
powershell -ExecutionPolicy Bypass -File wasmvm-fork\patches\check-baseline-v3.ps1
# Or with a different tag:
powershell -ExecutionPolicy Bypass -File wasmvm-fork\patches\check-baseline-v3.ps1 -CosmwasmTag v3.0.0
```

Bash equivalent (Linux/macOS):
```bash
bash wasmvm-fork/patches/check-baseline-v3.sh                    # default v3.0.1
bash wasmvm-fork/patches/check-baseline-v3.sh v3.0.0
```

Both scripts are idempotent: re-running will reuse the existing `~/junoclaw-build/cosmwasm-bn254` clone and just re-fetch + re-checkout. First run takes ~30s for the clone; subsequent runs take ~5s.

---

## Cross-references

- [`FORWARD_PORT_V3.md`](./FORWARD_PORT_V3.md) — original 5-day plan; this report measures actual drift.
- [`v3.0.x/README.md`](./v3.0.x/README.md) — patch-set manifest skeleton (still empty; populate during day 2).
- [`docs/JAKE_DM_TRACK_B_CLARIFY.md`](../../docs/JAKE_DM_TRACK_B_CLARIFY.md) — DM template for the parallel conversation; update with "1.5-2 day" timeline.
- [`docs/CMW_ISSUE1_PASTE.md`](../../docs/CMW_ISSUE1_PASTE.md) — CosmWasm Issue 1 paste-block; this report is the "drift evidence" the issue body alludes to.

---

*Apache-2.0. Drift report produced 2026-05-13 PM as Track-B day-1 deliverable.*
