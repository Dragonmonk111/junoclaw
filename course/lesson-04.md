# Lesson 4 — Give your agent an identity

**Goal:** first write on-chain — your agent becomes a real, queryable
entity with a work history.

## The concept (5 min)

`agent-registry` is the identity layer. Registering writes a profile:
name, description, capabilities hash, model — bound to your address
forever. It costs a registration fee (read the live value with
`get_config` — currently 1,000,000 ujclaw).

## Do it (20 min)

With your `wallet_id`:

```
execute_contract:
  chain: junoclaw-g1
  contract: <agent-registry address>
  msg: {
    "register_agent": {
      "name": "<your agent's name>",
      "description": "<the 3 sentences from lesson 1>",
      "capabilities_hash": "<sha256 of a short capabilities file>",
      "model": "<model you run>"
    }
  }
  funds: [{ "denom": "ujclaw", "amount": "1000000" }]
```

Gas is `"auto"` — the chain simulates the tx first and prices it
correctly. The execute returns a **tx hash — save it.**

## Prove it worked — two ways

1. `query_contract` → `{ "get_agents_by_owner": { "owner": "<you>" } }`
   — your profile comes back: active, trust_score 0, registered_at.
2. `query_tx` on your saved hash — `code: 0` at a committed height.

Both are receipts. Your agent now exists where anyone can check it.

## Discuss

Why does registration cost money? (Spam resistance + skin in the game —
a registered agent has value at stake before it's trusted.)

## Homework

`query_contract` → `{ "get_agent_stats": {} }` (or the stats variant
the contract answers) — how many agents are registered on the chain now?
Find classmates' agents by their owner addresses.
