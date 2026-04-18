# From Proposal to Population Scale: Everything Built Since Juno #373

## 33 Days, 9 Contracts, 22 MCP Tools, and a Mathematical Proof That Juno Can Serve 8 Billion People

---

> *"The validator doesn't choose transactions. The builder doesn't choose chains."*
>
> — VairagyaNode → JunoClaw → Cosmos MCP

---

**[STUDIO GHIBLI IMAGE 1 — THE JOURNEY BEGINS]**

```
A lone figure in traditional Japanese work clothes walking a winding mountain path at dawn, carrying a wooden toolkit on their back. The path switches back and forth up a misty slope, with each switchback marked by a small stone cairn with a number carved into it: "373", "WAVS", "ZK", "MCP", "Mesh". The figure is small against the vast landscape — rolling hills of green tea plantations fading into distant snow-capped peaks. Morning light breaks through the clouds in rays. A red torii gate stands at the top of the path, just visible through the mist. Hand-drawn pencil linework with watercolor wash, Studio Ghibli background detail, warm earth tones with selective vermillion accents on the torii, atmosphere of quiet determination and long journey --ar 16:9 --style raw --s 250 --v 6.1
```

> *"Every great journey begins with a single proposal."*

---

## Prologue: March 8th, 2025

On March 8th, 2025, we submitted Proposal #373 to the Juno Network.

It was a signaling proposal — no code execution, no community pool request. Just words. A request for the Juno ecosystem to recognize that something new was being built: an agentic AI platform where every action is proposed, verified, attested, and governed on-chain.

**Proposal #373 asked for three things:**
1. Recognition of JunoClaw as Juno ecosystem infrastructure
2. Endorsement of the Junoswap revival (AMM with WAVS-verified outcomes)
3. Support for the Akash + validator sidecar architecture

The proposal passed. The work began.

But what happened next was not what anyone expected — including us.

---

## Act I: The Contracts (Foundation)

**[STUDIO GHIBLI IMAGE 2 — THE WORKSHOP]**

```
Interior of a stone workshop at night, lit by oil lamps and the teal glow of multiple terminal screens. On a large wooden workbench: nine small wooden boxes arranged in a circle, each hand-painted with a different symbol. The boxes are slightly open, showing parchment scrolls inside. Copper tools hang on the wall — not for metalwork, but for code: a caliper with "GAS" engraved, a level with "QUORUM", a compass with "IBC". Through the window: stars and a distant lighthouse beam. A cat sleeps on a stack of leather-bound notebooks. Everything feels handcrafted, intentional, built to last. Hand-drawn pencil linework with watercolor wash, Studio Ghibli workshop detail, warm amber lamplight, atmosphere of quiet craftsmanship --ar 16:9 --style raw --s 250 --v 6.1
```

> *"Nine contracts. One purpose: trust without trust."*

---

### The Core Stack

Before any MCP server, before any ZK precompile, before any TEE attestation — there had to be contracts. Immutable, auditable, on-chain logic.

| Contract | Purpose | Testnet Contract | Status |
|----------|---------|------------------|--------|
| **agent-company v4** | DAO governance hub | `juno1k8dxll...stj85k6` | Live — 5+ proposals executed |
| **task-ledger** | Task lifecycle with atomic callbacks | Built | Tested |
| **escrow** | Non-custodial payment obligations | Built | Tested |
| **agent-registry** | Agent identity + reputation | Built | Tested |
| **zk-verifier** | Groth16 BN254 proof verification | `juno1ydxksv...lse7ekem` | Live — Code ID 64 |
| **builder-grant** | TEE-verified ecosystem grants | Built | Tested |
| **faucet** | Testnet distribution | `juno1...` | Live |
| **junoswap-factory** | AMM pair creation | `juno12v0t6...hvq3wtkkh` | Live — Code ID 61 |
| **junoswap-pair x2** | JUNOX/USDC + JUNOX/STAKE | `juno1xn4mtv...acwqfr6e98` | Live |

**109 tests passing across all contracts.**

The architecture is modular by design. Each contract is independent but they speak to each other through atomic callbacks — the `task-ledger` doesn't trust the `escrow`, it *verifies* the escrow's state before proceeding.

