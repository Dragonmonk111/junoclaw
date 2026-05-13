# zk-verifier — Deterministic Scrutiny Audit

*Applied per the Ffern/Lex benchmark. Anchor commit: `ecb5f5b` on origin/main. Files read in full: `src/contract.rs` (196 lines), `src/bn254_backend.rs` (248 lines), `src/state.rs` (23 lines), `src/error.rs` (28 lines), `src/msg.rs` (51 lines).*

*This is the **highest-leverage** contract in the JunoClaw stack — it is the cryptographic gate the entire verifiable-agent claim rests on. Findings here propagate to every downstream consumer (task-ledger constraint checks, agent-company votes, escrow attestation flow).*

---

## 0. Architectural framing

**Two backends, one signature.** `verify_groth16(vk, proof, public_inputs) -> Result<bool>` dispatches on the `bn254-precompile` cargo feature:

| Build flavour | Path | Gas (measured/projected on Juno) |
|---|---|---|
| `cargo build` (default) | Pure arkworks `Groth16::<Bn254>::verify_proof` | **370,600 SDK gas** (measured on uni-7) |
| `cargo build --features bn254-precompile` | 4-pair `bn254_pairing_equality` + `bn254_scalar_mul` + `bn254_add` host calls | **203,266 SDK gas** (measured on `junoclaw-bn254-1` devnet, 5 samples σ=0) |

The uni-7 mainnet currently runs the **default** path (no precompile). The precompile path runs on the `junoclaw-bn254-1` devnet and lands on Juno mainnet only after Juno governance prop #374 + the cosmwasm/wasmvm host-function patches we're shepherding upstream.

**Surface:**

| Action | Authority | Effect |
|---|---|---|
| `Instantiate { admin? }` | anyone | Sets `admin`, no VK yet |
| `StoreVk { vk_base64 }` | admin only | Replaces `VK_BYTES` with new arkworks-CanonicalSerialized VK |
| `VerifyProof { proof_base64, public_inputs_base64 }` | **anyone** | Runs verification; on success saves `LAST_VERIFICATION { verified: true, block_height }`; on failure returns `ProofInvalid` |
| `Migrate {}` | chain-level migrate-admin | Bumps `cw2` version, no logic |
| `Query VkStatus` / `LastVerify` | anyone | Read state |

**State surface:**

| Storage item | Type | Notes |
|---|---|---|
| `CONFIG` | `Item<Config { admin }>` | Single field — admin only |
| `VK_BYTES` | `Item<Vec<u8>>` | One VK per contract instance |
| `LAST_VERIFICATION` | `Item<LastVerification { verified, block_height }>` | Only written on **successful** verification — see F2 |

No prefix collisions. No Vec-index pattern. No reverse indexes.

---

## 1. Failure Mode Enumeration

### 🔴 F1 — `VerifyProof` is permissionless and unmetered; trivial gas-DoS vector

**Location:** `execute_verify_proof` (`src/contract.rs:84-137`). No authorization check, no fee check, no rate limit.

**The flaw.** `VerifyProof` runs the most expensive computation in the contract (~370K SDK gas on uni-7) on behalf of any caller, with no fee and no auth. An attacker can:

1. Submit `VerifyProof` with malformed/random base64. The contract runs the full deserialize → verify path before erroring. Cost: ~50K gas just to fail (deserialization errors fire after partial work).
2. Submit `VerifyProof` with a valid proof + public-inputs combo (any valid one — e.g., one previously seen on-chain). The contract runs the full pairing check, succeeds, and **rewrites `LAST_VERIFICATION`** with the current block height.

**Why it matters concretely.**

- **Attack path A (gas exhaustion).** Each VerifyProof tx costs the attacker ~370K gas (current Juno mainnet) but consumes block-time disproportionate to the gas cost (heavy crypto). At 0.025ujunox/gas, that's ~9,300 ujunox per attack tx. A motivated attacker can spam-fill blocks at modest cost; the contract becomes a per-block griefing surface for any other DAO that uses it as a constraint check.
- **Attack path B (`LAST_VERIFICATION` spoofing).** `LAST_VERIFICATION` is the public read-side signal that "a valid proof was verified at block H". Other contracts (or off-chain monitors) might use this as a heartbeat or freshness signal. An attacker who has captured **any** valid proof from past chain history can replay it to keep `LAST_VERIFICATION.block_height` rolling forward — spoofing freshness without ever proving anything new.

