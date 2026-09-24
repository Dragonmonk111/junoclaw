# Plan: Article Publish Timing, Airdrop Readiness, IBC Path

Date: 2026-09-19

## 1. Is the airdrop ready for everyone to claim?

**No — not yet.** The contract is proven end-to-end on devnet, but public claiming requires a live chain.

### Done
- `airdrop-claim` contract: full lifecycle proven on devnet (Sept 19) — instantiate, fund, Merkle-proof claim, bank payout
- Real Merkle tree: root `ad087a46…50627d`, 213,385 leaves, proofs for every eligible account (`junoclaw-chain/snapshot/merkle-proofs.json`)
- Snapshot math: 35,039,975 ujclaw, 1:1 staked JUNO at block 41,655,615, 250K cap, 17.9M excess → community pool
- `tx-sender` tooling: store-code, instantiate, execute, query, send — all working

### Blocking public claim
1. **Mainnet launch** — the sovereign chain is devnet-only. No public chain = nothing to claim on.
2. **Contract deployment on mainnet** — instantiate with real root `ad087a46…`, `total_amount=35039975077948`, real claim window (90 days), then fund with 35.04M ujclaw from the DAO wallet.
3. **Claim UX for 213k users** — recipients hold Juno keys; they need a way to submit claims. Options:
   - Web UI: connect Keplr/Leap → derive junoclaw address (same secp256k1 key, `juno` prefix already matches) → auto-fetch proof from a hosted `merkle-proofs.json` → sign + broadcast. This is the only realistic path for 213k users.
   - CLI fallback: `tx-sender claim --address <juno1...> --key <key>` for power users.
4. **Proof serving** — `merkle-proofs.json` is large (213k proofs). Host it (or a per-address API) so the UI can fetch `proof[address]` on demand.

### Not tradable — correct
No DEX pair, no IBC channel. ujclaw is claimable but non-transferable in practice until IBC opens. Article should say this plainly.

## 2. When to publish the article

The article is the announcement vehicle. Three viable windows:

| Option | Timing | Pros | Cons |
|--------|--------|------|------|
| A. Publish now | Before launch | Builds anticipation; "claims open at launch" | Can't say "claim now"; risk of hype without a date |
| B. Publish at launch | Day mainnet + airdrop live | "Yes, you can claim" is literally true; maximum impact | Nothing public until then |
| C. Two-stage | Teaser now → launch post | Best of both; Medium allows edits | Two posts to write |

**Recommended: C.** Publish the current article now as the story piece with an explicit "airdrop claims open at mainnet launch — here's how it will work" section. Post a short follow-up on launch day with the claim link. If launch slips, the story post still stands on its own.

**Minimum bar before publishing:** the article must not promise anything that isn't true. Today that means: airdrop = "ready at launch," IBC = "built, channel planned," tradable = "not yet."

## 3. IBC plan ("cover up" → honest framing + real path)

Article currently implies live IBC ("an agent on Osmosis can send proofs via IBC"). Reality: `cw-ics20-transfer` is built + unit-tested; no channel exists. Fix framing to "built, channel setup planned."

### Path to a live channel
1. **BLS light client (the core work item)** — junoclaw's Commonware BLS12-381 threshold consensus isn't Tendermint, so counterparties need a custom ICS-02 client. Deployable as a CosmWasm `08-wasm` light client on Osmosis/Juno — permissionless, no counterparty chain upgrade. Precedent: Union's cometbls (BLS light client in production). The client verifies junoclaw threshold certificates + Merkle state proofs.
2. **junoclaw side** — implement ICS-07 Tendermint client verification in Rust (tendermint-rs) inside the state machine, plus the IBC module surface (connections, channels, packets).
3. **Hermes relayer** — config exists at `deploy/hermes/`. Needs junoclaw chain registered (chain-id, gRPC/RPC endpoints, key) + osmo-test-5 counterpart.
4. **Channel handshake** — `setup-ibc-channel.mjs` exists. Client/connection/channel open between junoclaw devnet and osmo-testnet.
5. **Contract side** — deploy `cw-ics20-transfer` on junoclaw; Osmosis side uses native ICS-20.
6. **E2E test** — transfer ujclaw devnet→osmo-test, verify escrow + denom trace, test timeout/ack paths.
7. **Mainnet** — repeat against Osmosis mainnet at launch. This is what makes ujclaw tradable.

