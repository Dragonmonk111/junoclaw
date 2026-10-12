# JunoClaw website narrative (canonical copy source)

*This is the single source of truth the landing page, `llms.txt`, the lightpaper and the
robotics page copy from. If wording changes, change it here first.*

## One line

JunoClaw is a **Layer 1 blockchain for verifiable agents and DAOs**, powered by Commonware consensus and CosmWasm execution. A DAO can command an agent, agents can form the DAO, or there is no DAO at all and a single agent's work is simply verifiable — and every action in between is proven.

## Hero

- **Eyebrow:** Layer 1 · Commonware + CosmWasm
- **H1:** An L1 for verifiable agents and DAOs.
- **Sub:** JunoClaw is a Layer 1 blockchain for verifiable agents and DAOs. Agents act. DAOs govern.
  Every action is sealed by a post-quantum proof that binds the result — so anyone can check the work
  without trusting the worker.
- **CTAs:** `Read the lightpaper →` (→ `/docs/lightpaper/`) · `See what's live` (→ `#status`)

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

## Agents and DAOs

- **A DAO commands an agent.** A DAO defines an agent, hires it, verifies its work, pays or slashes
  it. `WavsPush` carries the task, execution tier and escrow; `WeightChange` sets the roster.
- **Agents form the DAO.** A body whose members are agents, whose decisions are produced by
  verifiable computation rather than show-of-hands. `truth-market`, `moultbook`, sortition for
  arbitration.
- **A single agent, verified cryptographically.** No DAO anywhere. One agent and one payer. `task-ledger`
  accepts a task with no `proposal_id`, `escrow` records the obligation against a plain payer, and
  `zk-verifier` gates the release. This is the base case the other two are built on.

Members in `agent-company` are typed `Agent | Human | SubDAO`, weighted in basis points, summing
10,000. 51% quorum ordinary, 67% constitutional. The first two cases run through `agent-company`;
the third needs none of it.

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
| Buzz agent coordination | Agent economy end to end (122/122 checks) | Faucet, status page and wallet gateway |
| On-chain PQ + ZK verification (MAYO-1/2/3/5, Groth16) | Truth markets + J-Lens | Chain-native gossip on Commonware p2p |
| TEE attestation quote on-chain | Sealed signer | Chain-layer controllability lock |
| Tier-1.5 constraints on `uni-7` | Robotics attestation (sim-to-real) | Read-only fleet policy RPC |
| Fleet kill-switches, compile-time hardening | DAO governance | |

**Devnet digest, Oct 5–6:** 0.16 s blocks (4 validators, one host — G1 measures real networks,
1–2 s before the Oct 6 fix), 122/122 agent end-to-end checks, two real exploits found on our own
deployed contracts and fixed in place, 15 chaos events in 24 h, zero state divergences.

## Where it runs

JunoClaw is its own L1. The G1 testnet is running; mainnet is planned.

- **Today:** the G1 testnet is running on real networks. Unpaid, no token sale; validator keys are
  generated on each operator's own machine and never leave it.
- **Heritage:** JunoClaw stands on the shoulders of giants. The agent layer was proven first on Juno
  testnet (`uni-7`); `skill-registry`, `zk-verifier`, `jclaw-credential` and `moultbook` still run
  there and on mainnet (`juno-1`). The BN254 precompile came out of that work — ≈ 203k gas per
  `VerifyProof`, a measured 1.823× cut, which makes verifying *every* task cheap instead of sampling.
  The chain descends from Lay3r Labs' `layer-sdk` and its `slay3rd` node, on a `commonware` branch of
  it; consensus primitives from Commonware, execution from CosmWasm, and WAVS (Web Assembly
  Verifiable Services) as the runtime the agent layer uses to run work that can be checked.

## The substrate (landing page section, id `powered`)

- **Kicker:** The substrate
- **H2:** JunoClaw is standing on the shoulders of giants.
- **Lede:** Not a Cosmos SDK fork and not a Tendermint derivative. The agent layer was proven on Juno,
  the chain is built on Lay3r Labs' `layer-sdk` and its `slay3rd` node, consensus runs on Commonware's
  primitives, and execution is CosmWasm.
- **Four cards:**
  1. **Juno** — the agent layer was proven first on Juno testnet `uni-7`; `skill-registry`,
     `zk-verifier`, `jclaw-credential` and `moultbook` still run there and on mainnet `juno-1`. The
     BN254 precompile came out of that work: ~203k gas per `VerifyProof`, a measured 1.823× cut.
  2. **Lay3r Labs** — the chain descends from Lay3r Labs' `layer-sdk` codebase and its `slay3rd` node.
     **WAVS** (Web Assembly Verifiable Services) is the runtime the agent layer uses to run work that
     can be checked rather than assumed.
  3. **Commonware** — consensus, broadcast and storage primitives beneath a Simplex-family BFT
     protocol. Equal-weight validators, deterministic finality, no staking at the consensus layer. The
     chain repo is `layer-sdk` on a `commonware` branch.
  4. **CosmWasm** — Rust contracts on a deterministic KV store, with Cosmos-style gRPC (`BroadcastTx`,
     `GetTx`, `Simulate`) plus bank and wasm queries. Escrow, the task ledger, `zk-verifier` and the
     on-chain light client are all contracts.

## Chain evidence (G1, measured)

- **Sub-2 s BLS-threshold finality** — roughly 2 s per block in steady state on G1, with
  leader-timeout spikes quantified in the soak. The 0.16 s figure is the 4-validator single-host
  devnet, not G1.
- **MAYO2 PQ-hybrid certificates on every block** — a 616-byte hybrid envelope at a 3-of-4 signer
  quorum.
- **An on-chain light client** — an 08-wasm light-client contract deployed on `junoclaw-g1`, with
  positive and negative verification paths both passing at height 508,995.
- **Airdrop claim and governance** — both flows tested end-to-end on-chain.

This is the section that makes "verifiable" mean something: no other agent stack can point at a
light client verifying post-quantum certificates on-chain.

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
