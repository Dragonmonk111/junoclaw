# BN254 Pairing Precompile — The Case for Juno

> Preparation for Q3 of the ZKP discussion with Jake:
> *"Is there any appetite for a BN254 pairing precompile in a future Juno upgrade?"*

---

## 1. Gas Cost Analysis

### The Three Scenarios

| Scenario | Gas Cost | Fits Juno? | Notes |
|----------|----------|------------|-------|
| **Current** (store hash only) | ~200K SDK gas | Easy | No verification, trust-the-operator |
| **Groth16 in pure CosmWasm** | ~5–10M SDK gas | Borderline | Soft limit ~10M per tx on Juno |
| **Groth16 via BN254 precompile** | ~187K SDK gas | Trivial | Same cost as storing a hash |

### How the numbers work

**Ethereum (with EIP-196/197 precompiles):**
Nebra's analytical formula for Groth16 verification with `l` public inputs:

```
gas ≈ (181 + 6 × l) kgas
```

For JunoClaw's use case (1–2 public inputs: `data_hash` + `task_type`):
- `l = 2` → **(181 + 12) = 193K gas**

This is cheaper than a typical token transfer.

**Ethereum precompile gas costs (post EIP-1108, Istanbul fork):**

| Operation | Gas |
|-----------|-----|
| BN254 point addition (ECADD) | 150 |
| BN254 scalar multiplication (ECMUL) | 6,000 |
| BN254 pairing check (base) | 45,000 |
| BN254 pairing check (per pair) | 34,000 |

A Groth16 verification requires: 1 ECADD, ~3 ECMULs, 1 pairing check with 3 pairs.
Total: `150 + 18,000 + 45,000 + 102,000 ≈ 165K gas` (plus overhead = ~187K).

**Pure CosmWasm (no precompile):**
BN254 field arithmetic requires modular exponentiation, extension field operations, and Miller loop computation — all in Wasm. Each Wasm instruction is individually metered by CosmWasm's gas model at ~1 Teragas/ms CPU time, converted to SDK gas via `DefaultGasMultiplier = 100`.

Rough estimate for a full BN254 pairing in Wasm:
- Miller loop: ~5B Wasm gas → ~50M SDK gas (way over limit)
- With optimized Rust (e.g. `ark-bn254`): ~500M–1B Wasm gas → **5–10M SDK gas**
- Juno block gas limit: ~10M per transaction (soft), ~40M per block

**Verdict**: Pure CosmWasm Groth16 *barely* fits in a single transaction and would consume a significant chunk of block gas. A precompile makes it 50× cheaper.

### Visual comparison

```
Current (store hash):     ████░░░░░░░░░░░░░░░░  200K  (2% of limit)
Groth16 + precompile:     ████░░░░░░░░░░░░░░░░  187K  (2% of limit)
Groth16 pure CosmWasm:    ██████████████████████ 5-10M (50-100% of limit)
                          ────────────────────── 10M SDK gas limit
```

---

## 2. Precedent: Who Has Native ZK Verification?

### Chains with built-in Groth16 support

| Chain | Curve | Method | Since |
|-------|-------|--------|-------|
| **Ethereum** | BN254 (alt_bn128) | EVM precompiles (EIP-196/197) | Byzantium, Oct 2017 |
| **Sui** | BN254 + BLS12-381 | Native Move module `sui::groth16` | Launch, May 2023 |
| **IOTA** | BN254 + BLS12-381 | Native Move module (Sui fork) | 2024 |
| **Polygon** | BN254 | EVM precompile (Ethereum-inherited) | Launch |
| **Arbitrum/Optimism** | BN254 | EVM precompile (Ethereum-inherited) | Launch |
| **Mina** | Pasta curves | Native protocol-level | Launch |

### Cosmos ecosystem status

| Chain | ZK Support | Notes |
|-------|-----------|-------|
| **CosmWasm (core)** | ❌ No BN254 | Issue #751 lists it as "Bonus Points" — secp256k1, ed25519, bls12_381 supported, but NOT BN254 pairing |
| **Sei** | Partial | Has EVM precompiles (via ethermint), but CosmWasm side lacks native BN254 |
| **Injective** | Partial | EVM module inherits Ethereum precompiles, CosmWasm side no BN254 |
| **Neutron** | ❌ | No ZK verification support |
| **Osmosis** | ❌ | No ZK verification support |
| **Juno** | ❌ | No ZK verification support |

### The gap

**No pure Cosmos SDK / CosmWasm chain has native BN254 pairing support.** Juno could be the first. This is both the challenge (no precedent to copy) and the opportunity (differentiation).

### How it would work technically

Two approaches for Juno:

**Option A: CosmWasm host function (recommended)**
Add `bn254_pairing_check`, `bn254_add`, `bn254_mul` as host functions in `wasmvm`, similar to how `secp256k1_verify` and `ed25519_verify` are implemented. This requires:
1. PR to `CosmWasm/cosmwasm` adding the host function signatures
2. PR to `CosmWasm/wasmvm` adding the Go implementation (using `gnark-crypto` or `cloudflare/bn256`)
3. PR to `CosmWasm/wasmd` wiring it into the keeper
4. Juno chain upgrade including the new `wasmd` version

