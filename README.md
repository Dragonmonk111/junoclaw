# JunoClaw

**A purpose-built L1 whose consensus certificates are already post-quantum — built for verifiable agents and robotics. Born from the Juno ecosystem.**

[junoclaw.xyz](https://junoclaw.xyz) · [Telegram](https://t.me/junoclaw) · [@junoclawdao](https://twitter.com/junoclawdao) · [Buzz](https://buzz.junoclaw.xyz) · [Juno Agents DAO](https://daodao.zone/dao/juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac)

> ~1 s state-certifying finality · BLS + MAYO2 hybrid consensus · ~175k blocks soaked, zero divergence · Apache-2.0

---

## The chain

Not a Cosmos SDK fork. Not a Tendermint derivative. A lean Rust chain on Commonware consensus, broadcast, and storage primitives, running a Simplex-family BFT protocol:

- **Instant, state-certifying finality** — every block final carries a threshold BLS certificate binding the executed `state_root`. No confirmations, no reorgs, no probabilistic settlement.
- **Post-quantum hybrid, live today** — `hybrid_consensus`: every vote carries a BLS partial *and* a 186-byte MAYO2 signature; every certificate needs quorum on both halves. `max(classical, PQ)` — a running configuration, not a migration roadmap.
- **Chain-linked certificates** — each cert binds the previous one: a succinct light-client proof path. Verify history with certificates, not headers.
- **Deterministic state machine** — KV + Wasm execution, fail-stop divergence detection, certified state-sync snapshots, height-range backfill, durable payload store, tx indexing, gRPC + Simulate.

## Honest comparison

| | Ethereum | Cosmos appchains | Sei Giga | JunoClaw |
|---|---|---|---|---|
| Finality | ~13–15 min economic | ~3–6 s per-block | ~250 ms *ordering only* | ~1 s, certifies state root |
| PQ posture | account-level research | none shipped | none | quorum-level, running |
| Light client | sync committee | header chains | — | chain-linked BLS certs |
| Application thesis | world computer | sovereign zones | trading | verifiable agents |

Speed isn't the differentiator — several chains chase sub-second, and Sei's ~250 ms freezes *ordering* only; its executed-state attestation lands later. Ours is the combination: ~1 s certificates that bind **executed state**, at quorum-level post-quantum security, with a chain-linked light-client path.

## What's running

- **24-hour chaos soak** — kill/restart, live partitions, container recreates, Byzantine-proposer fault injection; rotating victims, recovery measured per event. **~175k blocks, zero divergence.** Two real bugs found *by* the soak, fixed in code.
- **Buzz** — agent coordination channels (governance, dev, robotics, truth-market) on a sovereign relay, live at [buzz.junoclaw.xyz](https://buzz.junoclaw.xyz).
- **On-chain ZK + PQ verification** — MAYO-1/2/3/5 and Groth16 verified inside contracts.
- **Sealed signer** — a TEE component that holds keys in-enclave and signs its own transactions.
- **Robotics** — sim-trained locomotion policies with on-chain attestation (Ponyou walks).

## Roadmap

**Next: G1 public testnet** — external validators, key ceremony, faucet. If you run infrastructure, join [t.me/junoclaw](https://t.me/junoclaw).

## Born from Juno — the receipts

This project grew out of the Juno Agents DAO's agent-infrastructure work on Juno Network (Cosmos). Before the L1, the stack shipped on-chain there — still deployed, still queryable:

- **28 CosmWasm contracts** on Juno testnet (uni-7) + 4 on mainnet (codeIds 5145–5148)
- **Truth market**: 16 epochs finalized, 5 operators, 707,672 ujunox rewards, 290,000 slashed — all on-chain
- **DAO-mandated independent operator** (A052): 11 verdicts, 10 correct, real slashing on non-builder keys
- **Mainnet governance**: Props #373–375, #377 — all passed, including the BN254 precompile
- **Machine RWA**: `machine-0` NFT (Unitree Go2) minted; `GetWorkIntegrityScore` wired to Moultbook
- **5 ZK circuits** (Groth16/BN254), 187 ms prove time measured

The L1 exists because that work outgrew the substrate — the agent layer needed consensus-level guarantees no existing chain offered. Everything learned on Juno is given back to the ecosystem.

## Repo layout

- `crates/` — chain + supporting Rust crates (coordination, physics, miner, relayer, CLI, …)
- `devnet/` — local multi-validator devnet, chaos soak harness, fault injection
- `deploy/` — deployment configs (Akash, docker)
- `contracts/` — the Juno-era CosmWasm contract set (heritage, see above)
- `circuits/` — Groth16/BN254 ZK circuits
- `wavs/` — WAVS operator + sealed-signer component (TEE)
- `website/` — the [junoclaw.xyz](https://junoclaw.xyz) landing page (Cloudflare Workers static)
- `articles/` — project writeups and pitch articles
- `drafts/` — design docs, community bootstrap plan
- `docs/` — runbooks and reference material

## Security

5 published [security advisories](https://github.com/Dragonmonk111/junoclaw/security/advisories). See [SECURITY.md](./SECURITY.md).

## License

Apache-2.0
