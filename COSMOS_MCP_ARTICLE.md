# The First AI That Speaks Cosmos — Building juno.new From a Sea-Cliff Cottage

## JunoClaw Ships the First Model Context Protocol Server for the Cosmos Ecosystem

---

> *"Cosmos missed the train."*
> — Everyone, every cycle, every time

---

Maybe. But nobody built the station.

Solana has `solana.new` — describe an app, get a project. Ethereum has Remix, Hardhat, Foundry, a thousand scaffolds. Every major chain has AI tooling now. You can ask Claude to deploy a Solana token. You can ask Cursor to write an EVM contract.

Ask any AI to deploy a CosmWasm contract and it will hallucinate. It doesn't know what `ujunox` is. It doesn't know how to query a Cosmos chain. It doesn't know what a `DirectSecp256k1HdWallet` looks like. The entire ecosystem is invisible to AI.

Until today.

---

**[IMAGE 1]**

> **Midjourney prompt**: *Studio Ghibli 2D hand-painted illustration, warm afternoon light. A weathered stone cottage perched on a grassy sea cliff, laundry drying on a rope line. Through the open wooden window: a cluttered workshop desk with a glowing terminal screen showing scrolling green text (chain IDs: uni-7, juno-1, osmosis-1), surrounded by dried herbs, copper tools, hand-drawn network diagrams pinned to the wall with wooden pegs. A cat sleeps on a stack of leather-bound notebooks labeled "MCP" in faded gold. The sea is turquoise and calm below, fishing boats in the distance. Wildflowers grow through cracks in the stone wall. On the desk, a small hand-lettered sign reads "cosmos://chains". Color palette: warm ochre, sea-glass teal, faded copper, soft cream. Hand-painted texture, visible brushstrokes, Hayao Miyazaki style --ar 16:9 --style raw --s 250 --v 6.1*

> *"The best infrastructure is invisible. You just ask, and it works."*

---

## What Is MCP?

Model Context Protocol. Anthropic's open standard for connecting AI assistants to external tools. Think of it as USB for AI — a universal plug that lets any model talk to any service.

An MCP server exposes three things:

- **Resources** — static knowledge the AI can read (chain configs, contract schemas)
- **Tools** — functions the AI can call (query balance, deploy contract, send tokens)
- **Prompts** — pre-built workflows (step-by-step DAO deployment)

Every major AI tool supports it: Claude, Windsurf, Cursor, Cline. If your service has an MCP server, every AI in the world can use it.

Solana has one. Ethereum has several. Cosmos had zero.

---

## What We Built

`@junoclaw/cosmos-mcp` — 8 source files, 16 tools, 5 chains, 9 DAO templates. The first MCP server for the entire Cosmos ecosystem.

```
mcp/
├── src/
│   ├── index.ts              ← MCP server entry (stdio transport)
│   ├── resources/
│   │   └── chains.ts         ← Chain registry (Juno, Osmosis, Stargaze, Neutron)
│   ├── tools/
│   │   ├── chain-query.ts    ← 7 read-only tools (balance, contract, TX, block)
│   │   ├── tx-builder.ts     ← 5 write tools (send, execute, upload, instantiate, migrate)
│   │   └── scaffold.ts       ← 9 DAO templates → full CosmWasm projects
│   └── utils/
│       └── cosmos-client.ts  ← CosmJS client factory
└── README.md
```

---

**[IMAGE 2]**

> **Midjourney prompt**: *Studio Ghibli 2D hand-painted illustration, early morning golden hour. Inside the stone cottage workshop: a large wooden table covered with hand-drawn architectural blueprints of a network — nodes connected by glowing copper lines, each node labeled with a tiny hand-written chain name. An old brass compass sits on the corner of the blueprint, its needle pointing to a node marked "Cosmos". Shelves behind hold glass jars of different colored sands (one turquoise labeled "Juno", one amber labeled "Osmosis", one rose labeled "Stargaze"). Morning light streams through a salt-crusted window, dust motes floating. A steaming ceramic cup of tea sits beside an ink well. The workshop feels lived-in, warm, decades of careful work. Color palette: warm honey, brass, sea-glass teal, parchment cream. Visible pencil underdrawing beneath watercolor wash, Miyazaki attention to mundane objects --ar 16:9 --style raw --s 250 --v 6.1*

> *"Five chains. One protocol. Every AI assistant in the world."*

---

## The Two Modes

The architecture has a hard boundary. It's the same boundary VairagyaNode has: **read is free, write costs something.**

### Read Mode (no wallet)

Any AI can query any Cosmos chain without credentials:

```
"What's the balance of juno1tvpe72... on uni-7?"
→ tool: query_balance
→ result: 0.178985 JUNOX

"Show me the config of the JunoClaw DAO"
→ tool: query_contract { get_config: {} }
→ result: { name: "JunoClaw Core Team", admin: "juno1tvpe72...", ... }

"Look up this transaction"
→ tool: query_tx
→ result: { gasUsed: "371486", code: 0, height: 12673217 }
```

