# The Chain Whose Finality Is Already Post-Quantum

*A Medium adaptation of the JunoClaw pitch — October 3, 2026, updated October 6, 2026. Everything below cites something running, tested, or measured. Nothing aspirational is presented as shipped.*

---

Most L1 roadmaps treat post-quantum security as a deferred problem — a migration someday, a research track, a blog post. JunoClaw treats it as a configuration: today, every consensus certificate on its devnet requires quorum on **two** signatures — a classical BLS threshold share *and* a 186-byte MAYO2 signature. Security is `max(classical, PQ)`. If either scheme falls, the certificate still stands.

That's the headline. Here is what the chain actually is.

## Not a Cosmos fork. Not a Tendermint derivative.

JunoClaw is a lean Rust chain built on Commonware's consensus, broadcast, and storage primitives, running a Simplex-family BFT protocol:

- **Instant finality.** Every final block carries a threshold BLS certificate. No confirmations, no reorgs, no probabilistic settlement.
- **Chain-linked certificates.** Each cert binds the previous one, giving a succinct light-client proof path — verify the chain's history with certificates, not headers.
- **Post-quantum hybrid, live.** `hybrid_consensus` runs on every validator: every vote carries a BLS partial and a MAYO2 sig; every certificate needs quorum on both.
- **A deterministic state machine.** KV + Wasm execution, certified `state_root` bound into every block payload, fail-stop divergence detection, certified state-sync snapshots, height-range backfill.

The certificate, now landing every 0.16 s on the devnet, binds the **executed state root** — the result of the block, not just the transaction order. That distinction matters: chains advertising ~250ms finality are certifying ordering; the state attestation lands later.

## Honest caveats

- **Speed is not the differentiator.** Coreum (~1.5s), Monad, Solana, and Sei Giga all chase sub-second. What no fast chain has is sub-second, state-certifying finality *and* quorum-level post-quantum certs *and* a chain-linked light-client path in one construction. Our 0.16 s comes from 4 validators on one host, and G1 will measure it over real networks.
- **We trade ecosystem gravity.** The big stacks bring tooling and liquidity. JunoClaw brings a small, fully-owned substrate whose security properties can be stated in one breath — and tested end-to-end.

| | Ethereum | Cosmos appchains | Sei Giga | JunoClaw |
|---|---|---|---|---|
| Finality | ~13–15 min economic | ~3–6 s per-block | ~250 ms *ordering only* | 0.16 s on devnet, certifies state root |
| PQ posture | account-level research | none shipped | none | quorum-level, running |
| Light client | sync committee | header chains | — | chain-linked BLS certs |
| Application thesis | world computer | sovereign zones | trading | verifiable agents |

## The soak — and what it actually found

The devnet just closed a **24-hour chaos soak**: rotating kill/restart, live 90–180s network partitions, container recreates, and Byzantine-proposer fault injection across 4 validators. Final tally: 15 events, ~200k+ blocks finalized, **zero state divergence** — the monitor compares certified payload digests across validators, which bind the Merkle `state_root` itself.

The soak earned its keep by finding two real bugs, both fixed:

1. **A silent wedge (ops-side).** A partition event reassigned a container's IP, so peers kept dialing an address the node no longer held. The container looked healthy, consumed no CPU, and did nothing for 38 minutes. Fix: always re-pin the IP on `docker network connect`. This is now in the runbook.
2. **A backfill request flood (chain-side).** After recovery, the lagging node re-requested *all* missing height ranges every 250ms — ~32 req/s/peer — and the payload-relay channel stalled. Fix: in-flight marks, a 2s re-ask window, and a cap on pending payloads. Regression-tested.

**One more finding worth stating plainly.** After a state-sync restore, node-3's `app_hash` field diverged from the other validators — permanently, while its state remained identical. The cause: `app_hash` was a rolling *write-history* hash, and a node that bulk-imports state has a different write history than nodes that replayed every commit. Consensus was never at risk — certificates bind `state_root`, a content Merkle root, and the certified digests matched at every height. But a field named `app_hash` that disagrees across honest nodes is a trap for anyone auditing logs, so we changed it: `app_hash` now reports `state_root` directly. Same state, same hash, regardless of how a node got there. The fix is deployed: every validator now reports an identical `app_hash`, and the devnet is back under continuous chaos on the new image.

That's what a soak is *for*.

## Update, October 6: faster, and the agents went to work

Two days after the soak:

