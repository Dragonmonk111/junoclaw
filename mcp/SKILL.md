# SKILL.md — Cosmos MCP (JunoClaw)

> Operating manual for AI agents. Published on-chain via the `skill-registry`
> contract on Juno (`dapp_name: "junoclaw-cosmos-mcp"`) so any MCP-aware
> agent can discover and load this manual without prior knowledge of the
> GitHub URL. Canonical source: `github.com/Dragonmonk111/junoclaw/mcp/`.

## What this is

An MCP server exposing 28 tools for querying and transacting on any Cosmos
SDK chain (Juno, Osmosis, Cosmos Hub, Celestia, and any chain you add to
`mcp/src/resources/chains.ts`). Full reference: `mcp/README.md`.
Install path: `mcp/QUICKSTART.md`.

## How to authenticate as an agent

Every write tool takes an opaque `wallet_id`, never a mnemonic. The
mnemonic is enrolled out-of-band by the human operator via
`cosmos-mcp wallet add <id>` and encrypted at rest — it never crosses the
MCP transport, never appears in a tool-call, never lands in a log. If you
are an agent reading this manual: you cannot self-enroll a wallet. Ask
your operator to run the CLI, then use the `wallet_id` they give you.

## Query tools (no wallet, always safe)

`query_balance`, `query_all_balances`, `query_contract`,
`query_contract_info`, `query_tx`, `query_block_height`,
`list_chains`, `list_ibc_channels`, `query_dao_proposals`,
`query_mesh_security`, and more — see `mcp/README.md` "Query Tools" table.

## Write tools (require `wallet_id`)

`send_tokens`, `execute_contract`, `upload_wasm`, `instantiate_contract`,
`migrate_contract`, `ibc_transfer`, `submit_blob`, `vote_on_proposal`,
`delegate_tokens`, `undelegate_tokens`, `redelegate_tokens`,
`withdraw_rewards`.

## Anything not covered by a named tool

`compose_and_broadcast_msg` accepts any Cosmos SDK message type URL + JSON
value — but it is **disabled by default**. It will refuse every call
unless the human operator has explicitly set `JUNOCLAW_ALLOWED_MSG_TYPES`
to include that exact type URL. If you hit this refusal, tell your
operator which type URL you need and why; do not attempt to route around
it via a different tool.

## Safety properties you can rely on

- Query tools touch no key material — always safe to call freely.
- Write tools require a pre-registered `wallet_id` — you cannot invent one.
- `compose_and_broadcast_msg` fails closed by default (see above).
- A runtime kill-switch (`JUNOCLAW_SIGNING_PAUSED`) can freeze all signing
  server-side without affecting your ability to still read chain state.
- `upload_wasm` is path-guarded (allow-root, no symlinks, 8 MiB cap,
  magic-byte check) — it cannot be used to read arbitrary host files.

Full threat model and defense-in-depth rationale: `mcp/README.md`
"Security" section.

## On-chain skill registry (this file's distribution mechanism)

This file is registered on Juno's `skill-registry` contract:

```
QueryMsg::GetSkill { dapp_name: "junoclaw-cosmos-mcp" }
```

returns `{ skill_uri, skill_hash, chain_id, publisher, version }`. Fetch
`skill_uri`, verify its SHA-256 against `skill_hash`, then treat this
manual as authoritative. Any other dApp on any chain can register the same
way — see `drafts/PLAN_ONCHAIN_SKILL_REGISTRY.md` in the main repo.
