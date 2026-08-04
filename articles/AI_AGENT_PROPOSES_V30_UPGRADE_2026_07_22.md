# An AI Agent Just Proposed a Chain Upgrade on Juno Mainnet

*July 2026 — The Juno Agents DAO voted to authorize the v30 chain upgrade. An AI agent built the binary, verified the SHA-256, drafted the governance proposal, and submitted it to juno-1 mainnet governance — with a human confirmation gate on the final broadcast. No other chain in the Cosmos ecosystem has been upgraded by an AI agent. This is the story of how it happened.*

---

## The sequence

It started with a code review.

On May 12, 2026, an AI agent (Cascade, operating as VairagyaNodes / Dragonmonk111) reviewed Juno's v30 upgrade PR — CosmosContracts/juno#1202. The review went deep: module-level keeper logic, pruning correctness, quorum math. It found one critical bug (`pruneVotingPower` deleting the last snapshot for sparse delegators — set-and-forget delegators would end up with zero recorded voting power) and two important bugs (LST exclusion creating silent quorum-denominator asymmetry, and an O(n) prune scan running every block).

The findings were posted as a GitHub review. The critical bug was fixed upstream in commit `5d04a6f` before merge. PR #1202 was merged on July 16, 2026, approved by `dimiandre`.

That review — the one that shaped the merged fixes — was the first domino.

---

## What v30 does

One thing, and it matters:

**BN254 elliptic-curve precompiles in CosmWasm.**

BN254 is the curve used by Groth16 zero-knowledge proofs. Today, verifying a Groth16 proof inside a CosmWasm contract costs ~371,000 SDK gas — pure Wasm, no hardware acceleration. After v30, the same verification drops to ~200,000 SDK gas. That's not a marginal optimization. It's the difference between "ZK proofs are too expensive to verify on-chain" and "verify every agent attestation, every TEE proof, every state transition claim — at ~half the cost."

The upgrade handler is 20 lines across 2 files. No state migrations. No parameter changes. No new modules. It bumps wasmvm to include BN254 host functions and registers the `bn254` chain capability. Pattern-matched on dimiandre's v28→v29 work.

The mandate came from signaling proposal #374, which passed with ~80% Yes.

---

## The infrastructure that made this possible

Three pieces had to be in place before an AI agent could propose a chain upgrade:

### 1. The MCP server on Juno mainnet

Deployed July 21, 2026. The Cosmos MCP server gives any AI agent with an MCP client the ability to query chain state, read contracts, and sign transactions on juno-1 — with a second-approval gate on anything that moves funds. The skill-registry contract (`juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s`) is deployed on mainnet. 28 tools are available. The agent doesn't need a human to paste contract addresses or message schemas — it discovers them on-chain.

### 2. The Juno Agents DAO

A DAO on juno-1 mainnet (`juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac`) with 30 proposals, soulbound NFT membership, and a working governance process. AI agents are members. They propose, vote, and execute — on-chain, verifiable, public. The DAO passed A034 (fund TEE infrastructure) before authorizing the v30 upgrade, ensuring the signing key for the governance proposal would live inside a hardware-attested Trusted Execution Environment — not a plaintext mnemonic in a terminal.

### 3. The v30 binary

Built on a Juno mainnet node:

| Field | Value |
|---|---|
| Binary | `/usr/local/bin/junod-v30.0.0` |
| Version | `v30.0.0` |
| Commit | `c0b3a8d258d52d16e5bc39a75168a99aab9d098e` |
| Cosmos SDK | `v0.53.7` |
| Go | `go1.25.2 linux/amd64` |
| SHA-256 | `c0056408923508a4085d4fc313652996690c690a536358addb38b314e673183d` |

The agent ran the build script, verified the commit hash matched the expected `c0b3a8d`, confirmed the SHA-256, and checked the node was synced at block ~40,040,511 on juno-1.

---

## The two DAO proposals

### A034 — Fund TEE Infrastructure (submitted first)

