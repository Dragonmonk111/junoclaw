# Lesson 3 — Verify with your own eyes

**Goal:** the posture that defines this chain — never trust a claim you
can't check, and learn which checks are free.

## The rule (5 min)

Every read in this lesson costs nothing and requires no wallet. If a
claim can't be checked with a query, it shouldn't be believed — that
applies to the chain, to this course, and to your agent's future work.

## Look at the chain (25 min)

With your MCP client:

1. `query_block_height` — the chain's heartbeat.
2. `query_all_balances` on your own address — your account, on-chain.
3. `query_contract` on the agent-registry `{ "get_config": {} }` —
   read the economy's own config: the registration fee, the denom,
   and the `registry` map that wires task-ledger and escrow together.
4. Follow the map: `query_contract` on task-ledger `{ "get_config": {} }`
   and truth-market `{ "get_config": {} }` — see the real parameters
   (min stake, slash %, reward %).
5. `query_contract_info` on any of those addresses — the deployer's
   label, code id, admin — provenance you can inspect.

Each answer is a piece of state a quorum signed. You are not reading a
database someone controls — you're reading a certified record.

## Prove it worked

Write down the agent-registry address and its registration fee from
your own query — don't copy it from the docs. Compare with a neighbor:
if your reads match, you're both looking at the same certified state.

## Homework

Find the skill-registry contract and query `{ "get_config": {} }`.
Then try `{ "get_skill": { "dapp_name": "junoclaw-cosmos-mcp" } }` —
the chain teaches agents how to use it. What does the entry point at?
