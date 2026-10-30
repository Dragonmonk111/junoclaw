# Lesson 2 — Your cockpit: the MCP + your wallet

**Goal:** the student can drive the chain through natural language —
and understands why their key is never in the loop.

## The security model (10 min, teach it explicitly)

Your AI talks to the chain through an MCP server — 28 tools. Writes
require a `wallet_id`: an opaque handle to a key enrolled by the
operator (you, or your instructor) via a CLI. The key's mnemonic is
encrypted at rest, decrypted only inside the signing step, and scrubbed
immediately. **No tool call, log, or AI prompt ever contains the key.**
This isn't a nicety — it's the difference between an agent that can be
trusted with money and one that can't.

## Setup (20 min)

```bash
npm i -g @junoclaw/cosmos-mcp        # or run from mcp/ source
export JUNOCLAW_GRPC_ENDPOINT=51.195.201.74:9090   # classroom endpoint

# operator step — enrolls a wallet. It prompts for the mnemonic on stdin,
# encrypts it, and never shows it again. --chain sets the juno prefix —
# WITHOUT it the wallet gets a cosmos1... address and can't sign here.
cosmos-mcp wallet add my-agent --chain junoclaw-g1
cosmos-mcp wallet list               # verify: prefix=juno
```

Point your MCP client at the server (see `mcp/QUICKSTART.md` for
Windsurf/Claude/Cursor configs).

## First call (10 min)

Ask your AI: *"list the chains you can talk to."* Expect `junoclaw-g1`
in the list — that single entry is the sovereign transport working.
Then: *"what's my wallet address?"* — the address is public, sharing it
is safe; the key behind it isn't.

## Prove it worked

`query_block_height` on `junoclaw-g1` returns a live number. You are
reading the chain through your own agent cockpit.

## Homework

Find your `wallet_id`'s address and look it up with `query_balance`.
What's the denom? How much do you have? (If 0 — flag the instructor;
faucet comes in lesson 4.)
