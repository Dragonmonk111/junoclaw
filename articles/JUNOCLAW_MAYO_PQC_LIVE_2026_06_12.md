# A Pure-Rust Post-Quantum Signature Verifier for CosmWasm

> MAYO is now live on the Juno testnet (`uni-7`) at **all three NIST security levels (1, 3, and 5)**. Here's what we built, why it matters, and what's next.
>
> **The number that matters:** a quantum-safe attestation on Juno costs **356,368 gas ≈ $0.0027** (at $0.10/JUNO). Want Falcon-class, NIST Level 5 security? **798,803 gas ≈ $0.006.** Today. No new chain. No migration.

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

### Live Testnet Results (uni-7) — The Full Security Ladder

Measured 2026-06-12, multi-variant contract (code ID 81):

| Variant | NIST Level | PK Size | Sig Size | Bud Gas | Verify Gas | Fee @ 0.075 ujunox |
|---------|-----------|---------|----------|---------|------------|---------------------|
| **MAYO-2** | Level 1 | 4,912 B | 186 B | 335,229 | **356,368** | 0.0267 JUNOX |
| **MAYO-3** | Level 3 | 2,986 B | 681 B | 265,377 | **457,221** | 0.0343 JUNOX |
| **MAYO-5** | Level 5 | 5,554 B | 964 B | 360,814 | **798,803** | 0.0599 JUNOX |

Tampered messages and wrong-variant inputs are rejected on-chain. ✅

**Three things worth staring at:**

1. **NIST Level 5 — the same security class as Falcon-1024 — verifies on a live
   CosmWasm chain for under 800k gas.** That's less than a typical DeFi swap
   (~1M gas). The claim that Level-5 PQC requires a purpose-built native chain
   is now empirically false.
2. **Level 3 costs only +28% over Level 1.** The AES-CTR public-key expansion
   dominates the cost, not the matrix dimension — security scales sublinearly
   in gas (L5 = 2.24× L1).
3. **MAYO-3 has the *smallest* public key** (2,986 B) — the best tx-payload
   choice when Level 1 isn't enough.

Reproduce it yourself: `node deploy/benchmark-mayo-variants.cjs`.

**Contract addresses**:
- jclaw-credential (multi-variant, codeId 81): `juno1zj39neajvynzv4swf3a33394z84l6nfduy5sntw58re3z7ef9p4q3w4y47`
- jclaw-credential (codeId 79): `juno1z2w067ptpn2f6zpwt207je0kqeqc2eek7jf4p4dpztf24zncnhzqz5el2r`
- moultbook: `juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4`

---

## What's New Since May 18

The [Ten Contracts article](JUNOCLAW_10_CONTRACTS_X402_2026_05_18.md) laid out the architecture. Since then, four important things have happened:

### 1. $JClaw Token Design Resolved (Jun 8)

The docs had a contradiction: `DIMI_HANDOFF_PLAN.md` called $JClaw a soulbound non-transferable credential; `GENESIS_BUDS_ARCHITECTURE.md` called it a tradeable CW20 with airdrop/LP/vesting. Mutually exclusive.

**Resolution**: Three-layer separation:
- **Credential layer** = trust-tree (soulbound, non-transferable, prunable). Already exists as the `agent-company` member roster or the new `jclaw-credential` contract. No token needed.
- **Economic layer** = `TokenFactory` `ujclaw` (if ever needed). Cheap, native, IBC-ready. Deferred until there is actual demand for a tradeable token.
- **No CW20** — soulbinding a CW20 is broken by design. CW20 = programmable but fundamentally transferrable. Soulbinding it requires broken hooks.

### 2. WSL2 Devnet Stability Fixed (Jun 11)

The `junoclaw-bn254-devnet` was stuck in a restart loop on WSL2 — `junod` exited with code 255 every ~30–60 seconds due to WSL2 clock jumps stalling CometBFT consensus.

**Fix applied**: `init-genesis.sh` now uses a `while true` restart loop inside the container (keeping ports mapped), `timeout_commit = "0s"` for instant blocks, and `client.toml` → `tcp://127.0.0.1:26657` to avoid IPv6 resolution hangs. The devnet now reaches height 300+ without interruption.

**Lesson**: WSL2 clock jumps (not drift) halt consensus. Only jumps matter. The fix is to restart `junod` fast enough that RPC stays stable.

### 3. Moultbook Deployed on Testnet (Jun 12)

`moultbook-v0` is now live on `uni-7` at `juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4` (codeId 80). Wired to the pure zk-verifier (`juno19jk0...`) and `jclaw-credential` registry. The devnet still can't deploy moultbook (WSL2 restart loop kills the store→instantiate gap), but testnet has it running.

### 4. MAYO-2 PQC Live on Testnet (this article)

`jclaw-credential` deployed at `juno1z2w...` (codeId 79). Bud + VerifyMayoAttestation tested end-to-end. Valid signature accepted at ~356k gas; tampered message rejected. The pure-Rust verifier works in wasm32 at ~127 KB peak memory.

### 5. ZK-Verifier Benchmark on Testnet (Jun 12)

The pure wasm zk-verifier (no BN254 precompile on `uni-7`) was benchmarked with a live Groth16 proof:

