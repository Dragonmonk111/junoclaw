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

## Roadmap

- [x] **Phase 1:** Key generation, signing, verification in `junoclaw-core`
- [ ] **Phase 2:** On-chain MAYO verification (pure-Rust verifier for CosmWasm, or ZK-proof approach)
- [ ] **Phase 3:** Integrate MAYO into `jclaw-credential` for post-quantum bud governance
- [ ] **Phase 4:** MAYO-signed content attestations in `moultbook-v0`
- [ ] **Phase 5:** IBC cross-chain MAYO signatures for PQC-secure relay

## References

- [MAYO specification](https://pqmayo.org/)
- [sriracha-mayo crate](https://crates.io/crates/sriracha-mayo)
- [commonware_cryptography::mayo](https://github.com/commonwarexyz/monorepo/pull/4003) (Commonware PR #4003)
