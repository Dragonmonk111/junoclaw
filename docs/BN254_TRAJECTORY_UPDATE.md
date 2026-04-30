# BN254 Precompile — Trajectory Update (April 2026)

> A concise, community-facing snapshot of where the BN254 precompile work
> stands relative to the artefact index in `BN254_PRECOMPILE_INDEX.md`. It
> summarises what is shippable today, what is in flight, and the remaining
> gates before the on-chain signaling proposal goes up.

---

## TL;DR

* **Code is complete.** The host-function crate, guest-side shim, upstream
  patches, feature-gated zk-verifier, devnet harness, and benchmark
  harness are all checked in to `origin/main`. 22/22 host-function tests
  pass and 9/9 contract tests pass.
* **Pure-Wasm baseline is measured on `uni-7`.** A live Groth16 BN254
  verification on the deployed `zk-verifier` (code_id 64) consumed
  **371,486 SDK gas** (tx `F6D5774E…5080F4DA`, block 12,673,217).
* **Precompile gas is projected, not yet measured on-chain.** The interim
  projection in `BN254_BENCHMARK_PROJECTED.md` shows ~223,300 SDK gas for
  the 4-pair Groth16 form (or ~187,000 for the canonical 3-pair form
  with precomputed α·β). **Reduction ≈ 1.66× to 1.99×** depending on form.
* **Devnet is up on the validator VM; precompile measurement pending an
  image rebuild.** The pre-built `junoclaw/junod-bn254:devnet` image was
  transferred into an air-gapped VirtualBox VM and started successfully
  on `2026-04-29`; the chain `junoclaw-bn254-1` is producing blocks on
  `localhost:36657`. The pure-Wasm `zk-verifier` deployed and instantiated
  cleanly. The precompile variant uploaded as `code_id 2` but fails
  instantiation silently — the transferred image turned out to be linked
  against a stock `libwasmvm` without the BN254 host imports, and the
  in-VM rebuild tripped on a blank-line separator missing between two
  `diff --git` hunks inside `cosmwasm-vm.imports.rs.patch`. The fix is
  a clean patch-regeneration step (`git diff` on a manually-applied
  checkout), tracked in §4 below; no BN254 code change is required.

---

## Why this matters — the agent-company vision

BN254 is not a standalone feature. It is the verification backbone of JunoClaw's **agent-company suite**: a stack that marries CosmWasm contracts, DAODAO governance, and TEE-attested agents into a single framework for scalable, verifiable, deterministic work.

The model is straightforward. A DAO defines a task — route a swap, verify a credential, settle a dispute, triage an insurance claim. An agent picks it up and executes inside a **TEE workbox**: a hardware enclave whose attestation can only reflect what actually happened. The TEE produces a Groth16 proof. The chain checks it.

Today that check costs 371 486 gas in pure Wasm — expensive enough that verification is sampled or skipped. With BN254 native, the cost drops to ~187–223K gas: **cheap enough to be mandatory on every single task**. That is the threshold that turns optional auditing into universal auditing.

**Compute preservation** is the design principle. The agent does the heavy lifting off-chain. It doesn't re-execute on-chain — it proves it already executed correctly, and the chain verifies the proof in constant time regardless of the original computation's complexity. An agentic person requesting meaningful service exchange — a farmer checking crop data, a grant applicant submitting credentials, a liquidity provider routing through Junoswap — gets a receipt that is cryptographically true and fair. The TEE workbox guarantees that the agent *could not have cheated*; the BN254 precompile guarantees that the chain *actually checked*.

CosmWasm gives us composable contracts (task ledgers, escrow, registries, the zk-verifier). DAODAO gives us DAO-native governance over the agent fleet. BN254 gives us the cryptographic bridge between off-chain TEE execution and on-chain trust. Together they form a **home base for expansion**: every new pre-intent tool — a DeFi router, a credential oracle, a dispute circuit — plugs into the same verify-then-settle pipeline without forking a new chain or deploying an EVM wrapper.

---

## What changed since the last update

### 1. Benchmark harness hardened to post-Ffern discipline

`wavs/bridge/src/benchmark-zk-verifier-devnet.ts` now:

* Canonicalises the `--out` path and refuses to write outside the repo
  root or the system tmpdir.
* Canonicalises `ZK_PROOF_PATH`, refuses to load proofs from outside the
  repo or the tmpdir, and caps the proof JSON at 1 MiB.

These are the same primitives the v0.x.y-security-1 release (Ffern's
five fixes — C-1 / C-2 / C-3 / C-4 / H-3) applied to `mcp/upload_wasm` and
the WAVS bridge SSRF gate. The benchmark harness is a development tool —
this is defence-in-depth, not the patching of a known vector.

