# Validator Sidecars — TEE-Only Attestation for Juno Validators

> Sidecars are NOT Akash. Sidecars are NOT the regular operator.
> Sidecars are a TEE-only process that validators run alongside their Juno node.

---

## What Is a Sidecar?

In blockchain, a "sidecar" is a lightweight process that runs **next to** a validator node. It doesn't touch consensus. It doesn't affect block production. It's a separate Docker container that does extra work.

```
┌────────────────────────────────────────────────┐
│  VALIDATOR SERVER                               │
│                                                 │
│  ┌──────────────┐     ┌──────────────────────┐ │
│  │  junod        │     │  WAVS SIDECAR        │ │
│  │  (validator)  │     │  (TEE container)     │ │
│  │               │     │                      │ │
│  │  Produces     │     │  Watches Juno events │ │
│  │  blocks       │     │  Runs WASI component │ │
│  │  Validates    │     │  INSIDE SGX/SEV      │ │
│  │  transactions │     │  enclave             │ │
│  │               │     │  Submits hardware-   │ │
│  │  Port: 26657  │     │  signed attestation  │ │
│  │  (RPC)        │     │                      │ │
│  └──────────────┘     └──────────────────────┘ │
│         │                       │               │
│         │ local RPC             │ submits TX    │
│         └───────────────────────┘               │
└────────────────────────────────────────────────┘
```

The sidecar talks to the validator's own local RPC (no external calls needed). It submits attestation TXs to the chain using the validator's operator wallet.

---

## Why TEE-Only?

The whole point of the sidecar is **hardware attestation**. Without TEE, the sidecar is just... another computer running code. Anyone could tamper with it.

| | Without TEE | With TEE (sidecar) |
|---|---|---|
| **Computation** | Correct (same code) | Correct (same code) |
| **Proof** | "Trust me, I ran it" | Hardware chip signed the result |
| **Tamper-proof** | ❌ Operator could modify output | ✅ Even the operator can't modify output |
| **Who trusts it** | Only if you trust the operator | Anyone — the chip is the witness |

**If a validator doesn't have SGX/SEV hardware, they don't run the sidecar.** There's no point. The Akash operator handles non-TEE verification.

### Hardware Requirements

Most cloud validators already have TEE-capable hardware:

| CPU | TEE Type | How to Check |
|-----|----------|-------------|
| Intel Xeon (3rd gen+) | SGX | `ls /dev/sgx_enclave` |
| AMD EPYC (Milan+) | SEV | `ls /dev/sev` or `dmesg | grep SEV` |
| AMD EPYC (Genoa+) | SEV-SNP | `dmesg | grep SEV-SNP` |

Bare-metal servers from Hetzner, OVH, Latitude.sh, Equinix Metal commonly have these chips.

---

## How Sidecars Differ from Akash

This is the key distinction:

```
AKASH OPERATOR                      VALIDATOR SIDECAR
──────────────                      ─────────────────
Runs on rented Akash server         Runs on validator's own server
Regular compute (no TEE)            TEE REQUIRED (SGX or SEV)
One instance, always on             Many instances (one per validator)
Ensures verification happens 24/7   Ensures verification is hardware-attested
Anyone can run it                   Only validators with TEE hardware
Paid in AKT                         Free (validator's existing hardware)
Single point of trust               Distributed trust across validator set
```

**They complement each other:**

```
                    ┌─────────────┐
                    │  Juno Chain  │
                    │  (agent-co.) │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
    ┌─────────▼──┐  ┌──────▼─────┐  ┌──▼──────────┐
    │  Akash     │  │ Validator  │  │ Validator   │
    │  Operator  │  │ Sidecar A  │  │ Sidecar B   │
    │  (no TEE)  │  │ (SGX)      │  │ (SEV)       │
    │            │  │            │  │             │
    │  Always on │  │  TEE proof │  │  TEE proof  │
    │  Baseline  │  │  Trust ++  │  │  Trust ++   │
    └────────────┘  └────────────┘  └─────────────┘
         │                │                │
         ▼                ▼                ▼
    attestation_1    attestation_2    attestation_3
    (software)       (SGX-signed)     (SEV-signed)
```

The Akash operator is the **fallback** — it always runs, ensures no proposal goes unverified. Validator sidecars are the **trust amplifiers** — when they exist, attestations are hardware-signed and the system reaches full trustlessness.

---

## What the Sidecar Actually Runs

The exact same WASI component that runs on Akash:

