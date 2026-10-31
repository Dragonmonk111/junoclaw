# Instructor checklist — arrange before lesson 1

## Access

- [ ] gRPC endpoint for the class: `JUNOCLAW_GRPC_ENDPOINT`
      (default `51.195.201.74:9090`, operator-restricted — request
      classroom access from the JunoClaw team ahead of time)
- [ ] OPTIONAL, worth it: a second, *independent* verification source.
      Two tiers, in order of strength:
      (a) an independent observer's JSON API exposing retained reads +
          certificate evidence — compare their verified cert/state-root
          at height H against what the student's own gRPC read returned
          (upstream-labelled archive data agreeing is expected; an
          independently-verified commitment matching is the lesson);
      (b) a second native node gRPC endpoint (observer seat with donor
          access) — the true "two operators" read for lesson 2/7.
- [ ] One funded test wallet per student (recommended
      ~2,000,000 ujclaw — covers the 1 JCLAW agent registration fee
      + gas with headroom). Ask the operator team; they run the faucet.
- [ ] Per-student `wallet_id` enrolled via
      `cosmos-mcp wallet add <id> --chain junoclaw-g1` — students never
      see the mnemonic. That is the security model, teach it as a feature.
      ⚠ The `--chain` flag is mandatory: without it the wallet gets a
      `cosmos1...` prefix and can't sign on this chain. Same for
      `wallet exec` — the juno prefix auto-infers to `juno-1` mainnet,
      so always pass `--chain junoclaw-g1` explicitly.

## Tooling (per student machine, or one shared lab machine)

- [ ] Node.js 18+ and npm
- [ ] The MCP server: `npm i -g @junoclaw/cosmos-mcp` (or build from
      `mcp/` in the repo) — verify with `list_chains`
- [ ] An MCP-capable client (Windsurf, Claude Desktop, Cursor — any)

## Environment

- [ ] `JUNOCLAW_GRPC_ENDPOINT` set to the classroom endpoint
- [ ] `JUNOCLAW_WALLET_PASSPHRASE` set IF the enrolled wallets use the
      passphrase backend (keychain-backend wallets need nothing)
- [ ] `JUNOCLAW_SIGNING_PAUSED` unset (or "0") — the kill-switch is a
      deliberate lever; demonstrate it in lesson 5 if you want

## Before lesson 5 (the "earn" workshop)

- [ ] A small funded task posted to task-ledger + escrow per student —
      students can't fund their own first task; the requester role is
      yours. Verify each lock shows in escrow before class starts.

## Know before you teach

- Chain ID is `junoclaw-g1` — a tx signed for `junoclaw-1` fails
  `InvalidSignature`. Every config in this pack already uses g1.
- Every account reports `accountNumber = 17` — that is the chain's
  design (fixed account number), not a bug.
- `query_tx` only works with a hash you already have — there is no
  tx-search endpoint yet. Teach students to save tx hashes.
- Fee facts: agent registration = 1,000,000 ujclaw; truth-market
  minimum stake = 1,000,000 ujclaw; gas ≈ tens of ujclaw per tx.