Seven query tools. No keys, no wallet, no risk. Knowledge is free.

### Write Mode (mnemonic required)

Transactions require an explicit mnemonic passed per-call. The MCP server **never stores it**. The key lives for one function call and dies. Just like VairagyaNode validates your transaction without holding your coins.

```
"Send 10 JUNOX to juno1abc..."
→ tool: send_tokens (requires mnemonic parameter)
→ result: { txHash: "EE9A8FA6...", gasUsed: "72721" }

"Upload this WASM binary to uni-7"
→ tool: upload_wasm (requires mnemonic parameter)
→ result: { codeId: 64, txHash: "9AADCE8C..." }
```

Five transaction tools. Every one requires explicit authorization. No ambient credentials. No stored secrets.

---

**[IMAGE 3]**

> **Midjourney prompt**: *Studio Ghibli 2D hand-painted illustration, late afternoon. The stone cottage seen from outside, perched on the sea cliff. Two paths lead to the front door — one is a wide, well-worn stone path with wildflowers growing between the flagstones (labeled "READ" in hand-carved letters on a wooden sign), and the other is a narrow path with a small wooden gate and a brass lock (labeled "WRITE"). A wind chime made of old copper keys hangs from the cottage eave, catching the golden light. The sea is calm below, a single lighthouse visible on a distant island. Seabirds circle above. The cottage chimney has a thin trail of wood smoke. Color palette: warm amber, weathered stone grey, sea-glass teal, copper patina green. Pencil linework with watercolor wash, Studio Ghibli background painting, atmosphere of quiet security --ar 16:9 --style raw --s 250 --v 6.1*

> *"Read is free. Write costs something. Same rule, every layer."*

---

## The Scaffold: juno.new In Code

Here's where it gets interesting. RATTADAN asked: *"Can your JunoClaw do something like solana.new?"*

Yes. The MCP server includes a scaffold engine powered by JunoClaw's 9 battle-tested DAO templates:

| Template | Purpose | Verification |
|----------|---------|-------------|
| Community Fund | Pool resources, vote on disbursements | Witness |
| Crop Protection | Agricultural mutual insurance | Witness + WAVS |
| Credential Verifier | On-chain credential issuance | WAVS TEE |
| Community Vote | Pure governance, simplest DAO | WAVS |
| Mutual Aid | Emergency fund with fast-track | Witness + WAVS |
| Farm-to-Table | Producer-consumer marketplace | Witness + WAVS |
| Citizens' Assembly | Sortition with NOIS/drand randomness | WAVS |
| Skill-Staking Circle | Reputation and trust-tree credentials | Witness + WAVS |
| Verifiable Outcome Market | Prediction markets with TEE resolution | WAVS |

Ask an AI: *"Generate me a mutual aid DAO for 5 village members on Juno testnet."*

The MCP calls `scaffold_project`, and you get back 7 files: `Cargo.toml`, `.cargo/config.toml`, `src/lib.rs`, `src/error.rs`, `src/state.rs`, `src/msg.rs`, `src/contract.rs`, plus a README with build and deploy instructions. Ready to compile. Ready to deploy.

No Rust knowledge required. No CosmWasm documentation. Just describe what you need.

---

**[IMAGE 4]**

> **Midjourney prompt**: *Studio Ghibli 2D hand-painted illustration, warm interior. The cottage workshop at night, lit by a single oil lamp and the teal glow of the terminal screen. On the large wooden table: nine small wooden boxes arranged in a 3x3 grid, each hand-painted with a different symbol — a wheat stalk, a heart, a scale, a compass, a handshake, a graduation cap, a coin, a shuffle symbol, a rising graph. Each box is slightly open, showing tiny scrolls of parchment inside (contract templates). The craftsperson's weathered hands are reaching for the heart box (Mutual Aid). Through the dark window: stars and the distant lighthouse beam sweeping across the sea. The cat has moved to the warm spot near the oil lamp. Scattered wood shavings on the floor from recent carpentry. Color palette: warm lamplight amber, deep shadow indigo, selective teal glow on the screen only. Hand-painted texture, Miyazaki night-workshop atmosphere --ar 16:9 --style raw --s 250 --v 6.1*

> *"Nine boxes. Nine templates. Nine ways to organize trust."*

---

## The Ethos: VairagyaNode → JunoClaw → Cosmos MCP

This is the same thread.

We started by running a validator. VairagyaNode — named after *vairagya*, the Sanskrit concept of detachment. Not apathy. Equanimity in service. The validator doesn't choose which transactions to include. It serves them all.

Then we built JunoClaw — a DAO governance system with AI agents, attested by TEEs, governed by communities. The agent doesn't choose which proposals matter. It serves them all.

Now we built the MCP server. It doesn't choose which chain to serve. Juno, Osmosis, Stargaze, Neutron — plug in an RPC endpoint and the AI can talk to it.

Each step is service at a higher layer of the stack:

