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

### Transaction Tools (require a registered `wallet_id` — see *Wallet registry* below)
| Tool | Description |
|------|-------------|
| `send_tokens` | Send tokens to an address |
| `execute_contract` | Execute a message on a CosmWasm contract |
| `upload_wasm` | Upload a WASM binary to a chain (see *`upload_wasm` security* below) |
| `instantiate_contract` | Instantiate a contract from a code ID |
| `migrate_contract` | Migrate a contract to a new code ID |
| `ibc_transfer` | Transfer tokens across Cosmos chains via IBC — the first cross-chain primitive for AI agents |
| `submit_blob` | Submit data blobs to Celestia DA layer ([celestiaorg/celestia-app](https://github.com/celestiaorg/celestia-app), Apache 2.0). Sovereign rollup data availability. |

#### Wallet registry (Ffern C-3 — mnemonic → `wallet_id`)

In the post-Ffern security release, every write tool takes an opaque `wallet_id` instead of a raw `mnemonic` parameter. The mnemonic itself never crosses the MCP transport, never appears in the model's tool-call JSON, and never lands in conversation logs.

**Two backends ship out of the box:**

| Backend | Where the per-wallet DEK lives | Operator setup |
|---------|-------------------------------|---------------|
| `passphrase` | Derived from `JUNOCLAW_WALLET_PASSPHRASE` via scrypt | Set the env var; portable across machines |
| `keychain` | OS credential manager (DPAPI / Keychain / libsecret) holds a random 32-byte DEK per wallet | No passphrase to manage; DEK bound to OS user session |

The keychain backend uses the optional native dependency [`@napi-rs/keyring`](https://www.npmjs.com/package/@napi-rs/keyring), installed by default with `npm install`. Use `npm install --no-optional` (or `--omit=optional`) to skip it; the passphrase backend keeps working in that case.

**Enrolment is one-time and out-of-band:**

```bash
# ── Option A: keychain backend (preferred when @napi-rs/keyring is available)
# No passphrase to set; the OS credential manager holds the DEK.
cosmos-mcp wallet add neo --chain juno-1 --backend keychain --mnemonic-stdin

# ── Option B: passphrase backend (portable across machines)
export JUNOCLAW_WALLET_PASSPHRASE='choose-a-strong-passphrase'
echo "<your 12 or 24 words>" | cosmos-mcp wallet add neo --chain juno-1 --backend passphrase

# ── Backend default (no --backend flag)
#   1. JUNOCLAW_WALLET_DEFAULT_BACKEND if set
#   2. keychain if no JUNOCLAW_WALLET_PASSPHRASE is set
#   3. otherwise passphrase
echo "<words>" | cosmos-mcp wallet add neo --chain juno-1   # uses default

# ── Either backend: pull mnemonic from an existing env var
cosmos-mcp wallet add wavs-op --chain uni-7 --mnemonic-env WAVS_OPERATOR_MNEMONIC

# List registered wallets (metadata + which backend protects each)
cosmos-mcp wallet list
# 2 wallet(s) (backends loaded: passphrase, keychain):
#   neo                       juno1...    prefix=juno  backend=keychain     created=2026-04-26T...
#   wavs-op                   juno1...    prefix=juno  backend=passphrase   created=2026-04-26T...

# Remove a wallet (clears both the .enc file and any keychain entry)
cosmos-mcp wallet rm neo
```

**MCP write tools then reference the wallet by id:**

```jsonc
// AI tool call (model never sees the mnemonic)
{
  "tool": "send_tokens",
  "chain_id": "juno-1",
  "wallet_id": "neo",
  "recipient": "juno1...",
  "amount": "1000000"
}
```

**On-disk layout** (default `~/.junoclaw/wallets/`, override with `JUNOCLAW_WALLET_ROOT`):

```
.keystore.json    — KDF parameters for the passphrase backend
                    (scrypt N=2^17 r=8 p=1, fresh 32-byte salt; absent for keychain-only stores)
<id>.enc          — per-wallet AES-256-GCM envelope, records `backend`,
                    fresh 12-byte IV per encrypt
```

Every wallet file records the backend that protects its DEK (`backend: "passphrase" | "keychain"`). The `WalletStore` dispatches to the right backend on decrypt, so a single store can hold a mix of both. Phase 1 files (no `backend` field) are read as `passphrase` for backward compatibility.

Every encrypt operation uses a fresh random IV, so two wallets with the same mnemonic still have distinct ciphertexts. Tampering with a `.enc` file, supplying the wrong passphrase, or revoking the keychain entry all raise a clear *decryption failed* error — GCM's authentication tag catches the mismatch.

**Environment:**

- `JUNOCLAW_WALLET_ROOT` — storage directory (default `~/.junoclaw/wallets`).
- `JUNOCLAW_WALLET_PASSPHRASE` — passphrase for the *passphrase* backend.
- `JUNOCLAW_WALLET_PASSPHRASE_FILE` — read passphrase from a file (no trailing newline).
- `JUNOCLAW_WALLET_DEFAULT_BACKEND` — `passphrase` or `keychain` to override the auto-selected default for `wallet add`.

**Threat model.** The keychain backend binds the DEK to the OS user session: on Windows, DPAPI prevents another user on the same machine from reading the entry; on macOS, Keychain Services prompts to authorise other apps. The passphrase backend instead trusts the operator to protect a long-lived passphrase. Pick whichever model fits your deployment.

Run the regression suites from `mcp/`:

```bash
npm run wallet-store-test       # 19 Phase 1 (passphrase) tests
npm run keychain-store-test     # 21 Phase 2 (keychain) tests, incl. real DPAPI/Keychain round-trip
```

Covers: round-trip add/list/remove, tamper detection, wrong-passphrase / revoked-keychain-entry rejection, path-traversal id rejection, invalid-mnemonic refusal, IV freshness, POSIX `0600` file mode, multi-backend dispatch, Phase 1 backward compatibility, and a real on-OS keychain round-trip (skipped on platforms without `@napi-rs/keyring`).

##### Migration from the pre-Ffern API

If you previously called the MCP write tools with a `mnemonic` parameter, those calls now fail schema validation. Two-step migration:

1. Export your existing mnemonic from wherever it lives today (env var, `.env`, password manager).
2. `cosmos-mcp wallet add <id> --chain <chainId>` to enrol it once.
3. Replace `mnemonic` with `wallet_id` in your prompts / agent configurations.

The deleted `getSigningClient(chain, mnemonic)` helper in `src/utils/cosmos-client.ts` is the canonical signal that the unsafe path is gone — there is no public API in `mcp/` that takes a raw mnemonic anymore.

#### `upload_wasm` security (Ffern C-4)

The `upload_wasm` tool takes a local `wasm_path` argument. To prevent path-traversal and symlink-exfiltration attacks (where a malicious or careless agent uploads the contents of `~/.ssh/id_rsa` on-chain as "wasm"), every path is validated before the signing wallet is even constructed:

- **Allow-root** — the file must live under `JUNOCLAW_WASM_ROOT` (default `~/.junoclaw/wasm`). Set `JUNOCLAW_WASM_ROOT=/path/to/your/wasm/dir` to relocate.
- **Symlink reject** — neither the file nor any parent directory may be a symlink; the check uses `lstat` on the leaf and `realpath` on the full path.
- **Size cap** — 8 MiB maximum (the practical ceiling for an optimised CosmWasm contract).
- **Magic bytes** — the file must start with `\0asm` (the wasm v1 magic `0x00 0x61 0x73 0x6d`).

If any check fails, the tool returns a clear error naming the cause. No bytes are read beyond what the size cap allows; no wallet is derived from the mnemonic if validation fails. Run `npm run path-guard-test` from `mcp/` to exercise the defenses against a sandbox fixture.

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

The MCP server is the most-exposed surface in the JunoClaw stack — it brokers between an LLM's tool calls and signed Cosmos transactions. We document each defense honestly.

- **Wallet handles, not raw mnemonics (Ffern C-3).** Every write tool takes a `wallet_id`. Mnemonics are enrolled out-of-band via the `cosmos-mcp wallet ...` CLI and encrypted at rest under a backend-specific 32-byte DEK — either an OS-keychain entry (Phase 2: DPAPI / macOS Keychain / libsecret) or a scrypt-derived master key (Phase 1: passphrase). See *Wallet registry* above.
- **Path-guarded `upload_wasm` (Ffern C-4).** Allow-root, symlink reject, 8 MiB cap, magic-byte check. See *`upload_wasm` security* above.
- **Read operations require no wallet.** Query tools take only addresses and chain ids; no key material is touched.
- **No persistent process state.** The MCP server holds no in-memory key beyond a single `signFor()` call; the decrypted mnemonic buffer is zeroed in `finally{}` after one signing client is built.
- **Out-of-process secret material.** The DEK source is always external to the Node process — either `JUNOCLAW_WALLET_PASSPHRASE` (or `_PASSPHRASE_FILE`) for the passphrase backend, or an OS-credential-manager entry for the keychain backend. The MCP server cannot decrypt anything without the appropriate operator-supplied input.

**Reporting:** see [`SECURITY.md`](../SECURITY.md) at the repo root.

## License

Apache-2.0 — same as Cosmos SDK, CosmWasm, and the tools we build on.