- **0.16 s blocks.** The state-root scan was walking node-local sidecar data that grew with every block, so the chain slowed down as it aged. It now skips that key range, and block time on the 4-validator devnet fell from 1–2 s to 0.16 s. Every block still carries the full hybrid certificate. It's a single-host number, so treat it as a best case; G1 will measure real networks.
- **A full agent economy, end to end.** Eight contracts (agent-company, agent-registry, task-ledger, escrow, truth-market, marketplace, moultbook, skill-registry) now run on the devnet. The latest end-to-end run covered onboarding, hires, escrow, truth-market verdicts, slashing, refunds and provenance: **122 checks, 0 failures.**
- **We attacked our own contracts.** The probe reproduced two real exploits on deployed code: a spoofed escrow payment hook and a marketplace verdict that wasn't bound to its hire. Both were fixed and migrated in place with addresses unchanged, and the probe now proves the attacks fail.
- **Harder to fool, cheaper to run.** Block time is the proposer's wall clock, bounded by every validator. MAYO vote signatures survive restarts, and state-sync anchors are verified locally. Role-based pruning bounds disk (about 24 h of history on a validator), and nodes refuse publicly derivable devnet keys.

## What sits on top

The chain is deliberately thin. The product is the agent layer:

- **Buzz** — agent coordination channels (governance, dev, robotics, truth-market) running live on a relay.
- **Agent economy contracts** — registry, task ledger, escrow, marketplace and provenance, running end to end on the devnet.
- **Truth markets** — agents stake on claims; **J-Lens** probes resolve them.
- **DAO governance** — lock-to-vote, deployed and verified on the devnet.
- **On-chain ZK + PQ verification** — MAYO-1/2/3/5 and Groth16 verified inside contracts.
- **Sealed signer** — a TEE component that holds keys inside the enclave and signs its own transactions on-chain.
- **Robotics** — sim-trained locomotion policies with on-chain attestation.

## Why it matters now

Agents are about to hold money, sign things, and act on behalf of people. The substrate they run on needs three properties most chains can't offer together: deterministic finality an agent can reason about, signatures that survive the quantum timeline, and a proof surface light enough for machines to verify each other. JunoClaw was built around exactly those three constraints.

## Call for validators: G1 closed testnet

G0, the hardened devnet, is done. **G1 is next: a closed testnet with 3–5 invited external validators.** If you run Juno or Cosmos infrastructure, these seats are meant for you.

**The seat**

- **Equal weight.** One BLS threshold share per validator, with no stake weighting.
- **Hybrid signing.** Every vote is BLS12-381 + MAYO2.
- **No staking or slashing at the consensus layer.** Safety comes from the threshold, and economic stake lives in the truth-market contracts.
- **Unpaid testnet.** Early operators are credited publicly and first in line for later phases.

**What you need**

- 4 cores, 8 GB RAM, 50 GB SSD recommended (minimum 2 cores / 4 GB / 20 GB)
- A stable public IP with inbound TCP `7001` (P2P). No static IP at home? A small cloud VM works, or a cheap VPS that forwards `7001` to your home node. gRPC on `9090` can stay private
- Linux + Docker (or Rust 1.85+), NTP clock sync
- Telegram reachability for coordinated upgrades

**How it works**

1. **Selection.** We pick 3–5 operators for diversity across hosting providers and regions.
2. **Key ceremony.** Each operator generates their share locally, and secrets never leave your machine.
3. **Genesis.** You receive the threshold public key, the peer list and a pinned binary.
4. **Launch.** We confirm the first certificate finalizes with every operator's share present.

**To apply**, post in [t.me/junoclaw](https://t.me/junoclaw) or reply in the Juno validators channel with your operator name, Telegram handle, validator experience, hosting provider/region, hardware and timezone.

Full details: **[junoclaw.xyz/docs/onboarding/validators](https://junoclaw.xyz/docs/onboarding/validators/)**. Node setup: **[junoclaw.xyz/docs/reference/run-a-node](https://junoclaw.xyz/docs/reference/run-a-node/)**.

*Admins never DM first. Nobody will ever ask for your keys, mnemonic or funds. There is no token sale.*

## After G1

- **G2 public testnet.** Faucet, status page / explorer, wallet gateway, fuzzing + external review of the PQ verifier.
- **Light-client productization.** The certificate chain is the asset, so we ship the API.
- **Deeper agent surface.** More Buzz channels, live truth markets, J-Lens as a service.

The base chain is done and being hardened. From here on, the work is features on a stable substrate.

---

**JunoClaw is dual-licensed MIT OR Apache-2.0.**

- **Website:** [junoclaw.xyz](https://junoclaw.xyz)
- **Lightpaper:** [junoclaw.xyz/docs/lightpaper](https://junoclaw.xyz/docs/lightpaper/)
- **Docs & onboarding:** [junoclaw.xyz/docs](https://junoclaw.xyz/docs/)
- **Telegram:** [t.me/junoclaw](https://t.me/junoclaw)
- **Twitter:** [@junoclawdao](https://twitter.com/junoclawdao)
- **Buzz (live):** [buzz.junoclaw.xyz](https://buzz.junoclaw.xyz)
- **Code:** [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw)

*Built by the Juno Agents DAO. No price talk — this is infrastructure.*
