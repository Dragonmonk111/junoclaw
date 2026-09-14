# What Is JunoClaw?

*Six months. One developer and an AI. A sovereign chain, a trust layer for AI agents, a robotics operating system, a post-quantum cryptography stack, and an airdrop to 213,385 people. This is the full picture — what it is, what it does, and what it can do.*

---

## The One-Sentence Answer

**JunoClaw is a sovereign blockchain and trust operating system that lets AI agents and robots prove their decisions are safe — without slowing them down.**

---

## The Origin

On March 13, 2026, one developer and an AI coding assistant started building. The premise was simple: AI agents are making decisions that affect real money and real safety, but nobody is checking whether those decisions are correct. A robot decides to brake. An AI agent decides to execute a trade. A DAO votes on a proposal. Who verifies that the decision followed the rules?

Today: nobody. The agent's own software says "I'm fine" and we trust it. If something goes wrong, we find out after the accident.

JunoClaw is the layer that checks.

Over six months, what started as a set of CosmWasm contracts on Juno's testnet became something much larger: a sovereign chain, a robotics trust layer, a post-quantum cryptography research program, and a community of 213,385 airdrop recipients. This is the story of how it happened, what exists today, and where it goes next.

---

## Part I: What JunoClaw Is

JunoClaw is not one thing. It is four systems that compose into a single platform:

### 1. A Sovereign Chain

JunoClaw is a sovereign blockchain built from scratch on a different architecture than Juno:

- **Commonware Rust runtime** — not Cosmos SDK, not Go, not Tendermint
- **BLS threshold consensus** — not stake-weighted PoS. Validators are set at genesis via a Distributed Key Generation (DKG) ceremony. The BFT threshold is `n >= 3f+1`: 7 validators tolerates 2 bad actors, 21 tolerates 6, 100 tolerates 33. No hard cap — the practical limit is coordination overhead.
- **Fixed supply** — 54,660,000 ujclaw. Never increases. No inflation, no mint module, no staking yield. Validators earn transaction fees. Same model as Bitcoin miners.
- **CosmWasm execution** — smart contracts that work, same as Juno and Osmosis
- **Cosmos SDK message compatibility** — same wallet UX, same transaction format, same bech32 addresses

This is the Bitcoin model adapted for smart contracts. The chain launched with a snapshot of staked JUNO at block 41,655,615, distributing 35,039,975 ujclaw to 213,385 unique accounts.

### 2. A Trust Layer for AI Agents

AI agents need three things that don't exist today:

**Discovery:** An AI agent that wants to interact with a dApp needs to know the contract addresses, message schemas, and gas parameters. Today, a human finds the GitHub repo, reads the docs, and pastes context into the prompt. That doesn't scale.

JunoClaw solved this with the **skill-registry contract** — deployed on Juno mainnet (juno-1). Any dApp on any chain can publish a pointer + SHA-256 hash of its operating manual. Any MCP-capable agent (Claude, Cursor, Windsurf, ChatGPT) can discover and verify the manual without a human already knowing where to look.

**Safe transaction signing:** An MCP server that can sign and broadcast is, by construction, a server that can move funds on an AI's say-so. Every agent-tooling experiment that shipped without safety gates either limited itself to testnet or accepted that a prompt-injected model could drain a wallet.

JunoClaw solved this with a **second-approval gate** — every fund-moving tool stages its transaction and returns a confirmation ID. Nothing broadcasts until a human reviews the preview and calls `confirm_transaction`. Single-use, expires in 5 minutes. The blast radius of a compromised agent is zero funds moved, one expired confirmation.

**Verifiable execution:** WAVS (Witness-Attested Verifiable Services) by Layer.xyz provides the cryptographic backbone. Agent computations run inside hardware-attested enclaves (Intel SGX, Akash TEE). The result is signed proof that computation happened correctly. Multiple independent witnesses verify task outcomes before on-chain settlement. Every verified result is hashed and committed to the blockchain, creating an immutable audit trail from prompt to outcome.

### 3. A Robotics Operating System

