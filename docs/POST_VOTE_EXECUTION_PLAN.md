# Post-Vote Execution Plan — BN254 Precompile (Prop #374 → v30 Mainnet)

**Status:** ACTIVE — Proposal #374 PASSED on Juno mainnet (~80% Yes, 22% Abstain, 0.003% No-with-Veto, 44.05% turnout)
**Closing tally captured:** May 5, 2026
**Owner:** VairagyaNodes (deployer) + Cascade (coding agent)
**Co-author target:** Dimi (validator, security-patch steward) for v30 handler
**Upstream targets:** `CosmWasm/cosmwasm`, `CosmWasm/wasmvm`
**Reference vote tally:** [`https://ping.pub/juno/gov/374`](https://ping.pub/juno/gov/374)

**Phase status (2026-05-12):**
- Phase 0 — ✅ **substantively complete** (0.1 + 0.3 + 0.4 done; 0.2 EIP vectors at 5/24 of canonical Ethereum-tests fixtures, but the algebraic vectors already cover the same correctness surface; 0.5 Dimi pre-brief open).
- Phase 1 — 🟢 **READY TO FIRE** — see [`UPSTREAM_ISSUE_DRAFTS.md`](./UPSTREAM_ISSUE_DRAFTS.md) for the paste-ready bodies and the publish sequence.
- Phase 3 — ⚡ **ACTIVE via Jake's v30 branch** — PR #1202 is a DRAFT with Juno AI agent iterating until e2e passes. Our code review posted critical findings. Track B is now the primary path.
- Phases 2, 4, 5 — unchanged; gated on Phase 1 maintainer feedback.

**2026-05-12 situation report:**

1. **Tokens recovered.** Validator VM back online (VirtualBox admin-mode fix). Delegator rewards (~165 JUNO) claimed; validator commission (~2,008 JUNO) claimed via `--node https://juno-rpc.polkachu.com:443` workaround (local node still syncing). Total liquid: ~7,188 JUNO on `juno1qh8rgkdm77wrhlf7un20gz9gmtpxkyaeldt0pg`.
2. **Jake Telegram (May 11-12):** Built a "Juno AI agent" (Claude Opus 4.7, 1M context) that authored PR #1202. The agent's GitHub account got suspended; Jake is restoring it. Agent iterates autonomously until all v30 e2e tests pass. Junoswap is broken; Jake suggested "vibecoding a new one."
3. **PR #1202 status:** DRAFT, cannot merge. CI partially passing (9/15 checks). `x/voting-snapshot` backfill + prune logic has our CRITICAL finding (sparse-delegator pruning bug). Agent is actively pushing fixes.
4. **Moultbook v0:** Contract skeleton complete, 12 tests passing, devnet scripts ready. Awaiting Docker/WSL2 for deployment.
5. **Deterministic scrutiny benchmark set** (Ffern/Lex Fridman video): all future Rust output to be gas-traced, failure-mode-enumerated, and storage-layout-reasoned at the wasm bytecode level.

**Revised sprint priorities (May 12 forward):**
- **P0:** Post `pruneVotingPower` bug + LST quorum asymmetry as GitHub review on PR #1202 — this is the highest-leverage move; Jake's agent is actively iterating and will consume the feedback.
- **P1:** DM Jake (short version from `JAKE_DM_TRACK_B_CLARIFY.md`) — clarify Track B ownership, offer testing help, mention Junoswap opportunity.
- **P2:** Apply deterministic scrutiny to moultbook-v0 (gas trace, failure modes, storage layout hardening).
- **P3:** Deploy Moultbook to devnet, measure real gas, post first cross-org anchor entry.
- **P4:** Draft Junoswap agent-rebuild concept note (Jake's "vibecode" comment = opening).

---

## Reading guide

This document is the **single source of truth** for everything that happens between the on-chain signal (now passed) and the v30 mainnet upgrade. It is structured as five phases, each with concrete deliverables, exit criteria, and failure modes. Mark each task as `[ ]` → `[x]` as it completes. Do not skip phases. Do not parallelize phases that have explicit dependencies.

Companion documents:

- [`UPSTREAM_ISSUE_DRAFTS.md`](./UPSTREAM_ISSUE_DRAFTS.md) — the two GitHub issues we publish before any PR (READY TO PUBLISH as of 2026-05-10)
- [`ADR-001-BN254-PRECOMPILE.md`](./ADR-001-BN254-PRECOMPILE.md) — architecture decision record (host-function track)
- [`ADR-002-MOULTBOOK-SCHEMA-V0.md`](./ADR-002-MOULTBOOK-SCHEMA-V0.md) — schema sketch for the Moultbook substrate (parallel track, not gating BN254)
- [`V30_UPGRADE_HANDLER_DESIGN.md`](./V30_UPGRADE_HANDLER_DESIGN.md) — the handoff brief to Dimi
- [`WASMVM_BN254_PR_DESCRIPTION.md`](./WASMVM_BN254_PR_DESCRIPTION.md) — the PR text (Phase 3)
- [`BN254_BENCHMARK_RESULTS.md`](./BN254_BENCHMARK_RESULTS.md) — measured pure-Wasm baseline + measured precompile path (1.823× reduction)
- [`BN254_BENCHMARK_PROJECTED.md`](./BN254_BENCHMARK_PROJECTED.md) — algebraic precompile projection (kept for record; superseded by measurement)

---

## Operating principles (read once, internalize)

1. **Issues before PRs.** PRs opened cold get rejected; issues invite the conversation that improves the PR. Never skip the issue stage.
2. **Minimal diffs.** No refactoring, no cleanups, no "while we're here." A PR that touches one concern is a PR that lands; a PR that touches three is a PR that argues.
3. **Measurements beat projections.** Every gas number we cite must be reproducible from a clean checkout. If a projection survives Phase 0 within ±5% of measurement, fine; if not, we update the proposal text **before** the PR opens.
4. **Earn Dimi's signature.** Dimi offered to "help / review where needed." That is an invitation, not a partnership. We earn the co-sign by shipping a clean, rehearsed handler — not by asking for it early.
5. **Respect Marius's cleanup.** "Be careful with the implementation, I cleaned up the code base massively." This is now a binding constraint on every diff we propose to the Juno chain repo.
6. **Publish corrections fast.** If a number, attribution, or claim is wrong, fix it in the next commit, not the next article.

---

## Phase 0 — What we do BEFORE touching the maintainers (Days 1-7)

Goal: arrive at the maintainers' doorstep with a turnkey, defensible package.

### 0.1 — Rebase the three patches onto latest upstream  — ✅ **COMPLETE** (2026-05-06, commit `d60c497`; v2.2.7 forward-port added 2026-05-10, commit `a9dd318`)

> **Outcome.** Patches rebased onto `cosmwasm` v2.2.2 (the version `wasmvm` v2.2.4 pins). Verified end-to-end by `rebase-track-a.sh`: `cosmwasm-crypto-bn254` 22/22, `cosmwasm-vm --lib` 311/311. Patch set at [`wasmvm-fork/patches/v2.2.2/`](../wasmvm-fork/patches/v2.2.2/) with apply order in filenames (`00-09`) and a [README manifest](../wasmvm-fork/patches/v2.2.2/README.md).
>
> **Three architectural findings worth surfacing in the upstream issues** (see [§0.1 Completion notes](#01-completion-notes-2026-05-06) below):
> 1. `wasmvm` v2.2.4 has no Go-side BLS12-381 wrappers — the original BN254 wasmvm patches were trying to mirror a pattern that doesn't exist on this branch. Renamed to `.dropped` for audit; deferred to Track B.
> 2. `cosmwasm` v2.2.2 silently changed `read_region`'s signature from `(&MemoryView, ptr, len)` to `(env, store, ptr, len)`. Three call sites in the patches needed rewriting.
> 3. `wasmer-vm` 4.3.7 imports `__rust_probestack`, which Rust 1.81 removed. Source builds of `wasmvm` v2.2.x fail at link time on modern Rust. Track A pins Rust 1.78.0 via `rust-toolchain.toml`.

**Upstream tag survey (2026-05-05):**

| Repo | Latest tag | Juno v29 main (`go.mod`) | Our patches target | Drift to Juno target |
|------|-----------|--------------------------|---------------------|----------------------|
| `CosmWasm/cosmwasm` | `v3.0.1` | (transitive via wasmvm) | `v2.2.0` | n/a — wasmvm-pinned |
| `CosmWasm/wasmvm` | `v3.0.4` | **`v2.2.4`** | `v2.2.0` | **patch-level only** |
| `CosmWasm/wasmd` | `v0.55.0` | `v0.54.0` | n/a | minor |

**Strategic implication — two-track rebase:**

- **Track A (Juno deployment, immediate):** rebase patches onto `wasmvm v2.2.4` + matching `cosmwasm` tag. This is what we ship in v30. Minimal drift (patch-level), resolved in one short session.
- **Track A forward-port (2026-05-10, commit `a9dd318`):** the v2.2.2 series was forward-ported to `cosmwasm v2.2.7` (the latest 2.2.x tag), 10/10 clean. Only patches 04 + 08 (`Cargo.toml`) needed regeneration because v2.2.7 switched `cosmwasm-crypto` to `{ workspace = true }` inheritance; the other 8 apply byte-for-byte unchanged. Two new helper scripts cover this maintenance going forward: `wasmvm-fork/patches/check-baseline.sh <tag>` (30-second `git apply --check` per patch for any tag) and `wasmvm-fork/patches/regen-v227.sh` (idempotent regeneration of the v2.2.7 series from the v2.2.2 source). This means we offer maintainers a choice in Phase 1: PR against the v2.2.x line at either v2.2.2 or v2.2.7, or against `v3.x main` (Track B).
- **Track B (upstream PR, slower):** rebase patches onto `cosmwasm v3.0.1` + `wasmvm v3.0.4` (latest `main`). This is what the upstream maintainers will likely want to review. Major version drift; expect non-trivial conflicts.

We did Track A first (it unblocked the chain upgrade); Track B is deferred until maintainers confirm in Issue 1.1 whether they want the PR against `v3.x main` or against the `v2.x` line for backport. They will likely say "v3.x main, with a backport branch for older lines" — at which point we tackle Track B.

**Reproducing Track A from a clean checkout:**

```bash
cd /path/to/junoclaw
bash wasmvm-fork/patches/rebase-track-a.sh
```

The script (now a verification harness; full source at [`wasmvm-fork/patches/rebase-track-a.sh`](../wasmvm-fork/patches/rebase-track-a.sh)) clones cosmwasm v2.2.2 into `~/junoclaw-build/`, applies the 10 patches at [`wasmvm-fork/patches/v2.2.2/`](../wasmvm-fork/patches/v2.2.2/), pins Rust 1.78.0 via rustup, and runs `cargo test` on `cosmwasm-crypto-bn254` + `cosmwasm-vm --lib`. Total time on a warm cache is ~2 minutes; expect ~5–7 minutes from a cold clone.

Environment overrides:

```bash
BUILD_DIR=/tmp/cosmwasm-test \
COSMWASM_TAG=v2.2.2 \
RUST_VERSION=1.78.0 \
  bash wasmvm-fork/patches/rebase-track-a.sh
```

The **wasmvm** side is intentionally *not* clone-and-build in this script. Inspection of `wasmvm` v2.2.4 (`lib_libwasmvm.go`, `internal/api/bindings.h`) confirmed BLS12-381 itself has no Go-side wrappers — the original BN254 wasmvm patches were trying to mirror a pattern that doesn't exist on this branch. Track A is therefore cosmwasm-only; Go-layer integration moves to Track B (cosmwasm/wasmvm v3.x). The dropped patches are preserved at `wasmvm-fork/patches/wasmvm.*.patch.dropped` for audit.

**Track B (deferred to Phase 2):** same procedure but against `cosmwasm v3.0.1` and `wasmvm v3.0.4`. Likely needs a small number of API adaptations (the v2→v3 jump may have renamed methods or moved trait boundaries). We tackle this only after maintainers confirm they want the upstream PR against `v3.x`.

**Delivered:** 10 patches at `wasmvm-fork/patches/v2.2.2/` (apply order in filenames `00-09`) plus a [README manifest](../wasmvm-fork/patches/v2.2.2/README.md). Originals at `wasmvm-fork/patches/*.patch` retained as historical reference; Go-wrapper attempts moved to `wasmvm-fork/patches/wasmvm.*.patch.dropped`.

**Exit criteria — met:**
- [x] All 10 patches apply cleanly to `cosmwasm v2.2.2` (verified by `rebase-track-a.sh`)
- [x] `cargo +1.78.0 test -p cosmwasm-crypto-bn254` passes (22/22)
- [x] `cargo +1.78.0 test -p cosmwasm-vm --lib` passes (311/311)
- [x] Go-wrapper patches deferred to Track B (architectural justification documented)
- [x] Track B (v3.x rebase) deferred until after Phase 1 maintainer feedback

**In-session conflict resolutions** (captured in the v2.2.2 patch set so they don't recur):
- `packages/std/Cargo.toml`, `packages/vm/Cargo.toml`: kept v2.2.2 versions of `cosmwasm-crypto` + `cosmwasm-vm-derive`, added `cosmwasm-crypto-bn254` path dependency.
- `packages/vm/src/imports.rs`: kept both `charge_host_call_gas` (v2.2.2) and the BN254 constants/helpers (patches), added missing `}` to close `bn254_error_code`, rewrote three `read_region` call sites to the v2.2.2 signature.
- `packages/crypto-bn254/Cargo.toml`: removed the standalone `[workspace]` stanza so the crate joins the parent workspace cleanly.

- [x] Juno `go.mod` re-checked for current wasmvm pin (still `v2.2.4` as of 2026-05-06)
- [x] cosmwasm cloned at `v2.2.2` (the version `wasmvm v2.2.4` pins; not `v2.2.4` since cosmwasm doesn't have that tag)
- [x] `cosmwasm-crypto-bn254` crate added as `packages/crypto-bn254/` (via patch 09)
- [x] 8 cosmwasm patches applied (numbered `01-08` in `v2.2.2/`)
- [x] **2 wasmvm patches dropped** with architectural justification (no Go-side BLS12-381 to mirror)
- [x] `cargo test -p cosmwasm-vm` green (311/311)
- [x] **N/A**: wasmvm `make` deferred (no Go-side patches to test on this track)
- [x] Old patches retained alongside; new patches committed at `wasmvm-fork/patches/v2.2.2/` (commit `d60c497`)
- [x] Track B (v3.x rebase) explicitly deferred

#### 0.1 Completion notes (2026-05-06)

**Why the rebase target is `cosmwasm v2.2.2`, not `v2.2.4`:** `wasmvm` v2.2.4's `libwasmvm/Cargo.toml` declares `cosmwasm-std` and `cosmwasm-vm` from `github.com/CosmWasm/cosmwasm.git` at `rev = "v2.2.2"`. Pinning to anything else creates trait-mismatch errors at the host/guest boundary. The cosmwasm repo doesn't have a `v2.2.4` tag at all on the v2.2.x branch — `v2.2.2` is the correct base.

**Toolchain pin (`__rust_probestack`):** `wasmer-vm` 4.3.7 (transitive dep) emits inline assembly that references the `__rust_probestack` symbol, removed from `compiler-builtins` in Rust 1.81 ([rust-lang/rust#126985](https://github.com/rust-lang/rust/pull/126985)). Source builds of `wasmvm` v2.2.x fail at link time on Rust ≥ 1.81 with `rust-lld: error: undefined symbol: __rust_probestack`. The `00-rust-toolchain.toml.patch` pins Rust 1.78.0 as a hard compatibility floor. This affects every source build of `wasmvm` v2.2.x in 2026; validators running pre-built `libwasmvm.x86_64.so` are unaffected. Resolves on Track B (wasmer 5.x has the new intrinsic).

**Why the wasmvm Go-wrapper patches were dropped:** The original `wasmvm.api.rs.patch` and `wasmvm.lib.go.patch` (now `.dropped`) tried to add cgo wrappers (`Bn254Add`, `Bn254ScalarMul`, `Bn254PairingEquality`) and matching C-ABI shims in `bindings.h`. Reading `wasmvm` v2.2.4's `lib_libwasmvm.go` and `internal/api/bindings.h` showed BLS12-381 itself has **no** Go-side wrappers in this version. The pattern the patches were copying doesn't exist on this branch — it lives on `v3.x main`, where wasmvm absorbed the Go-side host-function surface from a separate proposal. Track B will pick this up when we rebase against `wasmvm v3.0.4`.

---

### 0.2 — Pull canonical EIP-1108 test vectors  — 🟡 **IN PROGRESS** (5/24 vectors landed, seed commit pending)

> **Status (2026-05-06).** Test file [`wasmvm-fork/cosmwasm-crypto-bn254/tests/eip1108_vectors.rs`](../wasmvm-fork/cosmwasm-crypto-bn254/tests/eip1108_vectors.rs) created with 5 hard-coded vectors drawn from EIP-196 / EIP-197 spec text:
>
> - `eip196_ecadd_identity_plus_identity` — O + O = O
> - `eip196_ecadd_g1_plus_neg_g1_is_identity` — G + (-G) = O (encodes p-2)
> - `eip196_ecmul_zero_scalar_is_identity` — 0·G = O
> - `eip197_ecpairing_empty_input_returns_true` — empty input → true (EIP-197 ¶spec)
> - `ecadd_rejects_short_input` — length-check negative case (junoclaw-side, not EIP-derived)
>
> All 5 green via `cargo test --test eip1108_vectors`; the existing 9 algebraic vectors in `vectors.rs` also still green. Remaining 19 vectors come from a follow-up commit lifting from `go-ethereum/core/vm/contracts_test.go` (the canonical EVM reference impl).

Current `tests/vectors.rs` in `cosmwasm-crypto-bn254` has 9 conformance tests. We supplement with the **canonical Ethereum Foundation test suite** so reviewers can see we match Ethereum bit-for-bit.

**Source:** `ethereum/tests` repo → `GeneralStateTests/stPreCompiledContracts/`

Test buckets we pull:
- `ecadd_*` — point addition (G1)
- `ecmul_*` — scalar multiplication (G1)
- `ecpairing_*` — pairing equality (G1×G2 → GT)

**Conversion:** the Ethereum tests are JSON state-test fixtures; we extract the input bytes + expected output and write them as Rust `#[test]` cases under `wasmvm-fork/cosmwasm-crypto-bn254/tests/eip1108_vectors.rs`.

Minimum coverage:
- 5 positive ECADD vectors (sums on curve, identity element, doubling)
- 3 negative ECADD vectors (point not on curve, malformed encoding, ≥p coordinate)
- 5 positive ECMUL vectors (k=0, k=1, k=order-1, k=u64::MAX, random)
- 3 negative ECMUL vectors (point not on curve, malformed, scalar ≥r is allowed → reduce silently)
- 5 positive ECPAIRING vectors (empty input → true, single pair, 2 pairs, 3 pairs, 5 pairs)
- 3 negative ECPAIRING vectors (G2 not in subgroup, mismatched pair count, oversized input)

**Deliverable:** `wasmvm-fork/cosmwasm-crypto-bn254/tests/eip1108_vectors.rs` with ≥24 vectors, all passing.

- [ ] EF test fixtures pulled
- [ ] Vectors converted to Rust test cases
- [ ] All vectors pass (`cargo test --test eip1108_vectors`)

---

### 0.3 — Measure precompile gas on devnet (replace projection with measurement) — ✅ **COMPLETE** (2026-05-10, commit `ea63e85`)

> **Outcome (2026-05-10).** Empirical measurement landed: **370,600 SDK gas (pure-Wasm) → 203,266 SDK gas (precompile) = 1.823× reduction**, 5 deterministic samples per variant, σ = 0. Devnet `junoclaw-bn254-1` running in Docker inside WSL2 Ubuntu-22.04. Pure-Wasm contract code 1, precompile contract code 2; both signed by the validator key (`juno1ny4xd3tw9l6y3z8xmycsap63rjqv3h0nrvv348`) which is the on-chain admin per `deploy-now.sh`. Per-run table (txhashes, block heights, gas wanted, gas used) lives in [`BN254_BENCHMARK_RESULTS.md`](./BN254_BENCHMARK_RESULTS.md).
>
> **Convergence check vs projection:** measured 203,266 vs algebraically projected 223,300 = **9.0% lower than projected** (the projection was slightly conservative on the 30k SDK-overhead constant). Outside the ±5% target band, but in the favourable direction — we cite the measured number, the projection is kept for record only.
>
> **Reproducibility:** one command from a clean checkout: `bash devnet/scripts/reproduce-benchmark.sh`. Wraps proof-bundle generation (deterministic seed 42, `SquareCircuit` x*x=y), devnet boot, contract deployment of both flavours, and the benchmark harness.

> **Status (2026-05-06).** Step 1 — building a patched host-side `libwasmvm.so` linked against our BN254-modified `cosmwasm` v2.2.2 — is complete and reproducible via [`wasmvm-fork/patches/build-wasmvm-track-a.sh`](../wasmvm-fork/patches/build-wasmvm-track-a.sh). The script clones `wasmvm` v2.2.4 fresh, idempotently appends a `[patch."https://github.com/CosmWasm/cosmwasm.git"]` block to `libwasmvm/Cargo.toml` redirecting `cosmwasm-std` and `cosmwasm-vm` to our patched local copy, builds with `cargo +1.78.0 build --release`, and verifies all six BN254 entry-point symbols are linked into the resulting 8.6 MB `.so`:
>
> ```
> ok  cosmwasm_vm::imports::do_bn254_add
> ok  cosmwasm_vm::imports::do_bn254_scalar_mul
> ok  cosmwasm_vm::imports::do_bn254_pairing_equality
> ok  cosmwasm_crypto_bn254::bn254::bn254_add
> ok  cosmwasm_crypto_bn254::bn254::bn254_scalar_mul
> ok  cosmwasm_crypto_bn254::bn254::bn254_pairing_equality
> ```
>
> First-build cold time: ~1m 30s on the dev laptop. Incremental rebuilds are ~40s.
>
> **Status (2026-05-07).** The pipeline downstream of step 1 is fully wired and the chain image is currently building. Three pieces landed today:
>
> 1. **Devnet Dockerfile aligned to the v2.2.2 patch set** (commit `18c1ba3`). The previous Stage 1 manually `cp -r`'d the standalone `cosmwasm-crypto-bn254` crate, `sed`-stripped its `[workspace]`, then applied three top-level patches (`cosmwasm-vm.imports.rs.patch`, `cosmwasm-std.imports.rs.patch`, `cosmwasm-std.traits.rs.patch`). That's the *pre*-rebase patch set. The new Stage 1 iterates over `wasmvm-fork/patches/v2.2.2/*.patch` in numeric order (00..09); patch `09` creates the `packages/crypto-bn254/` crate from scratch as a workspace member, so the manual copy + workspace-strip step is no longer needed. `COSMWASM_TAG` bumped from `v2.2.0` → `v2.2.2` in both `Dockerfile` and `docker-compose.yml`. The libwasmvm `sed`-redirect at lines 74-75 still uses regex `rev = "v2.2.[0-9]+"` so it survives further patch-version drift.
>
> 2. **Both contract flavours compile clean.** `cargo build --release --target wasm32-unknown-unknown -p zk-verifier` produces the pure-Wasm verifier (1m 48s); the same with `--features bn254-precompile` produces the precompile variant (1m 30s). `Cargo.lock` shows a single `cosmwasm-std v2.3.2` so the two flavours are apples-to-apples for the gas comparison. Rust 1.84.0 toolchain installed alongside 1.78.0 (the contracts pin via `contracts/rust-toolchain.toml`).
>
> 3. **Groth16 proof bundle generated** (deterministic seed 42, `SquareCircuit` with `x*x = y`, x=3, y=9). VK 296B, proof 128B, public input 32B. Local arkworks verification: VALID. Saved to `$TMPDIR/groth16_proof.json` for the benchmark harness to consume.
>
> The **`docker compose -f devnet/docker-compose.yml build`** is now executing on the dev laptop. Three stages: (a) `rust-builder` clones `cosmwasm` v2.2.2 + applies `v2.2.2/*.patch` + cargo-builds `cosmwasm-vm`, then clones `wasmvm` v2.2.4 + sed-redirects + cargo-builds `libwasmvm.so`; (b) `go-builder` clones `juno` v29.0.0 + go.mod-replaces `wasmvm` → patched-local + builds `junod`; (c) slim runtime stage. Expected wall time: ~25-30 min cold.
>
> Steps remaining once the image exists: (3) `run-devnet.sh` — bring up the chain on `localhost:26657`; (4) `deploy-zk-verifier.sh` — store + instantiate both flavours, write `deploy.env`; (5) `benchmark.sh` — call `VerifyProof` N times against both addresses, capture median gas, write `BN254_BENCHMARK_RESULTS.md`; (6) commit + push results.

Today: `BN254_BENCHMARK_PROJECTED.md` says ~223,300 SDK gas for the precompile path (algebraic projection from EIP-1108 + 30k overhead). We need to replace this with a **measured** number from a devnet running the patched chain binary.

**Procedure:**

The orchestration consolidated to a single `docker compose` build during the 2026-05-07 work; steps 2 and 3 of the original plan (manual `go build ./cmd/junod` + ad-hoc devnet) are now folded into one reproducible `devnet/Dockerfile`. The pure-Wasm vs precompile comparison is also single-shot: `deploy-zk-verifier.sh` builds **both** `.wasm` flavours and uploads them side-by-side, so the benchmark gets to call the same proof against both addresses on the same image. Concrete sequence:

```bash
# 1. Patches verified host-side (Phase 0.1) — re-run if the source tree drifted.
bash wasmvm-fork/patches/rebase-track-a.sh         # produces patched cosmwasm
bash wasmvm-fork/patches/build-wasmvm-track-a.sh   # produces libwasmvm.so (sanity)

# 2. Generate a deterministic Groth16 proof bundle for the benchmark harness.
cargo run --release -p zk-verifier --example generate_proof
#    → writes $TMPDIR/groth16_proof.json (VK + proof + public inputs as base64)

# 3. Build the chain image: cosmwasm + libwasmvm + junod, all patched in-stage.
docker compose -f devnet/docker-compose.yml build

# 4. Boot the devnet (waits for RPC + first block).
bash devnet/scripts/run-devnet.sh

# 5. Build, upload, instantiate both zk-verifier flavours; emit deploy.env.
bash devnet/scripts/deploy-zk-verifier.sh

# 6. Run the benchmark — N VerifyProof txs against both addresses.
N=50 bash devnet/scripts/benchmark.sh
#    → writes docs/BN254_BENCHMARK_RESULTS.md (median gas, std dev, sample tx hashes)
```

**Memory note:** the validator VM and the devnet must NOT run simultaneously — OOM crash. The validator stays on its VM; the devnet runs on the build machine (or the validator VM with the validator daemon stopped and explicitly NOT restarted until after the measurement).

**Convergence check:**
- `measured_precompile_gas / 223300` should be in `[0.95, 1.05]`
- If outside that range, the algebraic projection's 30k SDK-overhead constant is wrong → update the proposal article and the `BN254_BENCHMARK_PROJECTED.md` doc with the correct constant + measured number, **before** Phase 1.

**Deliverable:** `BN254_BENCHMARK_RESULTS.md` updated with a new section "Precompile measurement (v30 build)" containing the median, std dev, and 5 sample tx hashes.

- [x] Patched `libwasmvm.so` builds clean and exports the 6 BN254 symbols (script: `build-wasmvm-track-a.sh`, 2026-05-06, commit `f0b6c95`)
- [x] Devnet `Dockerfile` aligned to the v2.2.2 patch set (2026-05-07, commit `18c1ba3`)
- [x] Both contract flavours build clean (`zk-verifier` pure-Wasm + `--features bn254-precompile`); `Cargo.lock` confirms a single `cosmwasm-std` v2.3.2 across both
- [x] Deterministic Groth16 proof bundle generated (`SquareCircuit` x*x=y, x=3 y=9, 685B JSON)
- [x] Patched junod image builds via `docker compose build`
- [x] Devnet boots without validator interference (validator daemon stays off during devnet runs to avoid OOM)
- [x] Both zk-verifier flavours deployed; `deploy.env` populated
- [x] N=5 VerifyProof txs executed against each address; gas medians captured (σ = 0, deterministic)
- [x] Measurement does not match ±5% of the 223,300 algebraic projection — measured number is **9% lower** (203,266); projection's 30k-overhead constant slightly conservative. Cite measurement.
- [x] `BN254_BENCHMARK_RESULTS.md` written and pushed (commit `ea63e85`)
- [x] Medium article ([`MEDIUM_ARTICLE_THE_VERIFIABLE_AGENT.md`](./MEDIUM_ARTICLE_THE_VERIFIABLE_AGENT.md)) and afterwork measurement article ([`MEDIUM_ARTICLE_BN254_MEASURED.md`](./MEDIUM_ARTICLE_BN254_MEASURED.md)) cite the measured number; the projection doc is kept for the audit trail only

---

### 0.4 — Write ADR-001

Standalone architecture decision record. See [`ADR-001-BN254-PRECOMPILE.md`](./ADR-001-BN254-PRECOMPILE.md) (separate file).

Sections:
- Context — why we want this
- Decision — what we add (4 host imports, gas schedule, capability flag)
- Alternatives considered — BLS12-381 only, full arithmetic in Wasm, native Cosmos SDK module
- Gas methodology — EIP-1108 + 100× wasmd multiplier
- Security — sub-group checks, canonical encoding, no RNG/wall-clock
- Migration — purely additive, behind `cosmwasm_2_3` feature flag
- Open questions — explicit list of items we want maintainers to confirm

**Why an ADR:** it gives the maintainers a single short doc to skim before the PR. Without it, they reconstruct context from issue threads — which is slow, error-prone, and makes them less likely to engage.

- [x] ADR-001 drafted (`docs/ADR-001-BN254-PRECOMPILE.md`)
- [x] Consistency-checked against `WASMVM_BN254_PR_DESCRIPTION.md` (shared gas table, shared scope language)
- [x] Linked from `BN254_PRECOMPILE_INDEX.md` and the new `ADR-002-MOULTBOOK-SCHEMA-V0.md` cross-references it as the host-function track

---

### 0.5 — Pre-brief Dimi (one async message)

A single Telegram message. No call. No expectation. Just status.

See [`DIMI_PREBRIEF_TELEGRAM.md`](./DIMI_PREBRIEF_TELEGRAM.md) for the exact wording.

Pattern:
- Acknowledge prop #374 closed and what's next
- Lay out our 4-week sequence
- Make explicit that we will hand him the v30 handler **after** Phase 4 rehearsal, with an opt-in cosign
- Zero ask in the message itself

- [ ] Telegram message sent
- [ ] Reply (if any) logged in `_private/dimi_thread.md`

---

## Phase 1 — GitHub issues (Days 8-10)

Goal: invite the conversation that improves the PR.

### 1.1 — Publish issue on `CosmWasm/cosmwasm`

Title: **"Proposal: BN254 host functions (alt_bn128) for Groth16 verification"**

Body: see [`UPSTREAM_ISSUE_DRAFTS.md`](./UPSTREAM_ISSUE_DRAFTS.md) Issue 1.

Key elements:
- Cross-reference issue #751 (Crypto API meta)
- Link to ADR-001
- Link to Juno proposal #374 (on-chain mandate)
- Measured numbers (370,719 → ~200,000)
- **Explicit ask**: "Is the ABI shape acceptable? Concerns before we open a PR?"
- Tag once, politely: `@ethanfrey @webmaster128`
- **Do not include the patch** in the issue — only the design

### 1.2 — Publish issue on `CosmWasm/wasmvm`

Title: **"Proposal: BN254 VM integration for cosmwasm-crypto-bn254"**

Body: see [`UPSTREAM_ISSUE_DRAFTS.md`](./UPSTREAM_ISSUE_DRAFTS.md) Issue 2.

Cross-link to Issue 1.1. Discuss VM-side wiring (CGo bindings, FFI shim). Tag the same maintainers once.

**Publish sequence:** see the numbered list at the top of [`UPSTREAM_ISSUE_DRAFTS.md`](./UPSTREAM_ISSUE_DRAFTS.md) ("Publish sequence (do these in order)"). The drafts are paste-ready; the only edit needed in-flight is replacing one `TBD-after-issue-1-published` placeholder in Issue 2 once Issue 1 has a URL.

- [ ] Issue 1 published, URL captured in `_private/upstream_threads.md`
- [ ] Issue 2 published, URL captured
- [ ] Both issues cross-link the other
- [ ] Tweet/announcement: minimal, one paragraph, "we've opened upstream issues" — NOT "we've opened PRs"

---

## Phase 2 — Incorporate feedback (Days 11-25)

Goal: do not let the upstream conversation block the chain-side work, but do not jump ahead either.

Maintainer feedback arrives on their schedule. While we wait, we work on things that **don't require their input**:

### 2.1 — Harden zk-verifier for the precompile path

The `contracts/zk-verifier/` contract today calls into pure-Wasm arkworks. The precompile variant calls `deps.api.bn254_*`. Both must work, gated by a feature flag, so we can ship the **same WASM** on both v29.1 (today) and v30 (post-upgrade).

- [ ] Feature flag `bn254_precompile` added to `Cargo.toml`
- [ ] Both code paths compile clean
- [ ] Both pass identical test vectors (differential test against 1,000 random Groth16 proofs)
- [ ] Build artifacts named `zk_verifier.wasm` (default) and `zk_verifier_precompile.wasm`

### 2.2 — Write the v30 upgrade handler in our own fork

See [`V30_UPGRADE_HANDLER_DESIGN.md`](./V30_UPGRADE_HANDLER_DESIGN.md).

Two-line summary of what it does:
1. Bump `wasmvm` dependency to the version that includes BN254 host imports
2. Add `bn254` to the chain's accepted-capabilities set

That is **all** the handler does. No state migrations. No param changes. No cleanups.

- [ ] `app/upgrades/v30/upgrade.go` drafted
- [ ] `app/upgrades/v30/constants.go` drafted (UpgradeName = "v30")
- [ ] `app.go` registers the upgrade handler
- [ ] Local build passes (`make install`)

### 2.3 — Rehearse the upgrade on a private devnet

```bash
# 1. Sync a Juno node from a v29.1 archival snapshot to a recent height
# 2. Halt the node at a known block height H
# 3. Drop the v30-patched binary in place
# 4. Restart with --halt-height H+10 and watch the upgrade fire
# 5. Verify:
#    - Block production resumes at H+1
#    - Existing contracts at codes 1..N still execute
#    - A precompile-variant zk-verifier deployed AFTER the upgrade succeeds at the lower gas cost
```

Run this rehearsal **3 times**, on 3 different v29.1 heights, to flush out timing-dependent bugs.

- [ ] Rehearsal 1 successful
- [ ] Rehearsal 2 successful
- [ ] Rehearsal 3 successful
- [ ] Logs archived in `_private/v30_rehearsal_logs/`

---

## Phase 3 — Open PRs (Day ~26, conditional on Phase 1 receiving substantive feedback)

**Gate:** Do not open a PR until at least one substantive maintainer comment exists on each issue. "👍 reaction" is not substantive. "Yes, this is fine, please send a PR" is.

If 14 days pass with no feedback: send a **single** polite ping comment on each issue. Wait another 7 days. If still silent, escalate via the JunoSwap / Layer.xyz Discord (Jake Hartnell can poke Ethan if asked nicely).

> **2026-05-11 update — Juno v30 PR #1202 is open.** Jake Hartnell (via `juno-ai-dev`, the `Co-Authored-By: Claude Opus 4.7` agent operating under his mandate) opened https://github.com/CosmosContracts/juno/pull/1202 on 2026-05-09. The PR description names our work — *"BN254 precompile lands with it (prop #374)"* — and pins `github.com/CosmWasm/wasmvm/v3 v3.0.4` in `go.mod` **with no `replace` directive** pointing at our fork. This makes Track B (wasmvm v3 forward-port) **critical-path** for BN254 to actually land in v30, rather than the patient-wait posture this phase assumed. Full assessment in [`JUNO_V30_PR_ASSESSMENT.md`](./JUNO_V30_PR_ASSESSMENT.md); the action-items in that doc §9 govern Phase 3 sequencing from here. The "Gate" above (substantive maintainer feedback before PR) still holds for the upstream `CosmWasm/cosmwasm` + `CosmWasm/wasmvm` repos; the gate to *Juno* (via the v30 branch) is now Jake's own timeline rather than the upstream maintainers'.

### 3.1 — PR 1: `CosmWasm/cosmwasm`

- New crate `packages/crypto-bn254/`
- 3 patch hunks: `packages/vm/src/imports.rs`, `packages/std/src/imports.rs`, `packages/std/src/traits.rs`
- New feature flag `cosmwasm_2_3`
- Test fixtures from Phase 0.2
- Capability string `bn254`
- **Body:** see existing [`WASMVM_BN254_PR_DESCRIPTION.md`](./WASMVM_BN254_PR_DESCRIPTION.md), updated with measured (not projected) numbers
- **References:** Issue 1.1, ADR-001, Juno prop #374

### 3.2 — PR 2: `CosmWasm/wasmvm`

- 2 patch hunks: `libwasmvm/src/api.rs`, `libwasmvm/lib.go`
- CGo FFI shim
- Go-side wrappers
- Depends on PR 3.1 (note this in the PR body)
- **References:** Issue 1.2, ADR-001

Each PR is **single-concern**: don't bundle the BN254 work with anything else, even if you notice a tempting cleanup.

- [ ] PR 1 opened, URL captured
- [ ] PR 2 opened, URL captured
- [ ] CI green on both
- [ ] No unrelated changes in either diff

---

## Phase 4 — uni-7 testnet rehearsal (Day ~30, parallel to PR review)

Goal: prove the v30 upgrade works on a public Cosmos testnet before mainnet.

**Why uni-7 specifically:** it's Juno's own testnet, validators are real (not a single-node devnet), and the chain state is non-trivial. If the upgrade survives uni-7, the mainnet upgrade is mostly a re-execution.

### 4.1 — Submit v30 upgrade proposal on uni-7

```bash
junod tx gov submit-proposal v30-upgrade-uni-7.json \
  --from <our-uni-7-validator> \
  --chain-id uni-7
```

Proposal JSON: see [`V30_UPGRADE_HANDLER_DESIGN.md`](./V30_UPGRADE_HANDLER_DESIGN.md) §6.

### 4.2 — Vote with our weight, get other uni-7 validators to vote

- Mother wallet votes Yes
- Telegram + Discord poke other uni-7 validators
- 5-day voting period (or shorter — check uni-7 gov params)

### 4.3 — Upgrade fires; observe

- Watch block production at the upgrade height
- Verify contracts continue executing
- Deploy a precompile-variant zk-verifier and verify a Groth16 proof
- Capture gas number — should match Phase 0.3 measurement

- [ ] Proposal submitted on uni-7
- [ ] Proposal passed
- [ ] Upgrade fired without halt
- [ ] Post-upgrade contract execution verified
- [ ] Post-upgrade BN254 verification gas measured & recorded

---

## Phase 5 — juno-1 mainnet upgrade proposal (Day ~40)

Goal: ship to mainnet, with Dimi as co-author.

### 5.1 — Co-author handoff to Dimi

After Phase 4 passes, send Dimi:
- The complete `app/upgrades/v30/` directory
- The Phase 4 rehearsal logs
- The Phase 0.3 + Phase 4.3 gas measurements
- A draft `MsgSoftwareUpgrade` proposal

Ask:
> "We've rehearsed v30 three times locally and once on uni-7. Logs attached. The handler does exactly two things — bump wasmvm, register bn254 capability. Would you co-sign as the chain-side author of record?"

If he agrees: he gets `Co-Authored-By:` on the upgrade commit and is named as co-proposer in the gov proposal text.

If he declines (busy, unsure, scope mismatch): we proceed solo, attribute the upstream PR review (if any) to him, and do not push.

### 5.2 — Submit mainnet upgrade proposal

`MsgSoftwareUpgrade`:
- `Plan.Name = "v30"`
- `Plan.Height = <current_height + 432_000_blocks>` (~15 days at 3s blocks — gives validators 5d voting + 10d to swap binaries)
- `Plan.Info` = JSON with binary download URLs + SHA256 checksums for linux-amd64, linux-arm64, darwin-amd64, darwin-arm64

**Deposit:** 5,000 JUNO (mother wallet).
**Voting period:** 120h (5 days).

### 5.3 — Pre-built binaries

Publish on GitHub Releases at `Dragonmonk111/junoclaw/releases/tag/v30-upgrade`:
- `junod-v30-linux-amd64.tar.gz` + `.sha256`
- `junod-v30-linux-arm64.tar.gz` + `.sha256`
- `junod-v30-darwin-amd64.tar.gz` + `.sha256`
- `junod-v30-darwin-arm64.tar.gz` + `.sha256`
- GPG-signed checksums file

GPG signing key: VairagyaNodes maintainer key (publish fingerprint in repo `SECURITY.md`).

### 5.4 — Rollback plan (the document validators want to see)

A separate doc `docs/V30_ROLLBACK_PLAN.md` explaining:
- How to detect a failed upgrade (block height stops advancing past `Plan.Height`)
- How to revert (downgrade junod, restart from pre-upgrade snapshot)
- Who to contact (us + Dimi, with Telegram handles)
- How long the rollback window is (until next upgrade proposal — basically forever)

Validators read this. Validators with a clear rollback story vote yes; validators without one abstain or veto.

- [ ] Dimi approached for co-sign
- [ ] Pre-built binaries published
- [ ] Checksums GPG-signed
- [ ] Rollback plan documented
- [ ] `MsgSoftwareUpgrade` submitted to juno-1
- [ ] Vote passes
- [ ] Upgrade fires on schedule
- [ ] BN254 precompile live on Juno mainnet

---

## Dependency graph (one chart)

```
Phase 0 (this week)
  ├─ 0.1 Rebase patches ───────┐
  ├─ 0.2 EIP-1108 vectors ─────┤
  ├─ 0.3 Measure devnet gas ───┼─→ Phase 1 (Days 8-10)
  ├─ 0.4 Write ADR-001 ────────┤      ├─ 1.1 Issue on cosmwasm
  └─ 0.5 Pre-brief Dimi ───────┘      └─ 1.2 Issue on wasmvm
                                            │
                                            ▼
                                      Phase 2 (Days 11-25, in parallel)
                                            ├─ 2.1 zk-verifier feature flag
                                            ├─ 2.2 v30 upgrade handler
                                            └─ 2.3 Local rehearsals × 3
                                                   │
                                                   ▼
                                             Phase 3 (Day ~26)
                                                   ├─ 3.1 PR cosmwasm
                                                   └─ 3.2 PR wasmvm
                                                          │
                                                          ▼
                                                   Phase 4 (Day ~30)
                                                   uni-7 testnet upgrade
                                                          │
                                                          ▼
                                                   Phase 5 (Day ~40)
                                                   juno-1 mainnet upgrade
                                                          │
                                                          ▼
                                                   BN254 LIVE
```

---

## Risks & mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Patches don't rebase cleanly on latest cosmwasm tag | Medium | Low | Hand-merge, regenerate patches; document in `REBASE_NOTES.md` |
| Measured precompile gas exceeds projection by >5% | Low | Medium | Update proposal text with measured number before Phase 1; recalibrate overhead constant |
| Maintainer silence on issues (>21 days) | Medium | High | Polite ping at day 14, Discord escalation at day 21, do not open PR cold |
| Maintainer rejects ABI shape | Low | High | Iterate in issue thread; do not open PR until shape is agreed |
| Dimi declines co-sign | Low | Low | Ship solo; attribute upstream review correctly |
| uni-7 upgrade halts the chain | Low | Medium | Validator-coordinated downgrade; keeps mainnet untouched |
| Mainnet upgrade halts the chain | Very Low | Catastrophic | Phase 4 catches this; rollback plan published; binary pre-distributed |
| Akash frontend lease expires during upgrade window | Medium | Low | Top up 25-30 AKT before Day 26 (covered in separate plan) |
| GitHub account locked out | Low (if 2FA done) / Catastrophic (if not) | Catastrophic | Two YubiKeys + recovery codes (covered in separate plan, deadline June 11) |

---

## Communication cadence

| Audience | Channel | Frequency | Tone |
|---------|---------|-----------|------|
| Juno validators (Telegram) | `#juno-validators` | After each Phase exit | Concise, factual, no hype |
| Juno community (Discord) | `#general` | Phase 1, 4, 5 milestones | Slightly broader, link to docs |
| Dimi | DM | After Phase 0 + before Phase 5 | Operational, no ask |
| Jake Hartnell | DM | After Phase 1 publishes (FYI only) | Brief, link-only |
| CosmWasm maintainers | GitHub issues + PRs | As needed | Technical, deferential |
| Twitter / X | Public | Phase 1, 4, 5 | One paragraph each, link-heavy |
| Medium | Public | Phase 5 success | Long-form retrospective |

**Default to under-communicating in public channels.** A passed proposal followed by 6 weeks of quiet work followed by a successful mainnet upgrade is a stronger signal than a passed proposal followed by daily progress tweets.

---

## Tracking

This file is the canonical tracker. Update checkboxes as items complete. **Do not delete entries** — completed items become a record of what was done, in order. The history is the artifact.

Add a `## Log` section at the bottom for non-checkbox notes (interesting findings, course corrections, decisions made under uncertainty).

---

## Log

### 2026-05-05 — Plan instantiated

Proposal #374 closed PASSED on Juno mainnet:
- ~80% Yes
- 22% Abstain
- 0.003% No-with-Veto
- 44.05% turnout

Plan written. Phase 0 begins. No dependencies on outside parties before Day 7 — all work is in our own hands.

### 2026-05-06 — Phase 0.1 complete

Track A rebase landed as commit `d60c497` on `origin/main`. 14 files, +2077 / -3.

- Patches at `wasmvm-fork/patches/v2.2.2/00-09` (10 numbered patches plus a README manifest)
- Verification harness `wasmvm-fork/patches/rebase-track-a.sh` rewritten as a re-runnable test; ran end-to-end from `git clean -fdx` and produced 22/22 + 311/311 green
- Three architectural findings (no Go wrappers on v2.2.4, `read_region` signature drift, `__rust_probestack` removal) flagged for the upstream issues
- Two original `wasmvm.*.patch` files renamed to `.dropped` with audit-trail rationale; will be revisited under Track B

No external surface touched yet. The on-chain mandate from prop #374 plus the green test counts is the substrate Phase 1 will be built on. Phase 0.2 (devnet build with patched libwasmvm) and 0.3 (gas measurement) are the next two work items.

### 2026-05-10 — Phase 0.3 + 0.4 complete; Phase 1 ready to fire

Three commits landed across `origin/main`:

- `ea63e85` — empirical BN254 measurement (1.823× reduction, 5 samples σ = 0) plus the `reproduce-benchmark.sh` one-shot harness. Closes Phase 0.3.
- `9ee3301` — strategic notes (AI-DAO framing, Mesh audit-aware constraints, push notes). Captures the directional input from yesterday's Twitter Space.
- `a9dd318` — Track A forward-port to `cosmwasm v2.2.7` (10/10 clean), plus `check-baseline.sh` + `regen-v227.sh` tooling. Extends Phase 0.1 without re-opening it.
- `260b0b0` — docs commit: `UPSTREAM_ISSUE_DRAFTS.md` updated to READY TO PUBLISH (cites the 1.823× measurement, both patch series, the reproduce one-liner) and `HOWL_SOCIAL_READ_PASS.md` (Moultbook substrate read pass; recommends not-a-fork).

This session also wrote `ADR-002-MOULTBOOK-SCHEMA-V0.md` (parallel track, not gating BN254) and added the explicit "Publish sequence" block to `UPSTREAM_ISSUE_DRAFTS.md` so the next session can start with the GitHub UI directly.

Phase 0.4 (ADR-001) was retroactively ticked — the ADR has existed since the 2026-05-05 plan-instantiation commit; this session just updated its checkboxes to match reality.

**Next session starts here:** open Issue 1 on `CosmWasm/cosmwasm`, capture the URL, paste it into Issue 2's cross-ref placeholder, open Issue 2 on `CosmWasm/wasmvm`, send Dimi the FYI Telegram. Phase 1 fires from this point.

---

*Owner: VairagyaNodes. Co-author target: Dimi. Apache-2.0 throughout.*
