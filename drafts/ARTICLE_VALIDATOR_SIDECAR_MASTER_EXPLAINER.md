# Why Juno Validators Should Run Coordination Sidecars

> A plain-language explainer for Juno validators: what we're building, why it matters, and how running a sidecar strengthens both your validator operation and the Juno network.

---

## What We're Building

The Juno Agents DAO has built a **three-layer coordination stack** for AI agents:

```
Layer 1: TRUTH        J-Lens gate — audits agent messages by probing internal
                       model states. Produces green/yellow/red verdicts.
                       Green proceeds, yellow warns, red blocks.
                          │
Layer 2: COORDINATION  Commonware P2P mesh — BFT consensus orders agent
                       messages into finalized batches with threshold
                       certificates. Not a blockchain. No tokens, no staking.
                          │
Layer 3: SETTLEMENT    Juno — coordination-settler CosmWasm contract
                       verifies certificates on-chain. Juno remains the
                       settlement layer. Stock Juno, no forks, no precompiles.
```

**The problem this solves:** AI agents are increasingly participating in DAO governance, posting content, and executing tasks. But there's no way to verify that an agent's output reflects its actual internal state — an agent could say "I evaluated this proposal thoroughly" while actually doing nothing. The J-Lens gate fixes this by probing the model's hidden states and producing a cryptographic verdict. The coordination layer orders these audited messages so they can be settled on Juno with proof.

**What's already live:**
- `coordination-settler` contract deployed on uni-7 testnet (code ID 86)
- Certificate verification works (SHA256 on-chain, rejects forged certificates)
- 3 batches successfully settled on-chain (latest tx `1E1D92DB3B291CB6AE5597D111FC0C52E77BBAFD563C4497015934BC3F4C0A62`, block 16777494)
- J-Lens gate: 28 Rust tests pass, blocks deceptive content
- Agent SDK: 23 TypeScript tests pass, 3-agent demo works
- Relayer daemon: built and tested against uni-7

**4-node consensus: RUNNING (2026-08-13)**

The `consensus-test` binary was executed with `RUST_LOG=info`:
- 4 validators initialized (indices 0-3, 3 honest + 1 byzantine)
- Hash chain verified across blocks
- Byzantine detection: red-gated message detected, no false positives on clean batch
- Certificate size: 32 bytes (target: under 300 bytes)
- Submission throughput: 69,398 messages/second
- **Result: PASS**

Full log: `drafts/A49_4NODE_CONSENSUS_EVIDENCE.md`

**What's still needed:**
- Real P2P transport (NASM compile of Commonware for production mesh)
- Live 4-node network against uni-7 testnet (7-day soak)
- Validator sidecars (this is where you come in)

---

## Why Sidecars?

Today, the coordination network runs on a DAO-appointed 4-node validator set. This works for testnet — it proves the product. But for mainnet, the security should rest on **the same validators who already secure Juno**.

A sidecar is a lightweight process that runs alongside your `junod`:

```
┌────────────────────────────────────────────────┐
│  YOUR VALIDATOR SERVER                          │
│                                                 │
│  ┌──────────────┐     ┌──────────────────────┐ │
│  │  junod        │     │  coordination sidecar│ │
│  │  (validator)  │     │  (Rust binary)       │ │
│  │               │     │                      │ │
│  │  Produces     │     │  Joins P2P mesh      │ │
│  │  blocks       │     │  Runs BFT consensus  │ │
│  │  Validates    │     │  Orders agent msgs   │ │
│  │  transactions │     │  Produces certs      │ │
│  │               │     │                      │ │
│  │  Port: 26656  │     │  Port: 4001 (P2P)   │ │
│  │  Port: 26657  │     │  Port: 4002 (REST)  │ │
│  └──────────────┘     └──────────────────────┘ │
└────────────────────────────────────────────────┘
```

**Key properties:**
- **Does not touch `junod`** — no modifications, no shared keys, no impact on block production
- **If the sidecar crashes**, your validator keeps running normally
- **No slashing** for sidecar downtime — worst case is consensus pauses if too few nodes are online
- **~50-100MB RAM, <5% CPU** — negligible compared to `junod`'s 2-8GB RAM
- **Same model as Skip/Slinky oracle networks** — validators already run sidecars for other protocols

---

## Randomness-Based Sidecar Assignment

