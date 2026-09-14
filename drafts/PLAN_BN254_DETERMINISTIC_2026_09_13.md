# Deterministic Plan — BN254 Track Locked, Akita as Backup

> Snapshot: 2026-09-13, updated 2026-09-14. Supersedes the PQC-pathway sections of `PENDING_DETERMINISTIC_CLOSEOUT_PLAN.md` (2026-07-20).
> **Decision locked**: elliptic-curve / BN254 is the production ZK path for all of JunoClaw. Akita/lattice stays a non-blocking R&D backup.
> **2026-09-14 update**: the BN254 port to `junoclaw-chain/lib/cosmwasm` (v2.3.2) described as remaining work below is now DONE and verified. Submodule commit `1693bb5` (branch `junoclaw-bn254-v2.3.2`), parent repo commit `9e62f59` on `commonware`. `cargo test -p cosmwasm-vm --lib` → 311/311 passing; `cargo check -p layer-app` clean. See Phase 1 checklist for exactly what shipped.
> **2026-09-14 update 2**: the `jolt-cw-verifier` Phase 2 *contract wiring* is now DONE and committed (junoclaw `bd13a09`, jolt clone `3f21e6d`). `jolt-verifier` gained a `bn254` feature/module exposing `verify_bn254(preprocessing, public_io, proof)` — the concrete `Fr`/`DoryScheme`/`Pedersen<Bn254G1>`/`LegacyBlake2bTranscript` instantiation. The contract's `verify_jolt_proof_full` now calls it for real (bincode 2 serde decode of proof + `JoltDevice` public I/O + `JoltVerifierPreprocessing` VK), behind the `full-verification` feature. New `StoreVerifyingKey` admin msg + `public_io_base64` on `VerifyProof`. Builds clean for `wasm32-unknown-unknown` (4.7 MB wasm), `cargo test` 7/7. **Still gated on-chain**: actually flipping a deployed contract to `mode = "full"` requires the bulk-memory decision below — the code path is ready, the VM gate is not.

---

## 0. Critical correction: three separate things were being conflated

| Thing | What it actually is | Blocks JunoClaw? |
|---|---|---|
| **cosmwasm#2685** (ours) | BN254 `alt_bn128` host functions for cheap Groth16. Deferred by @DariuszDepta to **end Q3 / start Q4 2026**, Backlog milestone. Our reply posted 2026-06-29. | **No** — it's a gas optimization (371k → **77,590 measured**), not a correctness gate |
| **cosmwasm#2485** (upstream, not ours) | "Rust 1.87 requires bulk-memory proposal". Also assigned @DariuszDepta. Open, last updated 2026-02-27. | **No, not for our own chain** — see below |
| **wasmvm#735** (ours) | Resolved itself — BLS12-381 has no Go-side wrappers in v3.0.4, so BN254 is cosmwasm-only. | No — just needs closing |

**The unlock we already own**: `wasmvm-fork/` contains a complete, working patch series —
`cosmwasm-crypto-bn254`, `cosmwasm-crypto-mayo`, `cosmwasm-crypto-mldsa`, `cosmwasm-std-bn254-ext`, 101 patch items — verified **10/10 clean against v3.0.6** (`cosmwasm-crypto-bn254` 22/22, `cosmwasm-vm` 318/319, the one failure being a pre-existing Windows/wasmer-5 float flake that reproduces on vanilla v3.0.6). These are **already applied** in the WSL build tree (see Phase 1).

**Correction to the earlier correction (2026-09-13, second inspection)**: the first inspection only searched WSL `/root` and concluded no Commonware chain existed. That was wrong — the chain lives on the **Windows side** of this workspace at `junoclaw-chain/` (git branch `commonware`). It is a Layer-SDK/Slay3r-derived pure-Rust chain: `app/slay3rd/src/node.rs` implements `CertifiableAutomaton` bridging Commonware **simplex** consensus (BLS12-381 threshold certs) to the Layer `App<T>` state machine; `devnet/docker-compose.yml` runs **4 validators** (`junoclaw-node-0..3`, chain_id `junoclaw-1`, authenticated P2P on :7001, gRPC on :9090) with key material in `testnet-keys/validator-0..3`. Planning docs confirm phases 1 (foundation), 2 (Commonware consensus), 2.1 (functional node: cross-process P2P, gRPC, tx pipeline, RocksDB) and 2.2 (stability fixes) are complete.

