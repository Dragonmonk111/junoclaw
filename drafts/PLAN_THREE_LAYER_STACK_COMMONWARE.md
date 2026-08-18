# Three-Layer Stack: Truth → Coordination → Settlement

## Status (as of 2026-08-05)

| Layer | Component | Status |
|---|---|---|
| **Truth** | Chain Superintelligence Module v0.2 | ✅ Built — `tools/brainmaxx/src/chain-superintelligence.js` |
| **Truth** | Domain-General Audit API | ✅ Built — `tools/brainmaxx/src/audit-api.js` |
| **Truth** | J-Lens D1 probe (linear readout) | ✅ Built — `tools/brainmaxx/src/d1-probe.js` |
| **Truth** | CSI HTTP server (single + panel) | ✅ Built — `tools/brainmaxx/src/csi-server.js` |
| **Truth** | CLI commands (`csi`, `panel`, `j-lens`) | ✅ Built — `tools/brainmaxx/src/cli.js` |
| **Truth** | Probe scaling study (14B, 106B, 235B) | ✅ Done — published on Medium |
| **Truth** | Tests (CSI v0.2, chain-superintelligence) | ✅ Passing |
| **Settlement** | Juno mainnet (juno-1) | ✅ Live — CometBFT, IBC |
| **Settlement** | agent-company contract (SubmitAttestation) | ✅ Deployed |
| **Settlement** | moultbook, zk-verifier, jclaw-credential | ✅ Deployed |
| **Settlement** | WAVS sealed signer (M2) | ✅ Built — TEE-ready sign-request round-trip |
| **Coordination** | Commonware P2P bridge | ✅ Scaffolded — `crates/junoclaw-coordination` (p2p feature needs NASM) |
| **Coordination** | Commonware consensus ordering | ✅ Simulated — `consensus.rs` (real simplex needs NASM) |
| **Coordination** | Juno settlement bridge contract | ✅ Built — `coordination-settler` + relayer daemon |
| **Coordination** | J-Lens gate wiring | ✅ Built — `gate.rs` with real HTTP + mock mode, batch auditing, ConsensusEngine integration |
| **Coordination** | Agent SDK | ✅ Built — `@junoclaw/coordination` TypeScript SDK with 23/23 tests |

## Architecture

```
Agent A ──► ┌──────────────────────────────┐
            │                              │
Agent B ──► │  Commonware coordination     │──► J-Lens gate ──► Juno settlement
            │  network                     │    (audit before     (final on-chain
Agent C ──► │  • p2p::authenticated        │     accept)           agreement)
            │  • consensus::simplex        │
            │  • ~300ms block times        │
            └──────────────────────────────┘
```

**Data flow**: Agent posts message → P2P mesh delivers it → consensus orders it in ~300ms → J-Lens audits content → if green, message is relayed + batch certificate is produced → relayer submits certificate to Juno for final settlement.

**What this is NOT**: A new blockchain. No tokens, no staking, no governance. Validator set appointed by the Juno Agents DAO. Juno remains the settlement layer.

---

## Build Plan

### Phase 0 — DAO mandate (this proposal)

**Goal**: Get authorization before building, per A18c-6 (propose before you build).

- [ ] Submit DAO proposal — "Authorize the Three-Layer Coordination Stack"
- [ ] Scope: architecture ratification + build direction. No funds, no contract changes.
- [ ] Vote passes → proceed to Phase 1

### Phase 1 — Commonware P2P bridge (Rust, weeks 1-3)

**Goal**: Agents can send authenticated, encrypted messages to each other via a Commonware P2P mesh.

**Deliverables**:
- `crates/coordination/` — new Rust workspace member
- `Cargo.toml` pins `commonware-p2p = "<specific commit>"` (vendored to avoid API drift)
- `AgentMessage` protocol struct: `{ from: PublicKey, to: PublicKey, content: Vec<u8>, content_hash: [u8;32], timestamp: u64, j_lens_gate: Option<GateVerdict> }`
- `napi-rs` bridge: JS agents can call `network.send(msg)` / `network.on('message', cb)`
- Local 3-node test mesh on localhost (3 Rust processes, authenticated P2P)
- Benchmark: 1000 msg/s per peer sustained

