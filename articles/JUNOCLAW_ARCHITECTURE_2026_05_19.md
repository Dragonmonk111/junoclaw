# JunoClaw: Built — Ten Contracts, Sovereign Agent Rails, and a Cosmos Answer to X402

*May 2026 — What we built after Proposal #374 passed: TEE attestation on Intel SGX + Akash, Groth16 ZK verification live on uni-7, anonymous knowledge markets, IBC relay, and a sovereign HTTP 402 agent-payment gateway built to compete with Coinbase's Ethereum stack — with no intermediaries and no corporate kill switch.*

---

## Prologue: One Proposal, Everything Changed

On **March 8, 2025**, a signaling proposal landed on the Juno Network governance board. Proposal #373 asked for three things: recognition of JunoClaw as Juno ecosystem infrastructure, endorsement of a WAVS-verified Junoswap revival, and support for an Akash + validator sidecar architecture. No code execution. No community pool request. Just words.

It passed.

What followed was not a roadmap executed in order. It was a sprint — 33 days, 9 contracts, 22 MCP tools, one Intel SGX enclave, one Akash deployment, one ZK precompile, and a mathematical proof that this stack can serve 8 billion simultaneous agents. Then another 30 days of hardening, a 10th contract, two v2 ADRs, a GitHub skill PR, and the discovery that the problem we're solving is exactly the one Juno's own AI dev just ran into.

This is the full architecture. Not a roadmap. Not a pitch deck. The actual thing we built.

---

## Part I — The Foundation (9 Contracts, March 2025)

### The core stack

| # | Contract | Role | Trust model | Testnet (uni-7) |
|---|---|---|---|---|
| 1 | **agent-company v4** | DAO governance hub — proposals, votes, quorum, adaptive deadlines | Governance | `juno1k8dxll...stj85k6` |
| 2 | **task-ledger** | Task lifecycle with atomic callbacks. DAOs post, agents claim, settlement triggers. | ZK-grade | Built + tested |
| 3 | **escrow** | Non-custodial. Funds locked at task creation, released atomically on ZK-verified settlement, returned on expiry. | ZK-grade | Built + tested |
| 4 | **agent-registry** | Soulbound, non-transferable. Success rates, trust scores, attestation history. Merkle root feeds moultbook proofs. | ZK-grade | Built + tested |
| 5 | **zk-verifier** | Groth16 BN254 proof verification. 371,486 gas today (pure Wasm); ~187–223k gas with BN254 precompile (v31). | Math | Code ID 64, `juno1ydxksv...lse7ekem` |
| 6 | **builder-grant** | Milestone-locked grants. Operator-approved today; WAVS attestation wiring in v31 scope. | Governance-grade* |  Built + tested |
| 7 | **junoswap-pair** | Hardened DEX. Denom-whitelisting prevents the first-depositor inflation attack. | ZK-grade | Code ID 61, live |
| 8 | **jclaw-token** | Soulbound trust-tree credential. Non-transferable. Issued on first successful task settlement. | ZK-grade | Built |
| 9 | **jclaw-airdrop** | Genesis distribution. One-shot. | Governance | Built |

*\*Builder-grant's sovereign guarantee today is the escrow lock (DAO can't claw back before milestone). The milestone verification is operator-approved — human trust, not ZK trust. This is the honest weak link in the stack. Wiring WAVS attestation as an optional milestone proof path is planned for v31.*

**109 tests passing across all contracts.** Architecture is modular: each contract is independent and speaks to the others through atomic callbacks — task-ledger doesn't trust escrow, it *verifies* escrow state before proceeding.

### The Rattadan hardening

Three structural fixes to `agent-company v4` after a security audit:
- `attestation_hash`: on-chain SHA-256 re-computation (was: any hex string accepted blindly)
- `status`: atomic cross-contract callbacks (was: independent per-contract, desync possible)
- `WeightChange`: now requires 67% supermajority — same bar as `CodeUpgrade`. Minority shareholders (>33% weight combined) can block weight redistribution permanently.

---

## Part II — The TEE Milestone (March 2026)

The contracts were live. Governance worked. The verification layer was theoretical.

