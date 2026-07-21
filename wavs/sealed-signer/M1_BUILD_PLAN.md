# M1 Build Plan — WAVS WASI Sealed Signer

**Goal:** A self-contained WASI component that generates a Juno `secp256k1` key, seals it, derives a bech32 address, and signs arbitrary bytes. The raw key never leaves the component.

**Why this first:** If the signer works in WASI, we can later drop it into a WAVS workflow (M2) and a J-Lens probe (M3) without changing the crypto core.

---

## Deliverables

1. `wavs/sealed-signer/` — new `cargo-component` crate.
2. WIT interface exposing `generate-key` and `sign`.
3. Rust implementation using `k256` + `sha2` + `ripemd160` + `bech32`.
4. Simulated sealing: the private key is encrypted with a host-supplied passphrase and returned as an opaque sealed blob. The decrypted key is cached in memory for the component lifetime.
5. `cargo component build --release` succeeds and produces `junoclaw_sealed_signer.wasm`.
6. Host test script that runs the component with `wasmtime`, requests a signature, and verifies it against the derived address.

---

## File layout

```
wavs/sealed-signer/
├── .cargo/config.toml
├── Cargo.toml
├── wit/
│   └── sealed-signer.wit
├── src/
│   ├── lib.rs
│   └── crypto.rs
└── scripts/
    └── run-component-test.js
```

---

## WIT interface

```wit
package junoclaw:sealed-signer;

interface signer {
    /// Generate a fresh secp256k1 key from the supplied 32-byte seed, seal it with
    /// the passphrase, and return the Juno bech32 address plus an opaque sealed blob.
    generate-key: func(seed: list<u8>, passphrase: string) -> result<key-info, string>;

    /// Decrypt the sealed blob, sign the message (SHA-256), and return
    /// the 64-byte R||S signature as hex.
    sign: func(message: list<u8>, sealed-blob: list<u8>, passphrase: string) -> result<sign-info, string>;

    record key-info {
        address: string,
        pubkey: string,
        sealed-blob: list<u8>,
    }

    record sign-info {
        address: string,
        pubkey: string,
        signature: string,
    }
}

world sealed-signer-world {
    export signer;
}
```

---

## Crypto logic

1. **Key generation**
   - `secret = sha256(seed)`.
   - `signing_key = k256::ecdsa::SigningKey::from_bytes(secret)`.
   - The decrypted signing key is cached in a `Mutex<Option<SigningKey>>` for the component lifetime.

2. **Sealing (M1 simulation)**
   - Derive an AES-256-GCM key from the passphrase using PBKDF2-SHA256.
   - Salt and nonce are deterministically derived from `secret || passphrase`.
   - Return the encrypted secret as an opaque blob (`salt || nonce || ciphertext`).
   - In production the passphrase will be replaced by the TEE sealing key from the WAVS sidecar.

3. **Address derivation**
   - `compressed_pubkey = signing_key.verifying_key().to_encoded_point(true)`
   - `addr_bytes = ripemd160(sha256(compressed_pubkey))`
   - `bech32("juno", addr_bytes)`

4. **Signing**
   - ECDSA sign the raw message with `k256` (which hashes with SHA-256 internally).
   - Normalize to low-S.
   - Serialize as 64-byte `r || s`.

---

## Build steps

```bash
cd wavs/sealed-signer
cargo component build --release
```

Expected output:
```
target/wasm32-wasip1/release/junoclaw_sealed_signer.wasm
```

## Test steps

1. Install a WASI host if not present:
   ```bash
   cargo install wasmtime-cli
   ```

2. Run unit tests on the host (override the default wasm target):
   ```bash
   cd wavs/sealed-signer
   cargo test --target x86_64-pc-windows-msvc
   ```

3. Build the component:
   ```bash
   cargo component build --release
   ```

4. Run the end-to-end host test:
   ```bash
   node scripts/run-component-test.js
   ```

   The script will:
   - call `generate-key([0;32], "secret")` via `wasmtime`,
   - call `sign(b"hello", sealed-blob, "secret")`,
   - verify the signature with Node.js `crypto`,
   - print the Juno address.