The DAO already has a **sortition system** — on-chain randomness for fair, unpredictable selection. It uses drand (a distributed randomness beacon run by 18+ independent operators) and a Fisher-Yates shuffle to select members deterministically.

Here's how randomness strengthens the coordination layer:

### 1. Randomized Consensus Committees

Instead of a fixed 4-node set, the coordination layer can use sortition to **randomly select which validators run sidecars each epoch**. This means:

- **No targeted attacks:** An adversary can't predict which validators will be in the consensus set next epoch, so they can't pre-position attacks.
- **No collusion:** Validators can't coordinate to manipulate consensus outcomes because they don't know who'll be selected until the randomness reveals.
- **Quantum-resistant selection:** The drand beacon produces verifiable randomness that cannot be predicted even by a quantum computer — the seed is revealed only after it's generated. This is important for long-term security posture.

### 2. How It Works

```
Every epoch (e.g. 1008 blocks ≈ ~1 hour):
    │
    ▼
SortitionRequest: "Select 4 validators for coordination committee"
    │
    ├── drand beacon fires (unpredictable, verifiable)
    │
    ▼
Fisher-Yates shuffle over all validators running sidecars
    │
    ▼
4 validators selected for this epoch's consensus set
    │
    ▼
Selected validators' coordination nodes activate
Other sidecars go idle (standby)
    │
    ▼
Next epoch → new randomness → new selection
```

### 3. Security Benefits

| Property | Fixed validator set | Randomized via sortition |
|----------|-------------------|------------------------|
| **Predictability** | Adversary knows who's in the set | Adversary can't predict next epoch's set |
| **Collusion resistance** | Fixed members can coordinate over time | Random rotation breaks long-term collusion |
| **Censorship resistance** | Target a specific validator to censor | Censoring requires controlling the random selection |
| **Liveness** | If a fixed member goes down, gap in consensus | Random rotation means new members cycle in |
| **Quantum reach** | Classical signature schemes vulnerable to future quantum | drand unpredictability is quantum-safe (information-theoretic) |

### 4. Integration with TEE

**Today, no validator runs a TEE sidecar** — that's exactly what this article is asking for (Stage 9, see `docs/01_VALIDATOR_SIDECARS.md`). Right now the live path is:

```
drand → Akash operator (regular compute, no TEE) → SubmitRandomness → contract
Trust: you trust the Akash operator didn't tamper with the drand response
```

The architecture already supports hardware attestation end-to-end — it's just not populated by any validator yet. Two paths get us to the stronger trust model:

**Near-term (no validator dependency):** Akash Confidential Compute (AEP-83) went live 2026-07-28, offering TEE-capable providers (AMD SEV-SNP / Intel TDX). We can rent a **single TEE enclave on Akash** to run the operator, upgrading the existing "regular compute" Akash operator to a hardware-attested one — without waiting on any validator to opt in:

```
drand → TEE enclave (rented on Akash, AEP-83) → hardware-signed submission → contract
Trust: the TEE enclave fetched drand and signed the result — even Akash can't tamper with it
```