**Option B: Cosmos SDK custom module**
Add a new `x/zkverify` module to Juno's binary that exposes BN254 pairing as a native Cosmos SDK message. Contracts interact via `CosmosMsg::Stargate`. Simpler to implement but less reusable across chains.

**Option C: EVM interop (if Juno adds ethermint)**
If Juno ever adds an EVM module, BN254 precompiles come free from Ethereum's EIP-196/197. But this is a much larger architectural change.

### Effort estimate

| Approach | Dev effort | Chain upgrade? | Reusable? |
|----------|-----------|----------------|-----------|
| CosmWasm host function | 3–5 weeks | Yes | All CosmWasm chains |
| Custom SDK module | 1–2 weeks | Yes | Juno only |
| EVM interop | Months | Yes | N/A |

---

## 3. What JunoClaw Would Prove (Concretely)

### Current (attestation hash only)

```
WAVS TEE computes:
  data_hash        = SHA256(string inputs)
  attestation_hash = SHA256(COMPONENT_ID || task_type || data_hash)

On-chain stores: attestation_hash (64 hex chars)
On-chain verifies: re-computes SHA256 and checks match (Variable 1 hardening)
```

**Trust**: The `data_hash` itself is correct (i.e., the data sources returned what the TEE claims).

### With Groth16 (what changes)

```
WAVS TEE / ZK prover computes:
  data_hash        = SHA256(string inputs)
  attestation_hash = SHA256(COMPONENT_ID || task_type || data_hash)
  groth16_proof    = Prove(circuit, public=[attestation_hash], private=[inputs])

On-chain receives: attestation_hash + groth16_proof + public_inputs
On-chain verifies: Groth16.verify(vk, proof, public_inputs) → true/false
```

**Trust**: Mathematical certainty that the computation producing `attestation_hash` was correct — no hardware trust assumptions.

### The circuit

For JunoClaw's SHA-256 hash chain, the Circom/RISC Zero circuit would encode:

```
// Public inputs
signal input attestation_hash[256]; // expected output bits

// Private inputs (witness)
signal input component_id_bytes[];
signal input task_type_bytes[];
signal input data_hash_bytes[];

// Constraints
component sha = Sha256(...);
sha.in <== concat(component_id_bytes, task_type_bytes, data_hash_bytes);
attestation_hash === sha.out;
```

For RISC Zero (our recommended path), this is even simpler — the "circuit" is just the Rust WASI component we already have. RISC Zero's zkVM proves arbitrary Rust execution.

### Proof size on-chain

A Groth16 proof is 256 bytes (2 G1 points + 1 G2 point on BN254). The verification key is ~1KB and only needs to be stored once per circuit.

---

## The Ask for Jake

### Framing (suggested)

> "Juno could be the first pure CosmWasm chain with native ZK verification. The BN254 pairing precompile is a well-understood primitive — Ethereum has had it since 2017, Sui shipped it at launch. For JunoClaw, it drops ZK verification cost from 'barely fits in a tx' to 'cheaper than a token transfer.' The immediate use case is mathematically proving our WAVS attestation computations. The long-term value is that any CosmWasm contract on Juno could verify ZK proofs — enabling private DeFi, zkML, verifiable AI agents, cross-chain bridges with ZK light clients."

### Concrete next steps we could propose

1. **Signaling proposal**: "Explore BN254 pairing support for Juno" — low commitment, gauges community interest
2. **CosmWasm upstream PR**: Contribute to CosmWasm issue #751 with a concrete BN254 host function implementation. If accepted, all CosmWasm chains benefit and Juno gets it for free in the next `wasmd` upgrade.
3. **PoC on Juno testnet**: Deploy a pure CosmWasm Groth16 verifier (no precompile) to uni-7 to demonstrate both the feasibility and the gas cost problem. This makes the precompile case tangible.

### Risk if we don't

Other Cosmos chains (Neutron, Sei) could ship ZK verification first. Sui already has it. The narrative matters: "Juno is the ZK-enabled CosmWasm hub" vs "Juno is catching up."

---

## Appendix: Key References

- **Nebra Groth16 gas analysis**: https://hackmd.io/@nebra-one/ByoMB8Zf6
- **CosmWasm Crypto API meta-issue**: https://github.com/CosmWasm/cosmwasm/issues/751
- **EIP-196** (BN254 add/mul): https://eips.ethereum.org/EIPS/eip-196
- **EIP-197** (BN254 pairing): https://eips.ethereum.org/EIPS/eip-197
- **EIP-1108** (gas cost reduction): https://eips.ethereum.org/EIPS/eip-1108
- **Sui Groth16 docs**: https://docs.sui.io/guides/developer/cryptography/groth16
- **RISC Zero zkVM**: https://risczero.com
- **BN254 compression**: https://2π.com/23/bn254-compression/
