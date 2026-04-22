# Build & test recipe

This document is the reproducibility contract for everything in
`wasmvm-fork/`. If the commands below don't all pass, the
upstream PR is not ready.

All paths are relative to the `junoclaw/` repo root.

---

## 0. Toolchain

```bash
# Rust
rustup toolchain install 1.78.0
rustup default 1.78.0

# Wasm target (for the guest-side zk-verifier build)
rustup target add wasm32-unknown-unknown

# Go (for the wasmvm FFI patches)
# Use 1.22+ to match CosmWasm/wasmvm main.
go version    # expect >= 1.22.0
```

---

## 1. The Rust host-function crate

```bash
cd wasmvm-fork/cosmwasm-crypto-bn254

# Unit tests (includes round-trip and bilinearity identities).
cargo test

# Conformance vectors (public-API surface only — the shape cosmwasm-vm will
# call into).
cargo test --test vectors

# no_std verification — ensures the crate can be vendored into
# cosmwasm-vm's Wasm build without pulling in std.
cargo build --no-default-features

# Benchmarks (Criterion). Captures the numbers we cite in the governance
# proposal at `docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md`.
cargo bench
```

### Expected benchmark output (reference, 2023 M2 Pro, rustc 1.78, release)

| Benchmark                            | Median runtime |
|--------------------------------------|----------------|
| `bn254_add (G + G)`                  | ~4 µs          |
| `bn254_scalar_mul (k·G, k=u64::MAX)` | ~85 µs         |
| `bn254_pairing_equality (3 pairs)`   | ~2.2 ms        |

The headline number is the last one. At our pinned gas schedule of
`147 000 SDK gas` for a 3-pair pairing, and wasmd's default
`gas-per-ms ≈ 1 000 000`, the 2.2 ms measurement corresponds to **2.2 M
native-time gas**, which is comfortably inside the 147 M-gas budget we
charge in `src/gas.rs` — a 66× cushion against adversarial inputs.

---

## 2. Patch application (CosmWasm / wasmvm fork)

```bash
# Clone upstream at the versions we target.
git clone --branch v2.2.0 https://github.com/CosmWasm/cosmwasm ../cosmwasm-bn254
git clone --branch v2.2.0 https://github.com/CosmWasm/wasmvm     ../wasmvm-bn254

# Apply the patches.
( cd ../cosmwasm-bn254 && \
    cp -r ../junoclaw/wasmvm-fork/cosmwasm-crypto-bn254 packages/crypto-bn254 && \
    git apply ../junoclaw/wasmvm-fork/patches/cosmwasm-vm.imports.rs.patch && \
    git apply ../junoclaw/wasmvm-fork/patches/cosmwasm-std.imports.rs.patch && \
    git apply ../junoclaw/wasmvm-fork/patches/cosmwasm-std.traits.rs.patch && \
    cargo test -p cosmwasm-vm )

( cd ../wasmvm-bn254 && \
    git apply ../junoclaw/wasmvm-fork/patches/wasmvm.api.rs.patch && \
    git apply ../junoclaw/wasmvm-fork/patches/wasmvm.lib.go.patch && \
    make build-rust && \
    make test )
```

---

## 3. Private devnet (path B)

```bash
cd devnet
./scripts/run-devnet.sh         # boots a single-validator junod with the
                                # patched wasmvm in the background
./scripts/deploy-zk-verifier.sh # uploads + instantiates the precompile
                                # variant of zk-verifier
./scripts/benchmark.sh          # runs 50 VerifyProof txs + captures gas
```

The benchmark script writes its output to
`docs/BN254_BENCHMARK_RESULTS.md`, which is the artefact the Juno
governance proposal points at.

---

## 4. CI matrix (proposed)

The upstream PR carries a GitHub Actions matrix covering:

- `cargo test --no-default-features`
- `cargo test --all-features`
- `cargo clippy -- -D warnings`
- `cargo bench --no-run` (compile check only, no run cost)
- `cargo deny check` (supply-chain sanity, arkworks pinned)
- Go tests against the patched `wasmvm` (`make test`)
- Differential test: run 1 000 random Groth16 proofs through both the
  pure-Wasm `zk-verifier` and the precompile variant, assert identical
  accept/reject decisions on every proof.

---

## 5. Troubleshooting

**Crate fails to build under `--no-default-features`:**
Likely a new `std::` reference leaked in. Check `grep -R 'std::' src/`.

**Bilinearity test fails:**
Usually an encoder/decoder coordinate-order mismatch for G2. EIP-197
orders coordinates `(x.c1, x.c0, y.c1, y.c0)` which is reversed from
arkworks' native `(c0, c1)` pair layout; see `decode_g2` / `encode_g2`.

**Criterion benchmark times wildly larger:**
Ensure the `release` profile is used and that arkworks is compiled with
the `asm` feature (on by default under `curve`). On a machine without
BMI/ADX the bench can be 3× slower — flag this in the proposal rather
than hiding it.