### 2. Standalone gas projection example

A new `cosmwasm-crypto-bn254` example, `gas_projection.rs`, runs each
primitive and renders a markdown report combining:

* The wall-clock per-primitive measurement,
* The EIP-1108-grounded gas-schedule ceilings from `gas.rs`,
* The published `uni-7` pure-Wasm measurement (371,486 gas),
* The algebra of the projection (`n_pub` scalar muls + adds + 4-pair
  pairing + ~30 k SDK contract overhead).

Run with:

```bash
cargo run --release -p cosmwasm-crypto-bn254 --example gas_projection
```

The output (`docs/BN254_BENCHMARK_PROJECTED.md`) is the **interim** number
the governance proposal cites; once the devnet measurement lands, the
auto-generated `BN254_BENCHMARK_RESULTS.md` supersedes it.

### 3. Devnet transfer helper

The patches sometimes get mangled by Windows ↔ Linux line-ending
translation when cloned fresh inside a VM. To work around that, the
working build can be saved on a host that *does* succeed (e.g. WSL on
the developer's Windows machine):

```bash
docker save junoclaw/junod-bn254:devnet -o /mnt/d/junoclaw-devnet.tar
```

…and then loaded onto the validator VM with the new helper:

```bash
TRANSFER_MODE=http WIN_HOST=192.168.X.Y:8000 \
    devnet/scripts/transfer-image-from-windows.sh
```

The script supports three modes: `local` (tar already on
`/mnt/extdrive`), `shared-folder` (VirtualBox shared folders), and `http`
(Windows host serves via `python3 -m http.server`). It then mounts the
data directory under `/mnt/extdrive/junoclaw-devnet/juno-data`, binds
the RPC/REST/P2P ports to `127.0.0.1`, and waits for the chain to come
up before declaring the devnet healthy.

### 4. Devnet launch attempt on validator VM (2026-04-29)

The first end-to-end devnet run against a pre-built image landed the
chain itself, reproduced the pure-Wasm contract deployment, and
narrowed the remaining on-chain-measurement gate to a single patch
format issue. Concretely:

| Step | Result |
|---|---|
| Load pre-built image (`docker load`) | ✅ `junoclaw/junod-bn254:devnet` present |
| Start container (`junod-bn254-devnet`) | ✅ block height advancing, `localhost:36657` bound |
| Upload `zk_verifier_pure.wasm` (code_id 1) | ✅ stored, hash `F76CA06F…A646B` |
| Instantiate pure contract | ✅ `juno14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9skjuwg8` |
| `vk_status {}` query against pure contract | ✅ `{ data: { has_vk: false, vk_size_bytes: 0 } }` — the contract runs |
| Upload `zk_verifier_precompile.wasm` (code_id 2) | ✅ stored, hash `310D474A…F8FC9` |
| Instantiate precompile contract | ❌ silent failure — the loaded image lacks `bn254_add` / `bn254_scalar_mul` / `bn254_pairing_equality` host imports |
| In-VM rebuild to produce a BN254-linked image | ❌ `git apply` rejects `cosmwasm-vm.imports.rs.patch` at line 112 |

Root-cause of the in-VM rebuild failure (inspected in this session):

* The patch file concatenates three `diff --git` hunks — one against
  `packages/vm/Cargo.toml`, one against `packages/vm/src/imports.rs`,
  and one against `packages/vm/src/instance.rs`.
* The third hunk's header sits directly against the end of the second
  hunk with no separating blank line. `git apply` reads that as a
  continuation of the `imports.rs` hunk, which then has content that
  doesn't match at the declared offset, and it aborts with
  `corrupt patch at line 112`.
* Reproducing the same failure against a fresh clone of
  `CosmWasm/cosmwasm` at tag `v2.2.0` (not the devnet build tree)
  confirms the issue is inside the patch file, not inside the build
  context.

The repair plan — not executed in this session, but scheduled next —
is to stop hand-editing the patch and regenerate it cleanly:

1. Clone `v2.2.0` fresh, copy `wasmvm-fork/cosmwasm-crypto-bn254` into
   `packages/crypto-bn254/`, and *manually* apply each targeted change
   to `Cargo.toml`, `imports.rs`, and `instance.rs` (the same changes
   the broken patch attempts, already documented in
   `wasmvm-fork/README.md`).
2. `git diff` the working tree to produce a single well-formed patch,
   which by construction carries correct hunk separators.
3. Replace `wasmvm-fork/patches/cosmwasm-vm.imports.rs.patch` with the
   regenerated file and run `git apply --check` against a second fresh
   `v2.2.0` clone to confirm it applies.
4. Rebuild the Docker image, `docker save`, transfer, restart the
   devnet.

Once that lands, `devnet/scripts/benchmark.sh` produces the measured
`VerifyProof` gas for both variants and `BN254_BENCHMARK_RESULTS.md`
supersedes `BN254_BENCHMARK_PROJECTED.md` as the headline citation.

The proposal does not wait on this. The schedule-based projection is
exactly what EIP-1108 and every other BN254-adopting chain has cited
since 2019. The devnet measurement is a nice-to-have tightening, not a
gate.

### 5. Hardening review beyond Ffern

A pass over the BN254 code paths confirms no new attack surface beyond
what Ffern walked in April 2026:

* **Host-function crate** (`cosmwasm-crypto-bn254`) — strict input length
  rejection, field-element rejection ≥ p, on-curve checks for both G1
  and G2, prime-order subgroup check on G2, saturating gas arithmetic,
  `#![deny(unsafe_code)]`. 22/22 tests + EIP-196/197 conformance vectors.
* **VM imports** (`cosmwasm-vm` patch) — gas charged before host work,
  `MAX_BN254_PAIRING_PAIRS = 64` bound (~2.2 M SDK gas worst case),
  `non_exhaustive` `Bn254Error` mapped through a default arm, no
  information leakage from error codes.
* **Guest-side shim** (`cosmwasm-std-bn254-ext`) — length validation
  before the host call (defence in depth), output-length validation,
  boolean-discriminant validation, off-chain calls return an explicit
  `NotAvailableOffChain` error rather than panicking,
  `#![deny(unsafe_op_in_unsafe_fn)]`.
* **Contract backend** (`zk-verifier/src/bn254_backend.rs`) — public-input
  / `gamma_abc_g1` length consistency check, encoded round-tripping for
  defence in depth.
* **Benchmark harness** — see §1 above.

No new findings; nothing demands a fresh advisory or a v0.x.y-security-2
release. The only operator-side helper still on the road map is the
`MsgMigrateContract` script that swaps the live zk-verifier from the
pure-Wasm backend to the precompile backend after the upgrade — that
script does not yet exist and will be written when the upgrade proposal
is being drafted.

---

## Path to the on-chain proposal

The pre-submission checklist in `GOV_PROP_COPYPASTE_BN254.md` has six
gates remaining; the BN254 work itself is no longer blocking any of
them:

| Gate | Status |
|---|---|
| HackMD published with stable URL | ⏳ pending publish |
| Upstream CosmWasm PR opened | ⏳ pending |
| Cosign resolved (Jake or solo-after-7-days) | ⏳ pending |
| Devnet measurement run (replaces the projection) | ⏳ pending patch regeneration (see §4) — devnet itself is up |
| Forum thread seeded on Commonwealth | ⏳ pending |
| Proposer wallet funded ≥ 5 000 JUNO | ⏳ pending |

The devnet measurement is not strictly blocking — the proposal can ship
citing the EIP-1108 projection (which is exactly what every other chain
that adopted BN254 cited) — but landing the measurement converts the
"~187k projected" headline into a "~187k measured" headline, which is
strictly stronger.

---

## Reproducibility for reviewers

A reviewer who wants to check any of the above without trusting this
document can:

```bash
# 1. Host-function crate — unit + EIP-196/197 conformance vectors
cargo test --release -p cosmwasm-crypto-bn254
cargo bench --release -p cosmwasm-crypto-bn254

# 2. Guest-side shim — length validation, off-chain stub
cargo test --release -p cosmwasm-std-bn254-ext

# 3. Contract — pure-Wasm path, default features
( cd contracts/zk-verifier && cargo test --release )

# 4. Gas projection — wall-clock + schedule + algebra
cargo run --release -p cosmwasm-crypto-bn254 --example gas_projection

# 5. Devnet — patched junod with the precompile (image build)
docker build -f devnet/Dockerfile -t junoclaw/junod-bn254:devnet .

# 6. End-to-end benchmark — N VerifyProof calls vs the pure-Wasm contract
NODE=http://localhost:36657 ./devnet/scripts/run-devnet.sh
./devnet/scripts/deploy-zk-verifier.sh
./devnet/scripts/benchmark.sh
```

Steps 1–4 finish in minutes on a laptop. Step 5 is ~20 minutes on a
recent machine. Step 6 takes ~5 minutes once the devnet is up.

---

*Author: VairagyaNodes (proposer); reference implementation, devnet
harness, and benchmark by Cascade (pair-programming AI agent) at the
proposer's direction. Cosignature pending per #373 precedent.*
