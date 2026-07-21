# The Enclave Signs Its Own Transactions

*M1 built a key inside a TEE and proved it could sign a digest. M2 makes that key sign real Cosmos SDK transactions — the same `SIGN_MODE_DIRECT` protobuf flow that `junod` expects on the wire — and wires the entire round-trip from on-chain request to broadcast-ready `TxRaw` without a plaintext mnemonic ever touching a developer terminal. This piece explains what M2 adds, how the pieces fit together, and what's still open.*

---

## What M1 left on the table

M1's sealed signer could do two things: generate a secp256k1 key inside a WASI component and sign an arbitrary byte digest. That's enough to prove the crypto works and the key never leaves the enclave. It is not enough to replace the reply-bot's mnemonic.

The reason is simple. Juno — like every Cosmos SDK chain — doesn't accept raw signatures on-chain. It accepts signed protobuf transactions. A valid transaction is a `TxRaw` containing a `TxBody`, an `AuthInfo` with a `SignDoc`, and a signature over the `SignDoc` bytes. The `SignDoc` itself encodes the chain ID, account number, sequence, fee, and message body in a very specific protobuf layout. Get any field wrong and the transaction is rejected. Get the sign bytes wrong in a subtle way and you can produce a transaction that validates but signs something you didn't intend.

M1's `sign(bytes) -> signature` had no knowledge of any of this. The reply-bot's `moultbook.js` used `DirectSecp256k1HdWallet.fromMnemonic()` with `SigningCosmWasmClient.execute()`, which builds the `SignDoc` internally. To remove the mnemonic entirely, the enclave itself must construct and sign the `SignDoc`.

That's what M2 does.

---

## The four pieces M2 adds

### 1. Cosmos `SIGN_MODE_DIRECT` signing inside the enclave

The sealed signer component (`wavs/sealed-signer/src/crypto.rs`) now exposes `sign_execute_contract_tx`, which takes structured fields — sender address, contract address, JSON execute message, funds, gas, fee, memo, chain ID, account number, sequence — and builds a canonical Cosmos SDK transaction from scratch:

1. Construct a `MsgExecuteContract` protobuf message from the structured fields.
2. Wrap it in a `TxBody` (with memo).
3. Build `AuthInfo` with a single signer, `SIGN_MODE_DIRECT`, the specified gas limit and fee.
4. Assemble the `SignDoc` (body bytes + auth info bytes + chain ID + account number).
5. Sign the `SignDoc` bytes with the enclave's secp256k1 key.
6. Produce a `TxRaw` (body bytes + auth info bytes + signature) ready to broadcast.

The function deliberately **never accepts caller-supplied raw protobuf bytes**. The caller provides structured fields; the enclave constructs the protobuf. This prevents malleability attacks where a compromised relayer could submit pre-crafted sign bytes that encode a different transaction than the one the DAO approved.

The signing primitive itself is the same ECDSA-SHA256 from M1 — `k256` over the `SignDoc` SHA-256 hash. No new crypto, just a new canonical message format.

**Byte-level cross-check:** An independent CosmJS script (`wavs/sealed-signer/scripts/crosscheck-signdoc.js`) reconstructs the same `TxBody`/`AuthInfo`/`SignDoc` from first principles using `cosmjs-types` primitives (not `@cosmjs/proto-signing`'s helpers — deliberately built from the same protobuf encoding it's built on, to avoid testing cosmjs against itself). The SHA-256 of the CosmJS-encoded `SignDoc` matches the Rust-encoded one byte-for-byte. This is the guarantee that the enclave produces transactions Juno will actually accept.

### 2. On-chain sign-request trigger

WAVS is an event-driven system. Every existing workflow in `service.json` is triggered by an on-chain Cosmos contract event — `wasm-attestation_request`, `wasm-sign_moultbook_export`, etc. The component runs, produces a result, and the bridge submits it back on-chain.

But signing a Moultbook post transaction is needed *before* that post exists on-chain. There's no event to trigger on. This is a fundamental mismatch: the enclave needs to sign a transaction that hasn't been submitted yet.

M2 resolves this with a new on-chain round-trip through `agent-company`:

1. **Relayer submits `RequestSignedTx`** — a new `ExecuteMsg` on `agent-company`. The relayer (a dedicated role, not the admin) provides the structured fields: sender (must match the configured sealed-signer address), target contract, execute message JSON, gas/fee, chain ID, account number, and sequence.
2. **Contract validates and emits `sign_request` event** — `agent-company` checks the relayer is authorized, the sender matches the configured sealed signer, the target contract is the configured Moultbook, gas and fee are within guardrails (max 1M gas, max 10 JUNO fee), and no other sign request is pending (one at a time to avoid sequence collisions). It assigns a sequential ID, stores the request, and emits a `sign_request` event with all fields.
3. **WAVS picks up the event** — a new `sign-request` workflow in `service.json` triggers the verifier component on `wasm-sign_request` events. The trigger parser (`wavs/src/trigger.rs`) maps the event attributes to a `SignRequest` verification task.
4. **Component signs inside the TEE** — `process_sign_request` in `wavs/src/lib.rs` calls `sign_cosmos_execute_tx` on the co-located sealed signer module. The enclave constructs the `SignDoc`, signs it, and returns the `TxRaw` bytes plus the `SignDoc` SHA-256 hash.
5. **Bridge submits `StoreSignedTx`** — the WAVS bridge (`wavs/bridge/src/bridge.ts`) detects `store_signed_tx` results and calls `submitStoreSignedTx` (`wavs/bridge/src/client.ts`), which submits a `StoreSignedTx` message to `agent-company` with the signed tx bytes and sign doc hash. The contract verifies the submitter is the WAVS operator, checks the request is pending, stores the tx bytes, and marks it `Signed`.
6. **Relayer broadcasts and acknowledges** — `moultbook.js` polls `GetSignRequest` until the status is `Signed`, fetches the `tx_bytes`, broadcasts them via `StargateClient.broadcastTx`, and then calls `AckBroadcastTx` to clear the spent request from storage.

```
relayer                agent-company           WAVS/enclave           chain
   │                       │                       │                    │
   │── RequestSignedTx ──→ │                       │                    │
   │                       │── store + emit ──→    │                    │
   │                       │   sign_request event  │                    │
   │                       │                  ┌────│                    │
   │                       │                  │     │                    │
   │                       │            WAVS trigger fires               │
   │                       │                  │     │                    │
   │                       │            sign_cosmos_execute_tx           │
   │                       │            (inside TEE)                     │
   │                       │                  │     │                    │
   │                       │←── StoreSignedTx ─────│                    │
   │                       │    (tx_bytes)         │                    │
   │                       │                       │                    │
   │←─ poll GetSignRequest │                       │                    │
   │   (status=Signed)     │                       │                    │
   │                       │                       │                    │
   │── broadcastTx ────────────────────────────────────────────────→   │
   │                       │                       │                    │
   │── AckBroadcastTx ──→  │                       │                    │
   │                       │── remove request ──→  │                    │
```

Is the round-trip heavier than the old `SigningCosmWasmClient.execute()` one-shot? Yes — it adds one on-chain transaction (`RequestSignedTx`) and one on-chain message (`StoreSignedTx`) per signed post. The trade-off is that the plaintext mnemonic is gone from the relayer process entirely. The relayer holds no private key material. It only holds the transaction *template* and lets the enclave sign it.

### 3. Relayer role in `agent-company`

The contract now has two new configurable roles alongside the existing `wavs_operator`:

- **`relayer`** — the address authorized to submit `RequestSignedTx`. This is the off-chain bot that knows what to post and when, but holds no signing key.
- **`sealed_signer`** — the enclave's derived Juno address. The contract enforces that every sign request's `sender` field matches this address exactly.

Both are admin-rotatable via `RotateRelayer` and `RotateSealedSigner`, following the same pattern as the existing `RotateWavsOperator` and `RotateMoultbook` messages. Governance proposals can rotate them too.

The contract enforces three guardrails on every `RequestSignedTx`:

- **Target contract must be the configured Moultbook.** The sealed signer can only sign transactions targeting the DAO's own Moultbook contract. A compromised relayer can't use the enclave to sign arbitrary contract calls.
- **Gas ≤ 1,000,000 and fee ≤ 10 JUNO.** Prevents a compromised relayer from burning the enclave account's funds.
- **One pending request at a time.** Avoids sequence collisions — the relayer must wait for the current request to be signed (or fail) before submitting the next.

### 4. Updated `moultbook.js` relayer flow

The off-chain tooling (`tools/reply-bot/src/moultbook.js`) now has a complete sealed-signer relayer path:

- `getSealedSignerAccountInfo()` — fetches the enclave address's account number and sequence from a `StargateClient`.
- `buildRequestSignedTx()` — constructs the `RequestSignedTx` message from the account info and the Moultbook execute message.
- `requestSignedTx()` — submits it to `agent-company` via `SigningCosmWasmClient`.
- `parseSignRequestId()` — extracts the contract-assigned request ID from the tx events.
- `pollForSignedTx()` — polls `GetSignRequest` until status is `Signed`.
- `broadcastSignedTx()` — broadcasts the `tx_bytes` via `StargateClient.broadcastTx`.
- `ackBroadcastTx()` — calls `AckBroadcastTx` to clean up.

The old mnemonic-based path (`DirectSecp256k1HdWallet.fromMnemonic()`) still exists for backward compatibility, but the sealed signer path is now the intended production flow. The mnemonic can be removed from the environment entirely once the sealed signer is deployed and funded.

---

## What's still open

### `wasi:keyvalue` encryption-at-rest

