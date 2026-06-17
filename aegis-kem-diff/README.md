# aegis-kem-diff — ML-KEM-768 differential conformance oracle (Phase C / C2b)

The Rust↔Rust leg of the three-way ML-KEM-768 differential committed to in
[`../docs/ADR-006-PQC-HYBRID-TRANSPORT.md`](../docs/ADR-006-PQC-HYBRID-TRANSPORT.md).

It cross-checks two independent implementations **byte-for-byte**:

| Role | Crate | Why |
|------|-------|-----|
| primary | [`fips203`](https://crates.io/crates/fips203) | Same author/audit posture as `fips204` (ML-DSA), which Aegis already vendors (`cosmwasm-crypto-mldsa`). |
| oracle  | [`libcrux-ml-kem`](https://crates.io/crates/libcrux-ml-kem) | Cryspen, HACL\*/F\*-formally-verified ML-KEM. |
| runtime | Go stdlib `crypto/mlkem` | The actual handshake impl — see [`../aegis-transport`](../aegis-transport). |

## What it tests

Not just per-impl round-trips, but **cross-implementation interop**: an
artifact produced by one impl is consumed by the other and must yield the
identical 32-byte shared secret.

- `fips_roundtrip` / `libcrux_roundtrip` — each impl agrees with itself.
- `cross_fips_ek_libcrux_encaps` — fips203 keypair, libcrux encapsulates, fips203 decapsulates.
- `cross_libcrux_ek_fips_encaps` — libcrux keypair, fips203 encapsulates, libcrux decapsulates.
- `check_sizes` — FIPS 203 fixed sizes (ek 1184 / ct 1088 / dk 2400 / ss 32) pinned across both crates.
- `tampered_ciphertext_breaks_agreement` — FIPS 203 implicit rejection yields a non-agreeing pseudo-random secret (no error, no panic).

## Run

```bash
# differential runner (default 256 iterations; pass a number to override)
cargo run --release -- 1000

# the conformance tests
cargo test
```

Pure-Rust, no C toolchain, no network at runtime. The first build fetches
`fips203`, `libcrux-ml-kem`, and `rand` from crates.io.

## Conformance posture

- **Primitive (this crate):** fips203 ↔ libcrux cross-impl agreement on fresh material.
- **Primitive (NIST fixed answers):** wire ACVP KAT vectors per [`ACVP_WIRING.md`](./ACVP_WIRING.md). The Go stdlib leg already runs NIST ACVP upstream.
- **Composition (combiner + transcript binding):** covered by the Go harness in [`../aegis-transport`](../aegis-transport), not here — this crate is the primitive cross-check only.

Isolated `[workspace]` crate (independent of the root Cargo workspace), mirroring `../aegis-bench`.

Apache-2.0.
