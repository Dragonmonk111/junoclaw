# 🌌 Cosmos MCP Server

> AI-native interface to any Cosmos chain. Built by [JunoClaw](https://github.com/Dragonmonk111/junoclaw).
>
> 📖 [Read the article: The Final Bosses of Cosmos](https://medium.com/@tj.yamlajatt/the-final-bosses-of-cosmos-how-we-built-an-ai-agent-layer-that-scales-to-8-billion-3298a5b17be5)

The first Model Context Protocol server for the Cosmos ecosystem. Lets any MCP-compatible AI assistant (Claude, Windsurf/Cascade, Cursor) query chains, deploy contracts, and scaffold CosmWasm projects — without the developer needing to learn CosmJS, cargo schemas, or gas estimation.

## Philosophy

```
VairagyaNode → validates blocks    (service to the network)
JunoClaw     → governs DAOs        (service to communities)
Cosmos MCP   → enables builders    (service to the ecosystem)
```

The validator doesn't choose transactions. The MCP doesn't choose chains.

## Install

```bash
npm install @junoclaw/cosmos-mcp
```

Or from source:

```bash
cd mcp
npm install
npm run build
```

### Add to Windsurf

Add to your `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "cosmos": {
      "command": "node",
      "args": ["C:/path/to/junoclaw/mcp/dist/index.js"]
    }
  }
}
```

### Add to Claude Desktop

Add to `claude_desktop_config.json` (macOS: `~/Library/Application Support/Claude/`, Windows: `%APPDATA%/Claude/`):

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

### Add to Cursor

Add to `.cursor/mcp.json` in your project root:

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

### Add to VS Code + Continue

Install the [Continue](https://continue.dev) extension, then add to `.continue/config.yaml`:

```yaml
mcpServers:
  - name: cosmos
    command: node
    args:
      - /path/to/junoclaw/mcp/dist/index.js
```

### Add to Cline (VS Code)

Install [Cline](https://github.com/cline/cline) extension. Settings → MCP Servers → Add:

```json
{
  "cosmos": {
    "command": "node",
    "args": ["/path/to/junoclaw/mcp/dist/index.js"]
  }
}
```

### Add to Zed

Add to `~/.config/zed/settings.json`:

```json
{
  "language_models": {
    "mcp_servers": {
      "cosmos": {
        "command": "node",
        "args": ["/path/to/junoclaw/mcp/dist/index.js"]
      }
    }
  }
}
```

### Add to OpenAI ChatGPT

ChatGPT supports MCP via its desktop app. Settings → MCP → Add Server:

```json
{
  "cosmos": {
    "command": "node",
    "args": ["/path/to/junoclaw/mcp/dist/index.js"]
  }
}
```

### Local GPU (Ollama / llama.cpp / vLLM)

MCP uses **stdio transport** — any process that reads stdin and writes stdout can be an MCP client. Local LLMs can use the Cosmos MCP via bridge tools:

**Option 1: `mcp-client` CLI** (easiest)
```bash
npm install -g @anthropic-ai/mcp-client
mcp-client --server "node /path/to/junoclaw/mcp/dist/index.js"
```

**Option 2: Ollama + Open WebUI + MCP plugin**
```bash
# Run your local model
ollama run llama3.1:70b

# Open WebUI has MCP tool support via its Functions/Tools pipeline
# Configure the MCP server as a tool provider in Open WebUI settings
```

**Option 3: LangChain MCP adapter** (Python)
```python
from langchain_mcp_adapters.client import MultiServerMCPClient
from langchain_ollama import ChatOllama

# Connect to your local Ollama model
llm = ChatOllama(model="llama3.1:70b")

# Connect to Cosmos MCP
async with MultiServerMCPClient({
    "cosmos": {
        "command": "node",
        "args": ["/path/to/junoclaw/mcp/dist/index.js"]
    }
}) as client:
    tools = client.get_tools()
    # Your local model can now call all 22 Cosmos tools
```

**Option 4: `mcphost`** (Rust, any OpenAI-compatible API)
```bash
cargo install mcphost
mcphost --config mcp_config.json --api-base http://localhost:11434/v1
```

> **Bottom line**: Any model that can do tool-calling (Llama 3.1+, Mistral, Qwen2.5, DeepSeek) running on your own GPU via Ollama can use all 22 Cosmos MCP tools. Your keys, your GPU, your chain interactions. Full sovereignty.

## Tools (22 total)

### Query Tools (no wallet needed)
| Tool | Description |
|------|-------------|
| `query_balance` | Get token balance for any address on any Cosmos chain |
| `query_all_balances` | Get all token balances for an address |
| `query_contract` | Query a CosmWasm smart contract (read-only) |
| `query_contract_info` | Get contract metadata (code ID, creator, admin) |
| `query_tx` | Look up a transaction by hash |
| `query_block_height` | Get current block height |
| `query_code_info` | Get info about an uploaded WASM code ID |
| `query_zk_verifier` | Query a deployed zk-verifier contract (Groth16 BN254 VK status + last proof) |
| `query_mesh_security` | **NEW** Query mesh-security contracts ([osmosis-labs/mesh-security](https://github.com/osmosis-labs/mesh-security), Apache 2.0). Auto-detects provider vs consumer. |
| `list_chains` | List all supported Cosmos chains (now includes IBC channel metadata) |

### Transaction Tools (require mnemonic)
| Tool | Description |
|------|-------------|
| `send_tokens` | Send tokens to an address |
| `execute_contract` | Execute a message on a CosmWasm contract |
| `upload_wasm` | Upload a WASM binary to a chain |
| `instantiate_contract` | Instantiate a contract from a code ID |
| `migrate_contract` | Migrate a contract to a new code ID |
| `ibc_transfer` | Transfer tokens across Cosmos chains via IBC — the first cross-chain primitive for AI agents |
| `submit_blob` | **NEW** Submit data blobs to Celestia DA layer ([celestiaorg/celestia-app](https://github.com/celestiaorg/celestia-app), Apache 2.0). Sovereign rollup data availability. |

### Scaffold Tools (juno.new)
| Tool | Description |
|------|-------------|
| `list_templates` | List available DAO templates |
| `scaffold_project` | Generate a full CosmWasm project from a template |

## Resources

| Resource | URI | Description |
|----------|-----|-------------|
| Chain Registry | `cosmos://chains` | All supported chains |
| Individual Chain | `cosmos://chains/{chainId}` | Config for a specific chain |
| DAO Templates | `cosmos://templates` | All 9 DAO templates |

## Supported Chains

| Chain | ID | Denom | Type |
|-------|-----|-------|------|
| Juno Testnet | `uni-7` | ujunox | Testnet |
| Juno Mainnet | `juno-1` | ujuno | Mainnet |
| Osmosis | `osmosis-1` | uosmo | Mainnet |
| Stargaze | `stargaze-1` | ustars | Mainnet |
| Neutron | `neutron-1` | untrn | Mainnet |
| Celestia | `celestia` | utia | Mainnet |
| Celestia Mocha | `mocha-4` | utia | Testnet |

Adding a chain: edit `src/resources/chains.ts`.

## IBC Routes

The chain registry includes IBC channel metadata for cross-chain transfers:

| Route | Source Channel | Dest Channel |
|-------|---------------|---------------|
| Juno → Osmosis | channel-0 | channel-42 |
| Juno → Stargaze | channel-20 | channel-5 |
| Juno → Neutron | channel-548 | channel-4328 |
| Osmosis → Juno | channel-42 | channel-0 |
| Osmosis → Stargaze | channel-75 | channel-0 |
| Osmosis → Neutron | channel-874 | channel-10 |
| Stargaze → Juno | channel-5 | channel-20 |
| Stargaze → Osmosis | channel-0 | channel-75 |
| Neutron → Juno | channel-4328 | channel-548 |
| Neutron → Osmosis | channel-10 | channel-874 |
| Celestia → Osmosis | channel-2 | channel-6994 |
| Celestia → Neutron | channel-8 | channel-35 |

## DAO Templates (9)

1. **Community Fund** — Pool resources, vote on disbursements
2. **Crop Protection Pool** — Agricultural mutual insurance
3. **Credential Verifier** — On-chain credential issuance with TEE attestation
4. **Community Vote** — Pure governance, the simplest DAO
5. **Mutual Aid** — Emergency fund with fast-track disbursement
6. **Farm-to-Table Market** — Producer-to-consumer with provenance tracking
7. **Citizens' Assembly** — Sortition governance with NOIS/drand randomness
8. **Skill-Staking Circle** — Reputation staking and trust-tree credentials
9. **Verifiable Outcome Market** — Prediction markets with TEE resolution

## Security

- **The MCP server never stores keys.** Mnemonics are passed per-call and discarded.
- Read operations require no wallet.
- Write operations require an explicit mnemonic parameter.
- The server holds no state between calls.

## License

Apache-2.0 — same as Cosmos SDK, CosmWasm, and the tools we build on.