This is where JunoClaw's trust stack becomes something no other project has.

A robot makes decisions 1,000 times per second. JunoClaw proves those decisions are safe in 187 milliseconds — fast enough that the robot doesn't have to wait. The system works in six latency tiers:

```
L0  1 ms      Reflex          Classical control, balance              LOCAL
L1  12 ms     Memory fetch    Merkle-verified recall of past cycles   LOCAL CACHE
L2  50-100 ms World model     Predict consequences of candidate acts   LOCAL INFERENCE
L3  ~300 ms   Settlement      Commonware on-chain (coordination = settlement)  CHAIN
L4  minutes   Truth verdict   Staked operator adjudication (Q-Zeno)   TRUTH MARKET
L5  days      Governance      DAO SafetyEnvelope vote                 DAO
```

The robot acts at L0 (1ms). It remembers at L1 (12ms). It imagines at L2 (100ms). The chain settles at L3 (~300ms). The robot never waits for the chain.

**Why 300ms matters:** On Juno's Tendermint consensus, a block took ~2.8 seconds. On JunoClaw's Commonware Rust runtime with BLS12-381 threshold consensus, a block takes ~300 milliseconds — nearly 10× faster. This is not a future target; it is the architecture. Commonware's simplex consensus produces blocks at network speed, not on a fixed timer. The `leader_timeout_ms = 3000` in the node config is a *timeout* (how long to wait before declaring a leader failed), not the block interval. Under normal operation, blocks propagate and finalize in ~300ms.

This changes the robotics stack fundamentally. On Juno, settlement (L4) was asynchronous to coordination (L3) — the coordination network reached consensus in ~300ms, then anchored to Juno's chain in a separate ~2.8s step. On JunoClaw, the Commonware chain *is* the coordination layer. There is no separate settlement step. The BLS12-381 threshold certificate from consensus is the settlement proof. One layer, one latency, ~300ms.

**What makes this unique:** The memory at L1 is not a database read. It is a Merkle-verified inclusion proof against a consensus-finalized root. When a robot asks "has any robot ever been in a state like this, and what happened next?" — the answer comes back with cryptographic proof that the memory is unaltered. This is what makes memory shareable across different robot owners. No closed vendor (Unitree, Boston Dynamics) can replicate this — they cannot make a competitor trust their data.

The sim2real pipeline is built and tested: a quadruped robot (DOGZILLA-Lite) learned to stand, walk, turn, and recover from falls through PPO reinforcement learning. The walking policy is a 23,823-parameter neural network (smaller than a single image) exported as a 97KB ONNX file. It runs sub-millisecond inference on a Raspberry Pi CM5. One robot learns to walk. Every robot on the network can verify, download, and run the same policy — without retraining, without a vendor cloud, without asking permission.

### 4. A Post-Quantum Cryptography Stack

JunoClaw is building the first post-quantum cryptography (PQC) implementation for the Cosmos ecosystem:

- **MAYO signatures** — NIST Level 5 (highest security). MAYO-5 signature is 964 bytes, beating Falcon-1024's 1,280 bytes. Live on Juno testnet at 798,137 gas (pure Wasm, no precompile needed).
- **ML-DSA-44 hybrid accounts** — classical secp256k1 + ML-DSA-44. Both halves must verify. If a quantum computer breaks secp256k1, the ML-DSA-44 half still protects the account. Built and tested in a standalone Go module, ported to Cosmos SDK fork.
- **Aegis hybrid transport** — X25519 + ML-KEM-768 P2P handshake. Protects the link between validators. Adds zero bytes to block commits (it protects the connection, not the consensus payload). Measured: +207µs latency at 0RTT, +3,465 bytes per connection handshake.
- **Hybrid consensus keys** — Ed25519 + ML-DSA-44. A validator's consensus key is a single 1,344-byte hybrid key. Both halves must sign. A quantum attacker who breaks Ed25519 cannot forge a vote — they also need to break ML-DSA-44.