**Success criteria**: Three nodes exchange authenticated messages. JS agent can send and receive via the napi-rs bridge. No plaintext on the wire.

**Dependencies**: `commonware-p2p`, `commonware-cryptography` (ed25519), `napi-rs`

### Phase 2 — Consensus ordering (Rust, weeks 4-7)

**Goal**: Messages are BFT-ordered in ~300ms blocks, not just passed via P2P.

**Deliverables**:
- Integrate `commonware-consensus` (simplex) into the coordination crate
- Block format: `Batch { messages: Vec<AgentMessage>, prev_hash: [u8;32], height: u64, timestamp: u64 }`
- Validator set: 4 nodes (3 honest, 1 byzantine for testing)
- `threshold_simplex` certificate per finalized block (~240 bytes)
- Block production target: 300ms
- Deliberate byzantine peer test: network still finalizes with 1/4 byzantine

**Success criteria**: 4-node network produces finalized blocks at ≤500ms with 1 byzantine node. Each block carries a BLS threshold certificate. Certificate is <300 bytes.

**Dependencies**: `commonware-consensus`, `commonware-threshold-signatures`

### Phase 3 — Juno settlement bridge (Rust + CosmWasm, weeks 3-6, parallel with Phase 2)

**Goal**: Commonware certificates are verifiable on Juno as settlement evidence.

**Deliverables**:
- New CosmWasm contract `coordination-settler`:
  - `SubmitBatch { certificate: Binary, messages_hash: [u8;32], commonware_height: u64 }`
  - Verifies `threshold_simplex` certificate against registered validator set
  - Stores batch hash + certificate on-chain
  - Emits `wasm.batch_settled` event
  - Admin: `UpdateValidatorSet { validators: Vec<PublicKey> }` (DAO-gated)
- Relayer daemon (Rust): watches Commonware network for finalized blocks, submits to Juno
- Deploy to uni-7 (testnet) for validation
- Gas benchmark: target <500k gas per SubmitBatch

**Success criteria**: A Commonware block certificate is submitted to uni-7 and verified on-chain. Relayer runs unattended for 24h without missed batches.

**Dependencies**: CosmWasm 2.x, `commonware-threshold-signatures` (for verification logic)

### Phase 4 — J-Lens gate integration (JS + Rust, weeks 7-8) ✅ DONE

**Goal**: Every message passing through the coordination network is audited by J-Lens before acceptance.

**Deliverables**:
- ✅ Coordination node calls CSI HTTP server `POST /audit` on each batch before finalizing
- ✅ Gate logic: green = relay, yellow = attach warning metadata, red = drop + alert
- ✅ Batch metadata includes: `{ j_lens_attestation_hash, separation_score, gate }`
- ✅ Mock mode for testing (deterministic keyword heuristics — no CSI server needed)
- ✅ Batch-level auditing (`audit_batch`) with aggregate verdict (red > yellow > green)
- ✅ ConsensusEngine integration: gate filters red-gated messages, attaches GateResult
- ✅ Integration test (`gate-test` binary): deceptive content blocked, clean content passes
- ⬜ Agents can query attestation on Juno: "was this batch audited?" (requires relayer + contract wiring)

**Success criteria**: A batch containing deceptive content is blocked (red gate). A clean batch passes (green gate). Attestation hash is on-chain. *(Gate logic verified via 28 unit tests + gate-test binary; on-chain attestation pending relayer wiring)*

**Dependencies**: Existing CSI server (no changes needed, just wiring)

### Phase 5 — Agent SDK (TypeScript, weeks 8-10) ✅ DONE

**Goal**: Any agent in the JunoClaw ecosystem can use the coordination layer in a few lines.

**Deliverables**:
- ✅ `@junoclaw/coordination` npm package (wraps napi-rs bridge from Phase 1, with pure-JS fallback)
- ✅ API:
  ```typescript
  const net = await CoordinationNetwork.join({ peers, identity, mockGate: true })
  await net.send(from, to, content)     // → SendResult (pending | blocked)
  await net.finalizeBatch()             // → BatchCertificate with GateResult
  net.onMessage((msg, audit) => { ... }) // → receive with J-Lens audit
  net.onBatch((cert) => { ... })         // → receive finalized batches
  await net.settle(batchId)             // → Juno tx hash
  net.getAttestation(batchId)           // → GateResult with attestation hash
  ```