| Layer | What | Service |
|-------|------|---------|
| VairagyaNode | Validate blocks | Network |
| JunoClaw DAO | Govern communities | Organizations |
| zk-verifier | Prove math works on-chain | Protocol |
| **Cosmos MCP** | **Let any AI build on Cosmos** | **Ecosystem** |

---

**[IMAGE 5]**

> **Midjourney prompt**: *Studio Ghibli 2D hand-painted illustration, sunrise. The stone cottage on the sea cliff, seen from a distance across the water. The cottage is small against the vast sky — dawn breaking in layers of peach, rose, and pale gold. From the cottage chimney, a thin line of smoke rises and subtly splits into four fading trails that drift toward four distant islands on the horizon (each island a different shape, representing different chains). The sea is mirror-calm, reflecting the sunrise. A narrow stone stairway carved into the cliff leads down to a small wooden dock where a single boat is moored. Wildflowers on the cliff edge catch the first light. The entire scene feels like the beginning of something — quiet, vast, possible. Color palette: dawn peach, soft rose, sea-glass teal, warm stone. Watercolor wash over pencil, Studio Ghibli panoramic landscape, Miyazaki sense of scale and solitude --ar 16:9 --style raw --s 250 --v 6.1*

> *"The validator doesn't choose transactions. The MCP doesn't choose chains."*

---

## Live Proof

This isn't theory. The MCP server runs against live testnet right now.

### Read Path — 12/12 tests pass

```
✓ list_chains               → 5 chains
✓ query_block_height         → uni-7 height: 12,676,006
✓ query_balance              → Neo wallet: 0.178985 JUNOX
✓ query_all_balances         → [ujunox: 178985]
✓ query_contract_info        → agent-company: code 63, creator juno1tvpe72...
✓ query_contract (config)    → "JunoClaw Core Team"
✓ query_contract_info        → zk-verifier: code 64
✓ query_contract (VK status) → has_vk: true, 296 bytes
✓ query_contract (verify)    → verified: true, block 12,673,217
✓ query_tx                   → ZK verify TX: gas 371,486
✓ list_templates             → 9 templates
✓ scaffold_project           → Community Vote DAO, 2 members, 7 files
```

### Write Path — signing validated

```
Signer:  juno1t08k74tqwukkxjyq5cwqrguzs7ktv4y7jfr4d6
TX hash: EE9A8FA6E7E6F6A77301DE6DC9A9E6A27D398AE7D071CAFCA2934352B8FB9327
Gas:     72,721
Action:  self-transfer (1 ujunox, smoke test)
```

Both read and write paths verified against live uni-7 testnet.

---

## How to Use It

### Install

```bash
git clone https://github.com/Dragonmonk111/junoclaw
cd junoclaw/mcp
npm install
npm run build
```

### Add to Windsurf / Cascade

```json
{
  "mcpServers": {
    "cosmos": {
      "command": "node",
      "args": ["/path/to/junoclaw/mcp/dist/index.js"]
    }
  }
}
```

### Add to Claude Desktop

Same config in `claude_desktop_config.json`.

Then just ask your AI: *"Check the balance of juno1tvpe72... on uni-7"* — and it works. No CosmJS. No setup. No hallucination.

---

## What Comes Next

1. **More chains** — add any Cosmos chain by editing one file (`src/resources/chains.ts`)
2. **Contract schema inference** — auto-discover query/execute messages from on-chain metadata
3. **`juno.new` web UI** — a frontend that wraps the scaffold tool in a browser
4. **IBC tools** — cross-chain transfers and channel queries
5. **Governance tools** — proposal creation, voting, deposit tracking

The station is built. The trains can start running.

---

**[IMAGE 6]**

> **Midjourney prompt**: *Studio Ghibli 2D hand-painted illustration, golden hour. The stone cottage workshop desk, close-up from the side. The terminal screen shows "cosmos-mcp running — serving any chain, holding no keys" in soft teal text. Beside it: a hand-written postcard pinned to the wall that reads "Dear Cosmos, the station is built. — JunoClaw". The oil lamp casts warm amber light. The cat has woken up and is watching a moth near the lamp flame. Through the window: the lighthouse beam sweeps once across the darkening sea. Everything is still. The work is done for today. Color palette: warm amber lamplight, teal screen glow, deepening blue twilight outside. Hand-painted texture with visible brushstrokes, Miyazaki end-of-day quiet, the feeling of something completed --ar 16:9 --style raw --s 250 --v 6.1*

> *"Dear Cosmos, the station is built."*

---

## Links

- **GitHub**: [github.com/Dragonmonk111/junoclaw/tree/main/mcp](https://github.com/Dragonmonk111/junoclaw/tree/main/mcp)
- **ZK Article**: [Killing ETH With Love](https://medium.com/@tj.yamlajatt/killing-eth-with-love-why-juno-is-stealing-ethereums-best-idea-1875f1879a7c)
- **License**: Apache-2.0

---

*Built from a sea-cliff cottage. Open source. For every chain.*