---

## Known blockers to track

1. **True TEE sealing:** M1 uses host-passphrase encryption. Replacing it with WAVS/TEE sealing is M1.5.
2. **Host-provided seed:** M1 accepts a 32-byte seed from the host. In M2 this will be replaced by on-enclave randomness.
3. **In-memory cache:** The decrypted signing key lives only in component memory. A real deployment must combine TEE sealing with the sealed blob so the key can survive enclave restart.

---

## Success criteria

- [x] `cargo component build --release` produces `junoclaw_sealed_signer.wasm`.
- [x] Host test prints a valid `juno1...` address.
- [x] Signature verifies against that address.
- [x] Sealed blob decrypts correctly and the same key signs deterministically.

---

## M1.5 status (standalone signer + verifier co-location)

**Part A — standalone `junoclaw:sealed-signer` hardening (done):**
- `generate-key` now takes no seed; entropy comes from `wasi:random/random::get-random-bytes(32)`.
- Passphrase is read from `WAVS_ENV_SIGNER_PASSPHRASE` inside the component, not passed as an argument.
- Interface stayed **stateless**: `generate-key` returns the sealed blob to the caller, `sign` takes the sealed blob back in. No `wasi:keyvalue` in this standalone crate — persistence is the caller's problem (see Part B).
- `secret_from_seed` in `src/crypto.rs` is now `#[cfg(any(test, not(target_arch = "wasm32")))]`-gated; only used by host unit tests.
- `scripts/run-component-test.js` updated: no seed arg, passphrase via `--env WAVS_ENV_SIGNER_PASSPHRASE=secret`, and it asserts two `generate-key` calls produce **different** addresses (proves real randomness).