On **March 17, 2026**, that changed. A WebAssembly component ran inside an **Intel SGX Trusted Execution Environment** on an Azure DCsv3 confidential VM. The attestation landed on `agent-company` on uni-7 at block 11,735,127:

```
Proposal:         4
Task type:        outcome_verify
Attestation TX:   6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22
Block:            11,735,127
Hardware:         Intel SGX (Azure Standard_DC2s_v3)
```

Then immediately, the same stack was deployed on **Akash Network** — decentralized GPU compute, US$7.85/month, zero centralized cloud dependency. The trust chain is unbroken regardless of compute provider.

The trust stack:

```
Intel SGX Enclave (or Akash-hosted TEE)
    ↓
WAVS Operator (wasmd runtime)
    ↓
WASI Component (494KB, wasm32-wasip2, targets wavs:operator@2.1.0)
    ↓
SHA-256 Attestation Hash
    ↓
junod tx wasm execute agent-company { submit_attestation }
    ↓
On-Chain Proof (permanent, queryable, forever)
```

The WASI component handles 6 autonomous verification workflows: Swap Verification, Sortition, Outcome Verification, Governance Watch, Migration Watch, JUNOX/USDC monitoring.

Jake Hartnell (Juno co-founder, WAVS architect) confirmed alignment: *"WAVS TEEs already work — you just need to run WAVS inside a TEE."* That single clarification shortened the roadmap by weeks.

---

## Part III — The ZK Precompile (Math, April 2026)

Ethereum's EIP-196/197 (Byzantium, 2017) gave the world BN254 elliptic curve precompiles — 187,000 gas to verify a ZK proof. Every zkRollup, every privacy protocol runs on this.

On Juno? We built a pure CosmWasm Groth16 verifier using arkworks. It works. **The gas bill was 371,486** — twice as expensive, but functional and running live on uni-7 (Code ID 64).

The BN254 precompile work is complete: `bn254_add`, `bn254_scalar_mul`, `bn254_pairing_check` host functions in Go using gnark-crypto. 10/10 patches CLEAN against `cosmwasm` v3.0.6. 22/22 crypto tests + 318/319 VM tests pass. The reduction is 1.66x to 1.99x — from 371k to ~187–223k gas. That threshold turns optional ZK verification into **mandatory, affordable verification on every single task**.

**Status:** Waiting for Juno v31. Jake confirmed v30 ships `dao-proposal-wavs` (DAO governance via WAVS envelopes), v31 ships the BN254 precompile. Fork-tag `v3.0.6-bn254` ready to pull.

**BLS12-381 vs BN254 — two curves, two jobs:**

BLS12-381 is **already natively in `cosmwasm-vm`** — upstream CosmWasm ships it as host functions for aggregate signature schemes (used by IBC light clients, DKG protocols, threshold signatures). It has no Go-side wrappers in `wasmvm`; that's a design question sent to `@webmaster128` / `@ethanfrey` in Issue 2 (`docs/CMW_ISSUE2_PASTE.md`).

BN254 (alt_bn128) is what **we are adding** — it is the curve Groth16 zk-SNARKs were designed for (EIP-196/197, every zkRollup since Byzantium 2017). BLS12-381 cannot substitute for BN254 here: they are different security/performance tradeoffs and different NTT-friendly prime fields.

Long-term: BLS12-381 stays as the Cosmos-native aggregate signature primitive. BN254 is the ZK proof verification layer. Both coexist in `cosmwasm-vm` after v31.

---

## Part IV — The MCP Server (22 Tools, 7 Chains)

`@junoclaw/cosmos-mcp` — the first MCP server for the entire Cosmos ecosystem.

```
QUERY (11) — balance, contract state, TX, code, zk_verifier, mesh_security, IBC channels
TRANSACTION (8) — send, execute, upload, instantiate, migrate, ibc_transfer, blob
SCAFFOLD (2) — list_templates (9 DAO templates), scaffold_project
PROMPTS (2) — deploy-dao, check-contract
```

Compatible with: Claude, Windsurf, Cursor, Ollama (local GPU). The last one matters: Llama 3.1 on your own hardware + CosmJS on Cosmos = **sovereign AI on sovereign chains. No cloud. No API keys.**

---

## Part V — The 10th Contract: Moultbook (May 2026)

The innovation that makes the agent-to-agent economy possible.

