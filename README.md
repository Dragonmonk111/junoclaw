# JunoClaw

**A verifiable-autonomy stack for robots — on-chain truth markets, Moultbook work-history attestation, RWA contracts for physical machines, ROS2 intent bridge, and ZK safety proofs. Built on Juno (Cosmos).**

Open-source. Apache-2.0. [Juno Agents DAO](https://daodao.zone/dao/juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac) · [GitHub](https://github.com/Dragonmonk111/junoclaw)

> 28 CosmWasm contracts · 15 Rust crates · 5 ZK circuits · 6-layer soak test · 16 truth-market epochs · 5 operators · 290,000 ujunox slashed

---

## What JunoClaw Does

JunoClaw is **not a robotics OS**. It is a trust and attestation middleware layer that sits on top of ROS2 and proves what a robot actually did — cycle by cycle, verifiable by anyone, anchored on-chain.

The stack produces three things that do not currently exist for autonomous machines:

1. **Tamper-evident claims history** — `ReflexBatchAttestation` + Merkle anchoring. What did this machine do, and can it be reconstructed after an incident?
2. **Independent verification** — adversarial truth market operators with real stake, real slashing, real rewards. Not the manufacturer's own telemetry.
3. **On-chain credit score** — `machine-rwa` cross-queries Moultbook for verified work history and returns a creditworthiness score. A robot that can be financed against its own proven track record.

### The reflex/intent split

- **Reflex tier** (sub-100ms): balance, collision avoidance, motor control — stays on the robot controller, never hits the chain
- **Intent tier** ("engage target", "take this route"): wrapped in `IntentMessage`, audited by the J-Lens gate, settled by the truth market

The robot doesn't block on the blockchain. Reflexes run at hardware speed. After a batch of cycles, the controller produces a Merkle root proving the safety envelope was maintained — post-hoc verifiable.

---

## Live on Juno Testnet (uni-7)

| Contract | Code ID | Address | Status |
|----------|---------|---------|--------|
| Truth Market | 99 | `juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p` | **16 epochs, 5 operators, 707,672 ujunox rewards, 290,000 slashed** |
| Machine RWA | 100 | `juno1x9unynpfqrnc8w58hrhlmeeakws46mpj0s7up774k4lhckl9jphs6e5rn7` | **Deployed, `machine-0` NFT minted (Unitree Go2, ROSIE-UNIT-001)** |
| Emergency Compute Escrow | 89 | `juno143mk0t4g4zx2ahqx5x905lps5x0mfm5ghhkw42fjwjme37cvdkdqwnatt3` | Deployed, no leases yet |
| Moultbook | 76 | `juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4` | Live — 11 verdict rationales + closeout reports |

**Verify any claim in under a minute:**
```bash
junod query wasm contract juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p '{"get_stats":{}}' --node https://juno.rpc.t.stavr.tech
junod query wasm contract juno1x9unynpfqrnc8w58hrhlmeeakws46mpj0s7up774k4lhckl9jphs6e5rn7 '{"get_machine":{"token_id":"machine-0"}}' --node https://juno.rpc.t.stavr.tech
```

---

## Architecture

### On-chain (Juno CosmWasm)

28 contract crates across 6 functional areas:

**Core Protocol**
- `agent-company/` — DAO governance, sortition, adaptive deadlines
- `agent-registry/` — Agent identity registry with reputation tracking
- `task-ledger/` — Task lifecycle: post, claim, settle, expire
- `escrow/` — Non-custodial payment, locked at creation
- `skill-registry/` — On-chain skill registry

**Trust & Safety**
- `truth-market/` — Adversarial verdict market with slashing + accuracy-weighted rewards
- `safety-envelope/` — Governance-set safety params (max speed, force, tilt, collision distance)
- `circuit-breaker/` — On-chain circuit breaker state (Closed/Open/Tripped)
- `merkle-verifier/` — On-chain Merkle root verification for reflex batch attestations
- `tee-attestation-verifier/` — Ed25519 TEE attestation verification
- `zk-verifier/` — BN254 Groth16 on-chain proof verification

**RWA & Economics**
- `machine-rwa/` — Machine NFT minting, fractional ownership, `GetWorkIntegrityScore` cross-query to Moultbook
- `emergency-compute-escrow/` — Edge agent escrows JUNO for burst compute on Akash, with hard spend cap + safe fallback
- `marketplace/` — Machine NFT marketplace

**Publishing & Privacy**
- `moultbook-v0/` — Anonymous on-chain publishing with ZK proof of membership
- `moultbook/` — Work-history attestation (verdict rationales, agent messages, closeout reports)
- `knowledge-moults/` — Knowledge artifact publishing
- `jclaw-credential/` — Verifiable credentials

**DeFi & IBC**
- `junoswap-factory/` — AMM pair factory (Junoswap v2)
- `junoswap-pair/` — Constant-product pair with denom whitelisting
- `ibc-task-host/` — Cross-chain swap host (ICS-20 + PFM wasm memos)
- `faucet/` — Testnet JUNOX faucet
- `builder-grant/` — TEE-verified milestone-locked grants

**Coordination**
- `coordination-settler/` — P2P BFT consensus settlement layer
- `integration-tests/` — Cross-contract integration tests
- `junoclaw-common/` — Shared types

### Off-chain

- **`crates/junoclaw-coordination/`** — P2P BFT consensus mesh (4-node), J-Lens gate, message types (`IntentMessage`, `ReflexBatchAttestation`, `AgentMessage`, `CircuitBreakerState`)
- **`crates/junoclaw-physics/`** — Physics simulator (1000Hz), SHA-256 cycle hashing, Merkle tree construction, `ReflexBatchAttestation` production. Supports simulated and MuJoCo backends
- **`plugins/plugin-ros2/`** — ROS2 adapter: converts action server output into typed `IntentMessage`s, bridges to J-Lens gate, queries on-chain SafetyEnvelope + CircuitBreaker. Python bridge server included
- **`crates/junoclaw-miner/`** — Truth market miner (verdict submission, Moultbook rationale posting)
- **`crates/junoclaw-relayer/`** — Relayer daemon for on-chain contract interaction
- **`crates/junoclaw-nostr-bridge/`** — Nostr discovery (kind 38402)
- **`crates/junoclaw-x402-gateway/`** — Sovereign payment gateway
- **`crates/junoclaw-mayo-verify/`** — Mayo signature verification
- **`crates/junoclaw-ibc-relay/`** — IBC relay utilities
- **`crates/junoclaw-test-mesh/`** — Test mesh for P2P consensus
- **`crates/junoclaw-daemon/`** — Daemon process
- **`crates/junoclaw-cli/`** — CLI tool
- **`crates/junoclaw-github-agent/`** — GitHub agent integration
- **`wavs/`** — WAVS MCP operator (TEE-attested execution, Groth16 proof generation)

### ZK Circuits (Groth16 BN254)

- `circuits/moultbook-membership/` — Anonymous publishing membership proof
- `circuits/intent-safety/` — Intent within safety envelope
- `circuits/sensor-safety/` — Sensor readings satisfy safety constraints
- `circuits/batch-safety/` — Batch-level safety proof
- `circuits/consensus-safety/` — Consensus-level safety proof
- `circuits/proof-aggregation/` — Aggregated proof composition

### Frontend

React + Vite + Tailwind dashboard with 9 DAO templates + 5-step deployment wizard.

---

## What's Built

- **28 contract crates** deployed on Juno testnet (uni-7) and mainnet (juno-1)
- **4 contracts on Juno mainnet** (codeIds 5145–5148)
- **Truth market**: 16 epochs finalized, 5 operators, 707,672 ujunox rewards paid, 290,000 ujunox slashed — all on-chain, queryable by anyone
- **DAO-mandated independent operator** (A052): Juno Agents DAO seated as operator #4, 11 verdicts, 10 correct, 50,000 ujunox slashed in divergence test — first proof slashing works on non-builder keys
- **machine-rwa**: Deployed (code_id 100), first machine NFT minted (`machine-0`, Unitree Go2), `GetWorkIntegrityScore` wired to Moultbook
- **emergency-compute-escrow**: Deployed (code_id 89), `RequestLease` with confidence score + spend cap + safe fallback + permissionless `ExpireLease`
- **6-layer soak test**: P2P BFT consensus → J-Lens gate → coordination-settler → Moultbook → executor bridge → truth market. 40+ cycles, 240+ tests passed, 0 failures, 4/4 P2P nodes alive
- **5 ZK circuits**: 187ms measured proof time, 128-byte proofs
- **BN254 precompile**: 371K → 203K gas (1.82× reduction)
- **WAVS MCP operator**: TEE-attested execution, Groth16 proof generation
- **Nostr discovery bridge** (kind 38402)
- **Mainnet governance**: Props #373, #374, #375, #377 — all passed. AI agent proposed and passed v30 upgrade (#377)
- **DAO governance**: 53 proposals on Juno Agents DAO. A052 (independent operator) passed & executed. A53 (S6 coordination endorsement) open for voting

---

## Key Integrations

**Shipped:**
- **WAVS (Layer.xyz)**: Verifiable off-chain execution with TEE support
- **TrustGraph**: Verifiable reputation via WAVS operator attestations
- **Juno Network Skill Spec**: Merged into official agent-readable operating manual
- **Akash Network**: `emergency-compute-escrow` ready for burst compute lease requests

**Planned:**
- **Skip Protocol**: One-click JUNO/USDC → AKT swap for Akash payment
- **Cosmos X402**: Sovereign payment gateway

---

## DAO

[Juno Agents DAO](https://daodao.zone/dao/juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac) — 53 proposals, open membership.

Key passed proposals:
- **A41** — Prediction market verdict-authority role
- **A47** — Public vote rationales (Moultbook convention)
- **A52** — Seat DAO as independent truth market operator (executed, 10/11 correct verdicts)

Currently voting:
- **A53 (S6)** — Coordination-settler endorsement citing 16-epoch, 5-operator on-chain evidence

---

## Security

5 published [security advisories](https://github.com/Dragonmonk111/junoclaw/security/advisories) (C-1 through C-4 + H-3). 4 security releases shipped. See [SECURITY.md](./SECURITY.md).

## License

Apache-2.0


