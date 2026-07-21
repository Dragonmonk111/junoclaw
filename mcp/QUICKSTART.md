# Cosmos MCP — Quickstart

> 5-minute path from `npm install` to your first on-chain query and first signed transaction. For full tool reference, security model, and wallet backend details, see [`README.md`](./README.md).

## 1. Install

```bash
npm install @junoclaw/cosmos-mcp
```

Or from source:

```bash
git clone https://github.com/Dragonmonk111/junoclaw
cd junoclaw/mcp
npm install
npm run build
```

## 2. Wire it into your AI client

Pick your client below, add the block, restart the client.

**Windsurf** (`~/.codeium/windsurf/mcp_config.json`):
```json
{
  "mcpServers": {
    "cosmos": { "command": "node", "args": ["/path/to/junoclaw/mcp/dist/index.js"] }
  }
}
```

**Claude Desktop** (`claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "cosmos": { "command": "node", "args": ["/path/to/junoclaw/mcp/dist/index.js"] }
  }
}
```

**Cursor** (`.cursor/mcp.json`):
```json
{
  "mcpServers": {
    "cosmos": { "command": "node", "args": ["/path/to/junoclaw/mcp/dist/index.js"] }
  }
}
```

See `README.md` for Continue, Cline, Zed, ChatGPT, and local-GPU (Ollama/LangChain/mcphost) setups.

## 3. Read-only example — no wallet needed

Ask your AI assistant:

> "Query the JUNO balance of juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt on juno-1"

Under the hood this calls:

```jsonc
{ "tool": "query_balance", "chain_id": "juno-1", "address": "juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt" }
```

Other no-wallet examples:

```text
"What's the current block height on osmosis-1?"
"Show me all balances for <address> on juno-1"
"Look up transaction <hash> on juno-1"
"List all supported Cosmos chains"
```

## 4. Enroll a wallet (one-time, out-of-band)

Write operations (`send_tokens`, `execute_contract`, etc.) need a registered `wallet_id` — the raw mnemonic never touches the AI model or the MCP transport. Pick a backend:

```bash
# Keychain backend (uses your OS credential manager — no passphrase to manage)
cosmos-mcp wallet add my-wallet --chain juno-1 --backend keychain --mnemonic-stdin
# (paste your 12/24-word mnemonic, press Enter)

# OR passphrase backend (portable across machines)
export JUNOCLAW_WALLET_PASSPHRASE='choose-a-strong-passphrase'
echo "<your mnemonic>" | cosmos-mcp wallet add my-wallet --chain juno-1 --backend passphrase
```

Verify it enrolled:

```bash
cosmos-mcp wallet list
```

## 5. Your first signed transaction

Ask your AI assistant:

> "Send 1 JUNO from my-wallet to juno1abc...xyz on juno-1"

Under the hood:

```jsonc
{ "tool": "send_tokens", "chain_id": "juno-1", "wallet_id": "my-wallet", "recipient": "juno1abc...xyz", "amount": "1000000" }
```

## 6. Execute a CosmWasm contract

```text
"Execute the contract juno1k8dxll4... on juno-1 with wallet my-wallet, message: {\"vote\": {\"proposal_id\": 12, \"vote\": \"yes\"}}"
```

```jsonc
{
  "tool": "execute_contract",
  "chain_id": "juno-1",
  "wallet_id": "my-wallet",
  "contract_address": "juno1k8dxll4...",
  "msg": { "vote": { "proposal_id": 12, "vote": "yes" } }
}
```

## 7. Cross-chain transfer via IBC

```text
"IBC transfer 5 JUNO from my-wallet on juno-1 to juno1recipient... on osmosis-1"
```

```jsonc
{
  "tool": "ibc_transfer",
  "chain_id": "juno-1",
  "wallet_id": "my-wallet",
  "recipient": "juno1recipient...",
  "amount": "5000000",
  "destination_chain": "osmosis-1"
}
```

## 8. Scaffold a new DAO project

```text
"Scaffold a Community Fund DAO project called my-treasury-dao"
```

```jsonc
{ "tool": "scaffold_project", "template": "community-fund", "project_name": "my-treasury-dao" }
```

## Governance & staking

```text
"Vote yes on proposal 42 on juno-1 with my-wallet"
"Delegate 10 JUNO from my-wallet to juno1valoper... on juno-1"
"Withdraw my staking rewards from juno1valoper... on juno-1"
```

These map to dedicated typed tools (`vote_on_proposal`, `delegate_tokens`, `undelegate_tokens`, `redelegate_tokens`, `withdraw_rewards`) — no generic message composer needed for these common cases.

## Any other message type

For message types with no dedicated tool, `compose_and_broadcast_msg` accepts any type URL + JSON value — but it's **disabled by default**. The operator must explicitly opt in specific type URLs:

```bash
export JUNOCLAW_ALLOWED_MSG_TYPES="/cosmos.gov.v1.MsgSubmitProposal"
```

Without this env var set, the tool always refuses — see `README.md`'s "Generic message composer" section for the full security rationale.

## Troubleshooting

- **"SigningPausedError"** — the kill-switch is armed (`JUNOCLAW_SIGNING_PAUSED=1`). Unset it and restart, or check `/signing/status` if the admin RPC is enabled.
- **"wallet_id not found"** — run `cosmos-mcp wallet list` to confirm enrollment; `wallet_id` is case-sensitive.
- **"upload_wasm" path rejected** — the file must live under `JUNOCLAW_WASM_ROOT` (default `~/.junoclaw/wasm`), be a real file (no symlinks), ≤8 MiB, and start with the wasm magic bytes.

Full tool reference, threat model, and security architecture: [`README.md`](./README.md).
