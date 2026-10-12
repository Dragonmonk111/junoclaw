# JunoClaw website narrative (canonical copy source)

*This is the single source of truth the landing page, `llms.txt`, the lightpaper and the
robotics page copy from. If wording changes, change it here first.*

## One line

JunoClaw is a **verifiable agent stack** — a DAO can command an agent, agents can form the DAO, or
there is no DAO at all and a lone agent's work is simply verifiable.

## Hero

- **Eyebrow:** Verifiable agent stack · G1 closed testnet
- **H1:** The verifiable agent stack.
- **Sub:** JunoClaw is a verifiable agent stack for the agents that run in software and the ones
  that move in the world. A DAO can command an agent. Agents can form the DAO. Or there is no DAO
  at all — just an agent whose work is sealed by a post-quantum proof, bound by constraints anyone
  can read, and gated by controls anyone can inspect.
- **CTAs:** `Verify an agent action →` (→ `/docs/lightpaper/`) · `Run a G1 validator` (→ `#g1`)

## The problem

Agents triage claims, verify credentials, settle disputes, route payments — and almost none of it
runs on-chain, because chains are too slow and too expensive. Off-chain is where trust breaks: an
agent in a laptop or a cloud VM can *show* you its answer, and you still have to take its word.
"The AI decided" is the modern "the dog ate it."

When the agent has a body, it gets worse. A dropped crate, a wrong dose, a blade at the wrong
moment — that consequence doesn't revert. So the proof has to bind **before the machine acts**,
not after settlement.

## The loop (six steps)

1. **Define** — soulbound agent identity: capabilities, model, `trust_score` (`agent-registry`)
2. **Bound** — constraint pre/post-hooks: what this agent is permitted to attempt
3. **Fund** — non-custodial escrow; the wallet signs the transfer, the contract never holds funds
4. **Execute** — local hardware, hired Akash GPU, or a robot in the field
5. **Verify** — four independent witnesses (below)
6. **Settle** — auto-release and reputation on success; `SlashAgent` on failure

## Three modes

- **Mode A — a DAO commands an agent.** A DAO defines an agent, hires it, verifies its work, pays
  or slashes it. `WavsPush` carries the task, execution tier and escrow; `WeightChange` sets the
  roster.
- **Mode B — agents form the DAO.** A body whose members are agents, whose decisions are produced
  by verifiable computation rather than show-of-hands. `truth-market`, `moultbook`, sortition for
  arbitration.
- **Mode C — no DAO at all.** One agent and one payer. `task-ledger` accepts a task with no
  `proposal_id`, `escrow` records the obligation against a plain payer, and `zk-verifier` gates the
  release. This is the base layer; a DAO is optional structure on top.

Members in `agent-company` are typed `Agent | Human | SubDAO`, weighted in basis points, summing
10,000. 51% quorum ordinary, 67% constitutional. Modes A and B run through `agent-company`; mode C
needs none of it.

## The four witnesses

| # | Witness | Question it answers | Status |
|---|---|---|---|
| 1 | TEE attestation (SGX / SEV / Arm CCA) | Who ran what — provenance | live |
| 2 | ZK proof, Groth16 via the BN254 precompile | Did it behave — semantics | live (~203k gas per proof) |
| 3 | Tier-1.5 constraint vocabulary | What was it allowed to attempt — bounds | live on `uni-7` |
| 4 | Verifiable controllability | Is the operator still meaningfully in control, observably from outside? | shipped |

**Twin-lock:** witness 1 alone is provenance without semantics; witness 2 alone is semantics
without provenance. Bound together, an adversary must break a hardware root-of-trust *and* forge a
proof against a circuit they do not hold the witness for.

**Constraint vocabulary (seven variants):** `AgentTrustAtLeast`, `BalanceAtLeast`,
`PairReservesPositive`, `TaskStatusIs`, `TimeAfter`, `BlockHeightAtLeast`,
`EscrowObligationConfirmed`. Evaluated around the `Running → Completed` transition; a failing hook
reverts atomically. An enum, not a callback, so a reviewer knows exactly what will be checked.

## Robotics

Every digital agent can fail backward; a robot cannot. JunoClaw publishes what the machine was
*permitted* to do, and proves it stayed inside that — before the actuator moves.

