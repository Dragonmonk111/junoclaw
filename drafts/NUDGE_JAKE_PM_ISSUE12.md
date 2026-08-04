# Nudge comment — post on CosmosContracts/pm Issue #12

> Status check (GitHub API, 2026-07-28): PR #11 and Issue #12 both opened
> 2026-07-19, zero comments, zero reactions, no maintainer response since.
> A041 (DAO signal accepting the verdict_authority role) is held until this
> nudge gets a response or ~1 week passes with none, per
> `drafts/PENDING_DETERMINISTIC_CLOSEOUT_PLAN.md`'s own next-action note.

Post this as a new comment on https://github.com/CosmosContracts/pm/issues/12

## Copy-paste box: Comment

```
Hey Jake — checking in on this and #11. No rush, just want to make sure
these landed in your queue okay.

Quick recap of where things stand on our side: agent-company v7 is live
on uni-7 (Code ID 80) with OutcomeCreate/OutcomeResolve wired up, WAVS
invoke API is at 15/15 smoke tests, and cross-platform determinism is
proven (3/3 byte-identical on AMD EPYC). The liveness keeper in #11 is
read-only (no signing/broadcasting) so it should be low-risk to review
whenever you get a chance.

Happy to start with PR 1 (integration tests proving agent-company works
as verdict_authority) if that's an easier first review than the
attestation_hash change — genuinely fine to sequence however's easiest
for you. Let us know if you'd rather we hold off on more PRs until you've
had a look at what's already open.
```

## After posting
- If Jake/a maintainer responds (approve, request changes, or just
  acknowledges): unblock A041, optionally update its description with the
  response.
- If no response after ~1 week: re-raise whether to submit A041 anyway,
  submit without the PM contribution as a live dependency, or drop it for
  now.
