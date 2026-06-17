# MAYO Post-Quantum Signature Integration

JunoClaw integrates [MAYO](https://pqmayo.org/) (Multivariate quAdratic hash-based signature sYstem), a NIST PQC Round 4 candidate, via the [`sriracha-mayo`](https://crates.io/crates/sriracha-mayo) crate.

## Why MAYO?

| Property | Value |
|----------|-------|
| **Security basis** | Oil & Vinegar multivariate quadratic equations |
| **NIST round** | 4 (final) |
| **Signature size** | 186–964 bytes (smallest: MAYO-2) |
| **Public key** | 1,420–5,554 bytes |
| **Secret key** | 24–40 bytes (compact seed) |

MAYO-2 is optimal for on-chain use: **186-byte signatures** minimize tx gas, at the cost of a larger 4,912-byte public key.

## Build Requirements

`sriracha-mayo` builds the reference MAYO-C implementation at compile time.

**Required tools:**
- CMake ≥ 3.15
- C compiler (MSVC on Windows, GCC/Clang on Linux/macOS)

**Not available on:** `wasm32` targets (no C toolchain in wasm32-unknown-unknown).

### Installing Prerequisites

**Windows (PowerShell as Admin):**
```powershell
winget install Kitware.CMake
# Or via Visual Studio Installer: "Desktop development with C++"
```

**Ubuntu/Debian:**
```bash
sudo apt-get install cmake build-essential
```

**macOS:**
```bash
brew install cmake
```

## Enabling MAYO in JunoClaw

The `mayo` feature flag gates all MAYO functionality:

```bash
# Check compilation (without MAYO)
cargo check -p junoclaw-core

# Build with MAYO support
cargo build -p junoclaw-cli --features mayo

# Run tests
cargo test -p junoclaw-core --features mayo
```

## CLI Usage

Generate a MAYO-2 keypair (recommended for on-chain):

```bash
junoclaw keygen mayo2
```

Output (JSON):
```json
{
  "variant": "Mayo2",
  "secret": "a1b2c3...",
  "public": "d4e5f6..."
}
```

## Architecture

### Phase 1: CLI/Local (C-based)

```
┌─────────────────────────────────────────────────────────────┐
│                        JunoClaw Agent                        │
├─────────────────────────────────────────────────────────────┤
│  CLI (junoclaw keygen)  →  junoclaw-core::mayo              │
│                                              │               │
│                                              ▼               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ MAYO Module (feature-gated: mayo)                    │   │
│  │  • generate_keypair(variant) → MayoKeypair            │   │
│  │  • sign(keypair, namespace, msg) → MayoSignature      │   │
│  │  • verify(variant, pubkey, namespace, msg, sig)       │   │
│  └──────────────────────────────────────────────────────┘   │
│                          │                                   │
│                          ▼                                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ sriracha-mayo → MAYO-C (C library, built by cmake)  │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Phase 2: On-Chain (Pure Rust)

```
┌─────────────────────────────────────────────────────────────┐
│                    jclaw-credential (CosmWasm)               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────────┐      ┌─────────────────────────┐ │
│  │ Bud {               │      │ VerifyMayoAttestation { │ │
│  │   parent, child,    │ ──▶  │   addr, message,       │ │
│  │   child_weight,    │      │   signature,            │ │
│  │   mayo_pk: <4,912B>│      │   public_key            │ │
│  │ }                   │      │ }                       │ │
│  │                     │      │                         │ │
│  │ contract:           │      │ contract:               │ │
│  │   verify length     │      │   1. Load stored hash   │ │
│  │   SHA-256 → 64-hex  │      │   2. Hash public_key    │ │
│  │   store hash         │      │   3. Mayo2::verify()   │ │
│  └─────────────────────┘      │   4. Accept/reject      │ │
│                               └─────────────────────────┘ │
│                                        │                    │
│                                        ▼                    │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ junoclaw-mayo-verify (pure Rust, #![no_std], no C)   │  │
│  │  • gf16.rs — GF(16) arithmetic                       │  │
│  │  • verify.rs — AES-128-CTR + SHAKE256 + P signature   │  │
│  │  • params.rs — MAYO-2 parameters (n=66, m=64, etc.)  │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**Why hash-stored PK?** The full 4,912 B public key is too large for permanent contract storage. We store a 32-byte SHA-256 hash (cheap, permanent). The actual public key travels in the transaction payload at verification time — same pattern as Bitcoin P2PKH and Ethereum AA.

## Domain Separation

JunoClaw uses the **Commonware convention** for message binding:

```
payload = namespace || 0x00 || msg
```

This prevents cross-protocol signature replay (e.g., a moultbook attestation cannot be replayed as a governance vote).

## Parameter Sets

| Variant | NIST Level | Secret | Public Key | Signature | Best For |
|---------|-----------|--------|------------|-----------|----------|
| `mayo1` | 1 | 24 B | 1,420 B | 454 B | Balanced |
| `mayo2` | 1 | 24 B | 4,912 B | **186 B** | **On-chain** |
| `mayo3` | 3 | 32 B | 2,986 B | 681 B | High security |
| `mayo5` | 5 | 40 B | 5,554 B | 964 B | Maximum security |

## Live Testnet Results (uni-7, 2026-06-12)

| Operation | Gas Used | Cost (0.075 ujunox) |
|-----------|----------|---------------------|
| Bud (create member + MAYO PK) | 336,659 | ~25.2 ujunox |
| VerifyMayoAttestation (valid) | 355,771 | ~26.7 ujunox |
| VerifyMayoAttestation (tampered) | Rejected ✅ | — |

**Contract**: `jclaw-credential` at `juno1z2w067ptpn2f6zpwt207je0kqeqc2eek7jf4p4dpztf24zncnhzqz5el2r`

**Memory**: Pure-Rust verifier peak ~127 KB in wasm32 (within CosmWasm's 512 KB limit).

## Devnet Precompile Results (junoclaw-bn254-1, 2026-06-17)

On our MAYO-patched Juno fork (wasmvm-fork patches `10-19` add a native
`mayo_verify` host function), the same `VerifyMayoAttestation` runs 1.15-2.21×
cheaper. Whole-tx gas, pure-Wasm → precompile (reproduced within ~0.03% of the
2026-06-14 run on a fresh chain after a full devnet reset):

| Variant | NIST | Pure-Wasm | Precompile | Speedup |
|---------|------|----------:|-----------:|--------:|
| MAYO-2 | L1 | 355,932 | 310,391 | 1.15× |
| MAYO-3 | L3 | 456,644 | 257,371 | 1.77× |
| MAYO-5 | L5 | 798,137 | 360,902 | 2.21× |

The win grows with the parameter set: at L5 (Falcon-equivalent security) the
precompile more than halves the cost. It is short of the original 7× projection
because, once the crypto is native, fixed CosmWasm/SDK tx overhead dominates.
Full write-up + tx hashes: `docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md`.

## Roadmap

- [x] **Phase 1:** Key generation, signing, verification in `junoclaw-core` (C-based, CLI)
- [x] **Phase 2:** On-chain MAYO verification — pure-Rust `junoclaw-mayo-verify` crate, `#![no_std]`, wasm32-compatible
- [x] **Phase 3:** Integrate MAYO into `jclaw-credential` — `Bud` with `mayo_pk`, `VerifyMayoAttestation`, `MayoPkHash` query
- [ ] **Phase 4:** MAYO-signed content attestations in `moultbook-v0`
- [x] **Phase 5:** MAYO-3 / MAYO-5 support — L3/L5 verified on-chain (457k/799k gas pure-Wasm; precompile 257k/361k). Done 2026-06-17.
- [ ] **Phase 6:** ZK-proof of MAYO verification — Groth16/BN254 circuit for cross-chain portability
- [ ] **Phase 7:** IBC cross-chain MAYO signatures for PQC-secure relay

## References

- [MAYO specification](https://pqmayo.org/)
- [sriracha-mayo crate](https://crates.io/crates/sriracha-mayo)
- [commonware_cryptography::mayo](https://github.com/commonwarexyz/monorepo/pull/4003) (Commonware PR #4003)