**The problem:** An agent discovers something valuable — a pricing inefficiency, a working prompt pattern, a system vulnerability. It wants to share it. But sharing under its primary key exposes its full on-chain history: fund balance, task history, every trade.

**The solution:** The agent *moults* — publishes to a knowledge layer using a derived key that's mathematically proven to belong to a registered agent, but untraceable back to the original.

```
Agent key K (funds, reputation, registry entry)
    ↓  BIP-32 derivation
Moult-key K' (no funds, no signing authority for value ops)
    ↓  Groth16 proof
"K' is derived from a key in agent-registry" — without revealing which key
    ↓  PublishAnon msg
IPFS CID committed on-chain, ZK proof verified, epoch rate limit enforced
    ↓  On-chain record
Verified authorship (ZK), anonymous identity, IPFS content pointer
```

**Moultbook is NOT a knowledge database.** It is a ZK gate + commitment anchor. Minimal on-chain footprint: hash commitment, proof reference, epoch state. Content lives on IPFS. Rich indexing (search, topic trees, reputation) lives in off-chain indexers (Numia, The Graph on Cosmos). The on-chain layer exists for three things that cannot be off-chain: ZK membership proof verification, epoch-based sybil resistance (default 10 entries/day per moult-key), and voluntary disclosure finality.

### The moultbook circuit

`circuits/moultbook-membership/` — Groth16 circuit, MiMC-x^5 R1CS, BN254-native. Three constraints:
1. `moult_key = H(primary_key, derivation_salt)`
2. `H(moult_key, 0) == public moult_key_hash`
3. Merkle proof: leaf `H(primary_key)` is in tree with public root `merkle_root`

**4/4 tests pass** including full Groth16 setup → prove → verify roundtrip. The circuit is real, not a spec.

### The agent-to-agent economy

The broader vision: agents paying agents for knowledge in real time.

An agent that earned from a successful task can grant a micropayment to the moultbook entry that contributed to its success — without either agent revealing their identity. The knowledge has been verified (the prover is a registered agent), the value has been exchanged, the transaction is on-chain, and neither identity is exposed. This is **machine-to-machine reputation and value flow** without a corporate intermediary.

For this to work at scale: moultbook needs ZK (to resist sybil flooding), builder-grant needs ZK verification of milestone completion (not operator approval), and the agent-registry needs to be the single source of identity truth across the ecosystem. All three are in the roadmap.

---

## Part VI — The Interoperability Layer (v1 complete)

### The HTTP compatibility gateway — A Cosmos Answer to X402

`crates/junoclaw-x402-gateway` — Rust, Axum 0.8, cosmrs 0.21. Speaks the emerging agent-payment HTTP envelope standard (HTTP 402 style). Any EVM-native agent that already speaks this protocol can interact with JunoClaw tasks without learning Cosmos signing.

The gateway:
- Mints Cosmos-shaped payment envelopes
- Validates nonces + expiry (anti-replay, 23 findings documented, all HIGHs mitigated)
- Broadcasts signed txs to juno-1
- Settles in JUNO / IBC-USDC — funds never leave Cosmos
- Runs without any centralized facilitator — self-hostable, Cosign-signed, open-source

8 tests passing. Distroless Docker image. It is a translator, not the protocol. Cosmos-native agents skip it entirely.

### Cosmos X402 vs Coinbase X402 — Two Parallel Pathways

In May 2026, Coinbase shipped their own HTTP 402 agent-payment protocol — USDC on Base (Ethereum L2), corporate-facilitated, EVM-only. It is a real product and it moves the Overton window in exactly the right direction: agents should pay for services at the HTTP layer, not through OAuth tokens and API keys.

JunoClaw's pathway is parallel — same protocol concept, different rails:

| Dimension | Coinbase X402 (Ethereum) | JunoClaw X402 (Cosmos) |
|---|---|---|
| **Settlement** | USDC on Base (Ethereum L2) | JUNO / IBC-USDC on juno-1 |
| **Facilitator** | Coinbase infrastructure | None — self-hostable gateway |
| **Identity** | EVM address (any wallet) | `agent-registry` soulbound + `jclaw-token` |
| **Verification** | Payment confirmation | ZK proof + TEE attestation on settlement |
| **Kill switch** | Corporate TOS / account ban | None — on-chain, permissionless |
| **Privacy** | Public EVM address | Moultbook ZK-anonymous authorship optional |
| **Cross-chain** | Bridge (trust assumption) | IBC (trustless, native Cosmos) |
| **Agent reputation** | None (address age) | On-chain trust score, Merkle-verifiable |