So we control **two** VM surfaces:
- **Juno v30 path** (WSL `/root/junoclaw-build/`): patched cosmwasm **v3.0.6** + wasmvm v3.0.4 — needs Juno governance for mainnet, free for devnet.
- **Sovereign chain path** (`junoclaw-chain/`): cosmwasm-vm **v2.3.2** consumed via `[patch.crates-io]` → `lib/cosmwasm` submodule — **BN254 host functions now ported and verified here as of 2026-09-14** (different major version than the v3.0.6 patch series, so this was a manual re-application, not a clean `git apply`). No governance needed — we shipped it ourselves.

Upstream #2685 remains an *ecosystem contribution* that eventually retires our fork-maintenance burden — valuable, but not gating either path.

---

## Phase 1 — Ship BN254 (verified against the actual build tree)

### What already exists — found in WSL `Ubuntu-24.04` at `/root/junoclaw-build/`

| Path | State |
|---|---|
| `cosmwasm-v3-bn254/` | CosmWasm **v3.0.6**, BN254 patches applied in the working tree (uncommitted): 289 insertions across 8 files |
| `wasmvm-v3-bn254/` | wasmvm **v3.0.4**, rebuilt `libwasmvm.x86_64.so` |
| `cosmwasm-mldsa/` | CosmWasm **v2.2.2** + ML-DSA / MAYO / BN254 crypto packages (older track) |
| `juno/` | Juno **v30.0.0** (`c0b3a8d`), `junod` binary built at `juno/bin/junod` |
| `libwasmvm.x86_64.so` | 8.5 MB, built 2026-08-17 |

BN254 host functions are **already wired**: `packages/vm/src/compatibility.rs` registers `env.bn254_add`, `env.bn254_scalar_mul`, `env.bn254_pairing_equality`; `imports.rs` +125 lines; `instance.rs` +24; guest-side `std` externs/traits/mock all present.

**The sovereign chain node is real and lives in this workspace** at `junoclaw-chain/` — it was missed earlier because the search only covered WSL `/root`. Verified state: `LayerNode` CertifiableAutomaton in `app/slay3rd/src/node.rs`, `VmCache` wrapping `cosmwasm_vm::Cache` in `packages/app/src/wasm/vm/cache.rs` (capabilities: iterator/staking/stargate/cosmwasm_1_1–2_0), deterministic gas bridge (`SDK_TO_WASMER_GAS_FACTOR = 150_000`), BTreeMap-ordered pending payloads, 4-validator devnet compose + keys. Its `lib/cosmwasm` submodule is **stock v2.3.2** — BLS12-381 imports only, no BN254 yet. The articles' "sovereign Commonware chain" claim is now backed by a real node; what remains unbacked is BN254 *on that chain*.

### The bulk-memory blocker is one flag — but flipping it naively is unsound

This applies to **both** VM surfaces: the same `gatekeeper.rs` exists in the WSL v3.0.6 tree (`packages/vm/src/wasm_backend/gatekeeper.rs:65`) and in the sovereign chain's `junoclaw-chain/lib/cosmwasm/packages/vm/src/wasm_backend/gatekeeper.rs:61` — both ship `allow_feature_bulk_memory_operations: false`.

The handler already exists at line 340:

```rust
fn bulk_memory(&'b mut self, operator: Operator<'a>) -> Result<(), MiddlewareError> {
    if self.config.allow_feature_bulk_memory_operations {
        self.state.push_operator(operator);   // flat per-operator cost
        Ok(())
    } else { /* reject */ }
}
```

`push_operator` charges a **flat** cost. It never reads the length operand off the Wasm stack, so `memory.copy` of 1 byte and of 1 MB cost the same. Flipping the flag to `true` and shipping that to a chain with adversarial users is a **gas-underpricing DoS**. This is exactly why upstream #2485 is still open — and the adjacent comment on `allow_feature_reference_types` shows the maintainers reasoning identically about `table.grow`/`table.fill`.