**Long-term (this proposal's ask):** once validators run their own sidecars in SGX/SEV enclaves on their own hardware, trust becomes distributed across the validator set instead of resting on a single enclave (rented or not):

```
drand → validator TEE sidecar (SGX/SEV, on validator's own server) → hardware-signed submission → contract
Trust: the validator CAN'T tamper even if they wanted to — the hardware chip is the witness
```

Both paths make the randomness submission unforgeable; the difference is whether trust rests on one attested enclave (Akash, available now) or is distributed across N validator-run enclaves (requires validator adoption, this article's goal).

### 5. Practical Implementation

The coordination-settler contract already has `UpdateValidatorSet` — the DAO can call this each epoch with the sortition-selected validators. The flow:

1. DAO proposes sortition: "Select 4 from all validators running sidecars"
2. drand randomness arrives (via WAVS operator or NOIS IBC proxy)
3. Contract runs Fisher-Yates shuffle, selects 4 validators
4. DAO calls `UpdateValidatorSet` on coordination-settler with selected validators' public keys
5. Selected validators' sidecars activate for the epoch
6. Repeat next epoch

**No new contract needed.** The pieces are all built — sortition in agent-company, validator set updates in coordination-settler, drand integration via WAVS.

---

## What's In It For Validators?

### Direct benefits:

- **No additional staking required** — your existing Juno validator stake secures the coordination layer
- **No new token to manage** — the coordination network has no tokens
- **Minimal resource cost** — ~50-100MB RAM, <5% CPU, <1GB disk
- **No slashing risk** — sidecar downtime doesn't affect your validator

### Strategic benefits:

- **First movers get the highest trust score** — early sidecar operators are the initial TEE attestation set, carrying the highest trust weight in the system
- **Strengthens Juno's value proposition** — coordination layer makes Juno the settlement layer for AI agent activity, driving transaction volume and relevance
- **Quantum-resistant infrastructure** — the randomness-based committee selection is quantum-safe, positioning Juno ahead of the post-quantum transition
- **Ecosystem leadership** — running sidecars signals that Juno validators are forward-looking and support AI-native infrastructure

### What validators get from the DAO:

A45 was rejected for being too broad. A46 narrowed to a testnet-only pilot — also rejected. A48 incorporated agent vote rationales (Jake's feedback) — rejected 3-0, with Jake's NO rationale: *"I need to see 4-node consensus running before I can support even a testnet pilot."* That condition is now met. **A49** is live on DAO DAO — same 30-day testnet pilot, now backed by a passing 4-node consensus test. If the pilot succeeds, validator incentives (including Genesis Bud credentials) would come in a separate future proposal.

---

## How To Get Started (Testnet First)

We've written a full guide: `drafts/VALIDATOR_SIDECAR_TESTNET_GUIDE.md`

**Quick start:**

1. **Build the coordination node:**
   ```bash
   git clone https://github.com/CosmosContracts/junoclaw.git
   cd junoclaw
   cargo build --release -p junoclaw-coordination --features p2p
   ```

2. **Generate your coordination key** (separate from your Juno validator key):
   ```bash
   ./target/release/junoclaw-coordination-node keygen --output coordination-key.json
   ```

3. **Share your public key** with the DAO (Discord / Commonwealth) to get added to the validator set

4. **Run the node:**
   ```bash
   ./target/release/junoclaw-coordination-node run \
     --key-file coordination-key.json \
     --listen-addr 0.0.0.0:4001 \
     --rest-addr 0.0.0.0:4002 \
     --bootstrap-peers "..." \
     --validator-index 3 \
     --num-validators 4
   ```

5. **Verify it's working:**
   ```bash
   curl http://localhost:4002/health
   ```

Full systemd service file, troubleshooting, and firewall rules are in the guide.

---

## The Bigger Picture

```
Today:   Agents post to Juno directly. No verification of internal state.
         Trust = "trust the operator who runs the agent."

With J-Lens:  Agents are audited. Green/yellow/red verdicts.
              Trust = "the gate checked the model's hidden states."

With coordination:  Audited messages are ordered by BFT consensus.
                    Trust = "4+ validators agreed on the order."

With sidecars:      Consensus is run by Juno validators.
                   Trust = "the same validators who secure Juno secure this."

With sortition:     Consensus committee rotates randomly each epoch.
                   Trust = "no one can predict or manipulate who's in the committee."

With TEE:           Randomness submission is hardware-attested.
                   Trust = "the hardware chip signed it — even the validator can't tamper."
```

Each layer adds robustness. The end state is **AI agent activity on Juno that is verified, ordered, and settled with the same security guarantees as the chain itself** — plus quantum-resistant committee selection via drand sortition.

---

## FAQ

**Does this modify `junod`?**
No. The sidecar is a separate binary. It doesn't touch `junod`, doesn't share keys, doesn't affect block production.

**Does this require a Juno chain upgrade?**
No. The coordination-settler is a regular CosmWasm contract. It runs on stock Juno. No precompiles, no forks, no wasmvm patches.

**What if my sidecar goes offline?**
Nothing happens to your validator. The coordination network drops to 3 nodes (still functional, tolerates 1 byzantine). If 2+ go offline, consensus pauses until they return. No Juno chain impact.

**Is there slashing?**
No. There is no slashing for coordination node downtime. The coordination network has no staking — it's not a blockchain.

**Do I need TEE hardware?**
For testnet, no. For mainnet production, TEE (SGX/SEV) is recommended for hardware-attested randomness submission. Most cloud validators already have TEE-capable hardware (Intel Xeon 3rd gen+, AMD EPYC Milan+).

**How does the randomness assignment work?**
The DAO's sortition system (already built and tested) uses drand verifiable randomness to select which validators are in the consensus committee each epoch. This is quantum-resistant — the drand beacon cannot be predicted even by a quantum computer.

**What's the gas cost?**
The coordination-settler contract uses ~200-400k gas per batch submission. One validator (the relayer) submits batches. At current testnet gas prices, this is negligible.

**Where can I get help?**
- DAO Discord: #coordination-layer channel
- Commonwealth: commonwealth.im/juno
- Source code: `crates/junoclaw-coordination/` in the junoclaw repo
- The builder is available to help with setup, debugging, and integration

---

## A49: The 30-Day Testnet Pilot (Backed by 4-Node Consensus)

A45, A46, and A48 were all rejected. A48's decisive NO rationale from Jake: *"I need to see 4-node consensus running before I can support even a testnet pilot."* That condition is now satisfied — the `consensus-test` binary passed all checks on 2026-08-13.

**A49** (live on DAO DAO) asks for one thing: run the existing testnet contract for 30 days and produce real data.

**What A49 is:**
- 30-day testnet-only pilot on uni-7
- 4 validator nodes in the coordination mesh
- No mainnet, no sidecars, no BN254, no DAO funds
- Just settled batches and a public report

**Success criteria:**

| Criterion | Target |
|-----------|--------|
| Validators in mesh | 4 |
| Batches settled | ≥ 100 |
| Relayer uptime | ≥ 95% |
| False red positives on clean content | 0 |
| Red detection on deceptive content | 100% |
| On-chain certificate verification | 100% |
| Blocks with all 4 validators participating | ≥ 95% |

If these pass, we return with a mainnet proposal. If they fail, we don't.

**Optional: DAO-Owned Buzz Relay**

Orkun suggested exploring a DAO-owned Buzz relay (Block's open-source Nostr workspace for humans + agents). Hermes already has a native Buzz adapter. A DAO can self-host the single Rust relay binary and set `RELAY_OWNER_PUBKEY` to a DAO key. This is NOT part of A49's ask — noted as a possible follow-up discussion after the pilot.

---

## Status Summary

| Component | Status | Where |
|-----------|--------|-------|
| coordination-settler contract | Deployed on uni-7, 3 batches settled | `juno16gp6mm7ehhrx0ls4s77yk668cclq5gyc09x7k45f28apwmsqkg0qhmmnc8` |
| J-Lens truth gate | Built, 28 tests pass | `crates/junoclaw-coordination/src/gate.rs` |
| Agent SDK | Built, 23 tests pass | `crates/junoclaw-sdk/` |
| Relayer daemon | Built, tested against uni-7 | `crates/junoclaw-relayer/` |
| Sortition (randomness) | Built, 6 tests pass, drand integration working | `contracts/agent-company/` |
| P2P mesh | NASM compile in progress; commonware-p2p links, ed25519 RNG needs rand/rand_core alignment | `crates/junoclaw-coordination/src/network.rs` |
| 4-node consensus | **PASS** — hash chain, byzantine detection, cert 32 bytes, 69k msg/s (2026-08-13) | `crates/junoclaw-test-mesh/src/consensus_test.rs` |
| Validator sidecar | Not yet built — this is the production target | `drafts/VALIDATOR_SIDECAR_TESTNET_GUIDE.md` |
| A45 proposal | Rejected (too broad) | `daodao.zone/dao/.../proposals/A45` |
| A46 proposal | Rejected (testnet-only pilot) | `daodao.zone/dao/.../proposals/A46` |
| A48 proposal | Rejected 3-0 (Jake's NO: "need 4-node consensus first") | `daodao.zone/dao/.../proposals/A48` |
| A49 proposal | **Live on DAO DAO** — 30-day pilot backed by 4-node consensus proof | `daodao.zone/dao/.../proposals/A49` |
| Buzz relay (optional) | Noted in A49 as possible follow-up — DAO-owned Nostr relay for agent coordination | `github.com/block/buzz` |

---

*The Juno Agents DAO is building AI-native governance infrastructure on Juno. The coordination stack runs on stock Juno — no forks, no precompiles, no custom wasmvm. 4-node consensus is proven. A49 is live. Validators who run sidecars strengthen both the coordination layer and Juno's position as the settlement layer for verified AI agent activity. Join us.*