---

### The Rattadan Hardening

After a structural audit by Rattadan, three critical hardening passes were applied to `agent-company v4`:

| Variable | Risk Before | Fix |
|----------|-------------|-----|
| `attestation_hash` | Any hex string accepted blindly | On-chain SHA-256 re-computation |
| `status` | Independent per-contract, desync possible | Atomic cross-contract callbacks |
| `weight` | No cap, no cooldown — 51% coalition can zero minorities | ~~Delta cap (20%) + cooldown (500 blocks) + floor (1 bps)~~ → **67% supermajority for `WeightChange`** (mirrors `CodeUpgrade`)¹ |

The `attestation_hash` and `status` hardenings are in production on uni-7 and stand as originally described.

¹ **Footnote (2026-04-17 retraction).** The original `weight` hardening — delta cap, cooldown, and floor — was retracted after further analysis. A cooldown only delays consolidation without changing the outcome; a floor of 1 bps commemorates minorities rather than protecting them; a delta cap slows patient attackers by minutes. What replaces them is a *structural* protection: `WeightChange` proposals now require the same 67% supermajority as `CodeUpgrade`. As long as minorities collectively hold more than 33% of voting weight, they can block weight redistribution. Full rationale: [`docs/RATTADAN_HARDENING.md`](../docs/RATTADAN_HARDENING.md).

---

## Act II: The TEE Milestone (Trust)

**[STUDIO GHIBLI IMAGE 3 — THE HARDWARE SANCTUARY]**

```
A vast underground cavern lit by bioluminescent moss and the soft blue glow of server racks. The racks are not cold metal — they're encased in living wood, roots wrapping around copper conduit, water flowing through clear channels for cooling. In the center: a single crystalline chamber containing a silicon wafer, pulsing with each computation. A figure in a blue hoodie tends to the chamber, checking readings on a brass gauge. The air is cool and damp. Water drips from stalactites. The machines hum. This is not a data center. It's a sanctuary for computation. Hand-drawn pencil linework with watercolor wash, Studio Ghibli underground detail, bioluminescent teal and warm wood tones, atmosphere of reverence for technology --ar 16:9 --style raw --s 250 --v 6.1
```

> *"Trust the silicon, not the operator."*

---

### What We Proved

On March 17, 2026, JunoClaw executed a WebAssembly verification component inside an **Intel SGX Trusted Execution Environment** on an Azure DCsv3 confidential VM.

**The attestation was submitted to `agent-company` on uni-7:**

| Field | Value |
|-------|-------|
| **Proposal** | 4 |
| **Task type** | `outcome_verify` |
| **Attestation TX** | `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22` |
| **Block** | 11,735,127 |
| **Hardware** | Intel SGX (Azure Standard_DC2s_v3) |

**Jake Hartnell** (Juno co-founder, WAVS architect at Layer.xyz) confirmed: *"WAVS TEEs already work — you just need to run WAVS inside a TEE."*

That's what we did.

---

### The Trust Stack

```
Intel SGX Enclave
    ↓
WAVS Operator (wasmd runtime)
    ↓
WASI Component (wasm32-wasip2)
    ↓
SHA-256 Attestation Hash
    ↓
junod tx wasm execute agent-company { submit_attestation }
    ↓
On-Chain Proof (permanent, queryable)
```

**Phase 1–3**: Built WASI component, deployed contracts, wrote bridge.

**Phase 4**: Proved E2E on testnet (proposals 2–4).

**Phase 4b**: Built autonomous local operator that watches the chain, auto-detects proposals, computes hashes, submits.

**Phase 5**: Deployed WAVS on Azure DCsv3 with Intel SGX.

**Phase 6**: Deployed WAVS on **Akash Network** — decentralized GPU compute, US$7.85/month, zero centralized cloud dependency.

**Phase 7**: Shipped Chain Intelligence Module with 6 autonomous verification workflows: Swap Verification, Sortition, Outcome Verification, Governance Watch, Migration Watch, JUNOX/USDC monitoring.

