---
title: "Lesson 6: Truth Markets — The Job This Course Trains You For"
type: lesson
difficulty: intermediate
tags: [truth-market, adjudication, staking, verdicts, junoclaw]
lesson_number: 6
---

# Lesson 6: Truth Markets — The Job This Course Trains You For

**Duration:** 90 minutes
**Format:** Theory + Practice
**Result:** you understand the adjudication economy's incentive math —
and can trace a real verdict on-chain.

## Prerequisites

- Lesson 2 done (you read truth-market's config already)
- Lessons 3–5 give context: you now know what gets disputed

---

## The problem (10 minutes)

An agent says it did the work. The requester says it didn't. Who
decides? Not a company — a market. `truth-market` operators stake on
verdicts: right verdicts earn a reward, wrong ones lose stake.

This is the layer a robotics lab, a medical team, a legal firm will
pay for — machine-produced work, verified by agents paid to be right.
**Course graduates are that workforce.** You're not learning to deploy
agents; you're being trained to sit in the verification economy.

---

## Part 1: Pull the incentive math from the contract

**Why:** the rules are live parameters — read them, don't recite them.

```
Query truth-market on junoclaw-g1 with { "get_config": {} }.
Contract: juno1y4zmmggk6awzrnyfpjyvcm8ggygc6scewuy9efp3fd76cxfhz6wsvg4nd8
Give me: min stake, slash_percent, reward_percent, min_operators.
```

Expected (verify yourself, numbers may drift): min stake `1000000`
ujclaw, slash `10`%, reward `5`%, min operators `3`.

Now do the math — ask your AI:

```
An operator stakes 1,000,000 ujclaw on a verdict.
- If right: earns reward_percent of stake. How much is that?
- If wrong: loses slash_percent of stake. How much is that?
- If 3 operators are needed for a verdict and all must stake: what's
  the total value at risk per verdict?
- Why must the slash exceed the reward for the market to be honest?
```

That last answer matters: being right must beat guessing; losing must
hurt more than a coin flip pays. If slash ≤ reward, operators profit
by voting randomly — the market dies. The asymmetry IS the security.

---

## Part 2: Find the operators and open disputes

**Why:** discover who's already adjudicating and what's live.

```
Query truth-market for its operator list and any open/pending
verifications. Try variants: { "list_operators": {} },
{ "get_operators": {} }, { "list_verifications": {} },
{ "get_pending": {} } — read error messages for valid names.
Show me what exists.
```

If nothing is open: the market is young — that means *you're early*,
not that it's broken. min_operators=3 means a verdict needs a real
quorum — three independent staked judgments, not one opinion.

---

## Part 3: Adjudication as a career path

**Why:** connect the mechanism to the job.

Ask your AI:

```
A robotics lab pays to verify 500 inspection reports produced by a
contractor's agents. A medical team pays to verify 200 triage
summaries. Walk me through how truth-market adjudication serves each —
who stakes, who verifies, what the receipt looks like to the customer.
```

The customer doesn't buy a blockchain — they buy *"N independent
verifiers staked real value that this work is correct."* Your agent,
with a real work history from lessons 3–5, is what sits in that seat.

And it compounds: every verdict you issue correctly adds to a public
record — `trust_score` and task history are what get you picked for
better work.

---

## Part 4 (stretch): What would you adjudicate?

Not all work is judgeable by the same agent. Ask:

```
Given my agent's capabilities, what kinds of tasks could it truthfully
adjudicate — code review, data quality, summaries, image checks?
Which could it NOT? A verifier who can't judge the domain is a coin
flip with a stake.
```

Save the honest self-assessment to `agent-record.md` — knowing your
jurisdiction is part of being a good operator.

## Prove it worked

Explain to the class — using the live numbers you pulled — how much an
operator gains or loses per verdict on a 1 JCLAW stake, what 3
operators put at risk, and why the slash/reward asymmetry keeps the
market honest.

## Homework

Watch the truth-market over the next days:

```
Query truth-market again — any new verifications or operators since
class? Append what changed to my-verification-log.md.
```

The capstone (lesson 7) will ask you to participate in or observe one
real adjudication round.
