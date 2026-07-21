# Phase 3 Endgame — TEE-Sealed Agent Signer + J-Lens Audit Layer

**Goal:** The Juno Agents DAO never relies on a plaintext mnemonic in a developer terminal. Every agent action — Moultbook posts, J-Lens snapshots, AI-generated commits — is signed inside a TEE and attested on-chain. The DAO controls which enclave measurements are authorized to act.

---

## 1. The end-state architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     DAO governance layer                     │
│  agent-company contract  ←  authorized TEE measurements    │
│  j-lens-bank contract    ←  DAO-governed forbidden concepts │
│  policy.json mandate     ←  all agent posts need attestation│
└──────────────────────┬──────────────────────────────────────┘
                       │
    ┌──────────────────┴──────────────────┐
    │                                       │
┌───▼──────────────┐            ┌───────────▼────────────┐
│  Indexer service │            │   Sealed signer service │
│     (WASI)       │            │        (WASI)          │
│  fetches DAO     │            │  holds secp256k1 key    │
│  proposals and   │            │  exposes sign(bytes)    │
│  calls signer    │            │                         │
└────────┬─────────┘            └───────────┬─────────────┘
         │                                  │
         │   ┌──────────────────┐           │
         │   │  J-Lens probe    │           │
         │   │     (WASI)       │           │
         │   │ audits model     │           │
         │   │ hidden states    │           │
         │   └──────────────────┘           │
         │                                  │
         └──────────────────┬───────────────┘
                            │
                    TEE quote / attestation
                            │
                    ┌───────▼────────┐
                    │   Moultbook     │
│  post {commitment, attestation_ref} │
└─────────────────────────────────────┘
```

**M1 shortcut:** The indexer and sealed signer can run as a single WAVS component so we only have one integration to test. M2 splits them into separate services with a narrow WIT interface (`sign(bytes) -> result<...>`).

### What each component does

| Component | Responsibility |
|---|---|
| **Sealed signer** | A dedicated WAVS service that holds the reply-bot signing key inside the TEE. Exposes a minimal `sign(bytes)` WIT function. Key is generated inside the enclave and sealed to the enclave identity. |
| **Proposal indexer** | Reads DAO DAO state, builds AKB exports, calls the sealed signer, posts to Moultbook, and emits an attestation. |
| **J-Lens probe** | Loads open-weight models, computes Jacobian lens snapshots for forbidden concepts, records them as TEE-attested traces. |
| **agent-company contract** | Maintains a registry of authorized TEE measurement hashes. Verifies attestations. |

---

## 2. Key management — no mnemonic in RAM outside the enclave

### Current state
- The reply-bot key is a BIP-39 mnemonic.
- It is loaded into Node memory for one-shot runs or long-lived servers.

### Endgame state
1. **Key generation inside the TEE**
   - The sealed signer component generates a Juno `secp256k1` key from entropy supplied by the TEE RNG (simulated in M1, real hardware RNG in M2+).
   - The private key never leaves the enclave.
   - The public address is exported and registered as the DAO's official agent signer.

2. **Sealing**
   - The key is encrypted with the enclave's sealing key and persisted to encrypted storage.
   - It can only be unsealed by an enclave with the exact same measurement.
   - Changing the code changes the measurement, which invalidates the old sealed key.

3. **DAO-controlled measurement registry**
   - The DAO votes on which TEE measurement hashes are authorized to sign as the agent.
   - `agent-company::SubmitAttestation` records the measurement hash on-chain.
   - A verifier checks that any Moultbook post claiming to be from the agent came from an authorized enclave.

4. **Fresh key per enclave with DAO registration**
   - Each authorized enclave instance generates its own key and is registered separately.
   - The DAO registers each address and measurement hash.
   - A future phase can add a 2-of-3 threshold or hybrid post-quantum layer once those protocols are integrated.

5. **Post-quantum migration gateway**
   - `agent-company` stores both a `secp256k1` pubkey and an optional `pqc_pubkey` slot.
   - The signer interface returns a signature format version so consumers can upgrade to hybrid PQ signatures without breaking existing posts.
   - Keep key derivation, address format, and signing envelope versioned so we can plug in Aegis/ML-DSA when the DAO votes for it.

---

## 3. J-Lens integration inside the TEE

### Why it belongs in the enclave
- Hidden-state snapshots are sensitive and must not be tampered with before being recorded.
- Running the probe inside the TEE produces a cryptographic attestation that a specific model/checkpoint produced a specific trace.

### Flow
1. An AI agent (e.g., Juno AI) prepares a draft commit or a Moultbook export.
2. The draft is sent to the TEE J-Lens probe along with the model checkpoint ID.
3. The probe loads the open-weight model, computes the Jacobian lens for a configured set of forbidden concepts, and captures the top-k verbalizable representations.
4. The probe outputs:
   - `snapshot_hash` — SHA-256 of the hidden-state snapshot.
   - `concept_report` — flagged concepts with confidence scores.
   - `model_checkpoint` — exact model and revision.
   - `tee_quote` — attestation quote from WAVS.
5. The indexer includes `attestation_ref = tee_quote` in the AKB export.
6. The DAO DAO policy can require `attestation_ref` on every AI-authored shared-root commit.

### Forbidden-concept bank (DAO-governed)
- Stored in a dedicated **`j-lens-bank`** contract.
- `agent-company` holds the authorized `j-lens-bank` address so probes cannot be pointed at an unvetted bank.
- Concepts are hashed to avoid model-specific prompt leakage.
- Updates require DAO vote.

---

## 4. On-chain verification path

```
Moultbook entry
  ↓ content.resolver fetches plaintext AKB export
  ↓ export.structured.attestation_ref = tee_quote
  ↓ query agent-company::VerifyAttestation { quote, measurement }
  ↓ contract checks measurement is in DAO-authorized registry
  ↓ consumer trusts the entry
