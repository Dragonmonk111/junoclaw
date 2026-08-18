# A46 — Talking Points for Spaces / Twitter / Discord

## One-liner

"A46 is a 30-day, testnet-only pilot. No mainnet, no sidecars, no BN254, no funds. If it works, we go to mainnet. If it doesn't, nothing happens."

## Main talking points

### 1. We learned from A45
- A45 asked for architecture ratification and was rejected.
- A46 is a much smaller ask: just run the existing testnet contract for 30 days.
- This is not a second attempt — it's a deliberate narrowing.

### 2. The code already works
- coordination-settler deployed on uni-7
- 2 batches already settled on-chain
- J-Lens gate blocks deceptive content, passes clean content
- Relayer daemon tested against live chain
- Commonware P2P now compiles with NASM

### 3. The ask is zero-risk
- Testnet only. uni-7. No mainnet.
- No funds. No tokens. No membership changes.
- No validator sidecars. No BN254.
- No chain upgrade. No wasmvm patches.
- Hard success criteria. If unmet, no follow-up.

### 4. Why this matters
- AI agents are coming to governance.
- We need to verify their outputs (J-Lens) and order their messages (coordination).
- Juno should be the settlement layer for this.
- The pilot proves the whole stack before any bigger commitment.

### 5. Answer to "why not just use Commonware's chain?"
- Commonware is building high-TPS chains. That's not our product.
- We're building a coordination layer for agent messages that settles on Juno.
- Commonware's P2P is a component, not a replacement for Juno.

### 6. The 30-day deliverable
- 100+ batches on-chain
- 95% relayer uptime
- 0 red false positives on clean content
- 100% red blocking on deceptive content
- Public report at the end

## Counter-arguments and responses

**"This is still too early."**
- It's only testnet. The contract is already there. We're not asking for mainnet or funds. If it doesn't work, we stop.

**"I don't want to run a sidecar."**
- You don't have to. No sidecars in this proposal. This is a DAO-appointed 4-node testnet.

**"What about the BN254 precompile?"**
- Not in this proposal. We can discuss it later if the Juno team is interested.

**"Why should the DAO spend time on this?"**
- It doesn't cost anything. The builder pays testnet gas. The DAO only votes. If it passes, we get real data.

**"A45 just failed. Why should A46 pass?"**
- A46 is a different proposal. Smaller. No ratification. No sidecars. No BN254. Just a 30-day testnet run.

## Call to action

Vote **YES** on A46 if you want to see 30 days of real data from the coordination stack on uni-7.
