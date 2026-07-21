# DM to Rattadan — Skill-Registry Mainnet Live

*Re: Rattadan's question about deploying to mainnet since testnet faucets are dead and no JUNOX available.*

---

Hey Rattadan,

Good news on the mainnet question — skill-registry is now **live on Juno mainnet** (`juno-1`):

- **codeId**: 5145
- **Contract**: `juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s`
- **First entry**: `junoclaw-cosmos-mcp` (self-registered)

Txs on mintscan:
- Store: `56F71E023795466E1A1035CA77D0F5BF7F9DD011E38392EF71C51890A75ECEED`
- Instantiate: `1457C5B4967F8ADB46DB86637AEAF3895AF0E441BFCF3C488A2F0CC7C56ACFD5`
- Self-register: `09E3A46116DDA976629B757A382FDD78FD1D7BB715100355600FB670682FCD26`

The Builder wallet got funded with 13 JUNO — that was enough for all three txs. Deploy used the MCP's own encrypted WalletStore (Windows DPAPI keychain backend), so no raw mnemonics in env vars or shell history. The deploy script is patched for mainnet safety: correct denom (`ujuno`), mainnet RPC, zero-balance guard, and a confirmation banner.

Testnet (`uni-7`) deployment is still live too (codeId 82, same contract address pattern) — so you can test against either chain. The MCP tools (`get_dapp_skill`, `list_dapp_skills`) work against both.

If you want to register your own dApp's manual on mainnet, just call `publish_skill` on the contract with your `dapp_name`, `skill_uri`, `skill_hash`, and `chain_id`. Permissionless — no admin approval needed, first-publisher owns the name.

Repo: `github.com/Dragonmonk111/junoclaw` — contract at `contracts/skill-registry`, MCP at `/mcp`.
