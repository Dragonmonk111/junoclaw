# PR #929 GitHub-comment paste-draft

*Posting plan: **Option B** from [`MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md`](./MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md) §9. Open with the two HIGHs in one comment ending with one question. Hold the four LOW/MEDIUMs for follow-up if the conversation continues.*

*Comment URL to paste at: [`https://github.com/DA0-DA0/dao-contracts/pull/929`](https://github.com/DA0-DA0/dao-contracts/pull/929) → "Add a comment" at the bottom of the conversation. Paste the body below verbatim. Do **not** add labels or request reviews — let the maintainers do that.*

---

## Body (copy-paste this into the GitHub textarea)

````markdown
Substantive review pass on this — applying the same deterministic-scrutiny benchmark we use on the JunoClaw contracts. The full review is at [MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md) for the long form (≈19K, 9 sections). Headline: **the architecture is correct.** The thin-consumer-of-`x/voting-snapshot` shape genuinely fixes the sparse-delegator bug from #832 at the architectural level — the chain owns the snapshot, the cw contract owns the routing.

Two findings I'd want to discuss before merge. Both have concrete worked examples in the long form.

---

### F1 (HIGH if confirmed) — Same-block multi-event accumulation

When two staking events for the same delegator land in the same block, the `prev_power = chain[h-1]` formula is too stale, and the emitted `StakeChangedHookMsg::{Stake, Unstake}` deltas are cumulative-against-block-start rather than incremental.

**Worked example.** Voter A enters block H with `chain[H-1] = 100`. Two `MsgDelegate` in one tx land sequentially:

| Event | Chain after | Sudo computes | Emits |
|---|---|---|---|
| 1: AfterDelegationModified (now 150) | `chain[H] = 150` | new=150, prev=100 → +50 | `Stake { 50 }` |
| 2: AfterDelegationModified (now 175) | `chain[H] = 175` | new=175, prev=100 → +75 | `Stake { 75 }` |

Subscribers see `+125` for voter A in block H; the actual stake change is `+75`. The duplicate is `+50`.

This is invisible to **cumulative-power-tracking** subscribers (they re-read voting power at hook-fire time and treat the emitted delta as a wake-up signal). It's a real correctness bug for **flow-tracking** subscribers (a stake-velocity indexer, or a rewards system that proportions to deltas instead of balances).

The 9 unit tests don't fire two events for the same delegator at the same height; the bug isn't in the test set.

**One fix shape:** track the contract's own emitted-power per delegator per block in a `Map<&Addr, EmittedPower { height, power }>`. On each event: if `last_emitted.height == current_height`, use `last_emitted.power` as `prev_power` instead of `chain[h-1]`. ~25 LoC + 1 test. Costs one read + one write per sudo.

---

### F2 (HIGH) — `reply` handler is empty but README promises auto-unregistration on hook failure

The README says:

> Subscribers register via `AddHook { addr }` (gated to the DAO) and are auto-unregistered if their execute call errors (standard `reply_on_error` pattern from the rest of the dao-contracts hook surface).

But `contract.rs:reply` is a no-op:

```rust
pub fn reply(_deps: DepsMut<JunoQuery>, _env: Env, _reply: Reply) -> Result<Response, ContractError> {
    // Reserved for future auto-unregistration on hook failure. The
    // current dao_hooks call sites use plain SubMsg::new (no reply
    // requested), so this entry-point is unreachable in practice.
    Ok(Response::new())
}
```

The comment confirms `dao_hooks::stake::stake_hook_msgs` builds `SubMsg::new`, not `SubMsg::reply_on_error`. So a misbehaving subscriber's error propagates up and **fails the whole sudo**, which means the originating `MsgDelegate` / `MsgUndelegate` fails. A single bad subscriber halts delegation flow for every voter using this DAO until the DAO admin manually `RemoveHook`s them.

In a multi-DAO ecosystem with shared infrastructure (gauges, distributors), this is an availability concern.

**Two clean fix shapes** (depending on intent):

1. **Implement what the README promises** — switch `dao_hooks::stake::stake_hook_msgs` to `reply_on_error`, populate the reply handler with `HOOKS.remove_hook(deps.storage, failed_addr)?`. Map reply-id → addr via a `pending_addr: Map<u64, Addr>` written before send and read in reply. Standard cw-hooks pattern; the dao-contracts library has a reference impl elsewhere.
2. **Update the README to match the code** — acknowledge that subscriber failures halt sudo. Add a warning: "Subscribers must be deeply tested; a bad subscriber can break delegation flow."

I'd lean toward (1); (2) is technically honest but operationally untenable.

---

### One question before I write up the four LOW/MEDIUMs

Are F1 and F2 intentional (and we should adjust expectations / reread the README), or do you want patches? Happy either way — just don't want to spend the time on the LOWs if these two reframe the rest.

The LOWs/MEDIUMs in the long form (for completeness, no action needed yet):

- F3 MEDIUM — `BeforeValidatorSlashed` swallow trusts unverified lazy decay; cross-link to chain-side `x/voting-snapshot` decay impl in the README would resolve.
- F4 LOW — no `migrate` entry point; future schema evolution requires re-deploying.
- F5 LOW — `i64` cast for height (impractical to hit at any realistic block height).
- F6 LOW — `auto_register_staking_hooks: Some(false)` is dead surface; the field rejects `Some(true)` at runtime.

(Cross-pollinating: same architectural pattern would fix our own [agent-company F1](https://github.com/Dragonmonk111/junoclaw/blob/main/contracts/agent-company/DETERMINISTIC_AUDIT.md) — vote-weight-at-proposal-creation snapshot. Apache-2.0 throughout.)
````

---

## After posting

1. **Capture the comment URL** (e.g. `https://github.com/DA0-DA0/dao-contracts/pull/929#issuecomment-NNNNNNNN`) for the Jake DM follow-up.
2. **Wait for substantive reply.** Do not re-engage on the LOW/MEDIUMs until Jake responds to F1/F2. Posting four more findings before the first two get acknowledgement reads as overload.
3. **If F1 or F2 gets pushback** (e.g., "actually we considered this, here's the reasoning"): incorporate the response into the long form's footnotes; do not edit the GitHub comment.
4. **If F1 or F2 is accepted as a real finding**: offer to send a PR. The fix shapes in the comment are sized small enough that one of us can land them in a session.
5. **No Telegram broadcast** about the comment until at least one substantive maintainer reply. A silent comment + a public broadcast reads as performative.

---

## Edit-here-not-on-GitHub principle

If anything in the body needs adjustment after the first read-through, edit **this file**, then re-paste. Do not edit on GitHub directly. Same edit-discipline as `UPSTREAM_ISSUE_DRAFTS.md` — this avoids version drift between the markdown source-of-truth and the public artifact.

---

*Apache-2.0. Comment-draft for Option B posting plan; long form lives at [`MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md`](./MOULTBOOK_REVIEW_OF_DAO_VOTING_JUNO_STAKED_PR_929.md).*
