---
title: "Lesson 3: Give Your Agent an On-Chain Identity"
type: lesson
difficulty: beginner
tags: [agent-registry, execute, identity, first-write, junoclaw]
lesson_number: 3
---

# Lesson 3: Give Your Agent an On-Chain Identity

**Duration:** 90 minutes
**Format:** Practice
**Result:** your agent exists on-chain — a profile anyone in the world
can query. Your first signed transaction.

## Prerequisites

- Lesson 2 done
- Your wallet funded with **≥ ~1,100,000 ujclaw** (registration fee
  1,000,000 + gas). Check first:

```
What's my ujclaw balance on junoclaw-g1?
```

If it's short — tell the instructor before continuing.

---

## The idea (60 seconds)

`agent-registry` is the identity layer. Registering writes a profile —
name, description, capabilities hash, model — bound to your address
forever. It costs a fee (you read the live value yourself in lesson 2).
The fee does two jobs: spam resistance, and skin in the game — an
agent with value at stake is an agent that can start being trusted.

---

## Part 1: Write your agent's profile

**Why:** the description you write now is your agent's public
first impression — it stays on-chain. Three honest sentences: what it
does, who pays it, how a stranger verifies the work.

Draft it with your AI:

```
Help me write a 3-sentence on-chain profile for my agent:
what it does, who pays it, and how a stranger can verify the work.
Keep it under 280 characters.
```

Save the result — you'll paste it into the registration in a moment.

Also pick: a `name` (unique-ish, lowercase-hyphen style) and the
`model` your agent runs (e.g. the AI you're talking to right now).

---

## Part 2: Register — your first write

**Why:** this is a real transaction: your key signs, the chain
validates, state changes forever. Watch each stage happen.

Paste (fill the `<…>`):

```
Execute a contract call on junoclaw-g1 using my wallet_id.

Contract: juno1n7fx4duuy59dvscmwrvhqss74heaectwjyac8gn5w2w637l2dxmqy9jgwl
Message:
{ "register_agent": {
    "name": "<your-agent-name>",
    "description": "<your 3 sentences>",
    "capabilities_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "model": "<model-id>" } }
Funds: 1000000 ujclaw
Fee: auto

Tell me the transaction hash when it commits.
```

*(The capabilities_hash shown is the sha256 of an empty file — fine for
the course. Later you'd hash a real capabilities document.)*

**Save the tx hash** the AI returns — it IS your receipt. Ask it to
append the hash to `my-verification-log.md`.

What just happened, under the hood: the MCP simulated your tx for gas,
decrypted your key inside the signing boundary, signed a standard
SignDoc, broadcast, and polled until committed. You saw none of that —
you just saw a hash come back.

## Where your key lives — the sealed signer (read this before moving on)

Your `wallet_id` is not a key. It's a **handle to a key that stays
sealed**:

- Key material sits encrypted in the wallet store. It is decrypted
  inside the signing boundary **only at the moment of signing**, and it
  never reaches the AI, the chat, or the logs. Your AI can spend —
  it cannot *see* the secret it spends with.
- **Deterministic by design:** the same `wallet_id` always resolves to
  the same key, so your `juno1…` address is reproducible, not random.
  Anyone verifying your agent's receipts is always verifying the same
  identity.
- **Deliberate, not ambient:** signing is a discrete act with a kill
  switch (`JUNOCLAW_SIGNING_PAUSED`). A paused signer returns errors —
  it never signs "just in case".
- **The rule that survives graduation:** never paste a mnemonic, seed,
  or private key anywhere — not to an AI, not into a form, not to a
  "support" DM. Anything that asks for raw key material is asking you
  to break the model. Legitimate tools work with handles and receipts,
  never secrets.
- **Where secrets DO live:** `.env` files and secret stores — referenced
  by name, never by value. Keys, API tokens, and seeds go in `.env`
  (gitignored, mode 600) and get injected as environment variables;
  they never appear in a chat, a commit, a command line you paste to
  anyone, or a screenshot. If you can see the secret in a message, the
  flow is already broken.

---

## Part 3: Prove it — two independent ways

**Why:** a claim isn't real until you verify it. Verify yours twice.

**Check 1 — the registry knows you:**

```
Query agent-registry on junoclaw-g1 with:
{ "get_agents_by_owner": { "owner": "<your-address>" } }
Show me my agent's profile.
```

Expected: your name, description, `active: true`, `trust_score: 0`,
`registered_at` set. Trust score zero is correct — history is earned,
not granted. That's tomorrow's work.

**Check 2 — the transaction receipt:**

```
Query the transaction <your-tx-hash> on junoclaw-g1.
Confirm code is 0 and tell me the height it committed at.
```

Expected: `code: 0`, a real height. Two receipts, both public, both
yours.

Append the profile JSON to `my-verification-log.md`.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `insufficient funds` | Wallet short | Instructor: top up to ≥ ~1.05 JCLAW |
| `name already taken` | Someone has it | Pick a different agent name |
| `InvalidSignature` | Wrong chain_id | Should be impossible via the tools — flag instructor |
| tx "not found" on query_tx | Indexing lag | Wait ~30s, re-query the same hash |

## Homework

```
Query agent-registry with { "get_agent_stats": {} } — if that variant
isn't supported, read the error message: it lists the real variant
names. Try them until stats come back. How many agents are registered?
```

Then find a classmate's agent via their owner address — their profile
is as public as yours now.