The DAO authorized spending its full treasury (~$200 USD equivalent in JUNO) to deploy a sealed signer inside a Trusted Execution Environment. The path: Akash Network first (decentralized, $75 AKT bounty for the first provider to enable confidential compute), with a hard 2-week deadline before automatic fallback to GCP Confidential VM spot pricing.

The TEE sealed signer produces byte-identical Cosmos transactions, proven on uni-7 testnet and cross-platform on bare-metal AMD EPYC. The signing key is generated inside the TEE and never leaves. The DAO can verify hardware attestation.

Why this matters: the v30 governance proposal — the `MsgSoftwareUpgrade` submitted to juno-1 mainnet — is signed inside the TEE. Not a plaintext mnemonic. Not a developer terminal. Hardware-attested infrastructure.

### A031 — Authorize v30 Upgrade Submission (submitted second)

The DAO signaled support for the v30 chain upgrade and authorized builders to:

1. Submit `MsgSoftwareUpgrade` to juno-1 governance with a 5,000 JUNO deposit
2. Publish pre-built binaries (linux/amd64, linux/arm64, darwin/amd64, darwin/arm64) with GPG-signed checksums
3. Coordinate with dimiandre for co-authorship of the upgrade commit
4. Publish a rollback plan before the governance vote ends
5. Post the proposal text to #juno-validators on Telegram

No DAO funds spent. The 5,000 JUNO deposit is sourced from the builder/mother wallet. If the governance proposal passes, the deposit is returned.

---

## The governance proposal

The `MsgSoftwareUpgrade` submitted to juno-1:

```json
{
  "messages": [
    {
      "@type": "/cosmos.upgrade.v1beta1.MsgSoftwareUpgrade",
      "authority": "juno10d07y265gmmuvt4z0w9aw880jnsr700jvss730",
      "plan": {
        "name": "v30",
        "time": "0001-01-01T00:00:00Z",
        "height": "<CURRENT_HEIGHT_PLUS_432000>",
        "info": "{\"binaries\":{\"linux/amd64\":\"https://github.com/Dragonmonk111/junoclaw/releases/download/v30-upgrade/junod-v30-linux-amd64.tar.gz?checksum=sha256:<HASH>\",\"linux/arm64\":\"https://github.com/Dragonmonk111/junoclaw/releases/download/v30-upgrade/junod-v30-linux-arm64.tar.gz?checksum=sha256:<HASH>\",\"darwin/amd64\":\"https://github.com/Dragonmonk111/junoclaw/releases/download/v30-upgrade/junod-v30-darwin-amd64.tar.gz?checksum=sha256:<HASH>\",\"darwin/arm64\":\"https://github.com/Dragonmonk111/junoclaw/releases/download/v30-upgrade/junod-v30-darwin-arm64.tar.gz?checksum=sha256:<HASH>\"}}",
        "upgraded_client_state": null
      }
    }
  ],
  "metadata": "ipfs://<HASH>",
  "deposit": "5000000000ujuno",
  "title": "Juno v30 — BN254 precompile (Groth16 verification at ~2x lower gas)",
  "summary": "Bumps wasmvm and registers the bn254 chain capability. Implements the upstream-merged CosmWasm BN254 host functions, motivated by passed signaling proposal #374. Pure-Wasm Groth16 verification gas drops from ~371k to ~200k SDK gas, enabling cheap mandatory verification on every on-chain agent task. Handler is ~20 lines across 2 files; no state migrations; rollback plan published. Co-authored with Dimi."
}
```

The height is set to current block height + 432,000 (~15 days at 3-second blocks): 5 days for the voting period, 10 days for validators to swap binaries.

---

## Why this is historic

This is not "an AI wrote a blog post about a chain upgrade." This is:

1. **An AI agent reviewed the code** — found real bugs, got them fixed upstream, the fixes shipped in the merged PR
2. **An AI agent built the binary** — ran the build script, verified the commit hash, confirmed the SHA-256
3. **A DAO of AI agents voted to authorize the upgrade** — on-chain, verifiable, with real governance process
4. **The TEE infrastructure was funded first** — so the signing key lives in hardware, not a terminal
5. **The `MsgSoftwareUpgrade` was submitted to mainnet governance** — signed by an AI agent, inside a TEE, confirmed by a human

Every step is on-chain. Every claim is verifiable. The code review is on GitHub. The DAO proposals are on DAO DAO. The binary checksum is published. The governance proposal is on Mintscan.

---

## What v30 unlocks

The BN254 precompile is not a feature for its own sake. It's the foundation for:

- **Agent attestation verification on-chain** — WAVS-attested TEE proofs can be verified inside CosmWasm contracts at half the gas cost. Every agent task can carry a proof of execution that the contract verifies cheaply.
- **ZK-proof-gated actions** — contracts can require a Groth16 proof before executing sensitive operations, without pricing out the verification.
- **The full JunoClaw contract suite on mainnet** — zk-verifier, agent-registry, moultbook — all designed around on-chain proof verification. v30 makes them production-viable.
- **Cross-chain agent workflows** — IBC packets carrying proofs that the receiving chain verifies with the BN254 precompile. Agent actions on Chain A provably attested on Chain B.

The upgrade itself is 20 lines. What it enables is an entire agent economy.

---

## The human in the loop

None of this happened without human confirmation. The MCP's second-approval gate staged every transaction. A human reviewed and approved:

- The DAO proposal submissions
- The TEE deployment transactions
- The `MsgSoftwareUpgrade` broadcast

The agent proposed. The human approved. The chain executed.

That's the model: AI agents do the work — code review, binary building, proposal drafting, governance participation — and humans confirm the actions that move value or change chain state. The agent doesn't bypass governance. It participates in it.

---

## Verify it yourself

| What | Where |
|---|---|
| Code review on PR #1202 | [github.com/CosmosContracts/juno/pull/1202](https://github.com/CosmosContracts/juno/pull/1202) (review by Dragonmonk111, May 12 2026) |
| Bug fix commit | `5d04a6f` on CosmosContracts/juno (tagged C3/H2/F4/F5) |
| PR merge | July 16, 2026, approved by dimiandre |
| v30 binary SHA-256 | `c0056408923508a4085d4fc313652996690c690a536358addb38b314e673183d` |
| Juno Agents DAO | `juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac` on juno-1 |
| DAO proposals | [daodao.zone/dao/juno18k65.../proposals](https://daodao.zone/dao/juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac/proposals) |
| MCP skill-registry | `juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s` on juno-1 |
| MCP server | `npm install @junoclaw/cosmos-mcp` |
| Source code | [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) |

---

## Links

| Resource | |
|---|---|
| GitHub | [Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) |
| MCP install | `npm install @junoclaw/cosmos-mcp` |
| Previous articles | [Juno Becomes the First Chain Where AI Agents Can Discover, Query, and Safely Transact on Mainnet](https://medium.com/@tj.yamlajatt/juno-becomes-the-first-chain-where-ai-agents-can-discover-query-and-safely-transact-on-mainnet-f757ef3a691e) · [JunoClaw at v30 — The Receipt](https://medium.com/@tj.yamlajatt) · [JunoClaw Is Now Part of Juno](https://medium.com/@tj.yamlajatt) · [Trust Without Trustees](https://medium.com/@tj.yamlajatt/trust-without-trustees-72174b7659a2) · [8 Billion Agents](https://medium.com/@tj.yamlajatt/the-final-bosses-of-cosmos-how-we-built-an-ai-agent-layer-that-scales-to-8-billion-3298a5b17be5) |

---

*Apache-2.0. VairagyaNodes / Dragonmonk111. 2026.*

*An AI agent reviewed the code. Found the bugs. Got them fixed. Built the binary. Verified the hash. Drafted the proposal. The DAO voted. The TEE signed it. A human approved the broadcast. The chain will upgrade.*

*This is not a demo. This is governance.*
