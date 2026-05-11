# DM to Jake — Track B clarification

*Drafted 2026-05-11. Send when you next have Jake on Telegram or Signal. Tone matches the existing thread: warm, concise, technical, one ask.*

---

## Short version (recommended — Telegram-friendly length)

Hey Jake — quick one on v30 PR #1202 and the BN254 piece. The branch's `go.mod` pins `wasmvm/v3 v3.0.4` directly with no `replace` for our fork, so for prop #374 to actually land with v30 the patches need to forward-port from wasmvm v2.2.x (where we have the merged set + verification scaffolding) onto v3.0.x. Three readings of how that lands:

1. We do the v3 forward-port and you pull it in via a `replace` in v30's `go.mod` (~3-5 working days on our side, we have the tooling).
2. Juno AI coordinates merging the patches into upstream CosmWasm/wasmvm v3.0.x and v30 picks up a tagged release.
3. BN254 slips to v31 and v30 ships without it.

Happy with any of the three — just want to know which one you're shooting for so I can either start the rebase tomorrow or stand down. Either way I posted a Moultbook-style code review on the PR covering one thing I'd grade as a real bug in `pruneVotingPower` (sparse-delegator snapshots get evicted past the retention boundary, breaks the "latest at-or-before height" read semantics) — happy to send the fix patch if useful.

🦦📚

---

## Longer version (if Telegram lets you do paragraphs / if it's an email)

Hey Jake,

Reviewed PR #1202 in detail today — full assessment + Moultbook-anchored code review are on origin/main. Three things to surface, in priority order:

**1. Track B ownership / timing.** v30's `go.mod` pins `github.com/CosmWasm/wasmvm/v3 v3.0.4` directly, no `replace` directive. For prop #374's BN254 precompile to actually ship with v30 (your PR description names it), the v2.2.x patch set we landed and verified (22/22 + 311/311 baseline against v2.2.7) needs forward-porting onto v3.0.x. Three ways this lands:

  - **(a) We do the forward-port** — ~3-5 working days, we have the patch set, the rebase script, and the verification tooling already calibrated. v30 picks it up via a `replace github.com/CosmWasm/wasmvm/v3 => github.com/Dragonmonk111/wasmvm/v3 vX.Y.Z` in `go.mod`.
  - **(b) Juno AI coordinates upstream** — patches go into CosmWasm/wasmvm v3.0.x main, v30 picks up a tagged release. Cleaner long-term, slower if upstream is patient.
  - **(c) BN254 slips to v31** — v30 ships without it, we land it cleanly after Juno mainnet stabilises on the new stack.

I'm happy with any of the three. Just want to know which one you're aiming for so I can start the rebase tomorrow (if (a)), stand down and help with (b), or pivot Moultbook work to be ready for the v31 window (if (c)).

**2. Found one real bug in `pruneVotingPower`.** Sparse delegators (anyone whose last staking event predates `current_height - RetentionWindowHeights`) get their only snapshot evicted, which breaks the keeper's documented "latest at-or-before height" read semantics. Long-tail delegators are the typical case so this is not a theoretical concern. Posted a structured review on the PR (or here, depending on your preference for where it lands) with a worked example and a suggested two-pass fix that preserves the most-recent-snapshot-per-delegator across the retention boundary. ~30 lines of Go + a regression test. Happy to send as a PR to `jakehartnell/v30` if useful.

**3. Two more findings + question answers.** `VotingPower` sums vs `TotalPower` — LST-excluded delegators make the two inconsistent, DAO quorum arithmetic ends up silently harder than the parameter says. Suggested three resolution options in the review. Also direct answers to your three named open questions (retention default, wasmbinding gas, pre-upgrade query semantics).

Verifiable-agent thing — your `juno-ai-dev` + Claude Opus 4.7 (1M context) co-author trailer on every commit is exactly the pattern *The Verifiable Agent* article argued for. Moultbook is being built to be the on-chain substrate for this kind of agent-DAO operation. Once it's on devnet (next few days) the PR-1202 review re-lands as the first cross-org Moultbook entry citing the v30 commit-hash anchor — dog-fooding the dev-collab discipline by being its first user.

Yes — teaming up on v30 feels great. Let me know on (1) and I'll move accordingly.

—

🦦📚

---

## Notes on tone / what to keep vs cut

- **Keep**: the explicit "three readings, I'm happy with any" framing. Gives Jake an easy out + makes it clear we're not pressuring for ownership we haven't been granted.
- **Keep**: the bug callout in the middle, not the headline. The Track B question is the ask; the bug-found is the deliverable that proves you read the PR carefully. Order matters.
- **Cut (in the short version)**: anything about Moultbook positioning. He already knows; saying it again feels like upsell.
- **Cut**: dates / timelines unless he asks. He's running his own clock.
- **Tone calibration**: warm but not gushing. He's a busy maintainer with an AI dev partner running half his PRs; technical respect lands better than emotional warmth.

---

## After he responds

Three branches to be ready for:

- **If (a) — we do the forward-port**: I start the rebase next session. We can probably ship a draft PR against `Dragonmonk111/wasmvm` branch `bn254-precompile-v3` in 3-5 days, run the verification suite, then hand Jake the `replace` directive for v30.
- **If (b) — upstream coordination**: I prep the upstream PR description (we already have most of this in `WASMVM_BN254_PR_DESCRIPTION.md`), forward-port the patches onto cosmwasm v3.0.1 + wasmvm v3.0.4 main, and we open the PR jointly.
- **If (c) — v31 slip**: I pivot to the Moultbook devnet deploy + gas measurement work, and we re-engage on BN254 once v30 lands on juno-1.

All three are fine. Don't over-plan until Jake names one.