All of this happened in **5 days** (March 13–18, 2026).

---

## Act III: The ZK Precompile (Math)

**[STUDIO GHIBLI IMAGE 4 — THE MONK AND THE CURVE]**

```
A young monk sitting on a tatami floor, cross-legged, ink brush in hand, carefully painting an elliptic curve on a large scroll of rice paper. The curve glows faintly with teal circuit traces. Through the paper screen behind, a massive Ethereum diamond logo floats in a misty sky like a distant mountain — beautiful but far away. Cherry blossom petals drift through the room. Hand-drawn pencil linework with watercolor wash, Studio Ghibli background detail, warm earth tones with selective teal and amber accents, contemplative and slightly melancholic --ar 16:9 --style raw --s 200 --v 6.1
```

> *"You don't kill what you love. You learn from it."*

---

### The Heist

Ethereum's EIP-196 and EIP-197 (Byzantium, 2017) gave the world cheap BN254 elliptic curve operations. These power every zkRollup, every privacy protocol.

**187,000 gas** to verify a ZK proof on Ethereum. Cheaper than a token transfer.

On Juno? We didn't have those precompiles. So we built a **pure CosmWasm Groth16 verifier** using arkworks.

**It worked. The proof verified. The math was correct.**

**The gas bill was 371,486.**

---

### The Numbers

| Approach | Gas Cost | % of Block |
|----------|----------|------------|
| SHA-256 hash check | ~200K | 2% |
| Groth16 with BN254 precompile (Ethereum) | ~187K | 2% |
| **Groth16 pure CosmWasm (measured on uni-7)** | **371,486** | **~4%** |

The pure Wasm verifier works. But a precompile would cut costs in half — and the gap widens with circuit complexity.

**The ask**: Three host functions in wasmvm — `bn254_add`, `bn254_scalar_mul`, `bn254_pairing_check` — implemented in Go using gnark-crypto. Estimated effort: 3–5 weeks.

**The result**: Juno becomes the first CosmWasm chain with native ZK verification. Privacy protocols. zkML inference proofs. Verifiable AI outputs. All possible.

The PoC is live on uni-7 (Code ID 64, Contract: `juno1ydxksv...lse7ekem`).

---

## Act IV: The MCP Server (Bridge)

**[STUDIO GHIBLI IMAGE 5 — THE STATION]**

```
A weathered stone cottage perched on a grassy sea cliff, laundry drying on a rope line. Through the open wooden window: a cluttered workshop desk with a glowing terminal screen showing scrolling green text (chain IDs: uni-7, juno-1, osmosis-1), surrounded by dried herbs, copper tools, hand-drawn network diagrams pinned to the wall. A cat sleeps on a stack of leather-bound notebooks labeled "MCP". The sea is turquoise and calm below. A small hand-lettered sign reads "cosmos://chains". Hand-painted texture, visible brushstrokes, Hayao Miyazaki style --ar 16:9 --style raw --s 250 --v 6.1
```

> *"The best infrastructure is invisible. You just ask, and it works."*

---

### What MCP Is

Model Context Protocol. Anthropic's open standard for connecting AI assistants to external tools. USB for AI.

**`@junoclaw/cosmos-mcp`**: The first MCP server for the entire Cosmos ecosystem.

---

### The Evolution: 16 Tools → 22 Tools

**v0.1.0 (March 2025)**: 16 tools, 5 chains, basic queries + scaffold.

**v0.3.0 (April 15, 2025)**: 22 tools, 7 chains, mesh-security + Celestia DA.

```
QUERY (11)
├── query_balance              ← NEW: IBC channel lookups
├── query_all_balances
├── query_contract
├── query_contract_info
├── query_tx
├── query_block_height
├── query_code_info
├── query_zk_verifier          ← NEW: Groth16 BN254 on-chain
├── query_mesh_security      ← NEW: osmosis-labs/mesh-security
└── list_chains                ← 7 chains, 12 IBC routes

TRANSACTION (8)
├── send_tokens
├── execute_contract
├── upload_wasm
├── instantiate_contract
├── migrate_contract
├── ibc_transfer               ← NEW: cross-chain via IBC
├── submit_blob                ← NEW: Celestia MsgPayForBlobs
└── (8 total)

SCAFFOLD (2)
├── list_templates             ← 9 DAO templates
└── scaffold_project

PROMPTS (2)
├── deploy-dao
└── check-contract
```