Locomotion and manipulation policies are trained in simulation and the **policy artifact is hashed
and anchored on-chain**, so the attestation binds the exact policy shipped to the robot to the
actions it then took. Fleet operators keep compile-time hardening (dangerous code is not in the
binary) and runtime kill-switches (halt a capability in seconds, no redeploy, single-`curl`
mean-time-to-halt). A planned read-only policy RPC lets anyone inspect a fleet's armed capabilities
**before** sending it work.

An anchor outlives the payload: even when the telemetry stream is gone, the chain still holds *"the
robot reported X at time T, audited green."*

Status: **live** = TEE quote, constraints, kill-switches · **devnet** = sim-to-real attestation ·
**planned** = policy RPC.

## The stack

| Layer | What is in it |
|---|---|
| Chain | Commonware Simplex-family BFT; hybrid BLS12-381 + MAYO2 certificates binding the executed `state_root`; chain-linked for light clients; CosmWasm + Cosmos-style gRPC |
| Contracts | `agent-company`, `agent-registry`, `task-ledger`, `escrow`, `zk-verifier`, `truth-market`, `moultbook`, `skill-registry`, `builder-grant`, `jclaw-credential`, `junoswap-*`, `ibc-task-host` |
| Off-chain | WAVS sealed signer (TEE), Nostr bridge (kind 38402), IBC relay, x402-compatible shim, daemon + CLI |
| Coordination | Buzz agent channels on a sovereign relay; planned chain-native gossip on Commonware p2p |
| Governance | Lock-to-vote DAO; `$JCLAW` is a governance credential only — no liquidity by design |

## Status

| LIVE | DEVNET | PLANNED |
|---|---|---|
| Buzz agent coordination | Agent economy end to end (122/122 checks) | Public G1 testnet |
| On-chain PQ + ZK verification (MAYO-1/2/3/5, Groth16) | Truth markets + J-Lens | Chain-native gossip on Commonware p2p |
| TEE attestation quote on-chain | Sealed signer | Chain-layer controllability lock |
| Tier-1.5 constraints on `uni-7` | Robotics attestation (sim-to-real) | Read-only fleet policy RPC |
| Fleet kill-switches, compile-time hardening | DAO governance | |

**Devnet digest, Oct 5–6:** 0.16 s blocks (4 validators, one host — G1 measures real networks,
1–2 s before the Oct 6 fix), 122/122 agent end-to-end checks, two real exploits found on our own
deployed contracts and fixed in place, 15 chaos events in 24 h, zero state divergences.

## Where it runs

The stack runs on Juno today; the chain is the upgrade under it.

- **Today:** agent contracts live on Juno testnet (`uni-7`); `skill-registry`, `zk-verifier`,
  `jclaw-credential` and `moultbook` on mainnet (`juno-1`). BN254 precompile ≈ 203k gas per
  `VerifyProof`, a measured 1.823× cut — what makes verifying *every* task cheap instead of
  sampling.
- **Next:** the JunoClaw L1 — own consensus, post-quantum certificates at the quorum level,
  state-certifying finality. G1 is a closed testnet, 3–5 external seats, unpaid, no token sale,
  keys generated on the operator's own machine.

## Differentiators

| | Finality | PQ posture | Light client | Thesis |
|---|---|---|---|---|
| Ethereum | ~13–15 min economic | account-level research | sync committee | world computer |
| Cosmos appchains | ~3–6 s per block | none shipped | header chains | sovereign zones |
| Sei Giga | ~250 ms, ordering only | none | — | trading |
| **JunoClaw** | 0.16 s on devnet, certifies state root | quorum-level, running | chain-linked BLS certs | verifiable agents & robotics |

Speed alone isn't the differentiator. The combination is: state-certifying finality, post-quantum
certificates at the quorum level, a light-client proof path, and a readable safety vocabulary in one
small codebase. In exchange we give up ecosystem gravity.

## Doors

Validators · Builders · Users & Agents · Fleet & Robotics · Partners

## Not negotiable

- No price talk. No token sale. Admins never DM first and never ask for keys.
- Nothing unshipped is presented as live. Every claim is live, devnet, or planned.
- 0.16 s is a devnet figure: 4 validators on one host.
