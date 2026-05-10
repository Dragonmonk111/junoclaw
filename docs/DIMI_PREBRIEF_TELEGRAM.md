# Dimi Pre-Brief Telegram Message

**To send:** after Phase 0 of [POST_VOTE_EXECUTION_PLAN](./POST_VOTE_EXECUTION_PLAN.md) completes, before Phase 1
**Channel:** Telegram DM
**Tone:** status update, zero ask, low pressure

---

## Message text (copy-paste verbatim)

```
Hey Dimi —

Quick FYI on the BN254 work after #374 closed (~80% Yes, 22% Abstain,
near-zero veto, 44% turnout — clean mandate).

Sequence we're running over the next ~4 weeks:

1. Rebasing our 3 cosmwasm patches onto the latest tag, regenerating
   gas measurements on devnet to replace projections with measured
   numbers.

2. Opening *issues* (not PRs) on CosmWasm/cosmwasm and wasmvm, asking
   Ethan and Simon to confirm the ABI shape before we send any code.
   Their call.

3. While that conversation runs, we'll draft the v30 upgrade handler
   in our own fork — pattern-matched on your v28→v29 work. Goal is
   a ~20-line handler across 2 files: bump wasmvm, register bn254
   capability, RunMigrations. Nothing else.

4. Three local rehearsals on different v29.1 heights, then a uni-7
   testnet upgrade proposal with my own validator weight + community
   solicitation. Only after all of that lands cleanly do we go to
   juno-1 mainnet.

The mainnet step is when I'd want to ask if you'd co-sign as the
chain-side author of record. Not asking now — too early. Just
keeping you in the loop so it's not a surprise when the brief lands.

Marius's "be careful, I cleaned up the codebase massively" is a
binding constraint on every diff we propose. Anything outside the
two-line handler scope goes in a separate proposal.

Happy to share progress logs whenever useful, or stay quiet until
the brief is ready. Your call. No reply expected on this message.

— V
```

---

## Why this exact message

- **Opens with the vote outcome** — gives Dimi the headline number, no need to look it up
- **Numbered sequence** — easy to skim, easy to tell where we are when we send the next update
- **"Issues, not PRs"** — sets expectation that we're not bouncing PRs at maintainers
- **"Pattern-matched on your v28→v29 work"** — flatters the right thing (his prior work) without overdoing it
- **"Goal is a ~20-line handler"** — sets size expectation; if we end up at 200 lines he'll know to push back
- **Marius citation** — shows we're listening to other validator-class voices, not just sucking up to Dimi
- **Explicit "not asking now"** — removes any pressure to commit; the offer comes later when he can evaluate the actual artefact
- **"No reply expected"** — gives him permission to ignore the message; reduces the cost of receiving it

---

## What NOT to say in this message

- Don't link the POST_VOTE_EXECUTION_PLAN doc — it's 14 pages, too much for a Telegram DM
- Don't link the ADR — same reason
- Don't ask "what do you think?" — the answer is "I don't know yet, send me the artefact"
- Don't promise dates beyond "~4 weeks" — slippage is real, we don't pre-commit
- Don't pre-celebrate the prop closing — he saw the result; restating it sounds like fishing
- Don't apologize for the length — it's already short

---

## Follow-up cadence

| When | What |
|------|------|
| After Phase 1 (issues published) | Single message: "Issues live, links: ..., .... No update needed." |
| After Phase 2 (handler drafted, rehearsals done) | Send the [V30_UPGRADE_HANDLER_DESIGN.md](./V30_UPGRADE_HANDLER_DESIGN.md) — that's when the actual handoff happens |
| If maintainer silence at day 14 | No message to Dimi about it; this is our problem, not his |
| If maintainer engagement is positive | Brief: "Got constructive feedback on the issues, adapting the design. PR opening in ~1 week." |
| When uni-7 upgrade fires | "uni-7 upgrade fired clean, logs at ..." — link to gist or repo |
| When ready for mainnet brief | Full handoff per [V30_UPGRADE_HANDLER_DESIGN.md §9](./V30_UPGRADE_HANDLER_DESIGN.md) |

**Default to under-communicating.** Dimi is a validator with a day job and a security-patch portfolio. He doesn't need our weekly update; he needs our message at the moments his expertise actually compounds (mainnet upgrade brief). Save his attention.

---

## Stretch — if Jake Hartnell asks for an update

(Different audience, similar tone; only send if asked.)

```
Hey Jake —

#374 closed PASSED at ~80% Yes / 22% Abstain. Spinning up the
upstream conversation now — issues going to CosmWasm/cosmwasm and
wasmvm before any PR. Goal is to land BN254 host functions, then
roll a v30 chain upgrade with Dimi as co-author. Same playbook as
#373 in spirit: small, narrow, well-rehearsed.

Will share PR links when they go up. Apache-2.0 throughout, as ever.

Thanks again for the original "do it" — it shaped the whole arc.

— V
```

---

*Apache-2.0. Both messages are copy-paste-ready when the appropriate phase completes.*
