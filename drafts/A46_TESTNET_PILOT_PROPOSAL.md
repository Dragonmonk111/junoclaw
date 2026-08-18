# A46 — Coordination-Settler 30-Day Testnet Pilot: uni-7

> A45 asked for architecture ratification and was rejected. This proposal narrows the ask to a single, reversible, zero-funds action: run the already-deployed `coordination-settler` on uni-7 for 30 days, settle 100+ audited agent-message batches, and collect concrete data before any mainnet or sidecar decision is made.

---

## Copy-paste box 1: Title

```
A46 — Coordination-Settler 30-Day Testnet Pilot: uni-7
```

## Copy-paste box 2: Description

```
A45 asked the DAO to ratify the full three-layer coordination architecture and was rejected. The juno-ai steward again voted NO. This proposal takes a much smaller step: it asks the DAO to endorse a 30-day testnet-only pilot of the already-deployed coordination-settler contract on uni-7.

No mainnet. No sidecars. No BN254. No funds. Just 30 days of settled testnet batches.

WHAT THIS PROPOSAL DOES:

1. Authorizes a 30-day testnet pilot on uni-7 for the coordination-settler contract (already deployed at juno16gp6mm7ehhrx0ls4s77yk668cclq5gyc09x7k45f28apwmsqkg0qhmmnc8, code ID 86).
2. Directs the builder to settle at least 100 audited agent-message batches on-chain over the 30 days, using the existing relayer daemon and a DAO-appointed 4-node testnet validator set.
3. Requires every batch to pass the J-Lens truth gate before submission (green proceeds, yellow flagged, red blocked) so the pilot produces real data on gate + coordination + settlement working together.
4. Defines explicit, public success criteria that must be met before any mainnet or sidecar proposal can follow:
   - 100+ batches settled on uni-7 without manual intervention
   - 0 false positives at red level on known-good agent messages (clean content must not be blocked)
   - 100% of known-deceptive messages correctly blocked at red level
   - Relayer uptime >= 95% measured over the 30 days
   - All batch certificates verified on-chain (contract rejects forged certs)
5. States clearly that mainnet deployment, validator sidecars, and BN254 remain separate, future proposals — this vote is testnet-only.

WHAT THE PILOT WILL PROVE:
- The J-Lens gate audits agent message content in real time.
- The coordination engine orders audited messages into batches.
- The relayer submits finalized batches to Juno testnet reliably.
- The coordination-settler verifies certificates on-chain.
- The whole path can run for 30 days without breaking.

CURRENT STATUS (updated since A45):
- coordination-settler contract: deployed on uni-7, code ID 86; 2 batches already settled on-chain (txs 3D2EF675... and 2DD277E9...)
- J-Lens gate: built, 28 Rust tests pass, verified to block deceptive content and pass clean content
- Agent SDK: built, 23 TypeScript tests pass, 3-agent demo works
- Relayer daemon: built and successfully tested against live uni-7
- Commonware P2P mesh: compiles with NASM (aws-lc-sys builds successfully); real multi-node mesh needs the 30-day testnet to shake out
- Validator sidecar: still not built; will be a separate, later proposal after the pilot succeeds

WHAT IS NOT IN THIS PROPOSAL:
- No mainnet contract deployment.
- No validator sidecar requirement.
- No BN254 precompile mandate or ask.
- No funds, tokens, membership changes, or slashing.
- No architecture ratification beyond "run the testnet pilot."

This is not a blank check. It is a 30-day, testnet-only experiment with a hard stop. If the pilot fails any success criterion, the DAO can simply not proceed to a mainnet proposal — no harm, no chain change, no liability for the Juno team.

VOTING:
- YES = run the 30-day uni-7 testnet pilot with the success criteria above.
- NO = do not run the pilot at this time.
- ABSTAIN = defer to builders.

No funds requested. No mainnet contract changes. No membership changes. No tokens.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A46 — Coordination-Settler 30-Day Testnet Pilot: uni-7",
  "description": "A45 asked the DAO to ratify the full three-layer coordination architecture and was rejected. This proposal takes a much smaller step: endorse a 30-day testnet-only pilot of the already-deployed coordination-settler contract on uni-7. No mainnet, no sidecars, no BN254, no funds. The builder will settle 100+ J-Lens-audited agent-message batches over 30 days using a DAO-appointed 4-node testnet set. Success criteria: 100+ settled batches, 0 red false positives on clean content, 100% red blocking on deceptive content, >=95% relayer uptime, all certificates verified on-chain. Mainnet, sidecars, and BN254 remain separate future proposals. This is a reversible, testnet-only experiment.",
  "funds": []
}
```

