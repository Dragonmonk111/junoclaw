# A Pure-Rust Post-Quantum Signature Verifier for CosmWasm

> MAYO-2 is now live on the Juno testnet (`uni-7`). Here's what we built, why it matters, and what's next.

---

## What We Built

### The Problem

Post-quantum cryptography (PQC) is coming. NIST's selected signature schemes—starting with [MAYO](https://pqmayo.org/)—offer resistance to Shor's algorithm, but none ship with a `wasm32`-compatible verifier. The reference C implementation (`sriracha-mayo`) requires CMake and cannot compile to WebAssembly.

### What We Built

**`junoclaw-mayo-verify`** — a zero-dependency, `#![no_std]`-ready crate that verifies MAYO-2 signatures entirely in safe Rust. No C. No unsafe. No CMake. It compiles to `wasm32-unknown-unknown` out of the box.

**`jclaw-credential` integration** — a CosmWasm contract that stores a fingerprint (SHA-256) of a member's MAYO public key, then verifies attestations on-chain by accepting the full public key at verification time, checking the hash matches the stored fingerprint, and running the pure-Rust verifier.

### How the On-Chain Flow Works

**Bud creation** (once per member):
```
admin → contract: Bud { ..., mayo_pk: <4,912 B MAYO-2 PK> }
contract: verify length, compute SHA-256, store 64-char hex hash
```

**Attestation verification** (any time):
```
anyone → contract: VerifyMayoAttestation {
    addr, message, signature, public_key
}
contract:
  1. Load stored hash for addr
  2. Compute hash of provided public_key → must match stored hash
  3. Call Mayo2::verify(message, signature, public_key)
  4. Accept iff both checks pass
```

### Why This Pattern Matters

Storing the full 4.9 KB public key in contract state for every member would bloat storage. Instead, we store a 32-byte hash—cheap, permanent, and sufficient to bind a public key to a member. The actual public key travels only at verification time (in the transaction payload). This is the same pattern used by Bitcoin pay-to-pubkey-hash and Ethereum account abstraction.

### Deterministic Outcomes

MAYO verification is deterministic: given a fixed `(public_key, message, signature)` triple, the output is either `Ok(())` or `Err(VerifyFailed)`. There is no randomness, no oracles, no chain state dependency. This is critical for:

- **Reproducible testing**: same inputs → same result on every machine.
- **Light-client verification**: a third party can re-run the check without blockchain access.
- **ZK-proof friendliness**: deterministic functions are trivial to arithmetize into circuits if we later want a ZK-proof-of-verification.

### Live Testnet Results (uni-7)

| Operation | Gas Used | Cost (0.075 ujunox) |
|-----------|----------|---------------------|
| Bud (create member + MAYO PK) | 335,126 | ~25.1 ujunox |
| VerifyMayoAttestation (valid) | 355,771 | ~26.7 ujunox |
| VerifyMayoAttestation (tampered) | Rejected ✅ | — |

**Contract addresses**:
- jclaw-credential: `juno1z2w067ptpn2f6zpwt207je0kqeqc2eek7jf4p4dpztf24zncnhzqz5el2r`
- moultbook: `juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4`

---

## What Comes Next

### Immediate: MAYO-Signed Moultbook Attestations

With this foundation, `jclaw-credential` members can now sign moultbook attestations with MAYO-2 keys, and the chain validates them natively—no bridge, no C precompile, no trust assumption. As quantum computers advance, the trust tree remains intact.

### Phase 2: MAYO-3 & MAYO-5 for Maximum Security

MAYO-2 is the smallest parameter set (186-byte signatures, 4,912-byte public keys). For high-security use cases, we will extend to:

| Parameter Set | NIST Security Level | Signature Size | Public Key Size | Use Case |
|---------------|---------------------|----------------|-----------------|----------|
| **MAYO-2** | Level 1 | 186 B | 4,912 B | Standard governance, day-to-day attestations |
| **MAYO-3** | Level 3 | 277 B | 12,128 B | High-value proposals, treasury ops |
| **MAYO-5** | Level 5 | 964 B | 17,136 B | Critical infrastructure, multi-sig threshold |

**Challenge**: MAYO-3 and MAYO-5 public keys exceed the 512 KB CosmWasm memory limit when naively expanded. Solution: streaming `expand_pk` (row-by-row AES-128-CTR expansion) to compute verification matrices on-the-fly without materializing the full public key.

### Phase 3: ZK-Proof of MAYO Verification

A Groth16/BN254 circuit proving "I verified a MAYO signature" would let us:
- Verify MAYO on any chain with BN254 precompiles (including Ethereum L2s)
- Keep the verifier logic off-chain while retaining on-chain trust
- Potentially support MAYO-3/5 on chains with stricter gas limits

### Phase 4: IBC Cross-Chain MAYO Signatures

MAYO attestations verified on Juno can be relayed via IBC to any Cosmos chain. A light-client proof of the verification tx on Juno becomes portable trust for the entire ecosystem.

---

*Published 2026-06-12. Repo: [github.com/junoclaw/junoclaw](https://github.com/junoclaw/junoclaw) | Testnet: `uni-7`*