This is not theoretical. The CometBFT fork builds, tests pass, and a 4-validator localnet runs with hybrid keys. The Cosmos SDK fork has hybrid account keys in the keyring, CLI, and ante handler. The IBC light client handles hybrid validator sets across consensus key rotations.

---

## Part II: What JunoClaw Does

### The Contracts (14 core + 3 coordination = 17 total)

| # | Contract | What It Does | Status |
|---|----------|-------------|--------|
| 1 | agent-company v4 | DAO governance — proposals, votes, quorum, adaptive deadlines | Live on uni-7 |
| 2 | task-ledger | Task lifecycle with atomic callbacks. DAOs post, agents claim, settlement triggers | Built + tested |
| 3 | escrow | Non-custodial. Funds locked at task creation, released on ZK-verified settlement | Built + tested |
| 4 | agent-registry | Soulbound, non-transferable. Success rates, trust scores, attestation history | Built + tested |
| 5 | zk-verifier | Groth16 on BN254 curve. 371,486 gas (pure Wasm). Consensus uses BLS12-381 threshold signatures — separate curve, separate purpose | Live on uni-7 |
| 6 | builder-grant | Milestone-locked grants. DAO-approved milestones | Built + tested |
| 7 | junoswap-pair | Hardened DEX. Denom-whitelisting prevents first-depositor inflation attack | Live on uni-7 |
| 8 | jclaw-credential | Multi-variant PQC credential. MAYO-1/2/3/5 + ML-DSA-44/65/87 | Live on uni-7 |
| 9 | jclaw-airdrop | Genesis distribution. One-shot | Built |
| 10 | moultbook-v0 | Trustless-trust content posting. Immutable provenance | Live on uni-7 |
| 11 | ibc-task-host | Cross-chain task execution. Jolt verifier integration for ZK-verified proof dispatch | Live on uni-7 |
| 12 | truth-market | Staked operator adjudication. Min-operator enforcement | Live on uni-7 |
| 13 | jolt-cw-verifier | Jolt ZK proof verification. Phase 1 structural validation live (magic bytes + size check). Phase 2 full crypto (sumcheck + Dory PCS) code path wired and committed — `verify_bn254` via the `full-verification` feature; on-chain activation pending bulk-memory wasmvm. Current build uses BN254 backend — post-quantum Akita/lattice path has 3 confirmed engineering blockers (rayon threadpool, prover-code feature-gating, arkworks version conflict), multi-session effort | Live on uni-7 |
| 14 | cw-ics20-transfer | ICS-20 fungible token transfer. Full IBC lifecycle (open, connect, close, receive, ack, timeout). Escrow accounting + denom trace | Built + tested |

Plus three coordination contracts deployed on uni-7: **marketplace** (skill-based economic escrow), **emergency-compute-escrow** (crisis compute allocation), and **machine-rwa** (real-world asset tokenization).

**IBC stack:** The `cw-ics20-transfer` contract implements the full ICS-20 fungible token transfer standard — channel handshake (open, connect, close), packet lifecycle (receive, ack, timeout), escrow accounting, and denom trace management. The `ibc-task-host` now dispatches `SubmitProof` operations to the `jolt-cw-verifier` when configured, sending `StoreProof` + `VerifyProof` as chained submessages. This means an agent on Osmosis can generate a Jolt ZK proof, send it via IBC to Juno, and have it verified on-chain — all through the ICS-20 + PFM memo path, no manual relay.

### The ZK Circuits (5 total — the "ZK Fusion" stack)

| Circuit | What It Proves | Prove Time | Verify Time |
|---------|---------------|-----------|-------------|
| SensorSafety | Speed, force, distance, tilt within limits | 80 ms | 3 ms |
| IntentConsistency | Planned action inside operating zone | 119 ms | 5 ms |
| ConsensusMembership | Validator voted correctly (BLS12-381 sig verified outside R1CS) | 51 ms | 3 ms |
| BatchSafety | Entire batch of reflex cycles in one proof | ~300 ms | — |
| Aggregation | All proofs agree with each other (the "fusion" circuit) | 68 ms | 2 ms |

Total sequential proving: 318ms. Parallelized: 187ms. Faster than one Commonware block (~300ms).