### Three options, in order of soundness

- [ ] **(a) Length-aware metering — the real fix.** Read the length operand, charge `len × gas_per_byte`, treat it as an accounting operator so execution aborts mid-copy on gas exhaustion. This is the work #2485 is blocked on. Doing it for our own chain means we arrive at Dariusz's other blocked issue **with a solution**, not another request.
- [ ] **(b) Flip the flag for devnet only.** Acceptable where the validator set is trusted; must never reach a public chain.
- [ ] **(c) Sidestep entirely.** Compile contracts with `RUSTFLAGS=-Ctarget-cpu=mvp cargo +nightly build -Zbuild-std=panic_abort,std` so no bulk-memory instructions are emitted. Requires nightly; unblocks the Jolt verifier without touching the VM.

### Remaining Phase 1 work

- [x] Commit the currently-uncommitted v3.0.6 working tree in WSL — done, `1ce081165` on branch `bn254-v3.0.6`
- [x] **Port the BN254 host functions to `junoclaw-chain/lib/cosmwasm` (v2.3.2)** — done 2026-09-14. `cosmwasm-crypto-bn254` crate added; `compatibility.rs` registers `env.bn254_add`/`env.bn254_scalar_mul`/`env.bn254_pairing_equality`; `imports.rs` implements `do_bn254_*` with **time-based** gas costs (`measured_µs × GAS_PER_US`, see `crypto-bn254/src/gas.rs`); `instance.rs` wires them into `env_imports`; guest-side `Api` trait + `ExternalApi` externs + `MockApi` impls ported into `cosmwasm-std` behind a new `cosmwasm_2_3` feature; `VmCache::CAPABILITIES` advertises `"cosmwasm_2_3"`. Submodule commit `1693bb5` (branch `junoclaw-bn254-v2.3.2`), parent commit `9e62f59`.
- [x] Verify `junoclaw-chain` builds clean on the `commonware` branch — `cargo check -p layer-app` clean (only pre-existing deprecation warnings), `cargo test -p cosmwasm-vm --lib` → 311/311 passing. Prior uncommitted work (Cargo.lock, slay3rd, packages/app, devnet/, testnet-keys/) was committed at `c45772c` before this port.
- [ ] Choose (a), (b), or (c) above for bulk-memory — the decision now covers both the Juno devnet and `junoclaw-chain` (same gatekeeper code). **Not yet done** — the BN254 precompile port is independent of this; it does not require bulk-memory.
- [ ] Rebuild `zk-verifier` with the `cosmwasm_2_3` feature enabled and deploy against the sovereign VM (4-node devnet, `devnet/docker-compose.yml`)
- [ ] Re-upload `jolt-cw-verifier`; confirm it deserializes and its capability check passes against the updated `VmCache`
- [x] Wire `jolt-cw-verifier` Phase 2 code path — done 2026-09-14 (`bd13a09` + jolt `3f21e6d`): `verify_jolt_proof_full` calls `jolt_verifier::bn254::verify_bn254` for real; `full-verification` feature + `StoreVerifyingKey` + `public_io_base64` added; builds for wasm32, 7/7 tests pass
- [ ] Flip `jolt-cw-verifier` Phase 1 → Phase 2 *on a live chain* — still gated on the bulk-memory decision above; the contract code is ready, the VM gate is not
- [x] Re-measure `zk-verifier` gas with host functions live — done 2026-09-14: **verify_proof = 77,590 SDK gas** (vs 371,486 pure-Wasm, ~4.8×). Host fns recalibrated to time-based pricing after the initial 36k reading exposed a ~1500× under-pricing bug (constants were in a `/100` wasmd-style unit).
- [x] Update the articles: `WHAT_IS_JUNOCLAW_2026_09_11.md` Plan A section now states the BN254 port is done and verified, not pending

---

## Phase 2 — Upstream engagement (parallel, not blocking)

