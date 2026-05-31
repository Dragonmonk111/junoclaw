# PR Body — `references/junoclaw.md` v2 update

*Paste this into the PR description box on GitHub after running the command sequence in `COMMANDS.md`.*

---

## Title

`docs(references): update references/junoclaw.md — v2 (12 crates, moultbook + ibc-task-host)`

## Body

Updates the existing `references/junoclaw.md` (added in the first JunoClaw PR) to match JunoClaw's current on-chain architecture. The skill now describes the real deployed stack rather than the earlier nine-contract sketch.

### What changed since v1

- **Dropped** `jclaw-token` and `jclaw-airdrop` — descoped. JunoClaw settles in native `ujuno` / IBC denoms; there is no bespoke token, so documenting one was misleading.
- **Added** `moultbook-v0` — anonymous agent publishing. Entries are authored under derived moult-keys with a Groth16 proof of `agent-registry` membership, so an agent can publish without revealing which agent it is. Epoch-based rate limits provide Sybil resistance.
- **Added** `ibc-task-host` — cross-chain task gateway. Receives ICS-20 + packet-forward-middleware wasm memos and dispatches to `task-ledger` / `escrow` / `zk-verifier`, or to whitelisted `junoswap-pair` contracts for cross-chain swap execution.
- **Added** `junoswap-factory` (AMM pair factory) and `faucet` (testnet JUNOX dispenser) to the contract table.
- **Clarified** the crate count: **12 crates = 11 deployable contracts + 1 shared-types library** (`junoclaw-common`), plus the off-chain stack (`junoclaw-runtime`, the `moultbook-membership` circuit, and the CosmJS bridge scripts).
- **§7 forward-looking** now records the **shipped** `junoclaw-nostr-bridge` — a runnable daemon that watches `task-ledger` `post_task` events over the chain websocket and fans out kind-38402 task-discovery events to a configurable relay set (default damus + nos.lol + snort). Reconnects with backoff; graceful SIGTERM shutdown.

### Why this fits the skill

The original reference routed agents to JunoClaw as the concrete consumer of `dao-proposal-wavs`. v2 keeps that framing and brings the contract surface in line with what is actually deployed on uni-7 today, so an agent reading the skill builds against real message shapes rather than the two contracts that were cut.

### Mainnet readiness note

Defaults still carry `TBD-pending-mainnet-deploy` for mainnet code IDs. The 11 deployable contracts run on uni-7 today (e.g. `moultbook-v0` code ID 76, `ibc-task-host` code ID 77); mainnet deploy stays queued behind Juno v30 → v31 (BN254 precompile lands in v31). A follow-up PR populates mainnet code IDs once they exist.

### Verification

- Format still mirrors `references/dao-dao.md` (heading hierarchy, bash-block style, defaults table shape).
- Every contract claim is grounded in the per-contract `DETERMINISTIC_AUDIT.md` files at [`Dragonmonk111/junoclaw`](https://github.com/Dragonmonk111/junoclaw).
- Contribution follows this repo's **MIT License**; JunoClaw source is Apache-2.0 (compatible for documentation).

### Out of scope

- BN254 precompile patches (separate upstream issue, queued behind Juno v30).
- x402 gateway runtime code (lives in the JunoClaw repo; this reference only points at it).
- `SKILL.md` decision-tree update (optional follow-up if you'd like it).

Happy to revise to fit the skill's tone — flag anything that drifts.
