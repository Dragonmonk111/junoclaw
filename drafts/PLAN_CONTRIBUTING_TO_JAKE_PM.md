# Plan: Contributing to Jake's Prediction Market Repo

> **Repo**: https://github.com/CosmosContracts/pm
> **Our repo**: junoclaw — agent-company contract with OutcomeCreate/OutcomeResolve + WAVS/TEE attestation
> **Date**: Jul 19, 2026

---

## 1. What Jake Has Built

### cw-reality (live on juno-1, Code ID 5121)
- CosmWasm port of Reality.eth — bond-escalating crowdsourced oracle
- Questions, answers, bond escalation, history-hash chain, arbitration
- Arbitrator is an address permission (DAO DAO, x/gov, multisig, or None)
- 57 tests passing, Apache-2.0

### binary-market (in development, v0.1.0)
- FPMM (Funding Prediction Market Maker) binary YES/NO market
- Split/Merge/Buy/Sell/Redeem positions
- LP shares with fee accounting
- Bonded challenge gate → arbitration flow
- GovernanceVerdict entrypoint (only immutable verdict_authority can call)
- Configured as its question's arbitrator in cw-reality
- V1 pins Juno Agents DAO core (`juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac`) as verdict_authority

### market-factory (scaffolded)
- Permissionless market instantiation
- Versioned code ID, discoverability metadata, protocol bounds

### pm-types (shared wire types)
- Outcome, Payout, Question, TierId, Ujuno, OracleAnswer, ProtocolVersion

### Architecture (from GOAL.md)
```
market factory → binary-market instance → cw-reality (oracle)
                       ↑
              verdict_authority (Juno Agents DAO core)
```

### Key design decisions
- Arbitrator = address permission, not adapter contract
- V1 verdict_authority = Juno Agents DAO core (our agent-company contract!)
- No adapter contracts — the address IS the interface
- Collateral = ujuno only
- One contract per market (fund isolation)

---

## 2. What We Have That Jake Needs

### Our agent-company contract (v7, Code ID 80 on uni-7)
- **OutcomeCreate** proposal kind — creates verifiable outcome markets with WAVS trigger events
- **OutcomeResolve** proposal kind — resolves with WAVS-attested outcome + attestation_hash
- **WAVS attestation system** — SubmitAttestation entrypoint, stores attested results on-chain
- **TEE-sealed signer** — hardware-attested signing, deterministic tx generation
- **Invoke API** — off-chain WASM component execution with hardware attestation
- **DAO governance** — proposal → vote → execute flow with quorum + supermajority

### The critical connection
Jake's GOAL.md explicitly states:
> "V1 pins the active Juno Agents DAO core `juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac`"

**He's already using our DAO as the verdict authority.** The contribution angle is making this integration real, tested, and documented.

---

## 3. Contribution Plan — 5 Concrete PRs

### PR 1: WAVS-Attested Resolution Adapter (highest value)

**What**: Add a WAVS attestation verification path to binary-market's `GovernanceVerdict` flow.

**Why**: Currently, the verdict_authority (our DAO) calls `GovernanceVerdict` with an answer. But there's no on-chain proof that the answer came from a TEE-attested WAVS computation. We can add an optional `attestation_hash` field to the verdict call, and the binary-market contract verifies it against our agent-company's attestation registry.

**How**:
1. Add `attestation_hash: Option<String>` to `GovernanceVerdict` execute message
2. When present, binary-market queries agent-company contract for the attestation
3. If attestation exists and matches, emit a `wavs_verified_verdict` event
4. This creates the "verifiable outcome market" — every resolution has a hardware-sealed receipt

**Files to touch**:
- `contracts/binary-market/src/msg.rs` — add optional attestation_hash to GovernanceVerdict
- `contracts/binary-market/src/contract.rs` — add cross-contract query to agent-company
- `contracts/binary-market/src/state.rs` — store attestation_hash in Lifecycle
- `contracts/binary-market/tests/` — new test for attested verdict flow

**Effort**: ~1 day

---

### PR 2: Market Factory Integration with Agent-Company OutcomeCreate