**Part B — co-located signing in `junoclaw:verifier` (done):**
- New `junoclaw/wavs/src/sealed_signer.rs` module: same crypto core, but this one *does* persist via `wasi:keyvalue::store` (`open("sealed-signer")`, key `"sealed-key"`), because the verifier component is long-lived across triggers in the same workflow.
- `generate_key()` — random secret, seal with `WAVS_ENV_SIGNER_PASSPHRASE`, `kv_save`, cache in `Mutex<Option<(SigningKey, Vec<u8>)>>`.
- `sign(message)` — `kv_load`; if nothing persisted yet, lazily calls `generate_key()` first (so the very first `SignMoultbookExport` trigger self-provisions a key).
- New trigger variant `VerificationTask::SignMoultbookExport { export_json }` in `src/trigger.rs`, parsed when the cosmos event type contains `sign_moultbook_export`, reading an `export_json` event attribute.
- New processor `process_sign_moultbook_export` in `src/lib.rs`: SHA-256(export_json) -> sign digest -> `VerificationResult` with `signer_address`, `signer_pubkey`, `signature`.
- `service.json` gained a `sign-moultbook-export` workflow, `event_type: "wasm-sign_moultbook_export"`, `env_keys: ["WAVS_ENV_SIGNER_PASSPHRASE"]`, `allowed_http_hosts: []` (no network needed for signing).
- `junoclaw/wavs/Cargo.toml` gained `k256`, `aes-gcm`, `pbkdf2`, `ripemd`, `bech32` deps (mirrors sealed-signer crate's crypto deps).
- Verified: `cargo component build --release` succeeds, `cargo test --target x86_64-pc-windows-msvc` passes including 3 new `sealed_signer::tests`.

**Part C — runtime experiment findings (open risks investigated):**

1. **`wasi:keyvalue` TEE encryption-at-rest — still unverified.** Found no documentation confirming the WAVS runtime encrypts `wasi:keyvalue` data at rest inside the TEE. The `wasi:keyvalue/store` WIT spec itself is silent on encryption — it's host-implementation-defined. **Action needed:** ask the WAVS team directly, or treat the sealed blob's own AES-256-GCM encryption as the only guaranteed-at-rest protection (i.e., never store the raw secret in `wasi:keyvalue`, only the sealed/encrypted blob — which is what `sealed_signer.rs` already does).

2. **WAVS component composition (embedding sealed-signer inside verifier) — resolved via inlining, not wasm composition.** Rather than using `wasm-tools compose` to link the standalone `junoclaw:sealed-signer` component into `junoclaw:verifier`, we duplicated the crypto logic directly into `junoclaw/wavs/src/sealed_signer.rs`. This avoids unresolved questions about whether WAVS's operator runtime supports component-to-component calls / composed components at all. Trade-off: crypto logic now lives in two places (standalone crate for isolated testing, verifier module for production) and must be kept in sync manually.

3. **`wasm32-wasip1` vs `wasm32-wasip2` — resolved and fixed, use `wasip2`, and it's not just cosmetic.** `cargo-component-component 0.21.1` ignores the `target` key in `.cargo/config.toml` and always defaults to `wasm32-wasip1` unless `--target wasm32-wasip2` is passed explicitly on the CLI. Diffing `wasm-tools component wit` output between the two builds of **both** `junoclaw_wavs_component.wasm` and `junoclaw_sealed_signer.wasm` found a real functional difference, not just a version bump:
   - The default `wasm32-wasip1` build imports `wasi:filesystem/types` and `wasi:filesystem/preopens` — pulled in by the preview1-to-preview2 compatibility adapter — even though neither component touches the filesystem anywhere in the Rust source.
   - The explicit `wasm32-wasip2` build has **no filesystem imports at all**, and is otherwise interface-identical (same `wavs:operator` types, same exported `run` function, same `wasi:keyvalue`/`wasi:random` imports on the verifier).
   - `junoclaw/wavs/service.json` sets `"file_system": false` on every workflow. A component that silently imports the filesystem interface while the manifest declares no filesystem permission is exactly the kind of mismatch that can either get rejected by the WAVS runtime's permission check or, worse, get granted implicitly and widen the trust boundary of a supposedly network/fs-free signer.
   - **Fix applied:** `@/c:/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/wavs/README.md` build section and this crate's `.cargo/config.toml` now call out/target `wasm32-wasip2`; `scripts/run-component-test.js` now points at `target/wasm32-wasip2/release/junoclaw_sealed_signer.wasm` by default. Re-ran the full non-determinism + sign/verify test against the `wasip2` build — identical behavior, confirmed working, zero filesystem imports.
   - **Action for every future build/deploy of either component:** always run `cargo component build --release --target wasm32-wasip2`. Never rely on the bare `cargo component build --release` default.

**M1 / M1.5 determinism check (ran the actual experiment, not just reasoning about it):**
- Ran `scripts/run-component-test.js` twice against fresh builds (once wasip1, once wasip2). Two consecutive `generate-key()` invocations produced two different `juno1...` addresses both times — `wasi:random/random` is genuinely non-deterministic inside `wasmtime run`, no seeded/fixed-RNG surprise.
- `derive_salt_and_nonce` in `crypto.rs` derives the AES-GCM salt/nonce deterministically from `secret || passphrase`. This is **intentional, not a bug**: nonce reuse is impossible because each freshly-generated `secret` is only ever encrypted once, so a deterministic-but-unique-per-secret nonce is safe and makes `encrypt_key` reproducible for a given `(secret, passphrase)` pair, which is convenient for testing.
- No other deterministic leak found. The one real anomaly was the wasip1-vs-wasip2 filesystem-import difference above — that's a build-target artifact, not a crypto/logic bug.

**Remaining before this is fully "M1.5 done":**
- Get an authoritative answer on wasi:keyvalue-at-rest encryption from WAVS docs/team (item 1) — mitigated for now by only ever storing the AES-256-GCM sealed blob in `wasi:keyvalue`, never the raw secret (already true in `wavs/src/sealed_signer.rs::kv_save`).
- Decide whether to pursue true wasm composition later (item 2) or keep the duplicated-module approach permanently — currently keeping the duplicated module, since WAVS's component-to-component call support is unverified.
- ~~Force `cargo-component` to target `wasm32-wasip2` explicitly~~ — done, see item 3 above.