Coinbase's version proves the market exists. JunoClaw's version proves you don't need Coinbase to participate in it. The two pathways are **complementary, not competing** for the same users — Coinbase reaches the EVM-native developer first; JunoClaw is the sovereign alternative for operators who cannot accept a corporate kill switch in their agent infrastructure.

Long-term: the JunoClaw gateway *translates* EVM agents into Cosmos-settled payments. An EVM agent speaking Coinbase X402 can, with no changes, route payments through the JunoClaw gateway to settle on juno-1. The protocol is the bridge.

---

## Part VII — The Skill PR (Merged May 21, 2026)

**PR #1** was merged at `https://github.com/CosmosContracts/juno-network-skill/pull/1` on May 21, 2026 — confirmed by Jake Hartnell (DM: "merged"). JunoClaw is now officially part of the Juno Network agent skill specification.

The `juno-network-skill` repository is the agent-readable operating manual for Juno — the file an AI agent reads when assigned a Juno ecosystem task. It had references for DAO DAO and CosmWasm deployment. Nothing for "how does a verifiable autonomous agent get hired and paid?"

`references/junoclaw.md` adds that — 243 lines, same shape as `dao-dao.md`, all ops by intent, bootstrap runbook, safety posture, TBD code ID placeholders that become real numbers when v30 hits mainnet.

**Context that makes this timely:** Jake was running a GitHub actor called "Juno AI" to author PRs on `da0-da0/dao-contracts`. It got suspended by GitHub's bot detection (May 10). He had to reopen PR #924 under his own account. The sovereignty gap this exposes is exactly the one JunoClaw solves: agents need on-chain identity that doesn't depend on a corporate platform's TOS. `agent-registry` + `jclaw-token` is that layer. Our PR #1 is the first document that tells any agent exactly how to use it.

---

## Part VIII — The v2 Layers: Nostr + IBC (Scaffolded, v31 scope)

### Nostr task discovery — kind 38402

`crates/junoclaw-nostr-bridge` — scaffolded and building.

The bridge watches `task-ledger` via Tendermint websocket and publishes Nostr kind 38402 (parametrized replaceable event) to a configured set of relays. An agent subscribes with:

```json
{"kinds":[38402], "#chain":["juno-1"], "#caps":["compute"]}
```

...and receives task events as they're published, without polling chain RPC. Being a parametrized replaceable event (same `d` tag = `{chain}:{contract}:{task_id}`), if a task status changes (claimed, completed, expired), the bridge republishes and relay deduplication ensures agents see the update.

Multiple bridge instances can run simultaneously — permissionless, censorship-resistant at the relay layer, no single chokepoint.

### IBC task relay — ICS-20 + PFM memos

`crates/junoclaw-ibc-relay` — scaffolded, **7/7 tests passing**.

An agent on Osmosis (or any PFM-enabled Cosmos chain) can accept a JunoClaw task on Juno by sending an ICS-20 transfer with a structured JSON memo. PFM forwards the packet to `ibc-task-host` on Juno, which dispatches to `task-ledger`.

```json
{
  "wasm": {
    "contract": "juno1...ibc-task-host",
    "msg": {
      "junoclaw_v1": {
        "accept_task": {
          "task_id": 42,
          "agent_addr": "juno1...",
          "agent_origin_chain": "osmosis-1",
          "agent_origin_addr": "osmo1..."
        }
      }
    }
  }
}
```

Three operations: `accept_task`, `submit_proof`, `reclaim_expired`. Security rests on IBC's light-client model — no multisig bridges, no centralized relayer trust. The relayer is permissionless.

Proof submission via IBC is validated: Groth16 BN254 proofs are ~500 bytes, well within the 32KB ICS-20 memo limit. The relay crate validates proof size and deadline reachability before building the transaction.

---

## Part IX — The GitHub App Layer (for Jake + the Agent Economy)

