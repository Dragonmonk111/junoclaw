# What Is JunoClaw?

*Six months. One developer and an AI. A sovereign chain, a trust layer for AI agents, a robotics operating system, a post-quantum cryptography stack, and an airdrop to 213,385 people.*

---

## The One-Sentence Answer

**JunoClaw is a sovereign blockchain and trust operating system that lets AI agents and robots prove their decisions are safe — without slowing them down.**

---

## The Origin

On March 13, 2026, one developer and an AI coding assistant started building. The premise: AI agents make decisions that affect real money and real safety, but nobody checks whether those decisions are correct. A robot decides to brake. An agent executes a trade. A DAO votes. Who verifies the decision followed the rules?

Today: nobody. The agent's own software says "I'm fine" and we trust it.

JunoClaw is the layer that checks. What started as CosmWasm contracts on Juno's testnet became a sovereign chain, a robotics trust layer, a PQC research program, and a community of 213,385 airdrop recipients.

---

## Part I: What JunoClaw Is

Four systems that compose into one platform:

### 1. A Sovereign Chain

Pure Rust — no Go, no Cosmos SDK, no Tendermint:

- **Commonware runtime** — BLS12-381 threshold consensus, not stake-weighted PoS. Validators set at genesis via DKG. BFT `n >= 3f+1`.
- **Fixed supply** — 54,660,000 ujclaw. No inflation, no mint, no staking yield. Validators earn fees — same model as Bitcoin miners.
- **CosmWasm execution** — forked VM (v2.3.2) with BN254 host functions, metered bulk-memory, own gas schedule.

**Borrowed from Cosmos:** the wallet experience — same transaction format, bech32 addresses, gRPC queries, protobuf messages. Any Cosmos wallet works unmodified.

**Built from scratch:** consensus (Commonware simplex + BLS12-381, not Tendermint), P2P (authenticated Ed25519 + TLS), the app framework, bank/auth keepers, the CosmWasm keeper, and RocksDB storage — all Rust. No staking, governance, mint, or slashing modules — not needed.

Launched with a snapshot of staked JUNO at block 41,655,615 — 35,039,975 ujclaw to 213,385 accounts.

**Devnet proven (Sept 15):** a four-node devnet executed the full CosmWasm lifecycle — store-code, instantiate, store_vk, verify_proof — over Commonware P2P with BLS finality in ~400ms. Groth16 verification: 77,590 SDK gas. First smart contract execution on a sovereign JunoClaw chain.

**Jolt verified on-chain (Sept 18):** `jolt-cw-verifier` ran full cryptographic verification of a real 68,291-byte Jolt proof against a 104,937-byte stored verifying key via `verify_bn254` (sumcheck + Dory PCS) at 10.87M gas. First Jolt proof verified on a sovereign JunoClaw chain.

**Airdrop claim proven (Sept 19):** `airdrop-claim` ran the full claim lifecycle on devnet — instantiate with Merkle root, fund, claim with SHA-256 sorted-pair proof, payout via bank send. The same contract and tree format that will distribute 35,039,975 ujclaw to 213,385 accounts at mainnet.

### 2. A Trust Layer for AI Agents

AI agents need three things that don't exist today:

**Discovery:** the **skill-registry contract** (live on Juno mainnet) lets any dApp publish a pointer + SHA-256 hash of its operating manual. Any MCP-capable agent — Claude, Cursor, Windsurf, ChatGPT — can discover and verify it without a human pasting docs into a prompt.

**Safe signing:** a **second-approval gate**. Every fund-moving tool stages its transaction and returns a confirmation ID; nothing broadcasts until a human calls `confirm_transaction`. Single-use, five-minute expiry. Blast radius of a compromised agent: zero funds moved.

**Verifiable execution:** WAVS (Layer.xyz) runs agent computation inside hardware-attested enclaves (Intel SGX, Akash TEE). Independent witnesses verify outcomes before on-chain settlement — an immutable audit trail from prompt to outcome.

### 3. A Robotics Operating System

A robot decides 1,000 times per second. JunoClaw proves those decisions safe in 187ms. Six tiers:

```
L0  1 ms      Reflex          Classical control, balance              LOCAL
L1  12 ms     Memory fetch    Merkle-verified recall of past cycles   LOCAL CACHE
L2  50-100 ms World model     Predict consequences of candidate acts   LOCAL INFERENCE
L3  ~400 ms   Settlement      Commonware on-chain                     CHAIN
L4  minutes   Truth verdict   Staked operator adjudication (Q-Zeno)   TRUTH MARKET
L5  days      Governance      DAO SafetyEnvelope vote                 DAO
```

