# Audit-bot v2 — Design

*Drafted 2026-05-15. Anchor: [`memory/SESSION_PROTOCOL.md`](../memory/SESSION_PROTOCOL.md) §3 T10. Companion: [`memory/deterministic-audit-benchmark.md`](../memory/deterministic-audit-benchmark.md) §"Cross-cutting patterns (B-cluster expansion 2026-05-14)".*

## Summary (3 lines)

Audit-bot v1 ([`.github/workflows/audit-bot.yml`](../.github/workflows/audit-bot.yml)) is a **gate-only** CI check — it fails any PR that touches `contracts/<name>/src/**` without also touching the matching `DETERMINISTIC_AUDIT.md`. Audit-bot v2 keeps the gate but adds an **LLM-assisted lint pass** that scans the PR diff for the six concrete cross-cutting patterns (A-F) now documented across nine audited contracts, and posts a per-pattern advisory comment when a match is detected. Comments are advisory only — no merge blocking — until v2.1 hardens false-positive rates.

## Why now

The 9/9 audit completion (2026-05-14) surfaced six recurring patterns that show up across multiple contracts. Each pattern has:

1. A precise structural shape (so it's grep-able / AST-checkable).
2. A documented severity tier (so the comment can self-rate).
3. A worked-example fix (so the comment can offer concrete remediation, not just "investigate").
4. A measured false-positive risk (since the same pattern can be intentional in some places — e.g., `info.funds` *should* accept any denom for a generic-treasury contract).

This combination is exactly what an LLM-in-CI lint pass needs to be useful rather than noise. v1 was deliberately gate-only because we didn't yet know what to lint *for*. We do now.

## v1 → v2 delta

| Aspect | v1 (current) | v2 (proposed) |
|---|---|---|
| Trigger | PR touches `contracts/**/src/**` or `contracts/**/Cargo.toml` | Same |
| Action 1 | Verify `DETERMINISTIC_AUDIT.md` was touched | **Same** (kept) |
| Action 2 | None | **New:** run lint pass over the diff; post advisory comment per detected pattern |
| Block-merge surface | Action 1 failure | **Same** (Action 2 is advisory-only in v2.0; gating in v2.1 if FP rate < 5%) |
| LLM call | None | One call per PR, prompt-templated, deterministic temperature 0 |
| Cost ceiling | ~0 | ~$0.01-0.05 per PR (Claude Haiku or GPT-4o-mini scale) |
| New deps | None | `actions/github-script` already used; add an `anthropic` (or equivalent) GH Action wrapper, or self-host via a lightweight script that calls the API |

## The six patterns to lint for

Pulled directly from [`memory/deterministic-audit-benchmark.md`](../memory/deterministic-audit-benchmark.md) §"Cross-cutting patterns".

### Pattern A — Public state-mutating handler with insufficient permission/fee gate

**Detect.** Any `pub fn execute_*` or match-arm in `pub fn execute(...)` that:

- Mutates state (writes to a `Map`, `Item`, or sends a `BankMsg`/`CosmosMsg`)
- AND is reachable from `ExecuteMsg` without an early `if info.sender != config.<auth_field>` check
- AND does NOT take `info.funds` as a fee (cf. Pattern C)

**Severity.** MEDIUM unless the handler is documented as permissionless-by-design.

**LLM prompt fragment:**

```
Look for execute_<name> functions in this diff that:
1. Write to storage (call .save() on a Map or Item, or build a BankMsg/CosmosMsg in the response)
2. Don't have an early-exit on info.sender != config.<some_field>
3. Don't accept info.funds as a fee

For each match, report:
- The function name
- The line where the state mutation happens
- A 1-sentence explanation of why this is a permission gap

Skip if the function has a doc comment explicitly stating it's permissionless-by-design (e.g., "// Anyone can call this; it's a..." or "// Permissionless by design.").
```

**False-positive risk.** Medium. Some functions (e.g., `execute_fund` in builder-grant, `execute_claim` in faucet) are intentionally permissionless. The doc-comment escape hatch handles most.

**Worked example.** `agent-registry F1` — registration fees collected without a withdraw path. Detected by: `execute_register_agent` writes to `AGENTS` map, no `info.sender` check (anyone can register), takes `info.funds` (so Pattern C escape applies — fee gate present) — but no corresponding `execute_withdraw_fees`. The lint here is a **chained** condition: fee accepted but no withdraw exit. v2.0 detects the simpler form; v2.1 chains.

### Pattern B — Inherited public-protocol surface without inherited defenses

**Detect.** Comments or doc-strings referencing forks of well-known protocols (Uniswap v2, Compound, Curve, etc.) without a corresponding section that re-applies the upstream's accumulated security patches.

**Severity.** HIGH if the fork target has known-canonical patches that aren't applied.

**LLM prompt fragment:**

```
Look for doc comments or README content in this diff that say things like
"based on Uniswap v2", "ported from Compound v2", "fork of Curve", etc.
For each match, check whether the corresponding source code:

1. Implements the upstream's known security patches (e.g., for Uniswap v2: 
   MIN_LIQUIDITY first-depositor lockup, k-invariant check, reentrancy lock)
2. Has a section in the audit doc explaining which upstream patches were 
   considered and which (if any) were intentionally omitted

If neither, flag this as a Pattern B match. The risk is that forking the
protocol surface inherited the attack surface without inheriting the defenses.
```

**False-positive risk.** Low. Few diffs claim to fork a major protocol. When they do, the analysis is usually warranted.

**Worked example.** `junoswap-pair F1` — first-depositor inflation attack, a Uniswap v2 patch from a decade ago that the JunoClaw fork didn't re-apply. Detection: the contract's `lib.rs` has `//! Junoswap pair contract — port of the Uniswap v2 AMM` but no `MIN_LIQUIDITY` constant.

### Pattern C — `info.funds` accepted without denom validation in `execute_fund`

**Detect.** Any `execute_fund` (or any function taking `info.funds: Vec<Coin>` and depositing them as treasury) that filters by `config.denom` only at the *attribute* level, not at the *coin acceptance* level.

**Severity.** MEDIUM. Silent absorption of wrong-denom tokens, no recovery path.

**LLM prompt fragment:**

```
Look for execute_<name> functions in this diff that:
1. Take info.funds: Vec<Coin> as input
2. Have a .filter(|c| c.denom == config.denom) only used to compute an 
   attribute or sum, NOT used to early-return Err if a non-matching coin 
   is present
3. Don't iterate info.funds and explicitly reject mismatched denoms

For each match, report the function and propose the fix shape:

  for coin in &info.funds {
      if coin.denom != config.denom {
          return Err(ContractError::UnexpectedDenom { 
              expected: config.denom.clone(),
              received: coin.denom.clone() 
          });
      }
  }

Skip if the function is intentionally a multi-asset accumulator (look for a
struct with multiple denoms or a Vec<Denom> in Config).
```

**False-positive risk.** Low. The pattern is specific (filter-on-denom-for-attribute-but-not-rejection).

**Worked examples.** `faucet F1`, `builder-grant F1` — both flagged this session.

### Pattern D — Range queries scanning without `Bound::inclusive` / `Bound::exclusive`

**Detect.** Any `.range(deps.storage, None, None, Order::*).filter(...)` where a `start_after` parameter is in scope but isn't passed as a `Bound`.

**Severity.** LOW-MEDIUM. Becomes O(N) gas-leak as state grows.

**LLM prompt fragment:**

```
Look for .range() calls in this diff where:
1. The function signature accepts a start_after: Option<...> parameter
2. The .range() call passes None as the first argument (no lower bound)
3. The function then filters in memory with .filter() / .filter_map()

For each match, propose the fix:

  use cw_storage_plus::Bound;
  let start = start_after.map(Bound::exclusive);
  let items = MAP.range(deps.storage, start, None, Order::Ascending)
      .take(limit)
      .collect();

Skip if the function explicitly intends to scan the entire map (e.g., a 
"sum all" aggregator) — but in that case, flag a separate concern about 
unbounded scans.
```

**False-positive risk.** Medium. Some queries genuinely need to scan the full map (e.g., `query_total_supply`). Doc-comment escape works.

**Worked examples.** `junoswap-factory F3`, `builder-grant F3`, `agent-registry F3`.

### Pattern E — Unchecked `+= 1` accumulators on monotonic counters

**Detect.** Any `<config|state>.<field> += <expr>` or `<map_load>.<field> += <expr>` where the field is `u64` or `u128` and `+= 1` (or any addition) without `checked_add`.

**Severity.** LOW. Practically unreachable on `u64` but stylistically inconsistent.

**LLM prompt fragment:**

```
Look for `+=` or `+ 1` patterns in this diff applied to fields with type
u64, u128, Uint64, or Uint128. For each match where the addition is NOT
wrapped in checked_add() / checked_add().ok_or(), suggest the pattern:

  config.total_claims = config.total_claims
      .checked_add(1)
      .ok_or(ContractError::Overflow {})?;

Skip if the type is Uint128 and the addition is via `+` operator on
cosmwasm_std::Uint128 (which is overflow-checked by default).
```

**False-positive risk.** Low — Uint128 is the main exception and is easy to filter.

**Worked examples.** `junoswap-factory F4`, `builder-grant F4`, `faucet F4`.

### Pattern F — Missing `migrate` entry_point

**Detect.** Any contract whose `lib.rs` exports `instantiate`, `execute`, `query` but not `migrate`.

**Severity.** LOW. Future state-schema changes require redeploy + state reconstruction.

**LLM prompt fragment:**

```
For any contract directory in this diff (contracts/<name>/), check whether
src/contract.rs (or src/lib.rs) has a `#[entry_point] pub fn migrate` 
declaration. If not, suggest adding an empty stub:

  #[cfg_attr(not(feature = "library"), entry_point)]
  pub fn migrate(_deps: DepsMut, _env: Env, _msg: Empty) -> Result<Response, ContractError> {
      Ok(Response::default())
  }

This is a one-time addition; subsequent migrations replace the body.
```

**False-positive risk.** Zero. Either the entry_point exists or it doesn't.

**Worked examples.** All 9 application contracts in the audit sweep.

## Implementation phases

### Phase 1 — Gate kept, lint added (advisory)

`audit-bot.yml` gains a second job `lint-pass` that:

1. Diffs the PR with `git diff --name-only ${BASE_SHA} ${HEAD_SHA} -- 'contracts/**/src/**'`
2. Concatenates all changed source files into a single prompt context (cap at ~50 kB; truncate with a notice if over)
3. Calls an LLM with a templated system prompt embedding the six pattern fragments above
4. Parses the response (structured JSON: `[{pattern: 'A', file: 'contracts/x/src/contract.rs', line: 123, severity: 'MEDIUM', message: '...'}]`)
5. Posts a single PR comment summarizing all matches, **none of which block merge**

The comment format mirrors the existing v1 failure comment style:

```
### Audit-bot lint pass: 3 patterns detected

**Pattern C (MEDIUM)** — `contracts/new-contract/src/contract.rs:45` — `execute_fund` accepts `info.funds` without denom validation. Suggested fix: ...

**Pattern E (LOW)** — `contracts/new-contract/src/contract.rs:78` — `config.counter += 1` is unchecked. Suggested fix: ...

**Pattern F (LOW)** — `contracts/new-contract/src/lib.rs` — no `migrate` entry_point declared. Suggested fix: ...

These are advisory. Address them in this PR or surface in the audit doc as 
acknowledged design choices. See [`docs/AUDIT_BOT_V2_DESIGN.md`](docs/AUDIT_BOT_V2_DESIGN.md) for the full pattern catalogue.
```

### Phase 2 — False-positive measurement

Run Phase 1 silently for ~10 PRs (or 4 weeks, whichever first). Each detected match gets a manual triage:

- **TP** = true positive, the pattern was a real concern
- **FP** = false positive, the pattern was intentional or doesn't apply
- **TPN** = true-but-noisy, the pattern was real but already documented in the audit doc

Target: **≥80% TP+TPN** (i.e., FP rate ≤20%) before promoting any pattern to gating.

### Phase 3 — Gating selectively

Patterns with consistently low FP rates promote to gate-blocking:

- **Pattern F** (missing migrate) — zero FP, promote first
- **Pattern E** (unchecked `+= 1`) — low FP, promote second
- **Pattern C** (denom-eating) — moderate FP, promote with a doc-comment escape hatch
- **Patterns A, B, D** — keep advisory; FP rate too dependent on intent

A gating pattern means the PR can't merge until either (a) the pattern is fixed in the diff, or (b) a doc comment explicitly waives it (e.g., `// LINT-WAIVE-PATTERN-C: this is a multi-asset treasury, see docs/X.md §Y`).

### Phase 4 — Self-improvement loop

When a contract gets a fresh audit (per the v1 gate), the audit doc's **Findings** table is parsed back and any pattern not in A-F that recurs (i.e., shows up in 2+ contracts) becomes a new pattern G+. The lint prompt is regenerated to include it.

Cadence: review every quarter or whenever 3+ new audits land, whichever first.

## LLM choice

| Model | Cost/PR | Latency | Notes |
|---|---|---|---|
| **Claude Haiku 3.5** | $0.01 | ~3s | Recommended baseline. Strong code understanding; cheap enough for every PR. |
| GPT-4o-mini | $0.005 | ~2s | Slightly cheaper, slightly worse at structural code analysis. Acceptable fallback. |
| Local model (Llama 3.3 70B via Ollama on a runner) | $0 | ~30s | Eliminates API cost but requires a beefy self-hosted runner. Phase 5 candidate. |
| GPT-4o / Claude Sonnet 4 | $0.05-0.20 | ~5-10s | Overkill for a templated lint; reserve for "deep audit" mode (Phase 5+). |

Phase 1 uses Haiku 3.5. Switch points: cost > $50/month total → consider self-host; FP rate > 30% → consider Sonnet for accuracy.

## Security model for the LLM call

- **No secret exfiltration.** The prompt contains only PR-diff content (already public on a public repo). Private-repo PRs route through a self-hosted runner with the API key in an env-var, never in the prompt.
- **No code execution.** The LLM produces JSON; the runner parses and posts. The LLM does not run shell commands, fetch URLs, or execute generated code.
- **Idempotent.** Same diff in, same comment out (temperature 0). PRs that re-trigger the workflow get the same comment without spam — the runner deduplicates by prior comment hash before posting.
- **Rate limit.** Audit-bot v2 only triggers on `pull_request` events, not `pull_request_target` — so a malicious fork-PR can't run our LLM call against our API key. The merge-base of the comparison is verified to be on `main`.

## Cost projection

At today's PR cadence (~5-10 PRs/month touching contracts), Haiku 3.5 at $0.01/PR is **$0.05-0.10/month**. At 10× scale (50-100 PRs/month, post-Junoswap-fork-rebuild + v2 of multiple contracts): $0.50-1.00/month. Budget-irrelevant.

## Rollout sequencing

1. **Week 1.** Land this design doc. Add `docs/AUDIT_BOT_V2_LINT_PROMPT.md` extracted from §"The six patterns" above, ready for the runner script to embed verbatim.
2. **Week 2.** Land Phase 1 — write `.github/workflows/audit-bot-v2.yml` (or extend v1) with the lint job; add the runner script `scripts/audit-bot-lint.{sh|ts}`.
3. **Weeks 3-6.** Phase 2 measurement — silent runs, manual triage; weekly TP/FP/TPN counts logged in `memory/audit-bot-v2-tracking.md`.
4. **Week 7+.** Phase 3 — promote zero-FP patterns to gating. Phase 4 self-improvement runs as a quarterly review.

## Open questions for the user

1. **API key custody.** Where does the LLM API key live? Recommendation: GitHub Actions secret named `AUDIT_BOT_LLM_KEY`, scoped to this repo, regenerated quarterly. Alternative: fund a paid Claude Code / OpenAI Codex token that's already in the operator's secret rotation.
2. **Self-host vs API.** Start API-based for simplicity; self-host as a Phase 5 milestone if cost or privacy concerns surface.
3. **Forks-to-monitor list.** Should v2 also lint PRs into upstream `dao-contracts` or `juno` that we're tracking? (Probably no — out-of-repo PRs aren't ours to gate. But emitting a "review-recommended" advisory is feasible if we wire a separate workflow.)

## Cross-references

- [`.github/workflows/audit-bot.yml`](../.github/workflows/audit-bot.yml) — v1 gate.
- [`memory/deterministic-audit-benchmark.md`](../memory/deterministic-audit-benchmark.md) §"Cross-cutting patterns" — pattern catalogue.
- [`memory/SESSION_PROTOCOL.md`](../memory/SESSION_PROTOCOL.md) §3 T10 — open thread.
- Per-contract `DETERMINISTIC_AUDIT.md` files under `contracts/*/` — the worked-example source-of-truth that the lint pass reproduces in mechanical form.

---

*Apache-2.0. Design only. Implementation lands in a follow-up PR with the runner script.*