**What**: When a market is created via market-factory, emit an `outcome_create` event that our WAVS bridge can pick up for attested resolution.

**Why**: Our agent-company already has `OutcomeCreate` which emits WAVS trigger events. Jake's market-factory creates markets. Connecting them means every market created on Jake's platform gets WAVS-attested resolution for free.

**How**:
1. market-factory emits a standardized `outcome_market_created` event with question, market address, close_ts, opening_ts
2. Our WAVS bridge watches for these events (new trigger type)
3. When the market reaches resolution time, WAVS fetches external data inside TEE, produces attested answer
4. WAVS submits the answer to agent-company as a proposal
5. DAO votes → executes → calls GovernanceVerdict on the market

**Files to touch**:
- `contracts/market-factory/src/contract.rs` — emit outcome_market_created event
- `wavs/bridge/src/event-watcher.ts` — new trigger type for market creation
- `wavs/bridge/src/local-compute.ts` — new WASI component for market resolution

**Effort**: ~2 days

---

### PR 3: cw-reality WAVS Oracle Integration

**What**: Add a WAVS-powered answer path to cw-reality. Instead of only human bond-escalation, allow a TEE-attested WAVS operator to submit answers with hardware proof.

**Why**: cw-reality currently relies on bond-escalation (humans stake bonds on answers). For objective questions ("Did BTC exceed $100k at block X?"), a TEE-attested oracle is cheaper, faster, and more reliable than bond escalation. The WAVS operator fetches the data inside a TEE, produces an attested answer, and submits it. Bond escalation still works as a challenge mechanism on top.

**How**:
1. Add `SubmitAttestedAnswer` message to cw-reality
2. Accepts: question_id, answer, attestation_hash, attestation_contract (agent-company address)
3. cw-reality queries agent-company to verify the attestation exists and matches
4. If verified, the answer is posted with reduced bond requirement (TEE attestation = higher trust)
5. Bond escalation still applies for challenges

**Files to touch**:
- `contracts/cw-reality/src/msg.rs` — add SubmitAttestedAnswer
- `contracts/cw-reality/src/contract.rs` — implement attested answer path
- `contracts/cw-reality/src/state.rs` — store attestation reference
- `contracts/cw-reality/tests/` — new tests for attested answer flow

**Effort**: ~2 days

---

### PR 4: Test Integration — Agent-Company as Verdict Authority

**What**: Integration tests that wire our agent-company contract as the verdict_authority for a binary-market instance, end-to-end.

**Why**: Jake's GOAL.md says V1 pins our DAO core as verdict_authority, but there are no integration tests proving this works. We can provide:
1. Test that creates a binary-market with our agent-company as verdict_authority
2. Test that submits a DAO proposal → votes → executes → calls GovernanceVerdict
3. Test that verifies the full lifecycle: create market → trade → challenge → DAO verdict → resolve → redeem

**How**:
1. Use cw-multi-test to set up both contracts
2. Write test helpers that simulate DAO proposal flow
3. Test all lifecycle transitions through the DAO verdict path
4. Include edge cases: stale verdict, wrong sender, double resolution

**Files to touch**:
- `contracts/binary-market/tests/` — new `verdict_authority_integration.rs`
- May need a test helper crate or shared test utilities

**Effort**: ~1-2 days

---

### PR 5: Documentation — WAVS + PM Integration Guide

**What**: A comprehensive guide explaining how WAVS-attested resolution works with Jake's prediction market stack.

**Why**: Jake's repo has excellent architecture docs (GOAL.md, ARBITRATION.md). We can contribute a companion doc explaining the WAVS layer.

**Contents**:
1. How WAVS attestation works (TEE-sealed computation, on-chain verification)
2. How agent-company's OutcomeCreate/OutcomeResolve maps to binary-market's lifecycle
3. How the WAVS bridge watches market events and triggers attested resolution
4. Security model: what WAVS proves, what it doesn't, how bond escalation complements it
5. Deployment guide: wiring agent-company as verdict_authority for a live market

**Files to touch**:
- `docs/wavs-integration.md` (new)

**Effort**: ~0.5 days