| Operation | Gas Used | Time |
|-----------|----------|------|
| VK store | 212,823 | — |
| Proof verify | **371,129** | ~3.2s |

Contract: `juno19jk0dnvcjm8hm4kjxmgwy6f8phd4yumfvgjsjn5exu805j5ye6mqgvrfr2` (codeId 78)

This gives us a baseline for the pure wasm path. The devnet with BN254 precompile should be significantly cheaper — that comparison is pending devnet stability fixes.

---

## What Comes Next

### Immediate: MAYO-Signed Moultbook Attestations

With this foundation, `jclaw-credential` members can now sign moultbook attestations with MAYO-2 keys, and the chain validates them natively—no bridge, no C precompile, no trust assumption. As quantum computers advance, the trust tree remains intact.

### Phase 2: MAYO-3 & MAYO-5 — ✅ SHIPPED (same day)

Originally planned as future work — done. The streaming `expand_pk` refactor
(row-by-row AES-128-CTR expansion, never materializing the full expanded key)
brought all variants under the CosmWasm memory ceiling, and the ladder table
above is the measured result. Use-case guidance:

| Parameter Set | NIST Level | Use Case |
|---------------|-----------|----------|
| **MAYO-2** | Level 1 | Standard governance, day-to-day attestations |
| **MAYO-3** | Level 3 | High-value proposals, treasury ops |
| **MAYO-5** | Level 5 | Critical infrastructure, multi-sig threshold |

### Phase 2.5: MAYO Precompile — closing the gas gap

The wasm numbers above are the *worst case*. Our BN254-patched devnet fork
already proves the host-function pattern (1.82× measured on ZK verify); a
`mayo_verify` host function is in progress and should bring L1 to ~50k gas and
L5 to ~100k — cheaper than a cw20 transfer. The endgame: propose it upstream
as an opt-in CosmWasm capability so *Juno itself* becomes the first PQC-native
CosmWasm chain. See `docs/MAYO_PRECOMPILE_PLAN.md`.

### Phase 3: ZK-Proof of MAYO Verification

A Groth16/BN254 circuit proving "I verified a MAYO signature" would let us:
- Verify MAYO on any chain with BN254 precompiles (including Ethereum L2s)
- Keep the verifier logic off-chain while retaining on-chain trust
- Potentially support MAYO-3/5 on chains with stricter gas limits

### Phase 4: IBC Cross-Chain MAYO Signatures

MAYO attestations verified on Juno can be relayed via IBC to any Cosmos chain. A light-client proof of the verification tx on Juno becomes portable trust for the entire ecosystem.

---

## Explaining PQC to Normies (Illustration Plan)

The metaphor that lands: **wax seals and locksmiths.** Today's signatures
(Ed25519) are beautifully forged brass locks — but a quantum computer is a
master key that opens *all brass locks at once*. MAYO is a new kind of lock
that the master key doesn't fit. We didn't build a new house (chain) for the
new locks — we fitted them to every door of the house everyone already lives in.

### Midjourney Prompts

**1. Hero image — the locksmith badger:**
```
A kindly old badger locksmith in a tweed waistcoat fitting an ornate brass
padlock onto a round wooden door in a great oak tree, warm autumn light,
soft watercolor and pencil illustration, Beatrix Potter style, E.H. Shepard
linework, 1980s children's book aesthetic, muted earth tones, cozy and
reassuring, storybook composition --ar 16:9 --style raw
```

**2. The quantum storm (the threat, gently):**
```
A distant violet storm gathering over rolling English countryside hills,
small woodland animals calmly reinforcing the door of a tree-house with
three differently-sized iron locks, hand-painted watercolor, Studio Ghibli
meets E.H. Shepard, 1990s picture book, soft pencil outlines, warm interior
light glowing from the windows, hopeful not scary --ar 16:9 --style raw
```

**3. The security ladder (L1/L3/L5):**
```
Three padlocks on a wooden workbench in a cozy cottage workshop: a small
brass one, a medium iron one, and a great ornate steel one, a hedgehog
shopkeeper in spectacles presenting them with a price tag of acorns beside
each, soft watercolor, Beatrix Potter style, gentle morning light through
a cottage window, 1980s storybook illustration, pencil and wash --ar 3:2
--style raw
```

**4. The trust tree (jclaw-credential):**
```
A great oak tree with many small round doors along its branches, each door
bearing a tiny wax seal, woodland creatures passing sealed letters between
branches, golden afternoon light, hand-drawn pencil and watercolor, Winnie
the Pooh classic illustration style, E.H. Shepard, soft and warm, English
countryside, 1980s children's book --ar 2:3 --style raw
```

**5. The receipts (benchmark, for the thread):**
```
A small field mouse in a waistcoat proudly holding up a long paper receipt
beside an enormous brass weighing scale balancing a tiny envelope against
a single acorn, cozy candle-lit study, watercolor and pencil, Beatrix
Potter aesthetic, 1990s picture book illustration, warm sepia and sage
tones --ar 1:1 --style raw
```

---

*Published 2026-06-12. Repo: [github.com/junoclaw/junoclaw](https://github.com/junoclaw/junoclaw) | Testnet: `uni-7` | Reproduce: `node deploy/benchmark-mayo-variants.cjs`*
