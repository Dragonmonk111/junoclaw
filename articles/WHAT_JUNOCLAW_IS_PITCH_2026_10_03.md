# What JunoClaw Is

*October 3, 2026 — pitch version. Every claim below cites something running, tested, or measured in the repo.*

## The one-line version

JunoClaw is a purpose-built L1 whose consensus certificates are already post-quantum — and whose application layer is built for verifiable agents, not DeFi.

## What the chain actually is

Not a Cosmos SDK fork. Not a Tendermint derivative. A lean Rust chain on Commonware's consensus, broadcast, and storage primitives, running a Simplex-family BFT protocol:

- **Instant finality** — every block final carries a threshold BLS certificate. No confirmations, no reorgs, no probabilistic settlement.
- **Chain-linked certificates** — each cert binds the previous one, which yields a succinct light-client proof path: verify the chain's history with certificates, not headers.
- **Post-quantum hybrid, live today** — validators run `hybrid_consensus`: every vote carries a BLS partial *and* a 186-byte MAYO2 signature; every certificate needs quorum on both halves. `max(classical, PQ)` security — not a migration roadmap, a running configuration.
- **Deterministic state machine** — KV + Wasm execution, certified `state_root` bound into every block payload, fail-stop divergence detection, certified state-sync snapshots, height-range backfill, durable payload store, tx indexing, gRPC + Simulate.

## Why it differs from the obvious comparisons

| | Ethereum | Cosmos appchains | Sei Giga | JunoClaw |
|---|---|---|---|---|
| Finality | ~13–15 min economic | ~3–6 s per-block | ~250 ms *ordering only* | ~1 s, certifies state root |
| PQ posture | account-level research | none shipped | none | quorum-level, running |
| Light client | sync committee | header chains | — | chain-linked BLS certs |
| Codebase | enormous | SDK + Tendermint | large EVM client | small, auditable |
| Application thesis | world computer | sovereign zones | trading | verifiable agents |

Two honest caveats:

- **Speed is not the differentiator.** Coreum (~1.5 s), Monad, Solana and Sei Giga all chase sub-second — and Sei's ~250 ms number freezes *transaction order* only; its executed-state attestation lands blocks later. JunoClaw's ~1 s certificate binds the **executed `state_root`** — the result, not just the sequence. What no fast chain has is ~1 s *and* quorum-level post-quantum certs *and* a chain-linked light-client path in the same construction.
- **We trade ecosystem gravity.** The big stacks bring tooling and liquidity; JunoClaw brings a small, fully-owned substrate whose security properties can be stated in one breath — and tested end-to-end.

## What sits on top

The chain is deliberately thin. The product is the agent layer:

- **Buzz** — agent coordination channels (governance, dev, robotics, truth-market) running on a relay.
- **Truth markets** — agents stake on claims; **J-Lens** probes resolve them.
- **On-chain ZK + PQ verification** — MAYO-1/2/3/5 and Groth16 verified inside contracts; robotics/agent attestation paths.
- **Sealed signer** — a TEE component that holds keys inside the enclave and signs its own transactions on-chain.
- **Robotics** — sim-trained locomotion policies with on-chain attestation (Ponyou walks).

## Why it matters now

Agents are about to hold money, sign things, and act on behalf of people. The substrate they run on needs three properties most chains can't offer together: deterministic finality an agent can reason about, signatures that survive the quantum timeline, and a proof surface light enough for machines to verify each other. JunoClaw was built around exactly those three constraints.

## Battle-tested, not slide-tested

The devnet has run a 24-hour chaos soak: kill/restart, live network partitions, container recreates, Byzantine-proposer fault injection — rotating victims, recovery measured per event. Two real findings (an IP-pinning wedge, a backfill request flood) were found *by* the soak and fixed in code — 68 tests green. The monitor compares certified payload digests across validators; zero divergence, ~175k blocks and counting.

## What's next

- **G1 public testnet** — external validators, key ceremony, faucet and status surface.
- **Light-client productization** — the certificate chain is the asset; ship the API.
- **Deeper agent surface** — more Buzz channels, live truth markets, J-Lens as a service.

The base chain is done and being hardened. Everything from here is features on a stable substrate — and the story above it.

*The discipline: nothing in this document is aspirational without being marked as such.*
