# The On-Chain Skill Registry — and Cosmos MCP Hits Production

*Short recap for anyone tracking the Cosmos MCP server. Two things shipped together: a permissionless on-chain directory of dApp manuals, and the second-approval gate that makes the MCP safe to hand to an AI agent for real.*

---

## The problem: how does an AI agent find a dApp's manual?

An MCP client can query any Cosmos chain, sign transactions, execute contracts. But it still needs to *know* how to talk to a specific dApp — what messages it expects, what its contract address is, how it works. Today that means a human already knows the GitHub URL and pastes a `SKILL.md` into context by hand. That doesn't scale past the dApps you personally know about.

## The fix: `skill-registry`

A small CosmWasm contract. Deployed to `uni-7` testnet (`codeId 82`) and `juno-1` mainnet (`codeId 5145`, address `juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s`): any dApp publishes a pointer + SHA-256 hash of its own operating manual. Permissionless to publish, first-publisher owns the name, admin can resolve disputes. No wallet needed to read it.

Two new MCP tools ship on top of it:

- `get_dapp_skill` — look up one dApp's manual pointer + hash by name.
- `list_dapp_skills` — browse the registry, optionally filtered by chain.

JunoClaw self-registered as the first entry. Any MCP client, on any chain, can now discover and verify the manual for any registered dApp — without a human already knowing where to look.

## The other half: making the MCP safe to actually use

An MCP server that can sign and broadcast is, by construction, a server that can move funds on an AI's say-so. That's fine for querying a balance. It's a different risk profile for `send_tokens`.

So every fund-moving tool — sends, staking, IBC transfers, funded contract calls, the generic message composer — now stages its transaction instead of broadcasting on the first call. The tool returns a `confirmation_id` and a human-readable preview of exactly what would be signed. Nothing happens until a second, separate `confirm_transaction` call redeems that id. Confirmations are single-use and expire in five minutes.

Required by default. An operator who wants single-shot automation can opt out with one env var — but the default assumes a human should see what's about to move before it moves.

## Where this lands

28 tools total: queries, writes, the skill registry, staking, IBC, contract lifecycle, generic message composition — each gated the same way everything else in this server is gated: fail closed, narrow the blast radius, make the safe path the default path.

Mainnet deployment funded by The Builder wallet (`juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z`), enrolled via the MCP's own encrypted WalletStore (Windows DPAPI keychain backend) — no raw mnemonics in env vars or shell history.

**Mainnet txs:**
- Store: [`56F71E...`](https://mintscan.io/juno/tx/56F71E023795466E1A1035CA77D0F5BF7F9DD011E38392EF71C51890A75ECEED) → codeId 5145
- Instantiate: [`1457C5...`](https://mintscan.io/juno/tx/1457C5B4967F8ADB46DB86637AEAF3895AF0E441BFCF3C488A2F0CC7C56ACFD5) → `juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s`
- Self-register: [`09E3A4...`](https://mintscan.io/juno/tx/09E3A46116DDA976629B757A382FDD78FD1D7BB715100355600FB670682FCD26) → `junoclaw-cosmos-mcp` skill entry

Repo: `github.com/Dragonmonk111/junoclaw/tree/main/mcp`. Install: `npm install @junoclaw/cosmos-mcp`.