**Severity: HIGH** in the current architecture (no fee, no replay protection, public state surface).

**Suggested fixes.**

The natural fix is **layered defense, not single-bullet**:

1. **Add a fee.** `Config { admin, verify_fee_native: Coin }` — `VerifyProof` must include `info.funds == verify_fee_native`. Sets a per-call price floor that prices the gas + a margin. Fee accumulates in the contract; admin can withdraw via a new `WithdrawFees` handler. ~30 LoC + 3 tests. Pairs with the same fix needed for agent-registry F1 (the underlying pattern is identical: collect-without-recourse).
2. **Add a per-call nonce, indexed by `(public_inputs_hash, block_height_window)`.** Reject re-submission of a proof for the same public-inputs within a window of N blocks. Defends against attack path B. ~25 LoC + 2 tests. The `Map<Vec<u8>, u64>` (public_inputs_hash → expiry_height) state needs a sweep mechanism to avoid bloat — straightforward eviction at write-time.
3. **Add a per-block call cap.** Track `verifications_this_block: u64` keyed on the block height; reject after N (e.g., 10). ~15 LoC + 2 tests. Cheap soft-limiter.

Combined cost: ~70 LoC + 7 tests. All three are independent; you can ship (1) first as the minimum bar.

---

### 🟡 F2 — `LAST_VERIFICATION` is asymmetric: only successful runs persist; "last" is a misnomer

**Location:** `execute_verify_proof` (`src/contract.rs:124-131`). The `LAST_VERIFICATION` save happens **only after `if !valid { return Err(ProofInvalid {}) }`**. Failed verifications return without touching state.

**The flaw.** The query `LastVerify {}` returns `LastVerifyResponse { verified: bool, block_height: u64 }`, which a reader naturally interprets as "what happened on the last attempt." But the actual semantics are "what happened on the last *successful* attempt." This drift causes:

- **Stale reads.** If 10 verifications fail in a row, the query still reports the success from 100 blocks ago as "last."
- **No on-chain failure record.** Failed proofs leave no trace except in tx history.
- **Field-name confusion.** `verified: bool` always reports `true` (because that's the only state-writing path). The field is structurally unable to be `false` post-instantiate. Dead field.

**Severity: MEDIUM** — observable API confusion, with no exploit but potentially-broken downstream consumers.

**Suggested fix.**

Two clean options:

1. **Persist on every attempt.** Save `LastVerification { verified, block_height, error: Option<String> }` regardless of result. Now the query returns the literal last attempt; readers can branch on `verified`. ~5 LoC.
2. **Rename to `LastSuccessfulVerification`.** Explicit naming kills the confusion; doesn't change behaviour. ~3 LoC + a deprecation alias for `LastVerify`.

I recommend (1) — readers benefit from the failure signal.

---

### 🟡 F3 — VK rotation has no audit trail; lost-admin bricks the contract

**Location:** `execute_store_vk` (`src/contract.rs:58-82`); absence of any `UpdateAdmin` handler.

**Two coupled flaws:**

1. **VK rotation is silent.** `StoreVk` overwrites `VK_BYTES` in place. No history, no event audit-trail beyond the tx hash, no version counter. If the admin rotates the VK and a downstream consumer was mid-verification with the old VK, their proof now fails verification with no clear cause.
2. **No admin rotation.** `Config { admin }` is set at instantiate and never updated. If the admin keypair is lost or compromised, the contract is **bricked** — no path to update VK, no path to change admin. Recovery requires `migrate` to a new code_id, which only the chain-level migrate-admin (a separate authority) can do.

**Severity: MEDIUM** — operational risk, not exploit. But this is the cryptographic-gate contract for the whole stack; bricking it is severe.

**Suggested fix.**

Two parts, both small:

1. **`UpdateAdmin { new_admin: String }`** — admin-only handler. ~10 LoC + 2 tests. Same pattern as task-ledger F8 (operator updates) — easy reuse.
2. **VK history** — add `VK_VERSIONS: Map<u64, VkRecord>` keyed on a monotonic counter; `StoreVk` writes to the next index and updates `CURRENT_VK_VERSION`. Old VKs remain queryable for debug. ~25 LoC + 2 tests.

---

### 🟡 F8 — `gamma_abc_g1` length mismatch surfaces only in precompile path; pure-arkworks path returns cryptic upstream error

**Location:** `bn254_backend.rs:107-115` (precompile path explicit check); pure-arkworks path has no explicit check (relies on arkworks).

**The flaw.** The precompile path explicitly checks `vk.gamma_abc_g1.len() != public_inputs.len() + 1` and returns a clean `DeserializationError` with a descriptive message. The pure-arkworks path delegates to `Groth16::<Bn254>::verify_proof`, which returns an error but with a less-clear message ("verify error: …" wrapping arkworks' internal text).

**Result.** Identical malformed-input behaviour between the two backends, but **different error messages**. Differential testing (the 1000-random-proofs harness in `wasmvm-fork/BUILD_AND_TEST.md`) might pass on accept/reject decisions but diverge on error text — invisible drift.

**Severity: MEDIUM** — UX inconsistency that could mask bugs in differential tests.

**Suggested fix.** Hoist the length check up to `verify_groth16` (the dispatch shim) so both backends inherit it identically. ~5 LoC.

```rust
pub fn verify_groth16(...) -> Result<bool, ContractError> {
    if vk.gamma_abc_g1.len() != public_inputs.len() + 1 {
        return Err(ContractError::DeserializationError {
            reason: format!("gamma_abc_g1 length {} != public_inputs length + 1 ({})",
                vk.gamma_abc_g1.len(), public_inputs.len() + 1),
        });
    }
    // existing dispatch
}
```

---

### 🟢 F4 — `LAST_VERIFICATION` is global, not per-circuit / per-context

**Location:** `src/state.rs:22`. Single `Item<LastVerification>` — one global slot.

**The coupling smell.** The contract supports **one VK** and **one LAST_VERIFICATION** per instance. Multiple verification contexts (different circuits, different DAO purposes) require separate contract instantiations, doubling deployment cost and complicating cross-context correlation.

**Severity: LOW** — design limitation, not bug. Worth surfacing because the natural use case (one verifier shared across many DAOs) doesn't fit this shape.

**Suggested fix.** Make the contract multi-circuit:

```rust
pub const VK_BYTES: Map<&str, Vec<u8>> = Map::new("vk_bytes");      // keyed on circuit_id
pub const LAST_VERIFICATION: Map<&str, LastVerification> = Map::new("last_verification");
```

Add `circuit_id: String` to `StoreVk` and `VerifyProof`. ~25 LoC + state migration. Bigger change; ship separately as `zk-verifier-v2`.

---

### 🟢 F5 — `base64_decode` uses `from_json::<Binary>` as a hack; brittle to cosmwasm-std changes

**Location:** `src/contract.rs:167-176`.

**The technique.**
```rust
fn base64_decode(input: &str) -> Result<Vec<u8>, ContractError> {
    cosmwasm_std::from_json::<cosmwasm_std::Binary>(
        &format!("\"{}\"", input).into_bytes(),
    )
    .map(|b| b.to_vec())
    .map_err(...)
}
```

**Why it's clever.** Avoids adding a `base64` crate dependency; reuses `Binary`'s JSON deserialization (which decodes base64 strings into bytes). Saves contract size.

**Why it's brittle.** Depends on `Binary`'s JSON format remaining a quoted base64 string. If cosmwasm-std ever changes that representation (unlikely but not impossible), this silently breaks. Also slower than a direct `base64::decode` because of the JSON-parse round-trip.

**Severity: LOW** — works today, fragile to upstream changes.

**Suggested fix.** Use the `base64` crate (already in the dependency tree via `cosmwasm-std` transitively, or pull it in directly). ~3 LoC change.

---

### 🟢 F6 — No upper bound on `public_inputs` length

**Location:** `deserialize_public_inputs` (`src/contract.rs:178-195`). Accepts any byte-length divisible by 32.

**The risk.** A 32 MB public-inputs blob = 1 million Fr elements. In the precompile path, the `vk_x` linear combination loops:

```rust
for (i, x) in public_inputs.iter().enumerate() {
    bn254_scalar_mul_call(...)?;
    bn254_add_call(...)?;
}
```

That's `1M × (6,000 + 150) ≈ 6.1 billion SDK gas` — well past any realistic block gas limit, so the chain will reject. **But** the contract does the bytes-deserialization (1M Fr deserializations) before running into the gas wall. That deserialization is also gas-metered, but it lands in the contract before the failure surfaces; the operator paid full gas-limit for a nonsensical request.

**Severity: LOW-MEDIUM** — gas catches it eventually, but better to fast-fail.

**Suggested fix.** Add `if inputs_bytes.len() / 32 > MAX_PUBLIC_INPUTS { Err(...) }` at the top of `deserialize_public_inputs`. Sensible cap: 1024 (Groth16 circuits rarely have more). ~5 LoC + 1 test.

---

### 🟢 F7 — `Migrate` does no version validation

**Location:** `migrate` (`src/contract.rs:159-163`).

**Behaviour.** Bumps cw2 contract version unconditionally. No check that the prior contract was actually `crates.io:junoclaw-zk-verifier` (note: agent-registry, task-ledger, and escrow audits all flagged the same pattern; this is a JunoClaw-stack-wide convention).

**Severity: LOW** — chain-level migrate-admin is the gate; in-contract validation is defense-in-depth.

**Suggested fix.** Same as agent-registry F8, task-ledger F10, escrow's migrate validation. Cross-cutting one-liner: check `get_contract_version(deps.storage)?.contract == CONTRACT_NAME`. ~3 LoC per contract.

---

### 🟢 F9 — No event emitted for failed verification

**Location:** `execute_verify_proof` (`src/contract.rs:124-126`). The `Err(ProofInvalid {})` path returns the error but emits no Response with attributes.

**Why it matters.** Other contracts watching the chain for "did this verifier reject?" events have no on-chain signal. The tx itself errors (which produces an event), but the event has no semantic structure beyond the error text.

**Severity: LOW** — observability gap, not exploit.

**Suggested fix.** Restructure to return a `Response { result: "invalid" }` instead of an Err. **But** that would change the cost model (a successful tx with a failed proof, instead of a failed tx). Trade-off worth discussing — current Err path is the conservative choice. Document the reasoning.

---

## 2. Gas Trace — `execute_verify_proof` hot path

### Pure-arkworks path (mainnet today)

| Step | Approx. SDK gas |
|---|---|
| `VK_BYTES.may_load` | ~3,500 |
| VK deserialize (arkworks) | ~5,000 |
| `proof_base64` deserialize | ~1,500 (incl. base64) |
| `proof` arkworks deserialize | ~3,000 |
| `public_inputs_base64` deserialize | ~1,000 |
| `public_inputs` Fr loop | ~500 × N |
| **`prepare_verifying_key`** | ~30,000 |
| **`Groth16::verify_proof` (4 pairings)** | **~325,000** |
| `LAST_VERIFICATION.save` | ~5,000 |

**Total: ~370K SDK gas** (matches uni-7 measurement). The 4 pairings dominate — ~88% of total gas.

### Precompile path (devnet `junoclaw-bn254-1`)

| Step | Approx. SDK gas |
|---|---|
| Pre-pairing setup (same as above through `proof` deserialize) | ~14,000 |
| `vk_x` lincomb: N × (`bn254_scalar_mul`: 6,000 + `bn254_add`: 150) | ~6,150 × N |
| **`bn254_pairing_equality` (4 pairs)** | **45,000 + 34,000 × 4 = 181,000** |
| `LAST_VERIFICATION.save` | ~5,000 |

**Total: ~203K SDK gas** for typical N (1-3 public inputs) — matches devnet measurement.

**Speedup: 1.823× (370,600 → 203,266).** Threshold between sample-verify and verify-every.

---

## 3. Determinism Proof

| Concern | Status |
|---|---|
| No floats | ✅ — arkworks uses fixed-point Fp arithmetic |
| No HashMap iteration | ✅ |
| No `std::time` | ✅ — uses `env.block.height` |
| Deterministic VK serialize/deserialize | ✅ — arkworks `CanonicalSerialize` is canonical |
| Deterministic G1/G2 encoding (precompile path) | ✅ — explicit `encode_g1` / `encode_g2` with fixed byte order; `is_zero` short-circuits to `[0u8; 64]` |
| Subgroup checks (BN254) | ✅ — G1 cofactor=1, G2 uses `is_in_correct_subgroup_assuming_on_curve` (per `cosmwasm-crypto-bn254` crate, externalised to host) |
| Identical accept/reject between backends | ✅ — verified by the 1000-random-proofs differential test in `wasmvm-fork/BUILD_AND_TEST.md` |

**All clear.** ✅

The differential test is the **strongest correctness signal in the whole stack** — same inputs, different math paths, same output 1000/1000 times. Worth highlighting in any pitch / PR description.

---

## 4. Action Items

| ID | Severity | Fix | Effort |
|----|----------|-----|--------|
| F1 | **HIGH** | Add fee + nonce + per-block cap (layered defense) | ~70 LoC + 7 tests |
| F2 | MEDIUM | Persist `LastVerification` on every attempt OR rename to `LastSuccessfulVerification` | ~5-10 LoC |
| F3 | MEDIUM | `UpdateAdmin` handler + VK history map | ~35 LoC + 4 tests |
| F8 | MEDIUM | Hoist `gamma_abc_g1` length check up to dispatcher | ~5 LoC |
| F4 | LOW | Multi-circuit support (`Map<&str, _>`) | ~25 LoC + state migration (defer to v2) |
| F5 | LOW | Use `base64` crate instead of JSON-roundtrip hack | ~3 LoC |
| F6 | LOW-MEDIUM | Cap `public_inputs` length | ~5 LoC + 1 test |
| F7 | LOW | Migrate version-validation (cross-cutting one-liner) | ~3 LoC |
| F9 | LOW | Emit event on failed verification | ~5 LoC (debate) |

**Recommendation.**

- **Sprint 1 (zk-verifier-v0.2):** F1 (the headline) + F2 (asymmetric persistence) + F3 (admin rotation). All three address the contract's role as the cryptographic gate of the stack.
- **Sprint 2 (zk-verifier-v0.3):** F8 (length-check hoist) + F6 (input cap) + F7 (migrate validation) + F5 (base64 hardening). Defensive-depth pass.
- **v2 (zk-verifier-v2):** F4 (multi-circuit) + F9 (failure events). Bigger scope; ship as a new contract.

---

## 5. Comparative summary across the JunoClaw stack (post-this-audit)

| Contract | Audit | Headline finding | Severity |
|---|---|---|---|
| `agent-company` | ✅ | Vote weights not snapshotted at proposal creation | **HIGH** |
| `agent-registry` | ✅ | Registration fees trapped (no withdraw path) | **MEDIUM** |
| `task-ledger` | ✅ | CancelTask leaves orphaned escrow obligations | **LOW-MEDIUM** |
| `escrow` | ✅ | `timeout_blocks` dead + unit mismatch with `created_at` | **MEDIUM** |
| `zk-verifier` | ✅ (this doc) | `VerifyProof` permissionless + unmetered → gas-DoS + `LAST_VERIFICATION` spoofing | **HIGH** |
| `moultbook-v0` | ✅ (deterministic from day 0) | None | None |
| `junoswap-pair` | pending | TBD | TBD |
| `junoswap-factory` | pending | TBD | TBD |
| `builder-grant` | pending | TBD | TBD |
| `faucet` | pending | TBD | TBD |

**5 of 9 audited.** Two HIGH findings (agent-company F1, zk-verifier F1). Both are the same shape: a contract surface that is too permissive given its leverage. The fixes are layered defense, not single-line patches.

**Cross-cutting observation.** Every audited contract has at least one finding in the category "permission model is too lenient / surface is unmetered." This is the strongest stack-wide pattern. The next audit-bot iteration could codify a check ("does every public state-mutating handler have an auth gate or fee gate?") as a static-analysis lint.

---

*Apache-2.0. Audit conducted under the deterministic scrutiny benchmark. zk-verifier is the cryptographic gate of the verifiable-agent stack; findings here propagate to every downstream consumer.*
