# `memory/v30-upgrade-pr-1202.md`

## Summary (3 lines)

Juno v30 (CosmosContracts/juno PR #1202, by Jake Hartnell + Juno AI) introduces `x/voting-snapshot` (historical staking power with LST exclusion) and a `x/cw-hooks` overhaul, plus a hard pin to `wasmvm/v3 v3.0.4` with no `replace` directive. The pin is what forces our Track B forward-port. Our Moultbook-style review of the PR is at `docs/MOULTBOOK_REVIEW_OF_JUNO_V30_PR_1202.md` and was posted as a PR comment.

## Key facts

| Item | Value |
|---|---|
| PR | [`CosmosContracts/juno#1202`](https://github.com/CosmosContracts/juno/pull/1202) |
| Authors | Jake Hartnell, Juno AI (jointly) |
| Modules introduced | `x/voting-snapshot`, expanded `x/cw-hooks` (staking events) |
| LST handling | LST allowlist; LST-bonded stake contributes zero voting power |
| `wasmvm` pin | `v3.0.4` (no `replace` for our BN254 fork) |
| `cosmwasm` pin (transitive) | v3.0.1 |
| Our review | [`docs/MOULTBOOK_REVIEW_OF_JUNO_V30_PR_1202.md`](../docs/MOULTBOOK_REVIEW_OF_JUNO_V30_PR_1202.md) |
| Posted as | PR comment (verbatim from the review markdown) |
| Telegram amplification | Jake replied positively on Telegram (cf. `lessons-2026-05-13.md` §3) |

## Full context

### What v30 introduces

**`x/voting-snapshot` module.** Records `(delegator, height) → bonded power` on every staking event. Implements at-or-before semantics on read. Filters LSTs at write time using a chain-maintained allowlist. The contract-side consumer is `dao-voting-juno-staked` (DA0-DA0/dao-contracts PR #929).

**`x/cw-hooks` overhaul.** All staking lifecycle events fan out to registered cw contracts via sudo:
- Delegation events: `BeforeDelegationCreated`, `AfterDelegationModified`, `BeforeDelegationSharesModified`, `BeforeDelegationRemoved`.
- Validator events: `AfterValidatorCreated`, `AfterValidatorRemoved`, `BeforeValidatorModified`, `AfterValidatorModified`, `AfterValidatorBonded`, `AfterValidatorBeginUnbonding`.
- Slash events: `BeforeValidatorSlashed`.

Registration via `junod tx cw-hooks register-staking <contract_addr>`.

**Wasmvm pin.** `go.mod` line:
```
require github.com/CosmWasm/wasmvm/v3 v3.0.4
```
**No `replace` directive.** This is the constraint that forces our Track B forward-port — for our BN254 patches to land in v30, we need a `Dragonmonk111/wasmvm v3.0.4-bn254` tag that v30's `go.mod` can `replace`-reference. Without the replace, the upstream wasmvm wins and our patches are bypassed.

### Our review (highlights)

The full review is at `docs/MOULTBOOK_REVIEW_OF_JUNO_V30_PR_1202.md`. Headline findings:

1. **`x/voting-snapshot` design is sound.** Lazy decay on slash, at-or-before iterator semantics, LST allowlist enforcement at write time. ✅
2. **Question on the wasmvm pin.** Why no `replace`? If our BN254 fork is welcome, the pin makes that mechanically harder. (This is the question that triggered the Track B forward-port work.)
3. **Question on the cw-hooks fan-out semantics.** Multiple events for the same delegator in one block can produce surprising delta accumulations in subscribers — see PR #929 review F1 for the same finding in the cw-side consumer.

### Posting status

Posted as a PR comment on May 13, 2026 (per user pasting the review verbatim from the markdown body). Jake responded via Telegram with positive engagement on the structured-review style.

### Relationship to Track B

The wasmvm v3.0.4 pin is what made Track B (forward-port to cosmwasm v3.0.x) urgent. Without that pin, we could have continued shipping against v2.2.7 indefinitely. With it, our BN254 work either forward-ports or gets bypassed.

Day-1 baseline check (cf. [`memory/track-b-forward-port.md`](./track-b-forward-port.md)) showed the forward-port is much cheaper than feared — 1.5-2 days, not 3-5. So the pin is no longer a blocker; it's just a deadline.

## Cross-references

- [`docs/MOULTBOOK_REVIEW_OF_JUNO_V30_PR_1202.md`](../docs/MOULTBOOK_REVIEW_OF_JUNO_V30_PR_1202.md) — long-form Moultbook-style review.
- [`docs/PR_1202_GITHUB_REVIEW_COMMENT.md`](../docs/PR_1202_GITHUB_REVIEW_COMMENT.md) — the comment body posted to GitHub (verbatim subset of the review).
- [`docs/JUNO_V30_PR_ASSESSMENT.md`](../docs/JUNO_V30_PR_ASSESSMENT.md) — earlier draft of the assessment.
- [`memory/track-b-forward-port.md`](./track-b-forward-port.md) — the forward-port driven by the wasmvm pin.
- [`memory/dao-contracts-prs-928-929.md`](./dao-contracts-prs-928-929.md) — the cw-side companion PRs that consume `x/voting-snapshot`.
- [`memory/lessons-2026-05-13.md`](./lessons-2026-05-13.md) §3 — Jake's Telegram amplification.
- [`docs/V30_UPGRADE_HANDLER_DESIGN.md`](../docs/V30_UPGRADE_HANDLER_DESIGN.md) — our own v30-handler design notes.

---

*Apache-2.0.*