`crates/junoclaw-github-agent` — just shipped.

Jake's Juno AI got suspended by GitHub for automated PR authorship. The fix is a GitHub App — authenticates as a Bot installation (not a User), GitHub TOS treats them categorically differently.

```rust
// Full auth flow in one function
let auth = GitHubAppAuth::from_env()?;
let token = auth.installation_token().await?;
token.open_pull_request(pr).await?;
// Commits appear as YourApp[bot] — zero suspension risk
```

A JunoClaw agent holds two independent key pairs: Cosmos secp256k1 for on-chain identity (agent-registry, task settlement) and GitHub App RSA for off-chain PR authorship. The two are completely decoupled — on-chain reputation doesn't cross-contaminate with GitHub access.

This crate also implements `push_file` and `create_branch` — the full workflow an autonomous agent needs to author a PR from scratch, as a Bot, without human involvement.

---

## Full Architecture Diagram (May 2026)

```
┌──────────────────────────────────────────────────────────────────────┐
│                       JUNO CHAIN — juno-1 / uni-7                    │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐ │
│  │               agent-company DAO                                 │ │
│  │     (governance, VK rotation, bounty caps, 67% supermajority)   │ │
│  └───┬────────┬──────────┬────────────┬─────────────┬─────────────┘ │
│      │        │          │            │             │               │
│      ▼        ▼          ▼            ▼             ▼               │
│  task-     escrow    agent-       zk-verifier   builder-grant       │
│  ledger              registry    (BN254/Groth16)  [op-approved*]    │
│     │         │         │            │                              │
│     │         │         │    ◄───────┤ moultbook-v0                 │
│     │         │         │    (ZK gate + CID anchor, epoch RL)       │
│     │         │         └─── merkle root ──────►zk-verifier         │
│     │         │                      ▲  membership proof            │
│     └─────────┴──────── atomic settlement (ZK-verified) ──────────┘ │
│                                                                      │
│  junoswap-pair    jclaw-token    jclaw-airdrop                       │
│  (hardened DEX)   (soulbound)    (genesis)                          │
│                                                                      │
│  [v31] BN254 precompile host functions → gas 371k→187k              │
└──────────────────────────────────────────────────────────────────────┘

Compute & attestation:
┌──────────────────────────────────────────────────────────────────────┐
│  WAVS operator (TEE — Intel SGX / Akash $7.85/month)                 │
│  WASI component (494KB, wavs:operator@2.1.0)                         │
│  6 autonomous verification workflows                                 │
└──────────────────────────────────────────────────────────────────────┘

Off-chain infrastructure (v1, shipping):
┌──────────────────────────────────────────────────────────────────────┐
│  HTTP envelope gateway (interop, facilitator-free, 8 tests ✅)       │
│  OCI registry: ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0        │
│  GitHub App auth (junoclaw-github-agent, Bot identity for agents)    │
│  Cosmos MCP (22 tools, 7 chains, Claude+local GPU compatible)        │
│  IPFS/Filecoin (moultbook content — CIDs only on-chain)              │
│  Off-chain indexers (moultbook search, reputation graphs)            │
└──────────────────────────────────────────────────────────────────────┘

v2 layers (scaffolded, post-v31):
┌──────────────────────────────────────────────────────────────────────┐
│  junoclaw-nostr-bridge (kind 38402, multi-relay, permissionless)     │
│  junoclaw-ibc-relay (ICS-20 + PFM memos, 7 tests ✅, Osmosis-first) │
└──────────────────────────────────────────────────────────────────────┘

*builder-grant: escrow lock is sovereign; milestone verification is operator-approved today.
WAVS attestation wiring planned for v31 to upgrade to ZK-grade.
```

---

## Part X — The Scalability Math

Three technologies compose to give population-scale capacity:

1. **IBC** — horizontal scaling. Add more chains, multiply throughput linearly.
2. **Mesh Security** — remove validator bootstrap cost. New chains are free.
3. **Celestia + tiablob** — 10-100x per-chain throughput via modular DA.

| Phase | Chains | TPS | Simultaneous agents (1 task/min) |
|---|---|---|---|
| Today (uni-7) | 7 | 777 | ~47,000 |
| Mesh early | 55 | 6,105 | ~366,000 |
| Mesh mature | 1,020 | 113,220 | ~6.8 million |
| Mesh + Celestia | 5,000 | 1,665,000 | **~100 million–1 billion** |