```

### Contract additions needed

`agent-company` messages:

```rust
enum ExecuteMsg {
    RegisterMeasurement {
        role: String,        // e.g., "reply-bot", "j-lens-indexer"
        measurement: String, // TEE measurement hash
        pubkey: String,      // secp256k1 agent pubkey
    },
    SubmitAttestation {
        role: String,
        quote_hash: String,
        metadata: Binary,
    },
}

enum QueryMsg {
    IsAuthorized {
        role: String,
        measurement: String,
    },
    GetAgentPubkey {
        role: String,
    },
}
```

---

## 5. Milestone roadmap

### M1 — Sealed signer prototype (2–3 weeks)
- Build a WAVS WASI component that generates and seals a Juno secp256k1 key.
- Expose `generate-key`, `get-pubkey`, and `sign` via WIT.
- **Shortcut:** run inside WAVS without a real TEE first so we can iterate on the crypto and Moultbook integration without Nitro/SGX hardware.
- Confirm a Moultbook post from the enclave address in dry-run mode.

### M2 — Separate signer service + real TEE (2–3 weeks)
- Split the sealed signer into its own WAVS service with the narrow WIT interface `sign(bytes)`.
- Port `tools/proposal-indexer` to a WASI component that calls the signer service.
- Add AWS Nitro Enclaves support (Intel SGX as a follow-up target).
- Emit a WAVS attestation quote for each Moultbook post and record the quote hash on-chain via `agent-company::SubmitAttestation`.

### M3 — J-Lens probe in TEE (4–6 weeks)
- Integrate the **`burn`** model runtime inside a WASI component.
- Implement the Jacobian lens computation for single-token verbalizable representations.
- Connect the `j-lens-bank` contract and produce flagged snapshots.
- Link snapshots to Moultbook exports and commits.

### M4 — DAO policy + multi-enclave registration (3–4 weeks)
- Add measurement registry governance to `agent-company`.
- Deploy fresh-key-per-enclave signers across multiple clouds and register each measurement.
- Update A18c-9 policy to require TEE attestations for all AI-authored shared-root commits and Moultbook exports.

### M5 — Continuous audit dashboard (4–6 weeks)
- On-chain index of all TEE-attested agent actions.
- Explorer view: Moultbook entry → attestation → measurement → model checkpoint.
- Redmark flow: DAO can flag and revoke measurements if a probe is caught cheating.

---

## 6. Decisions resolved

| Question | Decision |
|---|---|
| TEE target first | ~~AWS Nitro Enclaves~~ **superseded by reality.** `TEE_MILESTONE_ARTICLE.md` documents that on 2026-03-17 the `junoclaw:verifier` component (compiled to `wasm32-wasip2`) already ran inside a real **Intel SGX** enclave on an **Azure DCsv3 confidential VM**, submitted an on-chain attestation (`agent-company` proposal 4, tx `6EA1AE79...D26B22`), then moved to a decentralized Akash deployment the same day. AWS Nitro was never actually used. **M2's real TEE target is Azure SGX** (proven) or reproducing that proof for the sealed-signer specifically — not a fresh Nitro integration. |
| Model runtime | **burn** — Rust-native autograd, compiles to WASI with the `NdArray` backend. |
| Signer placement | **Separate WAVS service** for production; M1 keeps it co-located as a build/integration shortcut. |
| Key model | **Fresh key per enclave** with DAO registration; threshold signing and Shamir schemes are deferred. |
| Forbidden-concept bank | Dedicated **`j-lens-bank`** contract, referenced by `agent-company`. |
| Article content | Final article will describe the chosen design only, not side-by-side comparisons with dropped alternatives. |

---

## 7. M1.5 — hardening + verifier co-location (done)

M1 shipped a seed-based, passphrase-argument signer. Before splitting the signer into its own WAVS service (M2), we hardened the M1 shortcut and proved it out inside the verifier component that will actually run in production:

- **Entropy:** `generate-key` no longer accepts a host-supplied seed. It pulls 32 bytes from `wasi:random/random` inside the component. Verified experimentally — two consecutive `generate-key()` calls against the same `.wasm` produce two different `juno1...` addresses.
- **Passphrase:** moved from a function argument to the `WAVS_ENV_SIGNER_PASSPHRASE` env var, matching how WAVS injects secrets via `env_keys` in `service.json`.
- **Co-location:** the signing logic now also lives inside `junoclaw:verifier` (`wavs/src/sealed_signer.rs`), with a new `SignMoultbookExport` trigger (`wasm-sign_moultbook_export` event) and a `sign-moultbook-export` workflow in `service.json`. This is the "M1 shortcut: single WAVS component" from section 1, now working end-to-end for signing (not yet for the indexer's HTTP fetch + sign combined flow).
- **Persistence:** the verifier module persists the sealed (encrypted) key via `wasi:keyvalue/store`, keyed `"sealed-signer"/"sealed-key"`. The raw secret never touches `wasi:keyvalue` — only the AES-256-GCM sealed blob does.
- **Build-target bug found and fixed:** `cargo-component build --release` (no explicit `--target`) silently compiles against `wasm32-wasip1`, whose preview1-compat adapter drags in unused `wasi:filesystem` imports — a mismatch against `service.json`'s `"file_system": false`. Building with `--target wasm32-wasip2` explicitly removes those imports entirely with no other interface change. **All future builds of both components must pass `--target wasm32-wasip2` explicitly.** Full details in `wavs/sealed-signer/M1_BUILD_PLAN.md`.
- **Open risk carried into M2:** whether WAVS's TEE actually encrypts `wasi:keyvalue` data at rest is unconfirmed by any docs found so far. Mitigated for now by the sealed-blob-only storage rule above; needs a direct answer from the WAVS team before this is a real security guarantee rather than a convention we're following.

## 8. M2 — separate signer service + real TEE (next)

M1.5 proved the crypto and the on-enclave RNG work. Before sequencing M2, two more facts came out of reading the actual production code (not just this draft):

- **`agent-company` is already far ahead of section 4's "Contract additions needed" list.** It already has `wavs_operator` (trusted attestation-submitter address, admin/governance-rotatable), `SubmitAttestation` with optional Groth16 proof cross-verification via a `zk_verifier` contract, `SubmitRandomness`, and full attestation storage/query. There is no separate "measurement registry" (`RegisterMeasurement`/`IsAuthorized`) — trust today is anchored to the `wavs_operator` **address**, not an SGX MRENCLAVE hash. That's a real, already-working, simpler trust model than section 4 proposed; M2 should extend it rather than build a parallel registry from scratch.
- **`tools/proposal-indexer` doesn't sign arbitrary bytes — it signs full Cosmos SDK transactions.** `tools/reply-bot/src/moultbook.js` uses `DirectSecp256k1HdWallet.fromMnemonic()` with `SigningCosmWasmClient.execute()`, which builds a `SIGN_MODE_DIRECT` protobuf `SignDoc` (account number, sequence, chain-id, fee, msgs) and signs *that*, not a raw digest. Our sealed signer's `sign(bytes) -> signature` (M1/M1.5) is **not sufficient** to replace this wallet as-is — it would need to construct and sign a correct Cosmos `SignDoc`, which is a materially bigger, security-sensitive scope (get the sign bytes wrong and transactions either fail or — worse — are malleable/replayable).

Revised concrete steps, in order:

1. **Confirm the `wasi:keyvalue` encryption-at-rest question** with the WAVS team/docs before relying on it for anything beyond the sealed blob. Unblocked today, no code dependency.
2. **Decide on real component composition vs. continued duplication** for the signer/indexer split (M1.5 Part C item 2 still open). Given the unresolved composition question, default to two independently-deployed WAVS services communicating only through on-chain state / Moultbook posts.
3. **Scope decision needed before writing tx-signing code:** does the sealed signer need to produce full Cosmos `SIGN_MODE_DIRECT` transaction signatures (to fully replace `JUNO_REPLY_BOT_MNEMONIC`), or is the nearer-term goal narrower — e.g. only sign the AKB export payload digest (as `SignMoultbookExport` already does today) while a separate, still-mnemonic-holding broadcaster wraps it in a tx? The former removes the plaintext mnemonic entirely (the actual PHASE3 goal) but is a much larger, security-critical build. **Flagging this rather than guessing** — see below.
4. **Extend `agent-company`, not a fresh registry:** once (3) is resolved, add the enclave's sealed-signer pubkey/address as a second trusted role alongside `wavs_operator` (e.g. reuse the existing rotatable-address pattern) rather than inventing `RegisterMeasurement`/`IsAuthorized` from section 4 — it duplicates trust anchoring that already exists and works in production.
5. **Reproduce (or extend) the Azure SGX proof for the sealed-signer specifically.** The verifier component has already run inside Intel SGX on Azure DCsv3 and been proven on-chain (section 6). M2 doesn't need to stand up new TEE infra — it needs to confirm the sealed-signer's `wasi:random`/`wasi:keyvalue` behavior holds inside that same SGX enclave (re-run the M1.5 determinism + filesystem-import checks there, not just locally in `wasmtime run`).
6. **Emit and record a real attestation quote** per Moultbook post once (3) and (4) land: extend `SubmitAttestation` (or a sibling message) to carry the quote hash/`attestation_ref`.

**Step 3 resolved: (b) Full scope**, chosen by the user. Built and tested this session:

- `wavs/sealed-signer/src/crypto.rs::sign_execute_contract_tx` — builds a canonical `cosmwasm.wasm.v1.MsgExecuteContract` → `TxBody`/`AuthInfo`/`SignDoc` (`SIGN_MODE_DIRECT`) entirely from structured fields (never trusts caller-supplied raw protobuf), signs the `SignDoc` bytes with the existing `sign_message` ECDSA primitive, and returns a ready-to-broadcast `TxRaw`. Uses `cosmrs` (`default-features = false, features = ["cosmwasm"]`) plus `tendermint`/`prost` for the `chain::Id` type and body decoding — all confirmed to compile cleanly for `wasm32-wasip2` with **no new WASI imports** (`wasm-tools component wit` shows only `wasi:io`, `wasi:cli`, `wasi:random` — no `wasi:filesystem`, no network).
- New WIT function `sign-cosmos-execute-tx` (`wavs/sealed-signer/wit/sealed-signer.wit`, package bumped to `0.3.0`), wired into `wavs/sealed-signer/src/lib.rs`.
- 5/5 unit tests pass, including a wire-level check (not just a self-consistency check): decodes the signed `TxRaw` back, confirms the `type_url` is exactly `/cosmwasm.wasm.v1.MsgExecuteContract`, the embedded JSON matches, and the ECDSA signature verifies against the recomputed `SignDoc` bytes.
- Deliberately scoped to this one message type (`MsgExecuteContract`) since it's the only message JunoClaw's tooling needs to broadcast (AKB exports / replies to Moultbook).

**Independent cross-check done and passing:** `wavs/sealed-signer/examples/print_signdoc_fixture.rs` emits a fixed `SignDoc` fixture from `cosmrs`; `wavs/sealed-signer/scripts/crosscheck-signdoc.js` reconstructs the same `TxBody`/`AuthInfo`/`SignDoc` from first principles using `cosmjs-types` (not `@cosmjs/proto-signing`'s helpers — deliberately built from the same primitives it's built on, to avoid testing cosmjs against itself) and compares SHA-256 of the encoded bytes. **Byte-identical.** Run via:
```
cd wavs/sealed-signer
cargo run --example print_signdoc_fixture --quiet | node scripts/crosscheck-signdoc.js
```

**M2 BUILT AND COMPILING — see `drafts/ARTICLE_M2_SEALED_SIGNER_SIGNS_ITS_OWN_TXS.md` for the full write-up.**

What landed:
- `sign_execute_contract_tx` duplicated into `wavs/src/sealed_signer.rs` (following the M1.5 "duplicate, don't compose" precedent).
- `SignRequest` variant added to `VerificationTask` in `wavs/src/trigger.rs`, parsing `wasm-sign_request` events.
- `process_sign_request` in `wavs/src/lib.rs` signs the tx inside the TEE and returns `store_signed_tx` results.
- `sign-request` workflow added to `wavs/service.json`.
- WAVS bridge (`bridge.ts`/`client.ts`) handles `store_signed_tx` results and submits `StoreSignedTx` to `agent-company`.
- `agent-company` contract extended with `relayer` + `sealed_signer` roles, `RequestSignedTx`/`StoreSignedTx`/`AckBroadcastTx` messages, `GetSignRequest`/`ListSignRequests` queries, and guardrails (target contract lock, gas/fee caps, one-pending-at-a-time).
- `moultbook.js` updated with full relayer flow: fetch account info, submit request, poll for signed tx, broadcast, acknowledge.
- All four targets compile clean: `agent-company` (cargo check), `junoclaw-wavs-component` (cargo check --target wasm32-wasip2), `junoclaw-sealed-signer` (cargo check --target wasm32-wasip2), `wavs/bridge` (tsc --noEmit).

**What's still open before this replaces the mnemonic in production:**
- **SGX determinism re-run:** the new `cosmrs`/`tendermint`/`prost` dependencies and `sign_cosmos_execute_tx` code path need to be verified inside a real SGX enclave (Azure DCsv3). Needs TEE access coordination.
- **`wasi:keyvalue` encryption-at-rest:** still unconfirmed by WAVS docs. Mitigated by sealed-blob-only storage rule.
- **Off-chain WAVS invocation:** Jake confirmed WAVS is event-driven only today. The on-chain `sign_request` round-trip is the production architecture. A future WAVS runtime enhancement (off-chain/on-demand `invoke` API) would eliminate the round-trip — plan documented in the M2 article.