```
wavs-sidecar/
├── docker-compose.yml          # One container, mounts /dev/sgx_enclave
├── .env                        # Validator's operator wallet mnemonic + local RPC
└── component/
    └── junoclaw_verify.wasm    # 355KB — identical to Akash version
```

The docker-compose is minimal:

```yaml
services:
  wavs-sidecar:
    image: ghcr.io/lay3rlabs/wavs:1.5.1
    devices:
      - /dev/sgx_enclave:/dev/sgx_enclave     # Intel SGX
      - /dev/sgx_provision:/dev/sgx_provision  # Intel SGX
      # OR for AMD:
      # - /dev/sev:/dev/sev                   # AMD SEV
    environment:
      - WAVS_MODE=operator
      - WAVS_CHAIN_RPC=http://localhost:26657  # Validator's own RPC
      - WAVS_CONTRACT=juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6
      - WAVS_CHAIN_ID=uni-7
      - WAVS_COMPONENT_REGISTRY=wa.dev
    restart: unless-stopped
```

**Key differences from Akash:**
- `devices:` mounts the TEE hardware device into the container
- `WAVS_CHAIN_RPC` points to `localhost` (validator's own node, not external RPC)
- No aggregator or IPFS needed (the sidecar is self-contained)
- The WAVS runtime detects the TEE device and automatically runs the component inside the enclave

---

## When Do Sidecars Come In?

**Not yet.** The staging hierarchy with dates:

```
Stage 4: Akash deployment (regular compute)        Mar 17, 2026 ✅ DONE
Stage 5: Juno governance proposal                  Mar 17, 2026 ✅ TODAY
Stage 6: Root → Genesis (if passes)               ~Mar 24, 2026
Stage 7: Mainnet contracts                         ~Mar 25-28, 2026
Stage 8: Genesis buds → 13                         ~Mar 28-31, 2026
Stage 9: VALIDATOR SIDECAR PROPOSAL               ~Apr 1-7, 2026 ← HERE
         ▲
         │
         FreeText proposal by the 13 buds (not Genesis).
         Asks validators with SGX/SEV to run the sidecar.
         SEPARATE from the Juno chain governance prop.
```

The sidecar proposal is a **FreeText proposal** within JunoClaw's DAO (not a Juno chain governance prop). It signals the community: "We're ready for TEE-grade attestation. Validators, please run the sidecar."

**Why wait?**
1. Need mainnet contracts first (validators won't sidecar for testnet)
2. Need the DAO active (13 buds voting, not Genesis auto-passing)
3. Need Jake/WAVS buy-in (already supportive — "get a few agents building it out")
4. Validators need clear ROI (JCLAW token incentive at Stage 10)

---

## The Validator Incentive

From the `VALIDATOR_PROPOSAL_THREAD.md`:

> Every validator who runs the sidecar for the first 30 days receives a JunoClaw Genesis Bud — a $JClaw soulbound trust-tree credential.

The first 5 validators to run sidecars become the initial TEE attestation set. Their attestations carry the highest trust score in the system. When JCLAW token launches (Stage 10), these early validators receive the largest allocation from the "WAVS operators" pool (10% of total supply).

---

## Multi-Operator Consensus (Future)

When multiple validators run sidecars, the aggregator (currently on Akash) collects their attestations and requires M-of-N agreement:

```
Validator A (SGX) ──── attestation_hash_a ────┐
Validator B (SEV) ──── attestation_hash_b ────┤──▶ Aggregator ──▶ On-chain
Validator C (SGX) ──── attestation_hash_c ────┘    (2-of-3)
                                                       │
                                              ┌────────┘
                                              ▼
                                     All 3 must produce
                                     the SAME hash
                                     (deterministic WASI)
                                     
                                     If 2 agree and 1 differs,
                                     the outlier is slashable
```

This is the end-state: **distributed TEE attestation across the Juno validator set**, with economic penalties for cheating. No single operator can fake a result.

---

## Summary Table

| Layer | Where It Runs | TEE? | Purpose | When |
|-------|--------------|------|---------|------|
| **Akash operator** | Rented Akash server | ❌ No | Always-on baseline verification | Stage 4 (next) |
| **Validator sidecar** | Validator's own server | ✅ Yes (required) | Hardware-attested distributed trust | Stage 9 (after buds) |
| **On-chain randomness** | Inside agent-company contract | N/A | Fair jury selection via sortition | Already built |
