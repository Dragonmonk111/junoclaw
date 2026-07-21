# Juno Becomes the First Chain Where AI Agents Can Discover, Query, and Safely Transact on Mainnet

*July 21, 2026 — The Cosmos MCP server is live on Juno mainnet. The skill-registry contract is deployed on juno-1. Any AI agent with an MCP client can now discover any dApp's operating manual on-chain, query chain state, and sign transactions — with a human confirmation gate on anything that moves funds. No other Cosmos chain has this.*

---

## The gap nobody had closed

AI agents can already read blockchain state. They can call RPC endpoints, parse JSON, and summarize governance proposals. What they can't do — or couldn't, until today — is:

1. **Discover how to interact with a dApp without a human telling them where to look.** An agent that wants to interact with Junoswap, or DAO DAO, or a prediction market contract needs to know the contract addresses, the message schemas, the gas parameters. Today that means a human finds the GitHub repo, reads the docs, and pastes context into the agent's prompt. That doesn't scale past the dApps you personally know about.

2. **Safely sign and broadcast transactions on mainnet.** An MCP server that can sign and broadcast is, by construction, a server that can move funds on an AI's say-so. That's fine for querying a balance. It's a different risk profile for `send_tokens`. Every agent-tooling experiment that shipped without safety gates either limited itself to testnet or accepted that a prompt-injected model could drain a wallet.

Both of these are now solved on Juno. Not on a testnet. On `juno-1`.

---

## What shipped

Two things, deployed together:

### 1. The skill-registry — on-chain dApp discovery

A CosmWasm contract deployed to Juno mainnet (`juno-1`):