### Fallback only
If the BLS light client slips past launch: a threshold-signed attestation bridge (junoclaw validators sign transfer attestations, verified by a CosmWasm contract on the Osmosis side). Weaker trust model — use only as a stopgap.

### Blockers to flag
- `ibc-task-host` cross-chain ZK dispatch is designed but untested over a real channel.

## 4. Article trim for Medium

Current: 262 lines / ~2,400 words (~11 min read). Target: ~150-170 lines / ~1,600 words (~7 min).

Cuts:
- Component comparison table → 2 lines of prose
- 14-row contract table → grouped highlights (keep status honesty)
- PQC ZK Pathway section → 3 lines (it's a footnote for a general audience)
- Robotics pipeline bullets → fold into the L0-L5 section
- "Today" list → trim to 6 items
- Keep: Origin, the four systems, Six-Month Arc, Why This Matters, closing

## 5. Copy-paste airdrop message

See bottom of article file — a ready-to-post blurb with placeholders for claim URL and dates.

## 6. Validator onboarding DM

```
Hey — you're running validators on [CHAIN], so I wanted to reach out directly.

I'm launching JunoClaw — a sovereign chain written entirely in Rust
(no Cosmos SDK, no Tendermint). Consensus is Commonware simplex with
BLS12-381 threshold signatures: ~400ms finality, validators fixed at
genesis via a DKG key ceremony.

We're onboarding the genesis validator set now. What's in it for you:

- Fixed supply (54.66M ujclaw), zero inflation — validators earn
  transaction fees. Bitcoin-miner model, not staking yield.
- ~400ms blocks, modest hardware (Rust node + RocksDB).
- Real work already proven on devnet: Groth16 verified at 77k gas,
  a 68KB Jolt ZK proof verified on-chain, airdrop claim lifecycle
  end-to-end.
- Day-one users: launches with a 213,385-account airdrop to Juno
  stakers — not an empty chain.

Commitment: run one node, join the DKG ceremony before genesis
(~30 min), stay online. I'll send the setup doc and walk you
through it personally.

Interested? Reply here and I'll send the validator guide +
ceremony schedule.
```

Short version (character-limited DMs):

```
Launching JunoClaw — sovereign Rust chain, BLS consensus, ~400ms
finality, fixed supply (validators earn fees, no inflation).
Onboarding genesis validators now: one node + a 30-min DKG ceremony.
Already runs real ZK proofs on devnet; launches with a 213k-account
airdrop to Juno stakers. Interested?
```

## 7. Validator requirements (reply to Ffern Institute)

```
Validator requirements for JunoClaw:

SOFTWARE
Single Rust binary (slay3rd). No Cosmos SDK, no Tendermint —
Commonware simplex consensus with BLS12-381 threshold signatures.
Docker image or build from source.

KEYS
Two keys per validator:
- Ed25519 identity key (authenticated P2P, TLS)
- BLS12-381 key share — generated in the pre-genesis DKG ceremony
  (~30 min, run together on a call). No share, no slot.

NETWORK
Two ports: P2P 7001 (authenticated, static peer list — no public
mempool gossip) and gRPC 9090. Static peering only.

HARDWARE (recommended)
4+ vCPU, 8GB+ RAM, ~100GB SSD, stable connection. The node is
light — Rust + RocksDB, ~400ms blocks.

NO BONDING
There is no staking module. The validator set is fixed at genesis
(BFT n >= 3f+1) — you're in the set or you're not. No slashing,
no unbonding period. Revenue: transaction fees to the block
proposer. Fixed supply, zero inflation.

OPS
Keep the node online. Leader timeout 3s, certification timeout 5s.
A missed slot just skips — no penalty — but liveness needs 2/3+
of the set online.
```