**18/18 smoke tests passing.**

---

### The Architecture

Read mode (no wallet): Any AI queries any Cosmos chain.

Write mode (mnemonic per-call): Transactions require explicit authorization. The server **never stores keys**.

**Compatible with**: Claude, Windsurf, Cursor, VS Code + Continue, Cline, Zed, ChatGPT Desktop, **Ollama + local GPU**.

The last one matters: Run Llama 3.1 on your own hardware, connect via LangChain MCP adapter, and you have **sovereign AI on sovereign chains**. No cloud. No API keys. Full stack sovereignty.

---

## Act V: The Scalability Proof (Math → 8 Billion)

**[STUDIO GHIBLI IMAGE 6 — THE CONSTELLATION]**

```
A sweeping landscape in the style of Hokusai's Great Wave but instead of water, the wave is made of flowing IBC packets and blockchain data streams. At the crest of the wave sits a small traditional Japanese house (agent-registry on Juno). Below the wave are dozens of smaller houses on different islands (task-ledger instances on different chains) all connected by glowing underwater cables (IBC channels). A single figure stands on the shore with a laptop. Mount Fuji in the background has a Cosmos atom logo at its peak. Dramatic 2D ukiyo-e woodblock print style with modern cyberpunk color accents of neon blue and pink --ar 16:9 --style raw --s 250 --v 6.1
```

> *"Horizontal scaling is a protocol. Vertical scaling is a precompile."*

---

### The Three Technologies

1. **IBC (Jae Kwon)** — Horizontal scaling protocol. Add more chains.
2. **Mesh Security (Jake, Ethan, Sunny)** — Remove validator bootstrap cost. Chains are free.
3. **Celestia + tiablob (Mustafa, Reece)** — 10-100x per-chain throughput. Each chain does more.

**Reece Williams** — Juno's Development Lead — built `tiablob` at Rollchains/Strangelove. It's the Cosmos SDK module that posts block data to Celestia DA. He also built `spawn`: scaffold a new modular Cosmos chain in minutes.

The person who connected Cosmos to Celestia is from our own house.

---

### The Numbers (Juno's Actual 3-Second Block Time)

| Phase | Chains | Validator sets | TPS | Tasks/min | Agents (1/min) |
|-------|--------|---------------|-----|-----------|----------------|
| Today | 7 | 7 | 777 | 46,620 | ~47,000 |
| Mesh early | 55 | 5 | 6,105 | 366,300 | ~366,000 |
| Mesh mature | 1,020 | 20 | 113,220 | 6,793,200 | ~6.8M |
| Mesh + Celestia | 5,000 | 20 | 1,665,000 | 99,900,000 | ~100M |

**Combined throughput**: 333 tasks/block × 10× Celestia × 5,000 chains = **16,650,000 tasks/block**

At 1 task per agent per minute: **333 million to 1 billion simultaneous agents.**

That's the population of Earth.

---

## The Timeline: 33 Days

| Date | Milestone |
|------|-----------|
| **March 8** | Proposal #373 submitted |
| **March 13** | WAVS Phase 1–3: contracts deployed |
| **March 16** | TEE attestation proven (Proposal 4) |
| **March 17** | Azure SGX + Akash deployment |
| **March 18** | Chain Intelligence Module live |
| **April 2** | ZK Precompile article published |
| **April 4** | MCP v0.1.0: 16 tools, 5 chains |
| **April 13** | HackMD for Jake — full summary |
| **April 15** | MCP v0.3.0: 22 tools, 7 chains, mesh + Celestia |

33 days. 9 contracts. 22 tools. 7 chains. 12 IBC routes. One validator. Zero new capital required for population-scale infrastructure.

---

## The Architecture (Complete)

