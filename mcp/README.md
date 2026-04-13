# 🌌 Cosmos MCP Server

> AI-native interface to any Cosmos chain. Built by [JunoClaw](https://github.com/Dragonmonk111/junoclaw).

The first Model Context Protocol server for the Cosmos ecosystem. Lets any MCP-compatible AI assistant (Claude, Windsurf/Cascade, Cursor) query chains, deploy contracts, and scaffold CosmWasm projects — without the developer needing to learn CosmJS, cargo schemas, or gas estimation.

## Philosophy

```
VairagyaNode → validates blocks    (service to the network)
JunoClaw     → governs DAOs        (service to communities)
Cosmos MCP   → enables builders    (service to the ecosystem)
```

The validator doesn't choose transactions. The MCP doesn't choose chains.

## Quick Start

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

Add to `claude_desktop_config.json`:

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

## Tools (16 total)

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
| `list_chains` | List all supported Cosmos chains |

### Transaction Tools (require mnemonic)
| Tool | Description |
|------|-------------|
| `send_tokens` | Send tokens to an address |
| `execute_contract` | Execute a message on a CosmWasm contract |
| `upload_wasm` | Upload a WASM binary to a chain |
| `instantiate_contract` | Instantiate a contract from a code ID |
| `migrate_contract` | Migrate a contract to a new code ID |

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

Adding a chain: edit `src/resources/chains.ts`.

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