The robot acts at L0, remembers at L1, imagines at L2. The chain settles at L3. The robot never waits for the chain. L4 is slow on purpose — adjudication with stake at risk shouldn't be reflex-speed.

**400ms, not 2.8s:** Commonware simplex + BLS12-381 threshold consensus — 7× faster than Tendermint. The threshold certificate *is* the settlement proof.

**Merkle-verified memory:** L1 reads are Merkle inclusion proofs against a consensus-finalized root — memory provably unaltered, shareable across robot owners. No closed vendor (Unitree, Boston Dynamics) can replicate this.

**Sim2real tested:** DOGZILLA-Lite quadruped learned stand, walk, turn, and fall recovery via PPO RL. 23,823-parameter policy, 97KB ONNX, sub-ms inference on Raspberry Pi CM5. One robot learns to walk — every robot on the network can verify and run the same policy.

### 4. A Post-Quantum Cryptography Stack

First PQC implementation for the Cosmos ecosystem:

- **MAYO signatures** — NIST Level 5, 964-byte signatures (smaller than Falcon-1024's 1,280). Live on Juno testnet at 798,137 gas.
- **ML-DSA-44 hybrid accounts** — secp256k1 + ML-DSA-44, both must verify. Quantum breaks secp256k1 → ML-DSA-44 still protects.
- **Aegis hybrid transport** — X25519 + ML-KEM-768 P2P handshake. +207µs, zero bytes added to block commits.
- **Hybrid consensus keys** — Ed25519 + ML-DSA-44, 1,344-byte key. Both halves must sign.

CometBFT fork builds; 4-validator localnet runs hybrid keys; SDK fork has hybrid accounts in keyring, CLI, ante handler.

---

## Part II: What JunoClaw Does

### The Contracts (14 core + 3 coordination)

**Live on uni-7 testnet:** agent-company v4 (DAO governance), junoswap-pair (hardened DEX), jclaw-credential (PQC: MAYO-1/2/3/5 + ML-DSA-44/65/87), moultbook-v0 (verifiable content), ibc-task-host, truth-market (staked adjudication), zk-verifier, jolt-cw-verifier — plus marketplace, emergency-compute-escrow, and machine-rwa.

**Built + tested:** task-ledger, escrow, agent-registry, builder-grant, cw-ics20-transfer.

**Devnet proven on the sovereign chain:** zk-verifier (Groth16 at 77,590 gas), jolt-cw-verifier (real 68KB Jolt proof, full crypto at 10.87M gas), and jclaw-airdrop (Merkle-proof claim + payout, Sept 19).

**IBC:** `cw-ics20-transfer` implements the full ICS-20 lifecycle (escrow, denom trace, ack/timeout) and `ibc-task-host` is wired to dispatch ZK proofs to `jolt-cw-verifier` — built and tested. The counterparty-side piece — a light client verifying Commonware's BLS12-381 threshold certificates — is built and passing tests: a pure-Rust `bls12_381` pairing check (no host precompile dependency), cross-verified against real threshold signatures generated by the same `blst`-backed DKG code `slay3rd` runs in production. Deployable as a CosmWasm `08-wasm` client (the pattern Union proved with cometbls) — no counterparty chain upgrade required.

### The ZK Circuits (5 total — the "ZK Fusion" stack)

| Circuit | What It Proves | Prove Time | Verify Time |
|---------|---------------|-----------|-------------|
| SensorSafety | Speed, force, distance, tilt within limits | 80 ms | 3 ms |
| IntentConsistency | Planned action inside operating zone | 119 ms | 5 ms |
| ConsensusMembership | Validator voted correctly (BLS12-381 signature verified outside R1CS) | 51 ms | 3 ms |
| BatchSafety | Entire batch of reflex cycles in one proof | ~300 ms | — |
| Aggregation | All proofs agree with each other (the "fusion" circuit) | 68 ms | 2 ms |

Total sequential proving: 318ms. Parallelized: 187ms — faster than one block.

**Two curves, two purposes:** ZK proofs on BN254 (compact Groth16); consensus signatures on BLS12-381 (threshold). ConsensusMembership proves validator membership without verifying BLS signatures inside R1CS.

**The fusion:** the Aggregation circuit verifies all four proofs are mutually consistent. One proof, five circuits, one verdict.

### The MCP Server

Live on Juno mainnet. 28 tools: chain queries, transaction signing (with the approval gate), skill-registry discovery, DEX operations, IBC transfers, wallet management. Any MCP-capable agent can connect — with a human gate on anything that moves funds.

### The Sovereign Chain

- **Snapshot** — block 41,655,615 on juno-1. 236,586 accounts, 324,312 delegations, 335 validators.
- **Airdrop** — 35,039,975 ujclaw (1:1 staked JUNO, 250K cap). 13 accounts capped; 17.9M excess to community pool.
- **Merkle tree** — root `ad087a46…50627d`, 213,385 leaves, proofs for all eligible accounts.
- **Genesis** — 54.66M ujclaw to DAO wallet; airdrop-claim contract at launch, 90-day claim window.
- **Fees** — block proposer collects all transaction fees. No inflation — fees are the only validator revenue.

### The Buzz Relay

Nostr-based communication layer on Akash. Four channels: governance, truth-market, robotics, and development. The sealed signer lets agents sign and broadcast their own transactions through a TEE-attested enclave.

### The Q-Zeno Truth Market

Named for the Quantum Zeno effect — frequent observation prevents state change. The truth market continuously observes agent behavior, collapsing uncertainty into verified state.

Staked operators vote green/yellow/red; consensus settles via BLS12-381 threshold certificate, divergent operators are slashed. `MultiOperatorGate` runs parallel J-Lens operators — any Red + majority blocks the batch.

**Proof-aware:** missing or invalid ZK proof → auto-Red. No proof, no passage.

**Three miner types:** Robot (Jetson Orin), GPU, Akash TEE. Open-weight models only — closed APIs can't be verified.

---

## Part III: What JunoClaw Can Do

### Today

- **Create a DAO** — token-weighted governance, adaptive deadlines, 10 capability templates
- **Verify AI agent decisions** — WAVS TEE attestation, ZK proofs, truth-market adjudication
- **Discover dApps** on-chain via skill-registry
- **Trade** on a hardened DEX with inflation-attack protection
- **Run a robot** — safety-gated RL policies, Merkle-verified memory, cross-fleet skill sharing
- **Post verifiable content** to the Buzz relay with immutable provenance
- **Verify PQC signatures** (MAYO-1/2/3/5, ML-DSA-44/65/87) on-chain
- **Verify Jolt ZK proofs on-chain** — a real 68KB proof against a 105KB verifying key, sumcheck + Dory PCS at 10.87M gas
- **Mine truth** — open-weight inference, staked verdicts

### Next

- **Launch JunoClaw mainnet** — recruit validators, generate BLS DKG keys, launch with genesis.json. Devnet has proven the full stack: P2P, consensus, CosmWasm, BN254 host functions, Jolt Phase 2
- **Deploy airdrop-claim** — 213,385 users claim ujclaw with Merkle proofs
- **Open IBC to Osmosis** — ujclaw tradeable on the largest Cosmos DEX. ICS-20 contract built; BLS light client for Commonware consensus built and tested; the remaining work is deployment via `08-wasm` on Osmosis (no chain upgrade required) and relayer integration
- **Build the UI** — buzz.junoclaw.xyz from engineering dashboard to consumer product (Home/Discover/Detail)
- **Deploy L1 MemoryIndex** — Merkle-verified memory; any robot recalls any past state in 12ms
- **Deploy L2 World Model** — trained on verified transitions, predicts consequences in 100ms

### The Future

- **Surgical-grade robotics** — the north star. A surgical robot that answers "show me every prior case like this, prove the record is unaltered, tell me what went wrong" in 12ms, mid-procedure.
- **Robinhood integration** — infrastructure they can't ignore: agent attestation, ZK compliance proofs, credentials, cross-chain settlement. Use JunoClaw for verification → need ujclaw for gas → list it because their own infrastructure depends on it.
- **Cross-fleet shared memory** — 10,000 robots write to one verified memory; all can read it. 10,000× effective sample size — the largest sample-efficiency lever in robotics, and it only works with exactly this crypto stack.
- **Plugin ecosystem** — the `Plugin` trait lets any open robotics stack (ROS2, LeRobot) plug into the trust layer. plugin-peaq bridges DID + machine credit; plugin-rsynth bridges verifiable execution proofs.

---

## The Six-Month Arc

| Month | What Happened |
|-------|--------------|
| March 2026 | Started. 9 CosmWasm contracts built. TEE attestation on Intel SGX. Akash deployment. WAVS verified. |
| April 2026 | Security audit (Ffern Institute). 5 vulnerabilities found and fixed. OCI artifact published + cosign-signed. |
| May 2026 | v30 PR review (Jake Hartnell cited our findings). Skill-registry deployed on Juno mainnet. MCP server live. Nostr bridge shipped. 203 tests passing. |
| June 2026 | PQC program: MAYO-5 live, ML-DSA-44 hybrid accounts, Aegis hybrid transport, CometBFT + Cosmos SDK + IBC-go forks. 4-validator localnet with hybrid keys. |
| July 2026 | v30 mainnet upgrade landed. Akash autonomous signing (J-Lens pilot). Sealed signer M2 (agents sign their own transactions through TEE). |
| August 2026 | ZK trust stack complete (5 circuits, 187ms parallelized). Truth market + marketplace + emergency-compute-escrow deployed. 7-day soak test (2,015 cycles, zero crashes). Robotics sim2real pipeline (stand, walk, turn, recovery). Hardware tested on DOGZILLA-Lite. |
| September 2026 | Sovereign chain snapshot (block 41,655,615). Merkle tree generated. Genesis built. Fee distribution implemented. Airdrop to 213,385 accounts. ICS-20 transfer contract built (full IBC lifecycle, escrow, denom trace). ibc-task-host wired to jolt-cw-verifier for cross-chain ZK proof dispatch. End-to-end Jolt proof example. BN254 host functions ported to sovereign chain VM (311/311 tests pass). **Four-node devnet live — first smart contract execution: zk-verifier deployed end-to-end, Groth16 proof verified on-chain at 77,590 SDK gas. Jolt Phase 2 proven: real 68KB Jolt proof cryptographically verified on-chain at 10.87M gas (Sept 18). Airdrop-claim e2e proven: Merkle-proof claim + payout on devnet (Sept 19). Commonware BLS12-381 light client built — pure-Rust threshold certificate verification, cross-tested against real `blst`-generated signatures (Sept 20).** UI redesign plan. |

---

## A Note on the ZK Path

The production ZK layer runs on BN254 — 20 years of cryptanalytic scrutiny, 4.8× cheaper via native host functions than pure-Wasm. A post-quantum PCS (Akita/lattice) is parallel R&D, not a launch blocker: ML-DSA and ML-KEM already secure accounts and transport today. Gas is priced by measured time, not a hardcoded table — it stays honest as hardware improves.

---

## Why This Matters

The internet was built on trust assumptions that no longer hold. Agents decide at machine speed, robots enter human spaces, finance automates. "Trust the software, audit after the fact" fails when software acts in milliseconds and audits take weeks.

JunoClaw is the verification layer for the age of autonomous systems — a **trust operating system** that proves decisions are safe, agents are honest, and robots follow the rules, without slowing any of them down.

One developer. One AI. Six months. A sovereign chain, 14 contracts, 5 fused ZK circuits, a robotics pipeline, a PQC stack, an MCP server, 213,385 airdrop recipients — and the first smart contracts on a sovereign JunoClaw chain: a Groth16 proof verified at 77,590 gas, a Jolt proof at 10.87M.

The next six months: launch the chain, open IBC, build the UI, deploy the memory layer, and prove that a robot can learn from every other robot's mistakes — in 12 milliseconds, mid-procedure.

---

*JunoClaw — Sovereign computation. Fixed supply. No inflation. No capture. Just code and community.*

*Built on Commonware. Secured by BLS. Governed by token holders. Executed in CosmWasm. Verified by ZK. Attested by TEE. Hardened by PQC.*

*Started March 13, 2026. Still building.*

---

## Airdrop Announcement (copy-paste when claims go live)

```
The JunoClaw airdrop is live.

If you had JUNO staked at block 41,655,615, ujclaw is waiting for you.
35,039,975 ujclaw across 213,385 accounts — 1:1 with your staked JUNO.

Claim: [CLAIM_URL]
Connect the wallet that held your stake. Your allocation and proof
are already on-chain. One signature, done.

Claim window: [START_DATE] → [END_DATE] (90 days).
Not tradable yet — claims first, markets after.
```