| Field | Value |
|---|---|
| codeId | 5145 |
| Contract | `juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s` |
| Store tx | [`56F71E...`](https://mintscan.io/juno/tx/56F71E023795466E1A1035CA77D0F5BF7F9DD011E38392EF71C51890A75ECEED) |
| Instantiate tx | [`1457C5...`](https://mintscan.io/juno/tx/1457C5B4967F8ADB46DB86637AEAF3895AF0E441BFCF3C488A2F0CC7C56ACFD5) |
| Self-register tx | [`09E3A4...`](https://mintscan.io/juno/tx/09E3A46116DDA976629B757A382FDD78FD1D7BB715100355600FB670682FCD26) |

Any dApp on any chain can publish a pointer + SHA-256 hash of its operating manual (`SKILL.md`-equivalent) to this contract. Permissionless to publish. First-publisher owns the name. Admin can resolve disputes. No wallet needed to read it — any agent queries it via plain RPC.

JunoClaw self-registered `junoclaw-cosmos-mcp` as the first entry. Two MCP tools sit on top of it:

- `get_dapp_skill` — look up one dApp's manual pointer + hash by name
- `list_dapp_skills` — browse the registry, optionally filtered by chain

Any MCP client — Claude, Cursor, Windsurf, Cline, Zed, ChatGPT — can now discover and verify the manual for any registered dApp without a human already knowing where to look. The agent fetches the `skill_uri`, checks the SHA-256 against `skill_hash`, and treats the manual as authoritative. If someone tampers with the hosted file, the hash check fails. If someone squats a dApp name, the admin can resolve it. The discovery layer is on-chain, cryptographically anchored, and permissionless.

### 2. The second-approval gate — safe to hand to an AI on mainnet

Every fund-moving tool in the MCP — `send_tokens`, `execute_contract`, `upload_wasm`, `instantiate_contract`, `migrate_contract`, `ibc_transfer`, `submit_blob`, `vote_on_proposal`, `delegate_tokens`, `undelegate_tokens`, `redelegate_tokens`, `withdraw_rewards`, and `compose_and_broadcast_msg` — now stages its transaction instead of broadcasting on the first call.

The flow:

1. Agent calls `send_tokens` with amount, recipient, `wallet_id`
2. MCP server constructs the tx, returns a `confirmation_id` + human-readable preview of exactly what would be signed
3. Nothing broadcasts
4. Human reviews the preview, calls `confirm_transaction` with the `confirmation_id`
5. Tx broadcasts. Confirmation is single-use and expires in 5 minutes

Required by default. An operator who wants single-shot automation can opt out with one env var (`JUNOCLAW_SKIP_CONFIRMATION=1`). But the default assumes a human should see what's about to move before it moves.

This is the difference between "safe to test on testnet" and "safe to hand to an AI agent on mainnet." An agent that gets prompt-injected or hallucinates a bad transaction can stage it — but it can't broadcast it. The blast radius of a compromised agent is zero funds moved, one expired confirmation.

---

## How the deploy was done — key management as a feature

The mainnet deployment used the MCP's own encrypted WalletStore to manage the deployer key. No raw mnemonics in environment variables. No shell history. No `process.env.MNEMONIC`.

The flow:

1. The Builder wallet (`juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z`) was funded with 13 JUNO
2. A one-time enrollment script (`deploy/enroll-builder.mjs`) read the mnemonic from the existing `parliament-state.json`, enrolled it into the WalletStore via Windows DPAPI keychain backend, and scrubbed the plaintext from memory
3. The deploy script (`deploy/deploy-skill-registry.mjs`) loaded the wallet via `WalletStore.signFor()`, which decrypts the mnemonic inside the OS keyring, constructs a signing client, and zeroed the buffer in a `finally` block
4. Three transactions broadcast on `juno-1` — store, instantiate, self-register — all signed with a key that never existed in plaintext outside the OS credential manager

This is the same WalletStore that MCP users get when they run `cosmos-mcp wallet add <id>`. The deploy dogfooded the product. The key management that makes the MCP safe for agents is the same key management that made the deploy itself safe.

---

## What this unlocks — the strategic picture

### For dApp builders on Juno

Any dApp builder can now:

1. Write a `SKILL.md` — their dApp's operating manual for AI agents (message schemas, contract addresses, gas parameters, safety notes)
2. Call `publish_skill` on the mainnet skill-registry (permissionless, ~0.001 JUNO gas)
3. Any MCP-connected agent on any chain discovers the manual on-chain, verifies its hash, and knows how to interact

Before: dApps shipped docs on GitHub and hoped someone read them. After: dApps ship their manual on-chain and agents find it automatically. Juno is the first Cosmos chain where this is live.

### For governance participation

An AI agent with an MCP client can:

- Query active governance proposals across Juno (and any other registered chain)
- Read the proposal text, summarize it, assess implications
- Stage a vote transaction for human confirmation via the second-approval gate
- Track proposal status, voting periods, and outcomes in real time

This directly addresses the governance participation problem. Low voter turnout is an effort barrier problem. An agent that reads the proposal, summarizes it, and stages a vote for human review lowers the effort from "go to ping.pub, read the proposal, connect wallet, vote" to "review the agent's summary, call confirm_transaction."

### For cross-chain agent workflows

The MCP is chain-agnostic. Today it supports `juno-1`, `uni-7`, `osmosis-1`, `cosmoshub-4`, `celestia`, and any chain added to the registry. An agent can:

- Query balances across multiple chains in one session
- Execute an IBC transfer from Juno to Osmosis (staged, human-confirmed)
- Query a DAO proposal on Juno, then check the relevant treasury balance on Osmosis
- Discover dApp manuals registered on any chain that deploys a skill-registry instance

Juno is the first chain with the discovery layer live — but the pattern replicates. Any CosmWasm chain can deploy their own skill-registry and wire it into the MCP. The contract is open-source, tested, and Apache-2.0.

### For the WAVS / TEE roadmap

The closeout plan has the WAVS sealed signer ready for TEE deployment (A034 proposal finalized, GCP Confidential VM deployment plan ready). Once the TEE is live:

- The sealed signer holds keys inside hardware-attested enclave
- The MCP's WalletStore can be pointed at the WAVS invoke endpoint instead of local encrypted files
- Agents sign transactions with keys that never existed in plaintext outside the TEE
- The second-approval gate becomes the human-in-the-loop checkpoint on top of hardware-grade key isolation

This is the path to autonomous agent operations — not fully autonomous (the gate still requires human confirmation for fund moves), but with key material protected by hardware attestation rather than OS-level encryption.

### For prediction markets

Jake Hartnell's prediction market repo (`CosmosContracts/pm` — `cw-reality`, `binary-market`, `market-factory`) plus JunoClaw's template #9 (Verifiable Outcome Market) with `OutcomeCreate`/`OutcomeResolve` proposal kinds:

- An MCP-connected agent can query active prediction markets, check resolution status, and stage trades
- The skill-registry can publish manuals for both Jake's market contracts and JunoClaw's outcome resolution system
- WAVS-attested TEE resolution + the MCP's second-approval gate = agents can participate in prediction markets with human confirmation on fund-moving steps

---

## The 28 tools, categorized

| Category | Tools | Wallet needed? |
|---|---|---|
| **Query (read-only)** | `query_balance`, `query_all_balances`, `query_contract`, `query_contract_info`, `query_tx`, `query_block_height`, `list_chains`, `list_ibc_channels`, `query_dao_proposals`, `query_mesh_security`, `get_dapp_skill`, `list_dapp_skills` | No |
| **Write (fund-moving, gated)** | `send_tokens`, `execute_contract`, `upload_wasm`, `instantiate_contract`, `migrate_contract`, `ibc_transfer`, `submit_blob`, `vote_on_proposal`, `delegate_tokens`, `undelegate_tokens`, `redelegate_tokens`, `withdraw_rewards` | Yes + confirmation |
| **Generic composer** | `compose_and_broadcast_msg` (any Cosmos SDK message type, disabled by default) | Yes + confirmation + operator allowlist |
| **Confirmation** | `confirm_transaction` (redeems staged tx) | Yes |

Every tool follows the same posture: fail closed, narrow the blast radius, make the safe path the default path.

---

## What needs to happen next

1. **Juno v30 mainnet upgrade** — unblocks BN254 precompile → full JunoClaw contract suite on mainnet → agents can interact with zk-verifier, attestation flows, prediction markets. The binary is built, testnet upgrade is live, staging on the mainnet node is the next step.

2. **A034 submission** — funds TEE infrastructure → sealed signer live → hardware-grade key isolation for agent signing. The proposal is finalized and ready for submission.

3. **More dApps register on the skill-registry** — each registration makes the discovery layer more useful. Jake's PM contracts, DAO DAO, Junoswap — each can publish a `SKILL.md`. Permissionless, ~0.001 JUNO per entry.

4. **Cross-chain skill-registry deployments** — Osmosis, Stargaze, or any CosmWasm chain deploying their own instance creates an interchain agent discovery network. The contract is open-source and replicable.

5. **Reece bot coordination** — Reece is already operational on Juno mainnet with ~928 JUNO and a governance mandate. The MCP + skill-registry gives Reece (and agents like it) a standardized way to discover and interact with dApps. Coordination avoids duplicate governance coverage.

---

## Verify it yourself

Every claim is on-chain:

| What | Where |
|---|---|
| Skill-registry contract | `juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s` on juno-1 |
| Store tx | [mintscan.io/juno/tx/56F71E...](https://mintscan.io/juno/tx/56F71E023795466E1A1035CA77D0F5BF7F9DD011E38392EF71C51890A75ECEED) |
| Instantiate tx | [mintscan.io/juno/tx/1457C5...](https://mintscan.io/juno/tx/1457C5B4967F8ADB46DB86637AEAF3895AF0E441BFCF3C488A2F0CC7C56ACFD5) |
| Self-register tx | [mintscan.io/juno/tx/09E3A4...](https://mintscan.io/juno/tx/09E3A46116DDA976629B757A382FDD78FD1D7BB715100355600FB670682FCD26) |
| Testnet (uni-7) | codeId 82, `juno1pug0zu6f93nmvjl559s0uymr92jhmn5t76p7knh9zg4sqlpygqyq0nn8gz` |
| MCP server | `npm install @junoclaw/cosmos-mcp` |
| Source code | [github.com/Dragonmonk111/junoclaw/tree/main/mcp](https://github.com/Dragonmonk111/junoclaw/tree/main/mcp) |
| Skill-registry contract | [github.com/Dragonmonk111/junoclaw/tree/main/contracts/skill-registry](https://github.com/Dragonmonk111/junoclaw/tree/main/contracts/skill-registry) |
| Deploy script | [github.com/Dragonmonk111/junoclaw/blob/main/deploy/deploy-skill-registry.mjs](https://github.com/Dragonmonk111/junoclaw/blob/main/deploy/deploy-skill-registry.mjs) |

Query the registry yourself — no wallet needed:

```bash
junod query wasm contract-state smart \
  juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s \
  '{"get_skill":{"dapp_name":"junoclaw-cosmos-mcp"}}'
```

---

## Links

| Resource | |
|---|---|
| **This article on Medium** | [Juno Becomes the First Chain Where AI Agents Can Discover, Query, and Safely Transact on Mainnet](https://medium.com/@tj.yamlajatt/juno-becomes-the-first-chain-where-ai-agents-can-discover-query-and-safely-transact-on-mainnet-f757ef3a691e) |
| GitHub | [Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) |
| MCP install | `npm install @junoclaw/cosmos-mcp` |
| Quickstart | [mcp/QUICKSTART.md](https://github.com/Dragonmonk111/junoclaw/blob/main/mcp/QUICKSTART.md) |
| Previous articles | [JunoClaw at v30 — The Receipt](https://medium.com/@tj.yamlajatt) · [JunoClaw Is Now Part of Juno](https://medium.com/@tj.yamlajatt) · [Trust Without Trustees](https://medium.com/@tj.yamlajatt/trust-without-trustees-72174b7659a2) · [8 Billion Agents](https://medium.com/@tj.yamlajatt/the-final-bosses-of-cosmos-how-we-built-an-ai-agent-layer-that-scales-to-8-billion-3298a5b17be5) |

---

*Apache-2.0. VairagyaNodes / Dragonmonk111. 2026-07-21.*

*Juno is the first Cosmos chain where an AI agent can discover any dApp's manual on-chain, query chain state, and safely transact on mainnet — with a human confirmation gate on anything that moves funds. The infrastructure is live. The registry is open. The first entry is published. What follows is the ecosystem.*

*If you're building a dApp on Juno and want it agent-discoverable, register your manual. If you're building AI agents and want them on sovereign infrastructure, install the MCP. The skill spec is open. The contract is deployed. The gate is on by default.*