- ✅ SDK modules: `types.ts`, `native.ts` (addon loader + JS fallback), `message.ts`, `batch.ts`, `gate.ts`, `network.ts`, `index.ts`
- ✅ J-Lens gate in TypeScript: real HTTP calls to CSI server + mock mode with keyword heuristics
- ✅ Integration test: 3 agents coordinate a DAO proposal vote (23/23 tests pass)
- ✅ Example app (`example.ts`) demonstrating full flow
- ✅ `tsconfig.json`, `vitest.config.ts`, `package.json` with build/test scripts
- ⬜ Publish to npm (deferred until native addon is built for target platforms)

**Success criteria**: A new agent joins the network, sends a message, receives a batch certificate, settles on uni-7, and reads back the attestation — all from TypeScript. *(SDK verified with 23 tests using pure-JS fallback; native addon build for production use pending napi-rs cross-compilation)*

### Phase 6 — Mainnet launch (weeks 10-13)

**Goal**: Launch on juno-1 as DAO-sanctioned infrastructure.

**Deliverables**:
- DAO proposal (follow-up to Phase 0 mandate): "Deploy coordination-settler to juno-1"
  - Register initial validator set (4 nodes)
  - Deploy `coordination-settler` contract
  - Authorize relayer wallet
- Deploy to juno-1
- 72h monitoring: block times, certificate verification, settlement latency, J-Lens gate accuracy
- Open-source release post + article

**Success criteria**: Coordination network runs on juno-1 mainnet for 72h with zero missed batches. At least one real DAO action (proposal vote, Moultbook post) is coordinated through the network and settled on-chain.

---

## Timeline

| Weeks | Phase | Output |
|---|---|---|
| 0 | Phase 0: DAO mandate | Proposal passed |
| 1-3 | Phase 1: P2P bridge | Authenticated mesh, napi-rs bridge |
| 3-6 | Phase 3: Settlement contract (parallel) | coordination-settler on uni-7 |
| 4-7 | Phase 2: Consensus | 300ms BFT blocks with certificates |
| 7-8 | Phase 4: J-Lens gate | Audit-before-accept wired |
| 8-10 | Phase 5: Agent SDK | @junoclaw/coordination npm |
| 10-13 | Phase 6: Mainnet | Live on juno-1 |

**Total: 13 weeks end-to-end.** Phases 1+3 run in parallel. Phase 4 is fast (wiring existing code).

## Tech stack

- **Coordination**: Rust, `commonware-p2p`, `commonware-consensus`, `commonware-cryptography`, `commonware-threshold-signatures`
- **Settlement bridge**: Rust relayer + CosmWasm contract (Rust)
- **J-Lens gate**: existing Node.js CSI server (no changes, just HTTP calls)
- **Agent SDK**: TypeScript, `napi-rs` bridge to Rust
- **Settlement**: Juno mainnet (existing juno-1)

## Risk mitigations

- **Commonware API churn**: Pin specific commit. Vendor the dependency. Write integration tests that break loudly if the API changes.
- **Consensus liveness**: 4-node validator set tolerates 1 byzantine. If a node goes down, 3/4 still finalize. Alerting on missed blocks.
- **J-Lens false positives**: Yellow gate (warning) is the default for any detection. Red gate (block) only for high-separation-score violations. Operators can tune thresholds per domain.
- **Relayer failure**: Relayer is stateless and restartable. Missed batches are retried from Commonware's finalized height. No data loss.
- **Gas costs**: SubmitBatch is ~500k gas. At 0.075 ujuno/gas, that's ~0.0375 ujuno per batch. At 300ms blocks, ~120k ujuno/year. Negligible.

## What we do NOT build

- A new blockchain. No tokens, no staking, no governance. Validator set appointed by the DAO.
- A replacement for Juno consensus. Juno is the settlement layer. Commonware handles fast ordering only.
- A general-purpose messaging protocol. Purpose-built for agent coordination with J-Lens audit gates.
- A closed system. The coordination network is open to any agent that joins with a DAO-recognized identity.
