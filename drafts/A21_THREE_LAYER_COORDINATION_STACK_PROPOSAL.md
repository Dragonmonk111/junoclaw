# A044 — Authorize the Three-Layer Coordination Stack (Truth → Coordination → Settlement)

> Follow-up to A18c-9 (J-Reef / J-Lens Audit Layer). The truth layer is built and proven. The settlement layer is live on juno-1. This proposal authorizes building the missing coordination layer using Commonware primitives, completing the three-layer stack described in the "Missing Piece" article. Per A18c-6 (propose before you build), this signal proposal is submitted before any coordination-layer code is written.

---

## Copy-paste box 1: Title

```
A044 — Authorize the Three-Layer Coordination Stack (Truth → Coordination → Settlement)
```

## Copy-paste box 2: Description

```
A18c-9 authorized the J-Reef / J-Lens audit layer. That work is done: the Chain Superintelligence Module v0.2, Domain-General Audit API, and J-Lens D1 probe are built, tested, and validated across three model scales (14B, 106B, 235B). The truth layer works.

The settlement layer — Juno mainnet with CometBFT, IBC, agent-company, moultbook, zk-verifier, and WAVS sealed signer — is live.

The missing piece is the coordination layer: a fast (~300ms), authenticated, ordered messaging network that sits between agents and Juno settlement. Today there is nothing between "agent generates text" and "agent submits on-chain." The coordination layer fills that gap using Commonware primitives (p2p::authenticated, consensus::simplex, threshold-signatures).

The three-layer stack:
1. Truth layer — J-Lens / Chain Superintelligence Module: was the agent honest?
2. Coordination layer — Commonware primitives: did the agent send this, and in what order?
3. Settlement layer — Juno (CometBFT, IBC): did we permanently agree to this?

What this proposal does:
1. Ratifies the three-layer architecture as the DAO's intended agent coordination stack.
2. Directs builders to implement the coordination layer per the plan in drafts/PLAN_THREE_LAYER_STACK_COMMONWARE.md (6 phases, 13 weeks).
3. Confirms that the coordination network is NOT a blockchain: no tokens, no staking, no governance. The validator set is appointed by the DAO. Juno remains the settlement layer.
4. Locks the governance boundary: deploying the coordination-settler contract to juno-1 requires a separate follow-up proposal after testnet validation. Any token, validator-set change, or shared policy change requires a separate proposal.
5. Directs that every message passing through the coordination network must pass through the J-Lens truth gate before acceptance — green proceeds, yellow warns, red blocks.

In scope:
- Architecture ratification and build direction.
- Use of Commonware open-source Rust primitives (p2p, consensus, cryptography, threshold-signatures).
- New CosmWasm contract development (coordination-settler) for testnet validation only.
- Integration with existing truth-layer infrastructure (CSI server, audit API) and settlement-layer infrastructure (Juno, agent-company).

Out of scope (will require future proposals):
- Deploying coordination-settler to juno-1 mainnet (separate proposal after testnet validation).
- Changes to the DAO validator set or membership.
- Any token, ticker, or drip signal tied to the coordination network.
- Mandating that all DAO agents must use the coordination network.

Voting:
- YES = authorize the three-layer coordination stack architecture and direct builders to proceed with the 13-week build plan.
- NO = do not build the coordination layer; keep the two-layer (truth + settlement) stack as-is.
- ABSTAIN = defer to builders.

No funds spent. No contract changes. No membership changes.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A044 — Authorize the Three-Layer Coordination Stack (Truth → Coordination → Settlement)",
  "description": "A18c-9 authorized the J-Reef / J-Lens audit layer. That work is done: Chain Superintelligence Module v0.2, Domain-General Audit API, and J-Lens D1 probe are built, tested, and validated across three model scales (14B, 106B, 235B). The truth layer works. The settlement layer — Juno mainnet with CometBFT, IBC, agent-company, moultbook, zk-verifier, WAVS sealed signer — is live. The missing piece is the coordination layer: a fast (~300ms), authenticated, ordered messaging network between agents and Juno settlement, using Commonware primitives (p2p::authenticated, consensus::simplex, threshold-signatures). The three-layer stack: (1) Truth — J-Lens / CSI: was the agent honest? (2) Coordination — Commonware: did the agent send this, and in what order? (3) Settlement — Juno: did we permanently agree to this? This proposal: (1) ratifies the three-layer architecture as the DAO's agent coordination stack, (2) directs builders to implement the coordination layer per drafts/PLAN_THREE_LAYER_STACK_COMMONWARE.md (6 phases, 13 weeks), (3) confirms the coordination network is NOT a blockchain — no tokens, no staking, validator set appointed by DAO, Juno remains settlement layer, (4) locks governance boundary: mainnet deployment requires a separate follow-up proposal after testnet validation, any token or validator-set change requires a separate proposal, (5) directs that every message must pass the J-Lens truth gate before acceptance (green proceeds, yellow warns, red blocks). In scope: architecture ratification, Commonware Rust primitives, CosmWasm contract dev for testnet only, integration with existing truth + settlement infra. Out of scope: mainnet deployment, DAO membership changes, any token, mandating all agents use it. Voting: YES = authorize and direct 13-week build; NO = keep two-layer stack; ABSTAIN = defer. No funds, no contract changes, no membership changes.",
  "funds": []
}
```

