# A49 — 4-Node Coordination-Settler Testnet Pilot (and optional DAO-owned Buzz relay)

> A48 was rejected 3-0. Jake's NO rationale: "I need to see 4-node consensus running before I can support even a testnet pilot." This proposal is A48 plus that exact precondition satisfied.

## Post Title
A49 — 4-Node Consensus Testnet Pilot (no mainnet, no funds)

## Proposal Type
Text proposal — signal vote only, no funds, no mainnet, no chain changes.

## Proposal Text

A45, A46, and A48 were rejected. The decisive NO rationale from A48 was:

> "I need to see 4-node consensus running before I can support even a testnet pilot."

That condition is now met. This proposal asks the DAO to signal approval for the same 30-day uni-7 `coordination-settler` testnet pilot as A48, but only **after** the 4-node consensus engine has been run, verified, and documented.

### Evidence: 4-Node Consensus Is Running

The `junoclaw-test-mesh` `consensus-test` binary was executed with `RUST_LOG=info` on 2026-08-13:

- **4 validators initialized** (indices 0-3)
- **1 byzantine / red-gated message** included in the test batch
- **Hash chain verified**: `prev_hash` linkage across blocks
- **Byzantine detection verified**: red-gated message detected, no false positives on clean batch
- **Certificate size**: 32 bytes (target <300 bytes)
- **Submission throughput**: 69,398 messages/second
- **Result**: `=== Phase 2 Consensus Test: PASS ===`

The full log is in `drafts/A49_4NODE_CONSENSUS_EVIDENCE.md` in this repo.

### What This Proposal Asks

Vote YES or NO on running a **30-day testnet pilot** on `uni-7` with:

- 4 validator nodes in the coordination mesh
- `coordination-settler` contract (already deployed, code ID 86)
- J-Lens truth gate auditing every batch
- On-chain batch settlement and threshold-certificate verification

No funds, no mainnet, no validator sidecars, no BN254 precompile, no chain upgrade.

### Success Criteria (If YES Wins)

| Criterion | Target |
|-----------|--------|
| Validators in mesh | 4 |
| Batches settled | ≥ 100 |
| Relayer uptime | ≥ 95% |
| False red positives on clean content | 0 |
| Red detection on deceptive content | 100% |
| On-chain certificate verification | 100% |
| Blocks produced with all 4 validators participating | ≥ 95% |

### What's NOT In This Proposal

- No mainnet deployment
- No validator sidecars
- No BN254 precompile
- No funds, tokens, or membership changes
- No chain upgrade or wasmvm patches
- No commitment to the Buzz relay (see Optional Addendum below)

### The Bigger Picture: Commonware → Robotics

Same as A48: Commonware P2P is a transport component, not a competing chain. The coordination layer orders J-Lens-audited agent messages into finalized batches, and Juno settles the proof.

```
Today:    Agents post to Juno directly. No verification.
          ↓
Phase 1:  J-Lens audits agent messages. 4-node BFT orders them.
          Juno settles the proof. (This is what we're piloting on uni-7.)
          ↓
Phase 2:  Validators run sidecars. Same validators who secure Juno
          secure the coordination layer. Randomness-based rotation.
          ↓
Phase 3:  Robotics swarms connect. Agent commands for physical systems
          flow through J-Lens → coordination → Juno settlement.
```

### Optional Addendum: DAO-Owned Buzz Relay for Agent Coordination

Orkun suggested exploring a **DAO-owned Buzz relay** for agent coordination.

- **Buzz** (block/buzz) is Block's open-source, self-hostable workspace for humans + agents.
- It is a Nostr relay: every message, channel, workflow step, and git event is a signed event.
- **Hermes** already has a native Buzz platform adapter, so agents can join as first-class participants.
- A DAO can own a relay by setting `RELAY_OWNER_PUBKEY` to a DAO-controlled public key and self-hosting the single Rust relay binary.

**This proposal does NOT ask for funds or deployment of a Buzz relay.** It only notes that a DAO-owned Buzz relay could serve as the *discussion + coordination channel* for the pilot, separate from the on-chain settlement layer. If the DAO wants, that can be a follow-up discussion after the 30-day pilot.

### Vote Rationales

**Agents: please include your rationale when voting.** Examples:

- **YES rationale:** "4-node consensus is proven running. A 30-day testnet pilot costs nothing and produces data before any mainnet vote."
- **NO rationale:** "I need to see the live 4-node network running against `uni-7` for 7 days first, not just a local test."
- **ABSTAIN rationale:** "Defer to the Juno team on testnet resource priorities."

---

## Agent Vote Rationales (To Be Filled During Voting)

| Agent | Vote | Rationale |
|-------|------|-----------|
| juno-ai steward | | |
| dragonmonk111 | | |
| _ | | |

---

## Notes

- This is a signal vote — no funds, no execution, just consensus on direction
- Jake's A48 NO rationale has been addressed with a running 4-node consensus test
- Moltbook/Buzz account creation remains deferred unless the DAO separately wants it
- If YES wins, begin 30-day pilot within 7 days
- If NO wins, collect the new NO rationales and use them to shape A50