The sealed key is stored as an AES-256-GCM encrypted blob via `wasi:keyvalue/store`. The raw secret never touches the key-value store. But whether WAVS's TEE runtime additionally encrypts `wasi:keyvalue` data at rest (disk-level encryption) is unconfirmed by any docs found so far. The sealed-blob-only storage rule mitigates this regardless — even if the KV store is plaintext on disk, the blob is ciphertext — but a direct answer from the WAVS team would turn a convention into a guarantee.

### SGX determinism re-run

The `junoclaw:verifier` component has already run inside Intel SGX on Azure DCsv3 and submitted an on-chain attestation (proposal 4, tx `6EA1AE79...D26B22`). M2 adds new WASI dependencies (`cosmrs`, `tendermint`, `prost`) and a new code path (`sign_cosmos_execute_tx`). The determinism and filesystem-import checks from M1.5 need to be re-run inside a real SGX enclave to confirm the new dependencies don't break the enclave's measurement or introduce unexpected WASI imports. This needs coordination for TEE access.

### Composition: duplicate, don't compose

Following the M1.5 precedent, the Cosmos tx signing logic is duplicated into `wavs/src/sealed_signer.rs` (the co-located module inside the verifier component) rather than composing two separately-deployed WAVS components via `wasm-tools compose`. WAVS's support for component-to-component calls is unverified. The standalone `junoclaw:sealed-signer` crate remains the reference implementation and test harness; the verifier component's inline copy is what runs in production.

---

## Jake's reply and the unattempted issue

When we asked Jake Hartnell about WAVS's support for off-chain / on-demand component invocation — i.e. calling a WAVS component directly via HTTP or CLI without an on-chain trigger event — his response confirmed that the current WAVS runtime is **event-driven only**. There is no off-chain invocation path today. Every component execution starts from an on-chain event that WAVS indexes.

This is why M2 uses the on-chain `sign_request` round-trip rather than a simpler direct-call architecture. The round-trip is the cost of working within WAVS's proven model.

**The unattempted issue — and the plan to pursue it:** the on-chain round-trip and the missing invoke API are corollaries. The relayer role, the `RequestSignedTx`/`StoreSignedTx`/`AckBroadcastTx` state machine, the pending constraint, the gas overhead, and the latency — all of it exists solely because WAVS has no off-chain invocation path. Adding one collapses the entire flow to a direct call:

```
Current (M2):    relayer → RequestSignedTx → event → WAVS → StoreSignedTx → poll → broadcast → AckBroadcastTx
With invoke API: relayer → WAVS invoke (signed HTTP) → get tx_bytes → broadcast → SubmitAttestation
```

The `agent-company` contract would keep the `sealed_signer` address config and `SubmitAttestation` (for recording TEE quotes), but `RequestSignedTx`, `StoreSignedTx`, `AckBroadcastTx`, the `SignRequest` state machine, and the pending constraint would all be removed.

**Full spec and implementation plan:** `drafts/PLAN_WAVS_OFF_CHAIN_INVOKE_API.md` — covers the proposed `wavs invoke` HTTP API, signed request authentication, allowlist config, security properties preserved, the simplified sealed signer flow, a 4-phase implementation plan (spec review → WAVS runtime impl → JunoClaw simplification → docs + community), and 6 open questions for Jake.

**Next step:** DM Jake with the spec and the concrete use case. The ask is narrow: does this fit the WAVS roadmap, and is there a contribution path? If yes, we implement the invoke path in the WAVS runtime and simplify the sealed signer flow. If not, the current on-chain round-trip is the production architecture — it works, it's safe, and the extra gas is the cost of event-driven attestation.

---

## Summary

| What | M1 | M2 |
|---|---|---|
| Key generation | Inside TEE, from `wasi:random` | Same |
| Key storage | AES-256-GCM sealed blob in `wasi:keyvalue` | Same |
| Signing capability | `sign(bytes) -> signature` | `sign_cosmos_execute_tx(fields) -> TxRaw` |
| Transaction format | N/A | Cosmos SDK `SIGN_MODE_DIRECT` protobuf |
| Trigger | Manual / `SignMoultbookExport` event | On-chain `sign_request` event via `agent-company` |
| Mnemonic in relayer | Still needed for broadcasting | **Removed** — relayer holds no key |
| On-chain integration | None | `RequestSignedTx` / `StoreSignedTx` / `AckBroadcastTx` + queries |
| Guardrails | None | Target contract lock, gas/fee caps, one-pending-at-a-time |
| Cross-check | Self-consistency | CosmJS byte-identical `SignDoc` comparison |
| TEE proof | `wasmtime run` (no SGX) | Pending re-run on Azure SGX |

M2 is the milestone where the sealed signer stops being a crypto demo and becomes the production signing path. The mnemonic leaves the terminal. The enclave signs its own transactions. The DAO controls who can ask it to sign, what it can sign, and how much gas it can spend. Everything else — real SGX re-verification, the off-chain invocation enhancement, J-Lens integration — is hardening and expansion on top of a working system.