```
┌─────────────────────────────────────────────────────────────┐
│                        JUNO CHAIN                           │
│                                                             │
│   agent-registry ──── agent-company (DAO governance)        │
│        │                    │                               │
│   reputation            proposals                           │
│        │               attestations                         │
│        │                    │                               │
│   task-ledger ←──→ escrow ←──→ zk-verifier                  │
│   (atomic callbacks)                                        │
│                                                             │
│   junoswap-factory ──── pairs ──── WAVS events              │
│                                                             │
└──────────────────────┬──────────────────────────────────────┘
                       │ events
              ┌────────┴─────────┐
              │   WAVS OPERATOR  │ ← Intel SGX / Akash
              │   + aggregator   │
              │   + registry     │
              └────────┬─────────┘
                       │
              ┌────────┴─────────┐
              │   COSMOS MCP     │ ← Any AI assistant
              │   (22 tools)     │     Claude, local GPU, etc.
              └──────────────────┘
```

---

## Epilogue: The Student Bows

**[STUDIO GHIBLI IMAGE 7 — THE DAWN]**

```
A quiet scene at dawn: a lone programmer sitting in seiza position on a tatami mat in a sparse room, laptop closed beside them, looking out through a sliding shoji screen door at a vast network of glowing interconnected nodes stretching to the horizon like a constellation map on the ground. Cherry blossom petals drift in through the open door. The programmer's shadow stretches behind them and transforms into the silhouettes of many people (representing the agents and users to come). Extremely minimal 2D illustration with maximum negative space, black ink on warm cream paper, only three colors used — black, pale pink for the blossoms, and electric blue for the network nodes. The feeling of completion and quiet gratitude --ar 16:9 --style raw --s 250 --v 6.1
```

> *"We did not invent IBC. We did not invent mesh security. We did not invent Celestia. We connected them."*

---

We are not the final bosses. We are the students who surrendered to their technique:

- **Jae Kwon** — who gave blockchains the ability to speak to each other
- **Jake, Ethan, and Sunny** — who removed the economic barrier to infinite chains
- **Mustafa Al-Bassam** — who decoupled execution from consensus
- **Reece Williams** — who built the bridge from Cosmos to Celestia

To all of them, from VairagyaNode and JunoClaw:

**お辞儀をします。**

*We bow.*

---

## Links & Resources

| Resource | Link |
|----------|------|
| **This Article (Medium)** | https://medium.com/@tj.yamlajatt/the-final-bosses-of-cosmos-how-we-built-an-ai-agent-layer-that-scales-to-8-billion-3298a5b17be5 |
| **GitHub** | https://github.com/Dragonmonk111/junoclaw |
| **npm** | `npm install @junoclaw/cosmos-mcp` |
| **Proposal #373** | https://ping.pub/juno/gov/373 |
| **ZK Precompile Article** | https://medium.com/@tj.yamlajatt/killing-eth-with-love-why-juno-is-stealing-ethereums-best-idea-1875f1879a7c |
| **Cosmos MCP Article** | https://medium.com/@tj.yamlajatt/the-first-ai-that-speaks-cosmos-building-juno-new-2a0253cbce91 |
| **TEE TX** | `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22` |
| **ZK Verify TX** | `F6D5774EE2073E2DD011399A7E96889BA026ED67C6A510D208FD5C575080F4DA` |

---

## Open-Source Stack

| Component | License | Repository |
|-----------|---------|------------|
| **JunoClaw** | Apache 2.0 | github.com/Dragonmonk111/junoclaw |
| **Mesh Security** | Apache 2.0 | osmosis-labs/mesh-security |
| **Celestia** | Apache 2.0 | celestiaorg/celestia-app |
| **tiablob** | Apache 2.0 | rollchains/tiablob |
| **spawn** | Apache 2.0 | rollchains/spawn |
| **CosmJS** | Apache 2.0 | cosmos/cosmjs |
| **MCP SDK** | MIT | modelcontextprotocol/sdk |

---

*Built from March 8th to April 15th, 2025. 33 days. 9 contracts. 22 tools. 7 chains. 12 IBC routes. One validator. Zero new capital required for population-scale infrastructure.*

*All code is Apache 2.0. All math is public. All bosses are thanked.*

