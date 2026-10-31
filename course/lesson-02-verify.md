---
title: "Lesson 2: Verify With Your Own Eyes"
type: lesson
difficulty: beginner
tags: [queries, contracts, verification, junoclaw]
lesson_number: 2
---

# Lesson 2: Verify With Your Own Eyes

**Duration:** 90 minutes
**Format:** Practice
**Result:** you've read the live economy's real config from the chain —
with zero trust in anyone's docs, including ours.

## Prerequisites

- Lesson 1 done: `list_chains` shows `junoclaw-g1`, block-height query works
- Your wallet address (from lesson 1, Part 4)

---

## The rule for the whole course

Every read is **free** and needs **no wallet**. If a claim can't be
checked with a query, it isn't believed — that applies to this chain,
to this course, and to your agent's future work.

---

## Part 1: The chain's heartbeat

Paste into your AI panel:

```
Query the block height on junoclaw-g1. Tell me the number.
```

Wait one minute, then:

```
Query it again.
```

The number increased. You're watching a BFT chain finalize blocks under
threshold certificates in real time.

**Write it down:** ask your AI to append both heights to a file
`my-verification-log.md` in your projects folder. This file becomes
your evidence trail for the whole course.

---

## Part 2: Your own account, on-chain

```
Show me all balances for my wallet address on junoclaw-g1.
```

Expected: your denom `ujclaw` and an amount. That's *your* account as
the certified ledger sees it — not a dashboard's version of it.

Append the result to `my-verification-log.md`.

---

## Part 3: Read the economy's own config

**Why:** the agent economy on JunoClaw is a set of smart contracts.
Each has a `get_config` query that returns its live parameters. You're
about to read the real rulebook — not a docs page about it.

The contract addresses you'll need (they're also in `mcp/SKILL.md`):

| contract | address |
|---|---|
| agent-registry | `juno1n7fx4duuy59dvscmwrvhqss74heaectwjyac8gn5w2w637l2dxmqy9jgwl` |
| task-ledger | `juno1ah3hkymcsr7nzm2sjpapl0r3yd72r9pekg723tdvr0lxe4vzacmqrm778u` |
| escrow | `juno15xg0w67jehudu4cmrsslm6cxkfyemwjf99728334jwj9nk3thaksr8gprp` |
| truth-market | `juno1y4zmmggk6awzrnyfpjyvcm8ggygc6scewuy9efp3fd76cxfhz6wsvg4nd8` |
| skill-registry | `juno1mu0rqss5gffycc7ju25xdk90zawav60hgmtah9n9gvnahutglw0qmj6ek0` |
| marketplace | `juno1t08z3ta2tq7fz73rlxw4y6mxualjqdvh6rw7zpvguunmd7ly45as8ay25e` |

Paste:

```
Query the agent-registry contract on junoclaw-g1 with this message:
{ "get_config": {} }
Contract: juno1n7fx4duuy59dvscmwrvhqss74heaectwjyac8gn5w2w637l2dxmqy9jgwl
Tell me the registration fee, the denom, and what the "registry" map points to.
```

Expected: `registration_fee` ~`1000000`, `denom: "ujclaw"`, and a
registry map wiring in task-ledger + escrow addresses. That map is how
the economy's contracts know each other — you just read the wiring.

Now repeat for the others — paste:

```
Query truth-market on junoclaw-g1 with { "get_config": {} } and tell me:
minimum stake, slash percent, reward percent, and min operators.
Contract: juno1y4zmmggk6awzrnyfpjyvcm8ggygc6scewuy9efp3fd76cxfhz6wsvg4nd8
```

Expected: min stake `1000000`, slash `10`, reward `5`, min operators `3`.
Those four numbers *are* the adjudication economy's incentive math —
you'll use them again in lesson 6.

Append both configs to `my-verification-log.md`.

---

## Part 4: Follow the wiring yourself

**Why:** don't trust the table above — make the chain prove the
connections.

```
The agent-registry config's "registry" map named a task-ledger
address. Query THAT contract with { "get_config": {} } — does its own
config point back at agent-registry and escrow?
```

Expected: yes — the contracts reference each other. You've traced the
trust graph yourself, from on-chain data, not docs.

---

## Part 5 (optional but recommended): independently-verified evidence

**Why:** you just read state through *one* operator's endpoint. An
independent observer (ANODE) captures blocks and verifies the
certificates itself — BLS + MAYO2 signatures, quorum bitmap, payload
and height bindings — and publishes the evidence publicly.

Open these two URLs in a browser (swap the height for a recent one):

```
https://explorer.anode.team/test/junoclaw/api/v1/blocks/3207254/
https://explorer.anode.team/test/junoclaw/api/v1/verification-evidence/3207254/
```

Look for: `verification.stages` — six checks, all `passed` —
`certificate.sha256`, the roster and group key, and the `source`
field honestly labelling which upstream donor the capture came from.
Now compare the block `digest` with what your own gRPC read produced
at the same height. Replicated data agreeing is expected — an
*independently verified commitment* matching is the lesson. (Note the
`scope` field: ANODE verifies the exact captured artifacts, nothing
more — precise claims are what honest verification looks like.)

If your instructor provided a second native gRPC endpoint too, re-run
the Part 3 query against it — that's the same check one level deeper.

---

## Prove it worked

`my-verification-log.md` should now contain: two heights, your
balances, and two contract configs — all from live queries. Show it to
a classmate: if your numbers match, you're reading the same chain.

## Homework

```
Query the skill-registry with:
{ "get_skill": { "dapp_name": "junoclaw-cosmos-mcp" } }
Fetch the skill_uri, tell me the sha256 of what it serves,
and compare it to the entry's skill_hash.
```

The match is the on-chain manual proving itself. You'll write an entry
like this for your own work later.
