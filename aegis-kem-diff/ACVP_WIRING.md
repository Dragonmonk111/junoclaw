# ACVP / KAT wiring note — ML-KEM-768 (Project Aegis Phase C / C2b)

This note records *exactly* how to wire NIST's official Known-Answer Tests into
the differential oracle, so the `fips203` ↔ `libcrux-ml-kem` cross-check (this
crate) and the Go stdlib runtime (`../aegis-transport`) are all pinned to the
same authoritative vectors. It is a wiring **specification**, kept small on
purpose: the harness stays dependency-light by default and only pulls a JSON
parser behind a feature flag when ACVP mode is actually exercised.

## What ACVP covers vs. what this crate covers

| Layer | Conformance source | Status |
|-------|--------------------|--------|
| ML-KEM-768 primitive (keyGen, encap, decap) | **NIST ACVP KAT vectors** | wire per below |
| `fips203` vs `libcrux-ml-kem` interop (cross-impl byte agreement) | **this crate** (`src/main.rs`) | ✅ implemented |
| Go stdlib `crypto/mlkem` primitive | Go standard library's own ACVP test suite (`crypto/internal/fips140test`) | ✅ inherited upstream |
| ADR-006 combiner + transcript binding (composition) | **`../aegis-transport`** (Go) | ✅ implemented |

ACVP validates each implementation against NIST's fixed answers; the
cross-check in this crate validates that the two Rust impls agree with *each
other* on freshly-generated material (catching divergences ACVP's fixed inputs
might miss, e.g. serialization edge cases). They are complementary.

## Vector source (authoritative)

NIST **ACVP-Server** demo vectors, `ML-KEM-keyGen-FIPS203` and
`ML-KEM-encapDecap-FIPS203`:

- Repo: `https://github.com/usnistgov/ACVP-Server`
- Path: `gen-val/json-files/ML-KEM-keyGen-FIPS203/` and
  `gen-val/json-files/ML-KEM-encapDecap-FIPS203/`
- Each test group fixes `parameterSet: "ML-KEM-768"`. Use only those groups.

Pin a commit hash when vendoring; do **not** track `main` (vectors get
regenerated). Drop the two `prompt.json` + `expectedResults.json` pairs under
`vectors/` (git-ignored; fetched in CI).

## Field mapping

### keyGen (`ML-KEM-keyGen-FIPS203`)
- Input: `d` (32 B hex), `z` (32 B hex) → seed = `d || z` (64 B).
  - `libcrux`: `mlkem768::generate_key_pair(seed_64)`.
  - `fips203`: use the deterministic `try_keygen_with_rng` seeded so the DRBG
    emits `d` then `z` (fips203 draws `d` then `z`), **or** validate via the
    derived `ek`/`dk` from the byte-exact path. The simplest robust check is to
    feed `seed` to libcrux and compare libcrux's `ek`/`dk` to the ACVP
    `expectedResults` (`ek`, `dk` hex), then separately confirm `fips203`
    parses those same `ek`/`dk` bytes and round-trips.
- Expected: `ek` (1184 B), `dk` (2400 B). Assert byte-equality.

### encapDecap (`ML-KEM-encapDecap-FIPS203`)
- `function: "encapsulation"`: input `ek` (1184 B), `m` (32 B randomness).
  Expected `c` (1088 B ciphertext), `k` (32 B shared secret).
  - `libcrux`: `mlkem768::encapsulate(&MlKem768PublicKey::from(ek), m)` →
    assert `c` and `k` match.
- `function: "decapsulation"`: input `dk` (2400 B), `c` (1088 B). Expected `k`.
  - `libcrux`: `mlkem768::decapsulate(&dk, &c)` → assert `k`.
  - `fips203`: `DecapsKey::try_from_bytes(dk).try_decaps(CipherText::try_from_bytes(c))`
    → assert `k`. (Covers the FIPS 203 implicit-rejection path too — ACVP
    includes invalid-ciphertext cases whose `k` is the pseudo-random reject
    value.)

## Implementing it (when needed)

1. Add behind a feature so the default build stays light:
   ```toml
   [features]
   acvp = ["dep:serde", "dep:serde_json", "dep:hex"]

   [dependencies]
   serde      = { version = "1", features = ["derive"], optional = true }
   serde_json = { version = "1", optional = true }
   hex        = { version = "0.4", optional = true }
   ```
2. Add `src/acvp.rs` (gated `#[cfg(feature = "acvp")]`) that:
   - reads `prompt.json` + `expectedResults.json`,
   - filters `parameterSet == "ML-KEM-768"`,
   - runs the field mapping above through **both** crates,
   - asserts every `ek`/`dk`/`c`/`k` matches the expected hex.
3. CI step: `cargo test --features acvp` after fetching the vectors.

Until then, the dependency-free differential in `src/main.rs` plus the Go
stdlib's upstream ACVP suite already give three-way coverage of the primitive;
this note is the contract for tightening it to fixed NIST answers.