---

## 4. Execution Order

| Priority | PR | Why first | Effort |
|---|---|---|---|
| 1 | PR 4 (Integration tests) | Proves the basic integration works before adding features | 1-2 days |
| 2 | PR 1 (Attested verdict) | Core value-add — makes resolutions verifiable | 1 day |
| 3 | PR 5 (Docs) | Document what we've built so Jake can review | 0.5 days |
| 4 | PR 3 (cw-reality WAVS oracle) | Deep integration — attested answers in the oracle itself | 2 days |
| 5 | PR 2 (Factory + bridge) | Connects market creation to WAVS trigger pipeline | 2 days |

**Total effort**: ~6-7 days

---

## 5. First Step: Fork + Clone + Validate

```bash
# Fork the repo on GitHub
# Clone our fork
git clone https://github.com/<our-org>/pm.git
cd pm/contracts

# Install pinned Rust toolchain
rustup install 1.85.1
rustup default 1.85.1

# Run validation
./scripts/validate.sh
```

Then start with PR 4 (integration tests) to prove the agent-company ↔ binary-market connection works.

---

## 6. Contribution Guidelines (from CONTRIBUTING.md)

- Currently accepts maintenance and research changes
- Don't change deployed behavior without an approved issue
- PRs must explain behavior impact and whether Wasm checksums change
- Run `./scripts/validate.sh` before submitting
- No secrets, keyrings, generated build output, or quarantined scripts
- Dependencies are lockfile-pinned
- Security reports follow SECURITY.md

**Strategy**: Open issues first for PR 1, 3, 4 (behavior changes). PR 5 (docs) can go directly. PR 2 touches our repo too, so it's a cross-repo coordination.

---

## 7. The Pitch to Jake

```
Hey Jake — saw your prediction market repo. We've been building the WAVS/TEE attestation layer on Juno and I think there's a natural fit.

Your GOAL.md already pins the Juno Agents DAO core as the verdict_authority for v1. We built that DAO core (agent-company contract) with OutcomeCreate/OutcomeResolve proposal kinds that emit WAVS trigger events for TEE-attested resolution.

What we can contribute:
1. Integration tests proving agent-company works as verdict_authority for binary-market
2. Optional attestation_hash on GovernanceVerdict — so every resolution has a hardware-sealed receipt
3. A WAVS oracle path for cw-reality — TEE-attested answers for objective questions, with bond escalation as the challenge layer on top
4. Docs explaining the WAVS integration

We've got:
- agent-company v7 on uni-7 (Code ID 80) with OutcomeCreate/OutcomeResolve + attestation registry
- WAVS invoke API prototype (15/15 smoke tests, E2E proven on uni-7)
- Cross-platform determinism proven (AMD EPYC, 3/3 byte-identical)
- TEE deployment plan finalized

Want me to open issues for these, or would you prefer to discuss the integration shape first?
```

---

## 8. Key Technical Details

### Our agent-company attestation flow
```
1. OutcomeCreate proposal → emits `outcome_create` event
2. WAVS bridge sees event → triggers WASI component inside TEE
3. WASI component fetches external data, produces answer + attestation
4. WAVS bridge calls SubmitAttestation on agent-company
5. DAO votes on OutcomeResolve proposal (includes attestation_hash)
6. Proposal passes → executes → calls GovernanceVerdict on binary-market
```

### Jake's binary-market verdict flow
```
1. Market reaches AwaitingResolution state
2. User posts challenge bond → RequestArbitration to cw-reality
3. Market enters PendingArbitration
4. verdict_authority (our DAO) calls GovernanceVerdict { question_id, answer, payee }
5. Market processes verdict → enters Resolved
6. Users redeem positions
```

### The merge point
Our DAO's OutcomeResolve execution should call `GovernanceVerdict` on the binary-market contract. This is already architecturally compatible — Jake designed it this way. We just need to:
1. Wire the proposal execution to emit the right WasmMsg
2. Add attestation verification
3. Test it end-to-end

---

## Status: READY TO START

Fork the repo, run validate.sh, start with PR 4 (integration tests).