**End state**: BN254 host functions land in upstream CosmWasm; our fork burden goes away.

- [ ] **Ping @DariuszDepta on #2685 in the window Sept 24 – Oct 7.** Not before. Our own note in `UPSTREAM_PING_cosmwasm_2685.md` says *"Do NOT re-ping before Q3-end — they explicitly asked for space."* Today (Sept 13) is ~2.5 weeks early; pinging now spends goodwill for no gain.
- [ ] Post the already-written `CMW_2685_COMMENT_DARIUSZ.md` (6 decision questions: capability home, ABI, gas schedule, empty-pairing semantics, subgroup checks, scope). It's drafted and ready — it just needs the right moment.
- [ ] On a "yes to the shape" reply → open the feature-branch PR (9 commits, one per patch)
- [ ] Close `wasmvm#735` once Dariusz confirms the BLS12-381 Go-wrapper absence is intentional
- [ ] Optionally comment on **#2485** offering our bulk-memory metering numbers — we'll have real data from Phase 1, which is exactly what that issue is stuck on

---

## Phase 3 — Chain launch (the actual critical path)

- [ ] Recruit validators; run BLS DKG ceremony
- [ ] Launch mainnet from genesis.json (54.66M ujclaw fixed supply)
- [ ] Deploy airdrop-claim contract; transfer 35.04M ujclaw; open the 90-day claim window for 213,385 accounts
- [ ] Wire the HomeScreen airdrop banner to the live claim contract (currently a static "Claim opens at launch" badge)
- [ ] Verify fee distribution in `begin_block`/`end_block` on a live chain with real traffic

---

## Phase 4 — IBC + liquidity

- [ ] Deploy `cw-ics20-transfer` (built + tested) on JunoClaw mainnet
- [ ] Stand up Hermes relayer; complete connection handshake to Osmosis
- [ ] Open channel; list `ujclaw/OSMO` and `ujclaw/USDC`
- [ ] Validate the cross-chain ZK path end-to-end: agent on Osmosis → proof via IBC → `ibc-task-host` → `jolt-cw-verifier` (now Phase 2 crypto, not structural)

---

## Phase 5 — Akita/lattice (backup track, explicitly non-blocking)

Revisit only when Phase 1–4 are done, or if quantum timelines change materially.

- [ ] `cfg(target_arch = "wasm32")` fallback in `jolt-akita/src/adapters.rs` so `with_backend_pool` runs inline instead of building a real `rayon::ThreadPool`
- [ ] Feature-gate the prover-only `trace_onehot` kernels out of verify-only builds
- [ ] Resolve the `getrandom`/`ark-std 0.6` conflict pulled in via Akita's `spongefish` dependency
- [ ] Re-run the wasm32 build to discover any further blockers (the first two were only found sequentially)

**Standing rationale for keeping this as backup, not primary**: Akita is a bespoke, un-reviewed lattice PCS. BN254/Groth16 has ~20 years of public cryptanalysis. Our NIST-standardized lattice components (ML-DSA, ML-KEM) are unaffected by this decision and stay deployed — this is a judgement about *one young construction*, not about lattice cryptography as a category.

---

## Immediate queue (next 7 days)

1. ~~Commit the uncommitted v3.0.6 BN254 work~~ — done, `1ce081165`
2. ~~Commit the uncommitted `junoclaw-chain` work~~ — done, `c45772c`
3. ~~Port BN254 host functions to `junoclaw-chain/lib/cosmwasm` (v2.3.2)~~ — done and verified, `1693bb5`/`9e62f59` (2026-09-14)
4. **Rebuild + redeploy `zk-verifier` with `cosmwasm_2_3` feature on the 4-node devnet** — gas confirmed at 77,590 in the in-process `VmCache` test; devnet deploy is the remaining end-to-end confirmation
5. Decide bulk-memory option (a)/(b)/(c) for `jolt-cw-verifier` Phase 2. If (a), that becomes the highest-value engineering item we have — it unblocks our chain *and* upstream #2485
6. Hold the #2685 ping until **Sept 24+**
7. Article narrative is now accurate — the chain exists and the BN254 port is live on it