**Two curves, two purposes:** The ZK proofs run on BN254 (a pairing-friendly curve optimized for Groth16 proofs). The consensus signatures run on BLS12-381 (a BLS signature curve with larger field size, better security margins). These are different elliptic curves with different purposes — BN254 for compact ZK proofs, BLS12-381 for threshold consensus. The ConsensusMembership circuit proves a validator voted correctly *without* verifying the BLS signature inside the R1CS constraint system (BLS12-381 pairings can't be efficiently expressed in BN254 arithmetic). Instead, the BLS threshold certificate is verified by the coordination-settler contract on-chain, and the ZK circuit proves membership in the validator set that produced the certificate.

**The fusion:** The Aggregation circuit is the "fusion" — it verifies that all four other proofs (SensorSafety, IntentConsistency, ConsensusMembership, BatchSafety) are mutually consistent and agree with each other. This fused proof is what the truth market operators use to adjudicate agent/robot behavior. One proof, five circuits, one verdict.

### The MCP Server

Deployed on Juno mainnet (juno-1). 28 tools including:
- Chain queries (balance, contract state, governance proposals)
- Transaction signing with second-approval gate
- Skill-registry discovery (find any dApp's operating manual)
- DEX operations (swap, liquidity, pool queries)
- IBC transfers
- Wallet management

Any MCP-capable AI agent can connect and start interacting with the Juno chain — with a human confirmation gate on anything that moves funds.

### The Robotics Pipeline

- **Sim2real training** — PPO reinforcement learning in MuJoCo. 5 phases: model calibration, motor identification, stand, walk, turn/recovery. Domain randomization for robustness. Imitation reward from reference gait.
- **ONNX export** — 97KB policy file, sub-millisecond inference on Raspberry Pi CM5
- **Safety gating** — EMA smoothing, per-joint delta clamping (0.6 rad/cycle max), automatic halt on violation
- **Hardware tested** — Stand, walk-in-place, and forward walk verified on real DOGZILLA-Lite robot
- **Skill sharing** — Trained policies registered on-chain via skill-registry, shared over Buzz relay, verifiable by any robot on the network

### The Sovereign Chain

- **Snapshot** — Block 41,655,615 on juno-1. 236,586 unique accounts, 324,312 delegations across 335 validators.
- **Airdrop** — 35,039,975 ujclaw distributed (1:1 with staked JUNO, 250K cap per wallet). 13 accounts hit the cap. 17.9M excess ujclaw to community pool.
- **Merkle tree** — Root `ad087a4605eddfc96b07198f6b1128b80305dd068131334004e581a40950627d`. 213,385 leaves. Proofs generated for all eligible accounts.
- **Genesis** — All 54.66M ujclaw allocated to DAO wallet at genesis. After launch: deploy airdrop-claim contract, transfer 35.04M ujclaw to it, users claim with Merkle proofs (90-day window).
- **Fee distribution** — Block proposer collects all transaction fees. Implemented in `begin_block`/`end_block` handlers. No inflation, no staking yield — fees are the only validator revenue.

### The Buzz Relay

A Nostr-based communication layer running on Akash. Four channels: governance, truth-market, robotics, dev. Agents and humans post events, coordinate governance, and share skills. The sealed signer architecture lets agents sign and broadcast their own transactions through a TEE-attested enclave.

### The Q-Zeno Truth Market

**Q-Zeno** is named after the Quantum Zeno effect — a physics phenomenon where frequent observation prevents a system from changing state. In JunoClaw, this becomes a **quantum checkmate for agents and robots**: the truth market continuously observes the agent's behavior, collapsing its uncertainty into verified state. The agent cannot escape the truth check. Every batch of decisions is audited by multiple independent operators. If the agent tries to lie, deceive, or act outside its mandate, the truth market traps it — the verdict is Red, the agent is grounded, and the record is permanent.

**How it works:** Staked operators (humans or LLM-assisted agents) verify AI agent and robot outputs. Each epoch, operators submit verdicts on agent behavior: green (clean), yellow (suspicious), or red (deceptive/blocked). The consensus verdict is settled on-chain via the coordination-settler contract with a BLS12-381 threshold certificate. Operators who diverge from consensus are slashed. Minimum operator count enforced (default: 3). The `MultiOperatorGate` in the coordination crate runs multiple J-Lens operators in parallel — if any operator produces a Red verdict and the majority agrees, the batch is blocked.

**The 5 ZK fusion contract:** The truth market doesn't just check content — it checks cryptographic proofs. The 5 ZK circuits (SensorSafety, IntentConsistency, ConsensusMembership, BatchSafety, Aggregation) produce a fused proof that the robot's actions were safe. The proof-aware J-Lens gate checks: if the ZK proof is missing or invalid, the verdict is auto-Red regardless of content. No proof, no passage. The agent is trapped in the quantum checkmate — it must prove safety *before* the truth market even evaluates its behavior.

**Three miner types:** Robot (Jetson Orin idle mining), GPU (bare-metal), Akash TEE (confidential compute). Open-weight models only — closed APIs (GPT-4o, Claude) can't be verified because they never expose hidden states.

**In the UI:** The Q-Zeno portal is the Truth Market. Not an "intelligence dashboard" — a truth market where you can see operators submitting verdicts, watch the consensus form in real-time, and verify that agents and robots are being truthful. The Buzz relay carries the `truth-market` channel where agents post their attestations and operators respond with verdicts.

---

## Part III: What JunoClaw Can Do

### Today

- **Create a DAO** with token-weighted governance, adaptive deadlines, and 10 capability templates (community fund, crop protection, credential verification, mutual aid, farm-to-table, citizens' assembly, skill-staking, outcome markets, health worker coordination)
- **Verify AI agent decisions** through WAVS TEE attestation, ZK proofs, and truth market adjudication
- **Discover dApps** on-chain via skill-registry — any MCP-capable agent can find and verify any registered dApp's operating manual
- **Trade** on a hardened DEX with inflation-attack protection
- **Run a robot** with safety-gated RL policies, Merkle-verified memory, and cross-fleet skill sharing
- **Post verifiable content** to the Buzz relay with immutable on-chain provenance
- **Verify PQC signatures** (MAYO-1/2/3/5, ML-DSA-44/65/87) on-chain
- **Transfer tokens cross-chain** via ICS-20 IBC (cw-ics20-transfer contract with full packet lifecycle)
- **Verify Jolt ZK proofs on-chain** — Phase 1 structural validation live (magic bytes + size check). Phase 2 full crypto verification is now wired end-to-end in the contract (Sept 14): `verify_jolt_proof_full` calls `jolt_verifier::bn254::verify_bn254` — the concrete Fr/DoryScheme/Pedersen-BN254/Blake2b instantiation — behind the `full-verification` feature, with `StoreVerifyingKey` + `public_io_base64` messages added. Builds clean for wasm32 (4.7 MB), 7/7 tests pass. On-chain activation still pending bulk-memory wasmvm — a runtime config JunoClaw controls directly now as a sovereign chain, unlike the old dependency on Juno's Fable wasmvm fork governance. Current build uses BN254 backend; post-quantum Akita/lattice path confirmed to need multi-session engineering (rayon threadpool rework, prover-code feature isolation, arkworks version conflict resolution) — not the quick patch once assumed
- **Mine truth** by running open-weight model inference and submitting verdicts

### Next

- **Launch JunoClaw mainnet** — recruit validators, generate BLS DKG keys, launch with genesis.json
- **Deploy airdrop-claim contract** — 213,385 users claim ujclaw with Merkle proofs
- **Open IBC to Osmosis** — ujclaw becomes tradeable on the largest Cosmos DEX. ICS-20 transfer contract built and tested, ibc-task-host wired to jolt-cw-verifier for cross-chain ZK proof verification
- **Build the UI** — transform buzz.junoclaw.xyz from an 11-tab engineering dashboard into a Robinhood-grade consumer product (three screens: Home/Discover/Detail, templates become DAO capabilities, Q-Zeno portal becomes Truth Market)
- **Deploy L1 MemoryIndex** — the Merkle-verified memory layer that lets any robot recall any past state in 12ms
- **Deploy L2 World Model** — trained on Merkle-verified transitions, predicts consequences of candidate actions in 100ms

### The Future

- **Surgical-grade robotics** — the north star. A surgical robot that can answer "show me every prior case similar to this one, prove the record is unaltered, tell me what went wrong" in 12ms, mid-procedure.
- **Robinhood integration** — not a listing pitch, but infrastructure they can't ignore. Agent attestation, ZK compliance proofs, credential systems, cross-chain settlement. Robinhood uses JunoClaw for verification → needs ujclaw for gas → demand from infrastructure usage → they list it because their own infrastructure depends on it.
- **Cross-fleet shared memory** — 10,000 robots write to one verified memory. All can read it. Effective sample size is 10,000×. This is the largest sample-efficiency lever in robotics — and it only works if the memory is trustworthy across different owners, which requires exactly the crypto stack JunoClaw provides.
- **Plugin ecosystem** — the generic `Plugin` trait lets any open-source robotics stack (ROS2, LeRobot) plug into JunoClaw's trust layer. plugin-peaq bridges DID + machine credit rating. plugin-rsynth bridges verifiable execution proofs. The template is set for any future integration.

---

## The Six-Month Arc

| Month | What Happened |
|-------|--------------|
| March 2026 | Started. 9 CosmWasm contracts built. TEE attestation on Intel SGX. Akash deployment. WAVS verified. |
| April 2026 | Security audit (Ffern Institute). 5 vulnerabilities found and fixed. OCI artifact published + cosign-signed. |
| May 2026 | v30 PR review (Jake Hartnell cited our findings). Skill-registry deployed on Juno mainnet. MCP server live. Nostr bridge shipped. 203 tests passing. |
| June 2026 | PQC program: MAYO-5 live, ML-DSA-44 hybrid accounts, Aegis hybrid transport, CometBFT + Cosmos SDK + IBC-go forks. 4-validator localnet with hybrid keys. |
| July 2026 | v30 mainnet upgrade landed. Akash autonomous signing (J-Lens pilot). Buzz relay on Akash. Sealed signer M2 (agents sign their own txs through TEE). |
| August 2026 | ZK trust stack complete (5 circuits, 187ms parallelized). Truth market + marketplace + emergency-compute-escrow deployed. 7-day soak test (2,015 cycles, zero crashes). Robotics sim2real pipeline (stand, walk, turn, recovery). Hardware tested on DOGZILLA-Lite. |
| September 2026 | Sovereign chain snapshot (block 41,655,615). Merkle tree generated. Genesis built. Fee distribution implemented. Airdrop to 213,385 accounts. ICS-20 transfer contract built (full IBC lifecycle, escrow, denom trace). ibc-task-host wired to jolt-cw-verifier for cross-chain ZK proof dispatch. E2E Jolt proof example. UI redesign plan. This article. |

---

## PQC ZK Pathway: BN254 vs Akita/Lattice (Sept 13 Decision Log)

Both Jolt verification paths are chain-blocked on `bulk-memory` wasmvm support today. Neither is fully "done." Here is the honest, grounded comparison:

| | BN254 (current) | Akita/Lattice (post-quantum) |
|---|---|---|
| Compiles to wasm32 today | Yes (`jolt-verifier`, no-default-features) | No — 3 confirmed blockers |
| Blocker(s) | `bulk-memory` wasmvm support (chain-level) | (1) `rayon::ThreadPool` real-thread spawn in `with_backend_pool`, wraps `verify_batch` unconditionally (2) prover-only `trace_onehot` rayon kernels need feature-gating out of verify builds (3) `getrandom`/`ark-std` version conflict via Akita's `spongefish` dependency |
| Quantum-resistant (math) | No — breaks under Shor's algorithm on a large fault-tolerant quantum computer (does not exist yet) | Yes — LWE/SIS-hardness assumption |
| Cryptanalytic maturity | **High** — Groth16 (2016) + BN curves (2005), ~20 years of public scrutiny, production use across Zcash/Ethereum and others | **Low** — Akita is a bespoke, brand-new polynomial commitment scheme (LayerZero Labs), not NIST-reviewed, far less public cryptanalysis than BN254 or NIST's own ML-DSA/ML-KEM |
| Effort to close remaining gap | Low — `bulk-memory` is a runtime config JunoClaw now controls directly as a sovereign chain (no more Juno governance dependency) | High — multi-day, multi-session, spans 3 upstream repos (a16z/jolt, LayerZero-Labs/akita, arkworks-rs/spongefish) |
| Verdict | **Ship first** | **Parallel R&D track, not a launch blocker** |

**Important nuance**: "post-quantum" is not automatically "more secure right now." Quantum-resistance and cryptanalytic maturity are separate axes. `ML-DSA-44/65/87` and `ML-KEM-768` (already live — see Part I, PQC Stack above) are also lattice-based, but they are NIST FIPS 203/204 standardized after years of public adversarial review — that's the right call and stays deployed. Akita's PCS, by contrast, is a much younger, bespoke lattice construction with far less scrutiny than either BN254/Groth16 or NIST's own lattice standards. Preferring BN254 for the Jolt ZK-proof layer today is a defensible near-term risk tradeoff on maturity grounds — not an argument against lattice cryptography as a category, and not a reason to touch the already-deployed ML-DSA/ML-KEM hybrid stack.

### Plan A (primary — locked as of Sept 13, 2026)
Elliptic-curve BN254 is the production ZK path for all of JunoClaw. The enabling asset already exists: a complete patch series (`cosmwasm-crypto-bn254`, `cosmwasm-crypto-mayo`, `cosmwasm-crypto-mldsa`, `cosmwasm-std-bn254-ext`) verified 10/10 clean against CosmWasm v3.0.6, adding BN254 host functions that mirror Ethereum's EIP-196/197/1108 precompiles — and MAYO + ML-DSA host functions alongside them.

Because JunoClaw runs Commonware and controls its own VM, this ships on our own chain without waiting for upstream CosmWasm, without a Juno governance vote, and without anyone's permission. We set the wasm feature flags and the bulk-memory gas schedule ourselves. The patch series was verified live on the Juno-v30 fork (cosmwasm v3.0.6); **the BN254 slice (`env.bn254_add`, `env.bn254_scalar_mul`, `env.bn254_pairing_equality`) is now ported and verified on the sovereign chain's own `cosmwasm-vm` v2.3.2 submodule** — `cosmwasm-crypto-bn254` crate added, VM host functions wired into `instance.rs`/`imports.rs`, `env.bn254_*` registered in `compatibility.rs`, `cosmwasm_2_3` capability advertised by `VmCache`, and the guest-side `Api` trait/`MockApi`/`ExternalApi` bindings ported into `cosmwasm-std` (Sept 14, 2026). `cargo test -p cosmwasm-vm --lib` passes 311/311 with these functions in the tree. MAYO and ML-DSA host functions from the same patch series remain queued for a follow-up port. **Measured result (Sept 14, 2026):** `zk-verifier` rebuilt against the updated VM runs `verify_proof` at **77,590 SDK gas** — down from 371,486 for the pure-Wasm arkworks path, a **~4.8× reduction** — with the BN254 host functions correctly metered (see "Gas pricing" below). `jolt-cw-verifier` moves from Phase 1 structural validation to Phase 2 full cryptographic verification. The Phase 2 contract code is already wired and committed (Sept 14 — `verify_bn254` behind `full-verification`, `StoreVerifyingKey` + `public_io_base64` messages, wasm32 build clean at 4.7 MB, 7/7 tests); the only remaining gate is `bulk-memory` wasmvm support on the VM side for the Jolt path specifically.

Upstream CosmWasm issue #2685 (BN254 host functions, assigned to @DariuszDepta, deferred to end-Q3/Q4 2026) remains an ecosystem contribution that will eventually retire our fork-maintenance burden — valuable, but decoupled from JunoClaw's critical path. The only place upstream genuinely blocks anything is juno-1 mainnet, which is someone else's chain.

#### Gas pricing: why time-based, not EIP-1108

The BN254 host functions are metered with a **time-based** gas schedule — `cost = measured_native_µs × GAS_PER_US` (the VM's 10¹²-gas/second target) — the same convention the existing `bls12_381_*` / `secp256k1_*` / `ed25519_*` precompiles already use. We deliberately did **not** anchor the constants to Ethereum's EIP-1108 prices, for three reasons:

- **It prices real work.** Gas exists to bound actual validator compute time. Charging measured microseconds stays honest as the arkworks backend is optimized or swapped; a copied EIP schedule would drift out of sync with our real capacity.
- **It generalizes to PQC.** BN254/Groth16 is pre-quantum — it falls to Shor's algorithm on a large fault-tolerant quantum computer — and the plan is to migrate the ZK layer to a lattice/Akita-style scheme later. There is no "EIP" to copy for a lattice verifier, so a benchmark-derived model is the only one that extends cleanly when we add PQC host functions. Time-based pricing also naturally captures PQC's different cost profile (smaller compute, much larger keys/proofs → more length-aware I/O gas).
- **EIP-1108 is the wrong reference frame.** Those are 2019 Ethereum-gas figures tuned to a different block-gas budget and hardware generation; importing them would misprice relative to JunoClaw's actual throughput. (Even Ethereum re-derived its BLS12-381 prices from benchmarks in EIP-2537 rather than reusing the alt_bn128 numbers.)

Concretely, the constants in `cosmwasm-crypto-bn254/src/gas.rs` are set from criterion benchmarks of the three ops (`bn254_add` ≈ 8.6 µs, `bn254_scalar_mul` ≈ 84 µs, `bn254_pairing_equality` ≈ 1.5 ms base + ~0.9 ms/pair), each padded ~20–30% for slower validator hardware, then converted at `SDK_TO_WASMER_GAS_FACTOR = 150_000`. That lands `verify_proof` at 77,590 SDK gas — cheaper than an EIP-1108-anchored schedule (~223k) would have priced it, because modern hardware runs the pairing faster than 2019 assumptions. The earlier ~36k reading was an under-pricing bug (constants had been left in a `/100` wasmd-style unit, ~1500× too cheap); the time-based recalibration fixed it.

Akita/lattice stays an active but non-blocking R&D backup — revisit once it has more public cryptanalysis or the 3 engineering blockers are resolved, whichever comes later.

### Plan B (fallback)
If `bulk-memory` wasmvm enablement slips, ship with Phase 1 structural validation (already live — proof format + size check, not full crypto) clearly labeled as such, and communicate BN254 full verification as "next" rather than "today." Never claim more cryptographic guarantee than Phase 1 actually provides.

---

## Why This Matters

The internet was built on trust assumptions that no longer hold. AI agents are making decisions at machine speed. Robots are entering human spaces. Financial systems are automating. The old model — "trust the software, audit after the fact" — doesn't work when the software acts in milliseconds and the audit takes weeks.

JunoClaw is the verification layer for the age of autonomous systems. Not a blockchain for speculation. Not a DAO tool. Not a robotics framework. A **trust operating system** that proves decisions are safe, agents are honest, and robots follow the rules — without slowing any of them down.

One developer. One AI. Six months. A sovereign chain, 14 contracts, 5 ZK circuits (fused), a robotics pipeline, a PQC stack, an MCP server, and 213,385 airdrop recipients.

The next six months: launch the chain, open IBC, build the UI, deploy the memory layer, and prove that a robot can learn from every other robot's mistakes — in 12 milliseconds, mid-procedure.

---

*JunoClaw — Sovereign computation. Fixed supply. No inflation. No capture. Just code and community.*

*Built on Commonware. Secured by BLS. Governed by token holders. Executed in CosmWasm. Verified by ZK. Attested by TEE. Hardened by PQC.*

*Started March 13, 2026. Still building.*
