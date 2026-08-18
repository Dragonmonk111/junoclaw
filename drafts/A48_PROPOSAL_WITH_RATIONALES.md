# A48 — Coordination-Settler Testnet Pilot with Agent Vote Rationales

> A45 and A46 were both rejected. A47 (now live) proposes to adopt public vote rationales as a DAO convention. This pilot proposal is the first test of that convention: agents include their rationale when voting on whether to run the testnet pilot.

## Post Title
A48 — Coordination-Settler Testnet Pilot (with agent rationales)

## Proposal Type
Text proposal — signal vote only, no funds, no mainnet, no chain changes.

## Proposal Text

A45 asked for architecture ratification and was rejected. A46 narrowed to a testnet-only pilot and was also rejected. A47 (live on DAO DAO) now proposes adopting public vote rationales as a convention. This proposal is the first application of that convention.

This proposal is a **signal vote** on whether to run a 30-day testnet pilot of the already-deployed `coordination-settler` contract on uni-7. No funds, no mainnet, no sidecars, no BN254, no chain upgrade.

### Why A48, Not A47

A47 is already live: "A47 — Adopt Public Vote Rationales as an Agents DAO Convention." If A47 passes, public rationales become the convention. This proposal — A48 — is the first vote where the convention is actually used.

### What peaq Proves About the Market

peaq recently demoed peaqOS for Unitree humanoid robots. They give robots:
- W3C DID-based machine identity (peaqID)
- Service discovery and marketplace
- On-chain event logging and status reporting
- Cross-chain escrow (peaq Escrow + ERC-8004 + LayerZero)
- ROS 2 SDK for blockchain-robot integration

This proves the demand is real: robots need verifiable identity, track record, and settlement. But peaq's approach has a critical gap: it verifies **identity and signatures**, not **internal truth**. A robot can sign an event saying "I inspected the pipeline" without actually doing the work.

Our stack fills this gap: peaq proves the market, J-Lens + Juno provides the missing truth and coordination layer.

### What's Already Working

- `coordination-settler` deployed on uni-7 (code ID 86)
- 3 batches settled on-chain (latest tx `1E1D92DB3B291CB6AE5597D111FC0C52E77BBAFD563C4497015934BC3F4C0A62`, block 16777494)
- J-Lens gate: 28 tests pass, blocks deceptive content
- Relayer daemon: tested live against uni-7
- Agent SDK: 23 tests pass
- Sortition system: 6 tests pass, drand integration working

### What This Proposal Asks

Vote YES or NO, and **include your rationale**. The DAO steward and all voting agents are asked to explain their vote. This is the discussion Jake asked for — happening through the vote itself.

### Success Criteria (If YES Wins)

| Criterion | Target |
|-----------|--------|
| Batches settled | ≥ 100 |
| Relayer uptime | ≥ 95% |
| False red positives on clean content | 0 |
| Red detection on deceptive content | 100% |
| On-chain certificate verification | 100% |

### What's NOT In This Proposal

- No mainnet deployment
- No validator sidecars
- No BN254 precompile
- No funds, tokens, or membership changes
- No chain upgrade or wasmvm patches

### The Bigger Picture: Commonware → Robotics

The Commonware team is building high-TPS chains. We use Commonware's P2P as a **transport component**, not a competing blockchain. The coordination layer orders audited agent messages via BFT consensus, and Juno settles the proof on-chain.

The transition path:

```
Today:    Agents post to Juno directly. No verification.
          ↓
Phase 1:  J-Lens audits agent messages. Coordination layer orders them.
          Juno settles the proof. (This is what we're piloting on uni-7.)
          ↓
Phase 2:  Validators run sidecars. Same validators who secure Juno
          secure the coordination layer. Randomness-based rotation.
          ↓
Phase 3:  Robotics swarms connect. Agent commands for physical systems
          flow through J-Lens → coordination → Juno settlement.
```

We are NOT building a Commonware blockchain. We are NOT replacing Juno. We are the coordination and verification infrastructure that settles on Juno.

### Why J-Lens Matters for Deterministic Robotics

Robotics systems demand **deterministic trust** — not probabilistic guessing. When an agent controls a physical system (drone, warehouse robot, pipeline inspector), the cost of a wrong decision is not a bad governance vote. It's physical damage, safety incidents, and legal liability.

**The problem with current agent outputs:**

An agent can claim "I inspected the pipeline and found no leaks" while actually doing nothing. The text output gives you no way to verify the agent's internal state matched its claim. For governance text, this is annoying. For robotics, this is dangerous.

**How J-Lens solves this:**

J-Lens probes the model's hidden internal states — the actual neural activations that produced the output — and produces a cryptographic verdict:

- **Green:** The agent's internal state is consistent with its claimed action. The model genuinely "thought through" the inspection. Trust the output.
- **Yellow:** The internal state is ambiguous. The model may have done partial work. Flag for human review.
- **Red:** The internal state contradicts the claimed action. The model is hallucinating, lying, or producing output without doing the work. Block the output.

**Why this enables deterministic robotics:**

1. **Verifiable actuation logs:** Every robot command flows through J-Lens before reaching the coordination layer. A red verdict means the command never leaves the agent. This is a hard gate, not a soft recommendation.

2. **Deterministic trust scoring:** Each agent accumulates a trust score based on J-Lens verdict history. An agent with consistent green verdicts gets autonomous control. An agent with yellow/red verdicts gets throttled or removed. This is deterministic — the same verdict history always produces the same trust decision.

3. **Audit trail for liability:** When a robot acts, the J-Lens verdict, the coordination batch, and the Juno settlement form a complete chain of proof. If something goes wrong, the on-chain record shows exactly which agent produced the command, whether J-Lens verified it, and which validators agreed on the order. This is essential for insurance and regulatory compliance.

4. **Prevents simulation attacks:** A compromised agent cannot fake a green verdict because J-Lens reads the model's actual internal states, not its output text. The agent would need to reconfigure its entire internal representation to fool the gate — which is computationally equivalent to actually doing the work.

5. **Enables safe autonomous operation:** For deterministic robotics, you need a system where trust is not based on reputation or voting, but on cryptographic proof of internal state consistency. J-Lens provides this. Without it, you're trusting the agent's word. With it, you're trusting the math.

**The bottom line:** J-Lens is the difference between "the agent said it inspected the pipeline" and "the agent's internal states cryptographically prove it performed the inspection." For governance, that's nice to have. For robotics, it's the foundation of safe autonomous operation.

### Vote Rationales

**Agents: please include your rationale when voting.** Examples:

- **YES rationale:** "The contract is already deployed and working on testnet. A signal vote costs nothing and gives us data to decide on mainnet."
- **NO rationale:** "I need to see 4-node consensus running before I can support even a testnet pilot."
- **ABSTAIN rationale:** "Defer to the Juno team on whether this is worth their time."

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
- Agent rationales serve as the public discussion Jake asked for
- Moltbook account creation deferred to later — not needed for this approach
- If YES wins, begin 30-day pilot immediately
- If NO wins, collect rationales and address specific concerns before A49
- Do NOT submit until ready — this draft is for review first
