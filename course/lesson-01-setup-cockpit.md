---
title: "Lesson 1: Your Cockpit — GitHub, IDE, AI, and the JunoClaw MCP"
type: lesson
difficulty: beginner
tags: [github, ide, mcp, wallet, setup, junoclaw]
lesson_number: 1
---

# Lesson 1: Your Cockpit — GitHub, IDE, AI, and the JunoClaw MCP

**Duration:** 2 hours
**Format:** Theory + Practice
**Result:** your AI can talk to a live blockchain — and you can prove it.

## Prerequisites

1. **Computer** (Windows / Mac / Linux) + internet
2. **Email account** — needed for registrations
3. **GitHub account** — sign up at [github.com](https://github.com) (free)
4. **An AI account** — Claude or any provider your IDE supports
5. **A `wallet_id` and address from your instructor** — your key already
   lives encrypted inside the MCP's wallet store. You will never see a
   mnemonic, and you don't need one — that is the security model, not a
   shortcut. (Instructors: see `instructor-checklist.md`.)

---

## What you're building today

```
You ──type──> AI (in your IDE) ──MCP──> JunoClaw blockchain
```

One program — the **MCP server** — sits between your AI and the chain.
It gives your AI 28 tools: read balances, query contracts, sign and
send transactions. You never touch crypto plumbing; your AI does.

---

## Part 1: Install Node.js (the engine under the MCP)

**Why:** the MCP server is a Node program. No Node, no tools.

Open a terminal (PowerShell on Windows) and ask your AI — or run
directly:

```bash
node --version
```

- If it prints `v18.x` or higher → done, skip to Part 2.
- If "command not found" → install from [nodejs.org](https://nodejs.org)
  (LTS version, default options).

> **⚠️ Windows:** if installs fail later with permission errors,
> reopen PowerShell via right-click → **Run as administrator**.

---

## Part 2: Install your IDE + connect your AI

**Why:** the IDE is where you talk to the AI. Any MCP-capable client
works; we use **Windsurf** as the reference path.

1. Download Windsurf: [windsurf.com](https://windsurf.com) → install
   like a normal program.
2. Sign in / connect your AI provider account inside it.
3. Open the AI panel and paste:

```
Hi! Explain in simple terms what you can do, and what "MCP tools"
means. Keep it under 100 words.
```

You should get a real answer. If you can chat, your AI connection works.

*(Alternatives: Claude Desktop or Cursor — config format in
`mcp/QUICKSTART.md`. The lessons work identically.)*

---

## Part 3: Install the JunoClaw MCP server

**Why:** this is the bridge. One install gives your AI the whole chain.

In your terminal:

```bash
npm install -g @junoclaw/cosmos-mcp
```

Or, if you're working from the repository clone:

```bash
cd junoclaw/mcp
npm install
npm run build
```

### Point it at the chain

Set the endpoint your instructor gave you (classroom default shown):

```bash
# Windows PowerShell:
$env:JUNOCLAW_GRPC_ENDPOINT="51.195.201.74:9090"

# macOS/Linux:
export JUNOCLAW_GRPC_ENDPOINT=51.195.201.74:9090
```

### Register it in your IDE

In Windsurf: **Settings → MCP servers** (or `mcp_config.json`) and add:

```json
{
  "mcpServers": {
    "junoclaw": {
      "command": "cosmos-mcp",
      "env": {
        "JUNOCLAW_GRPC_ENDPOINT": "51.195.201.74:9090"
      }
    }
  }
}
```

Restart the IDE after saving.

---

## Part 4: First contact

**Why:** everything else in this course rides on these three calls
working.

In your IDE's AI panel, paste **exactly**:

```
List the chains you can talk to.
```

Expected: a table including `junoclaw-g1` with transport
`junoclaw-grpc`. That single line is the sovereign transport working.

Then:

```
What is the current block height on junoclaw-g1?
```

Expected: a number around several million — and if you ask again a
minute later, a *bigger* number. That's a live chain finalizing blocks.

Then:

```
What wallets are enrolled, and what is the address of my wallet_id?
```

Expected: your wallet_id with a `juno1…` address. (If none appear,
tell the instructor — your wallet isn't enrolled yet.)

---

## Part 5: The security model, in two minutes

Read this once — it's why this whole setup is safe:

- Your key's mnemonic is encrypted at rest inside the MCP wallet store.
- It is decrypted **only** inside the signing step, used, and wiped.
- No tool call, no AI prompt, no log ever contains it.
- Every write command needs your `wallet_id` — an opaque handle, not a
  key.
- Reads (balances, contracts, height) need nothing and cost nothing.

An agent that can't leak its key is an agent you can trust with money.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Queries hang ~15s then fail | Endpoint filtered / wrong IP | Check `JUNOCLAW_GRPC_ENDPOINT`; ask instructor to allow your IP |
| `junoclaw-g1` not in list | MCP server not registered | Re-check `mcp_config.json`, restart IDE |
| `wallet_id not found` | Not enrolled | Instructor runs `cosmos-mcp wallet add <id> --chain junoclaw-g1` |
| Balance shows 0 ujclaw | Not funded | Instructor requests faucet/funding |

## Homework

Paste into your AI panel:

```
Show me every tool you have for junoclaw-g1, grouped into
"reads" (free) and "writes" (needs wallet_id).
```

Save the answer — you'll use it as your map for the rest of the course.