## Background: Why A45 Failed and What Changed

A45 failed for the same reason A44 did: the juno-ai steward (3/6 voting power) saw the scope as too broad and the ask as too far ahead of the deployed proof. Even though A45 removed the BN254 mandate and narrowed to "architecture ratification," it still bundled:
- endorsement of the three-layer architecture
- validator sidecar as the production target
- BN254 as an open question
- mainnet gating language
- a 4-node testnet validator set

That was still too much. The steward wanted to see the stack actually run on testnet before endorsing the next layer.

A46 fixes this by removing everything except the testnet pilot. The only ask is: "Let us run the existing code for 30 days and prove it works." This is harder to reject because:
- It does not ask for any architecture ratification.
- It does not ask validators to run anything.
- It does not ask for mainnet.
- It does not ask for funds.
- It has a hard stop and public success criteria.

## New Information Since A45

1. **Relayer now works on live uni-7.** Two batches have already been settled on-chain (txs `3D2EF675...` and `2DD277E9...`). The certificate verification path is real and verified.
2. **Commonware P2P compiles with NASM.** The `aws-lc-sys` build succeeds, `commonware-p2p` v2026.7.0 compiles, and the Commonware team is independently launching production clusters. This validates the P2P primitives.
3. **Validator sidecar guide and master article are drafted.** Outreach material is ready for after the pilot, not before.
4. **J-Lens trust scoring is wired.** Attestations from the coordination layer feed directly into `tools/context-agent/src/trust.js` (green +1, red -5, yellow tracked).

## The 30-Day Plan

### Week 1: Baseline
- Day 1-2: Finalize P2P compile and run 4-node testnet mesh (local or DO/hetzner)
- Day 3-5: Run relayer continuously, settle 20+ batches/day
- Day 6-7: Measure relayer uptime, gate accuracy, certificate verification

### Week 2: Load
- Day 8-11: Increase batch frequency, test with mixed green/yellow/red messages
- Day 12-14: Introduce a controlled byzantine node, verify BFT tolerance

### Week 3: J-Lens Integration
- Day 15-18: Wire J-Lens gate into the live message path
- Day 19-21: Collect data on false positive/negative rates

### Week 4: Reporting
- Day 22-25: Generate final metrics
- Day 26-30: Write pilot report, publish data, decide on mainnet proposal

## Success Criteria (Public and Testable)

| Criterion | Target | How Verified |
|-----------|--------|--------------|
| Batches settled | >= 100 | On-chain query of `coordination-settler` `Batch` state |
| Relayer uptime | >= 95% | Logs / Prometheus metrics over 30 days |
| Red false positives (clean content blocked) | 0 | Known-good test messages must pass gate |
| Red detection (deceptive content blocked) | 100% | Known-bad test messages must be blocked |
| Certificate forgery rejected | 100% | Contract tests and manual injection attempts |
| P2P mesh liveness | >= 99% | Peer count heartbeats from 4-node set |

## Risk and Mitigation

| Risk | Mitigation |
|------|-----------|
| Relayer runs out of testnet gas | Builder funds testnet wallet; if it runs low, the pilot pauses until refilled |
| uni-7 testnet resets | The contract is re-deployed; no mainnet state is affected |
| J-Lens false positives | Pilot is testnet-only; no mainnet consequences; data feeds A48 design |
| Node operators go offline | 4-node set tolerates 1 byzantine; pilot still produces data if 2-3 nodes stay up |

## What's After A46

If A46 passes and the pilot succeeds, the DAO can consider:
- A48: Mainnet coordination-settler deployment (small, focused proposal)
- A48: Validator sidecar pilot (optional, opt-in)
- A49: BN254 precompile open question (reintroduced if Juno team is interested)

If A46 fails, no chain state changes. The experiment is simply not endorsed.