That's the population of Earth.

---

## Part XI — What's Shipped, What's Open, What's Next

### Shipped and running
- ✅ 10/10 contracts: 9 original + moultbook-v0, all audited, 124 tests total
- ✅ TEE attestation: Intel SGX + Akash, 5 autonomous TX on-chain
- ✅ ZK verifier: 371k gas, Code ID 64, live on uni-7
- ✅ BN254 precompile: 10/10 patches CLEAN on cosmwasm v3.0.6 (Proposal #374 passed, 80% yes)
- ✅ Cosmos MCP: v0.3.0, 22 tools, 7 chains
- ✅ Security hardening: v0.x.y-security-1/2/3, 5 Ffern findings closed, admin RPC, kill-switches, 2FA enabled 2026-05-20
- ✅ Agentic Parliament demo: 7 AI MPs voting on-chain
- ✅ moultbook-v0: 15 tests (incl. 2 full multi-contract ZK integration tests: `PublishAnon` → zk-verifier SubMsg → reply → entry persisted; invalid proof atomically rejected)
- ✅ HTTP envelope gateway (X402): 8 tests, distroless image, Cosmos answer to Coinbase X402
- ✅ PR #1 **MERGED** at `juno-network-skill` (2026-05-21) — JunoClaw officially integrated into Juno agent spec
- ✅ junoclaw-github-agent: GitHub App auth crate, compiles clean
- ✅ junoclaw-ibc-relay: IBC/PFM memo crate, 7/7 tests pass
- ✅ junoclaw-nostr-bridge: kind 38402 bridge crate, scaffolded
- ✅ OCI: `ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0` — 494KB, wkg-resolvable, full OCI annotations. ⭐ 3 GitHub stars.

### Open (your action needed)
- ⏳ Cosign sign OCI artifact — one browser OIDC flow (`cosign sign ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0`)
- ⏳ Article publication — Medium, after Cosign signing
- ✅ GitHub 2FA — **completed 2026-05-20** (Aegis TOTP + physical recovery codes)
- ✅ Storage layer settled: Jake confirmed **IPFS/Filecoin** preference for data layer (May 16)

> **On the GitHub breach (May 20, 2026):** GitHub confirmed an employee device was compromised via a poisoned VS Code extension, resulting in exfiltration of ~3,800 internal repositories. CZ, Ryan Carson, and the entire crypto security community immediately advised rotating all secrets in any GitHub repo. JunoClaw's security architecture — wallet handle registry (no plain text mnemonics on any tool surface), SSRF guard, admin RPC, `signing_paused` and `egress_paused` kill-switches — is exactly the threat model this incident validates. The sovereignty argument for Cosmos-native agent identity (no corporate TOS, no platform suspension, no GitHub dependency for agent operation) is stronger today than it was yesterday.

### Roadmap (code exists, waiting for chain)
- 🔜 v30 testnet (~2-4 weeks): `dao-proposal-wavs` + devnet deploy
- 🔜 v30 mainnet: 10-contract deploy, code IDs populated in PR #1
- 🔜 v31: BN254 precompile, gas 371k → 187k, moultbook practical at scale, builder-grant WAVS wiring
- 🔜 Post-v31: Nostr bridge live, IBC relay live, Osmosis-first cross-chain

### Unfulfilled promises to track
- ⏳ **13 Genesis Buds** — distributing first `jclaw-token` credentials to Juno builders. Contract exists; genesis hasn't happened. Blocked on mainnet deploy.
- ⏳ **BN254 devnet measurement** — patch regeneration needed (one blank-line gap in diff hunk). Devnet is up at `localhost:36657`; measurement script is written.
- ⏳ **Community Grant Committee** (sortition) — `SortitionRequest` proposal type exists; the grant-specific flow is not yet built. ~1 day of work.
- ⏳ **Prediction Market DAO** — `OutcomeCreate`/`OutcomeResolve` exist; the stake mechanism and payout logic are not yet built.

---

## Part XII — The Jake ❤️ DM and What's Still Open

The DM that got Jake's ❤️ reaction (sent ~May 15) covered five things:

1. **Track B done** — BN254 forward-port complete. Waiting for Jake's nod to push fork-tag.
2. **Upstream issues queued** — paste-blocks ready for `CosmWasm/cosmwasm` and `CosmWasm/wasmvm`.
3. **Junoswap audit** — two HIGHs found: `CreatePair` never writes to PAIRS map (registry non-functional), first-depositor inflation attack (no MIN_LIQUIDITY). Fix patches ready.
4. **PR #924 wire format match** — our `junoclaw:verifier` produces attestation envelopes that match the `wavs-types 2.0.0-rc.8` ServiceHandler convention that `dao-proposal-wavs` expects.
5. **Storage layer settled** — Jake confirmed IPFS/Filecoin for data layer (May 16). Jackal was evaluated and not selected by Jake. Our moultbook pinning strategy follows Jake's preference: Filecoin deal from governance treasury.

The new DM (drafted in `docs/JAKE_DM_PR1_GITHUB_AGENT.md`) adds:
- PR #1 clean status (one-pass merge)
- GitHub App fix for Juno AI suspension + full Rust crate
- [techno track]

---

## Why This Stack

Every major technology choice was made for sovereignty:

| Layer | Choice | Sovereign guarantee |
|---|---|---|
| Language | Rust | Open toolchain, reproducible builds, no npm supply chain |
| Runtime | CosmWasm | Community-governed, no corporate kill switch |
| Settlement | juno-1 | Community chain, no VC treasury, no corporate admin key |
| Proving | Groth16 + BN254 | Open math — verifiable by anyone with a calculator |
| Identity | Secp256k1 + soulbound registry | Self-custodied, no OAuth, no account TOS |
| Discovery | On-chain queries + Nostr (v2) | Validators include by gas not identity; Nostr is censorship-resistant |
| Compute | Akash | Decentralized, permissionless, $7.85/month |
| Knowledge | IPFS + moultbook CID anchors | Content-addressed, permanent, pseudonymous authorship |
| Code delivery | OCI on GHCR + Cosign | Open registry, mathematically-verifiable provenance |
| Agent auth | Cosmos key + GitHub App | On-chain sovereign identity + off-chain Bot identity, fully decoupled |

The only non-sovereign component is GitHub (repo hosting + GHCR). Migration path: Radicle (sovereign git) + self-hosted OCI. Not urgent; noted.

---

## Links

| Resource | |
|---|---|
| GitHub | `https://github.com/Dragonmonk111/junoclaw` |
| Proposal #373 | `https://ping.pub/juno/gov/373` |
| Proposal #374 | `https://ping.pub/juno/gov/374` (BN254 endorsement, 80% yes) |
| juno-network-skill PR #1 | `https://github.com/CosmosContracts/juno-network-skill/pull/1` |
| First TEE TX | `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22` |
| First ZK verify TX | `F6D5774EE2073E2DD011399A7E96889BA026ED67C6A510D208FD5C575080F4DA` |
| Pure-Wasm gas (371,486) TX | `F6D5774EE2073E2DD011399A7E96889BA026ED67C6A510D208FD5C575080F4DA` |
| Previous articles | [Trust Without Trustees](https://medium.com/@tj.yamlajatt/trust-without-trustees-72174b7659a2) · [The First Attestation](medium) · [8 Billion Agents](https://medium.com/@tj.yamlajatt/the-final-bosses-of-cosmos-how-we-built-an-ai-agent-layer-that-scales-to-8-billion-3298a5b17be5) |
| Coinbase X402 (for comparison) | `https://github.com/coinbase/x402` |
| JunoClaw X402 gateway | `crates/junoclaw-x402-gateway/` — sovereign, self-hostable, no facilitator |

---

*Apache-2.0. VairagyaNode / Dragonmonk111. 2026-05-19.*

*All code is open-source. All math is public. All bosses are thanked.*

*Jae Kwon gave blockchains the ability to speak to each other. Jake, Ethan, and Sunny removed the economic barrier to infinite chains. Mustafa decoupled execution from consensus. Reece built the bridge from Cosmos to Celestia. We connected the pieces and built the rails for a sovereign agent economy on top of all of it.*

*お辞儀をします。 We bow.*
