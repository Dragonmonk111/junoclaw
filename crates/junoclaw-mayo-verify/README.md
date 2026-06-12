# junoclaw-mayo-verify

Pure-Rust MAYO post-quantum signature verifier for CosmWasm (`no_std` + `alloc`).

This crate implements MAYO signature verification without any C dependencies, making it suitable for `wasm32-unknown-unknown` targets (e.g. CosmWasm smart contracts).

## Supported Parameter Sets

| Variant | Security Level | `n` | `m` | `o` | `k` | Signature | Public Key |
|---------|---------------|-----|-----|-----|-----|-----------|------------|
| MAYO-1  | NIST Level 1  | 86  | 78  | 18  | 4   | 340 B     | 5,720 B    |
| **MAYO-2** | **NIST Level 1** | **81** | **64** | **17** | **4** | **186 B** | **4,912 B** |
| MAYO-3  | NIST Level 3  | 122 | 96  | 24  | 5   | 491 B     | 10,641 B   |
| MAYO-5  | NIST Level 5  | 154 | 128 | 32  | 5   | 805 B     | 18,757 B   |

## Usage

```rust
use junoclaw_mayo_verify::Mayo2;

let message = b"hello world";
let signature = &[0u8; Mayo2::SIG_BYTES];   // replace with real signature
let public_key = &[0u8; Mayo2::PK_BYTES];  // replace with real public key

match Mayo2::verify(message, signature, public_key) {
    Ok(true)  => println!("Valid signature"),
    Ok(false) => println!("Invalid signature"),
    Err(e)    => println!("Malformed input: {}", e),
}
```

## Features

- `std` — enables `std` support in dependencies (`sha2`, `sha3`).
  Without this feature the crate is `no_std` + `alloc` and works on `wasm32-unknown-unknown`.

## Architecture

The verifier is organised into modules:

- `params` — parameter sets (`Mayo1`, `Mayo2`, `Mayo3`, `Mayo5`) and the `ParameterSet` trait.
- `gf16` — GF(16) arithmetic and m-vector operations (packed 4-bit nibbles in `u64` limbs).
- `verify` — public-key expansion, public-map evaluation (`S · P · S^t`), and the top-level `verify` function.
- `error` — error types for malformed inputs.

### Public-Key Expansion

MAYO uses compact public keys. The verifier expands them via AES-128-CTR (software implementation, no AES-NI required) and unpacks the m-vectors.

### Verification Steps

1. Expand the compact public key into `P1`, `P2`, `P3`.
2. Hash the message with SHAKE256.
3. Decode the signature vector `s`.
4. Compute `t = H(digest || salt)`.
5. Evaluate the public map: `y = P(s)` via `S · P · S^t`.
6. Accept iff `y == t`.

### Memory Usage (Optimised)

Peak heap allocations during MAYO-2 verification:

| Buffer | Size (bytes) | Notes |
|--------|-------------|-------|
| `p1_p2_packed` | 101 376 | AES-CTR output for P1+P2; freed after unpack |
| `p1` | 8 320 | Unpacked P1 m-vectors |
| `p2` | 4 352 | Unpacked P2 m-vectors |
| `p3` | 608 | Unpacked P3 m-vectors |
| `row_acc` (PS) | 2 048 | **Row-wise accumulator** (was 165 888 B) |
| `ps` | 10 368 | P * S^t result |
| `bins` (SPS) | 256 | **Per-pair accumulator** (was 4 096 B) |
| `sps` | 512 | S * PS result |
| `temp` | 72 | Stack array (was heap-allocated) |
| **Total peak** | **~127 KB** | Down from ~290 KB pre-optimisation |

## Testing

Run unit tests (no C toolchain required):

```bash
cargo test -p junoclaw-mayo-verify
```

Cross-check against the reference C implementation (`sriracha-mayo`) requires CMake:

```bash
cargo test -p junoclaw-mayo-verify --features test-c
# or use the helper script:
./scripts/cross-check.sh
```

## License

Apache-2.0