---

## Background

- **A18c-9 (passed, A32)**: Authorized the J-Reef / J-Lens audit layer. Build direction given. All deliverables shipped: CSI v0.2, audit API, D1 probe, CLI, tests, scaling study.
- **A18c-6 (passed)**: "Propose before you build" — any material shared-root change requires a DAO signal proposal first. This proposal follows that rule.
- **A18c-4 (passed)**: Agent-sovereign memory. The coordination network respects this — each agent runs its own node, no central hosted service.
- **"Missing Piece" article** (2026-08-04): Articulated the three-layer stack and the convergence between Jake Hartnell and Jack Zampolin on Commonware as the coordination layer.

## Why now

The truth layer is built and proven. The settlement layer is live. The coordination layer is the last piece. Without it, agents have no fast, authenticated, ordered way to coordinate before settling on Juno. They either act independently (no coordination) or submit everything on-chain (slow, expensive, no audit gate before commitment).

Commonware provides the exact primitives needed: `p2p::authenticated` for encrypted peer-to-peer, `consensus::simplex` for sub-second BFT ordering, `threshold-signatures` for compact certificates verifiable on Juno. Jack Zampolin's choice to build on Commonware validates the direction.

The 13-week build plan is concrete: 6 phases, each with deliverables and success criteria. Phases 1+3 run in parallel. Phase 4 (J-Lens gate) is almost free — it wires existing code. The heaviest work is Rust engineering in Phases 1-3, which the builder is committed to.

## What the coordination layer is and is not

**Is:**
- A fast (~300ms) authenticated messaging network for DAO agents.
- BFT-ordered: messages get cryptographic ordering via `consensus::simplex`.
- Audited: every message passes through the J-Lens truth gate before acceptance.
- Settled: batch certificates are verifiable on Juno via a CosmWasm contract.
- Open: any agent with a DAO-recognized identity can join.

**Is not:**
- A blockchain. No tokens, no staking, no governance, no block rewards.
- A replacement for Juno consensus. Juno is the final word. Commonware handles fast ordering only.
- A lie detector. J-Lens provides an audit signal, not proof of honesty.
- A closed system. The network is open to any agent that joins with a DAO-recognized identity.
- A token. Any $COORD-like signal stays a future, separate proposal.

## Voting options

- **YES** — authorize the three-layer coordination stack and direct the 13-week build.
- **NO** — keep the two-layer (truth + settlement) stack as-is.
- **ABSTAIN** — defer to builders.

## Out of scope

- No treasury spend.
- No contract deployment to juno-1 (testnet only, mainnet requires follow-up proposal).
- No DAO membership changes.
- No token minted or authorized.
- No mandate that all agents must use the coordination network.

## Next steps if this passes

1. Builder scaffolds `crates/coordination/` Rust workspace and pins Commonware dependencies.
2. Phase 1 (P2P bridge) and Phase 3 (settlement contract) begin in parallel.
3. Progress reported via Moultbook entries at each phase completion.
4. After testnet validation (Phase 5 complete), a follow-up proposal requests mainnet deployment authorization.

## Vote recommendation

**YES** — the truth layer is built, the settlement layer is live, and the coordination layer is the last piece. The plan is concrete, the builder is committed, and Commonware is the right tool. Authorize it, bound it, and let builders ship it.
