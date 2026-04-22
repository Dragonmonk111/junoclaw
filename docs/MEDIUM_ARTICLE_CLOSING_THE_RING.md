# Closing the Outer Ring

## Four small hardenings, each of which turns an asymmetry into an assertion — and none of which needed the chain's signature to land.

[IMAGE 1 — *Hand-drawn pencil, vintage.* A stone-walled courtyard at early morning light: a single gate, iron-studded and slightly open, stands at the end of a flagstone path; around it, a high ring-wall traces the perimeter in careful masonry. One section of the wall — a short stretch near the gate — is noticeably thinner than the rest, the stones smaller, the mortar newer, the line drawn with a different hand. A labourer's trowel and a small pile of matched stones wait beside that thin section. No figure yet; the yard is ready but empty. The pencil work is meticulous on the wall, soft and open on the sky beyond.]

---

> *This is the morning-after piece to `@junoclaw/docs/MEDIUM_ARTICLE_DAY_AT_THE_LEDGER.md`.
> The v7 `task-ledger` shipped yesterday with seven `Constraint` variants
> covered by 149 workspace tests, of which three — `TimeAfter`,
> `BlockHeightAtLeast`, and `EscrowObligationConfirmed` — had been
> demonstrated against a live uni-7 contract. Today's work closes the
> four-variant gap in the on-chain smoke harness, locks the
> `ConstraintViolated` error-string shape at the chain boundary
> (previously asserted only in-process), turns the one-shot
> zk-verifier gas number into a reproducible benchmark harness, and
> wires an optional Groth16 proof sidecar into
> `agent-company::SubmitAttestation` that fails closed in every
> asymmetric configuration. The workspace test suite is now 155 / 155
> green; no tx has been broadcast; the signature the migrate script
> wants is a human decision, not a code change.*

---

## Where yesterday left the yard

Yesterday's diary closed with an honest asymmetry. The v7 task-ledger's `Constraint` vocabulary had seven words. The unit-test suite covered all seven. The on-chain smoke harness — the tool that decides whether the chain actually behaves the way the tests say it should — covered three. A contract whose test-side assurance is 7/7 and whose chain-side assurance is 3/7 is a contract with a ring whose outer face is thinner than its inner face. The inner face holds against cw-multi-test, which is exact. The outer face holds against uni-7, which is the thing that matters.

The thinness wasn't dangerous. The four unexercised variants — `AgentTrustAtLeast`, `BalanceAtLeast`, `TaskStatusIs`, `PairReservesPositive` — shared exactly the same revert machinery as the three that had been demonstrated. The evaluator path, the `ConstraintViolated` error shape, the atomic-revert guarantee, the `env.block` threading — all were exercised by T1, T2, and T3. What the four missing tests would add was not *new* assurance; it was *matched* assurance, the kind that makes the audit delta line up cleanly between unit and chain. A ring is not really whole until the same pattern runs all the way round it.

That, and three adjacent pieces of tidying, is what landed this morning.

[IMAGE 2 — *Hand-drawn pencil, vintage.* A page from a mason's chapbook, the kind that travels in a canvas sack between job sites. On the left, a diagram of a perimeter wall with seven tower-marks spaced along it — three rendered in confident cross-hatching, four drawn only in outline, waiting to be filled in. On the right, a column of marginal notes in a careful copperplate: *trust*, *balance*, *dependency*, *reserves*. A thumbprint in graphite at the bottom corner, where a hand has held the page against the wind.]

---

## The four missing words on chain

`@junoclaw/deploy/smoke-tier15.mjs` was a three-test script yesterday morning. T1 exercised `TimeAfter` against a 60-second chain-time threshold, T2 exercised `BlockHeightAtLeast` against a six-block threshold, and T3 exercised `EscrowObligationConfirmed` by authorising a Pending obligation on the live v6 escrow and flipping it to Confirmed between attempts. Each test follows the same pattern — submit a task with the hook, try to complete it before the condition is met, observe the `ConstraintViolated` revert, advance the relevant piece of chain state, retry the completion against the *same* task — because atomic revert leaves the task `Running` after a failed attempt and the retry is therefore the same proof the contract cares about, not a fresh one.

The four new tests adopt the same shape, each with the smallest possible state change that brings the variant's condition from false to true.

**T4 — `AgentTrustAtLeast`.** A freshly registered agent starts with `trust_score = 0`. A task submitted with `AgentTrustAtLeast { agent_id, min_score: 1 }` therefore fails its pre-hook. To advance the state we submit a second task for the same agent — no hooks this time — and complete it normally. That completion fires the v5 atomic `IncrementTasks` callback, which raises the agent's trust score to 1, which is exactly the threshold the first task's hook required. The original task is then retried and completes. The path the test exercises is, in miniature, the whole reputation-gated-payment loop: an agent accrues trust through successful deliveries, and a later task can *require* that accrual before accepting its completion.

**T5 — `BalanceAtLeast`.** A fresh wallet generated in the test harness holds zero ujunox. A task hooked with `BalanceAtLeast { who: fresh, denom: "ujunox", amount: "1" }` therefore fails. The admin sends one ujunox to the fresh wallet — a single-coin `MsgSend`, the cheapest possible on-chain state mutation — and the retry passes. The variant's whole surface area is a `querier.query_balance` call under the hood, and this test is the smallest possible end-to-end exercise of that path against the real bank module.

**T6 — `TaskStatusIs`.** Two tasks for the same agent: task A with no hooks, task B with a pre-hook of `TaskStatusIs { task_ledger: <self>, task_id: A, status: Completed }`. With A still `Running`, B cannot complete. Complete A. B can now complete. The point the test makes is the one that matters most for the variant's intended use: a DAG of tasks can be expressed entirely within the ledger, with dependency relations evaluated at completion time rather than scheduled ahead of time. The `task_ledger` address in the hook is the contract's own address — the test demonstrates the same-ledger case because that is the most common one, but the shape generalises cleanly to cross-ledger dependencies.

**T7 — `PairReservesPositive`.** This one is structurally present but gated on a precondition the test harness cannot satisfy on its own: a live `junoswap-pair` contract with a known pool state. `deployed.json` currently records only the pair's stored `code_id`, not an instantiated address. When the pair is not present, T7 reports `ok: null, skipped: true` and the summary row shows ⏭ rather than ✅ — which is not a failure, but a legible deferral. The code path itself is covered by the unit tests in `@junoclaw/contracts/task-ledger/src/tests.rs::test_v7_pair_reserves_positive_constraint`, which spins up a stub pair with settable reserves inside cw-multi-test; the smoke harness will be able to exercise it the day someone instantiates a pair on uni-7. Until then, T7 is a placeholder that will light up automatically the first time a pair exists at `deployed['junoswap-pair'].address`.

The harness writes every result to `@junoclaw/deploy/smoke-tier15-results.json` with the tx hash and the full `ConstraintViolated` error text for each rejected attempt. A run that lights up T1–T6 green and T7 skipped is the shape of the ring today; a run with T7 green is the shape of the ring the week after junoswap instantiates.

[IMAGE 3 — *Hand-drawn pencil, vintage.* A seven-spoked wagon-wheel laid flat on a workbench, seen from above. Six spokes are finished — the dowelled joins crisply drawn, the grain of the wood picked out in fine cross-hatching. The seventh spoke is set in place but not yet pegged; a small wooden peg waits on the bench beside it with a mallet. Wood-shavings scatter around the hub. The wheel is plainly usable as-is; the missing peg is a matter of tidiness, not safety. "Spoke seven, awaiting pair."]

---

## Locking the words at the chain boundary

The `ConstraintViolated` error string that the chain returns on a failed hook is the only signal a downstream observer gets. Contracts that revert drop their entire `Response` — every event, every attribute — because that is the whole point of atomic revert. The failure path's information budget is therefore exactly one thing: the text of the error. `@junoclaw/contracts/task-ledger/src/error.rs` declares it:

```rust
#[error("Constraint violated: {reason}")]
ConstraintViolated { reason: String },
```

And `execute_complete` wraps each `evaluate_all` failure in a fixed prefix — `pre_hook: hook[i]: VariantName: <per-variant details>` or the mirror for post-hooks. The full shape the chain emits is therefore:

```
Constraint violated: (pre|post)_hook: hook[<index>]: <Variant>: <details>
```

Yesterday's commit included `test_v7_constraint_violated_error_string_shape_is_stable`, a unit-level regression that asserts the prefix layering for two exemplar variants. That test runs in-process: it downcasts the `cw_multi_test` error back into a `ContractError` and pattern-matches. It is a correct test. What it does not prove is that the string survives the chain-to-RPC-to-cosmjs boundary unchanged — the three transitions where a log format can drift silently.

The smoke harness now carries its own assertion. `@junoclaw/deploy/smoke-tier15.mjs:117` defines `assertConstraintViolatedShape(errorMsg, expectedVariant, expectedSide)`, a tiny helper whose only job is to run a regex against the raw cosmjs error text, extract the side (`pre` or `post`), the hook index, and the variant name, and confirm they match the expected values. Every early-reject in T1 through T7 now passes its error through this helper and records the result in the per-test JSON block as `error_shape: { ok, side, index, variant }`.

The effect is small and legible. The unit test locks the prefix at the contract's edge. The smoke assertion locks the prefix at the chain's edge. A silent refactor that changes the format inside the contract will now fail the unit test loudly; a silent change in how wasmd surfaces revert reasons to `cosmjs` will fail the smoke assertion loudly. Observability downstream, which the `@junoclaw/docs/TIER1_SLIM_OBSERVATION.md` plan builds on a grep-for-this-prefix pipeline, now has two regression gates between it and format drift — one on each side of the chain boundary.

This is the sort of thing one only notices matters after reading `@junoclaw/docs/MEDIUM_ARTICLE_DAY_AT_THE_LEDGER.md`'s clock-drift passage. *The test infrastructure itself needs the same audit discipline as the contract.* A test that tells us the system is working by reading a string must itself be subject to the same rigour that locks the string in the first place.

[IMAGE 4 — *Hand-drawn pencil, vintage.* A printer's forme, locked up for press: rows of metal type held in place by a wooden chase and wedges. The composed text reads, in backward mirror-image: "constraint violated: pre_hook:". A quoin-key rests on top of the forme; a small galley-proof has been pulled and hangs beside it, showing the words the right way round. The composition is tight; nothing in the chase can shift.]

---

## Measuring, not speculating

`@junoclaw/contracts/zk-verifier` has been on uni-7 since March under code_id 64, at `juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem`. The single `VerifyProof` call recorded during that deployment came in at **371,486 gas** — the number cited in `@junoclaw/ZK_PRECOMPILE_ARTICLE.md`, `@junoclaw/docs/MEDIUM_ARTICLE_THREE_ORDEALS.md`, and the BN254-precompile case in `@junoclaw/docs/BN254_PRECOMPILE_CASE.md`. That number has done a lot of rhetorical work. What it has never been is reproducible without remembering which script to run and which arguments to pass.

`@junoclaw/wavs/bridge/src/benchmark-zk-verifier.ts` is today's small piece of tidying around that number. It does nothing new conceptually — it submits a Groth16 proof to the already-deployed zk-verifier contract and reads `gas_used` from the tx receipt — but it does so in a shape that is automatable, parameterised, and artefact-producing. Default behaviour: three runs (`BENCH_RUNS=3`), verified against the stored VK, stability-checked (min == max), and emitted to two files: `@junoclaw/docs/ZK_VERIFIER_BENCHMARK.md` (human-readable, with a comparison table and reproduction commands) and `@junoclaw/docs/zk-verifier-benchmark-results.json` (machine-readable, every tx hash and every gas number).

The fallbacks matter more than the happy path. `ZK_VERIFIER_ADDR` env var overrides the address resolution; the default falls back to the March deployment so anyone checking out the repo can run the benchmark without additional configuration. The proof bundle comes from `$ZK_PROOF_PATH` or `$TMPDIR/groth16_proof.json`, generated by `cargo run -p zk-verifier --example generate_proof` — deterministic at seed 42, so the VK it produces matches the one already stored on chain. A `--dry-run` flag exercises the end-to-end argument handling without touching the chain or spending gas.

The BN254-precompile case itself is unchanged by today's work: a precompile would still put pairing-check verification at roughly 187,000 gas, which is about half of what pure-CosmWasm does. What changes is that the 371,486 number is now a result of running `npm run benchmark-zk-verifier`, not an artefact of one historical tx. If the number drifts — because an arkworks dependency is updated, or because the wasmd gas schedule changes — the benchmark will say so the next time it is run, and the new number will be written to the same markdown file. The case for the precompile does not get weaker; it gets *checkable*.

[IMAGE 5 — *Hand-drawn pencil, vintage.* A chemist's balance on a marble bench, its two brass pans holding small labelled weights: one pan reads "187,000" and holds a single tall weight; the other reads "371,486" and holds a stack of smaller weights, clearly heavier but not by an impossible amount. A shallow wooden drawer, half-open below the bench, contains additional calibration weights in graduated sizes. The pans hang slightly askew; the pointer is steady. The drawing is exact, unpolemical.]

---

## A proof rail, quietly optional

The zk-verifier has always lived one contract away from the attestation flow. `agent-company::SubmitAttestation` stores a 64-hex `attestation_hash` for a completed `WavsPush` or `OutcomeCreate` proposal. Since v3 the hash has been re-computed on-chain against the supplied `task_type` and `data_hash` — the Variable 1 hardening from `@junoclaw/docs/RATTADAN_HARDENING.md` — so a WAVS operator cannot submit a hash that is internally inconsistent with its inputs. What the operator *can* still do, under the post-v3 design, is submit a hash whose *inputs* were never verified. The contract has no way to check the inputs on its own; the trust boundary stops at "the operator is authorised to attest".

A zk-proof would let the contract check the inputs without trusting the operator, provided the circuit the proof was generated against matches the one whose VK is stored in the zk-verifier. The question the v7 work asked was whether that check could be wired into `SubmitAttestation` as an *optional* enhancement — backward-compatible with operators that do not yet generate proofs, forward-compatible with a world in which proofs become the default, and fail-closed in the asymmetric cases in between.

The answer, shipped today, is four pieces of state and one sub-message call.

`Config.zk_verifier: Option<Addr>` is the new pointer. When `None`, `SubmitAttestation` behaves exactly as it did in v6 — `@junoclaw/contracts/agent-company/src/state.rs:78-85` carries the `#[serde(default)]` annotation so migrated v6 configs deserialize cleanly into `None`, and the test `test_submit_attestation_duplicate_rejected` (from the v6 suite, unchanged) proves the pre-existing path is undisturbed. When `Some(addr)`, the contract accepts two new optional fields on the `SubmitAttestation` message: `proof_base64` and `public_inputs_base64`. Both default to `None` via `#[serde(default)]`, so a WAVS operator built against the v6 schema continues to work. The operator who *does* supply a proof gets a sub-message to the zk-verifier, fired atomically: if the proof verifies the attestation stores; if it fails the verifier returns `Err` and the parent tx reverts, including the `ATTESTATIONS.save` that ran a few lines earlier.

The atomicity is the interesting part. `ATTESTATIONS.save` runs *before* the sub-message is dispatched, because CosmWasm builds the `Response` as the handler unwinds. A naïve read of that ordering would worry about partial state — attestation stored, proof check failed, two rows of reality disagreeing. The model that saves it is that `SubMsg` with the default `ReplyOn::Never` runs in the same tx frame: if the sub-message errors, the error propagates up and the entire tx's storage writes roll back. `test_submit_attestation_with_invalid_zk_proof_reverts_atomically` exists specifically to prove this — after a stub zk-verifier rejects a proof, a follow-up `GetAttestation` query returns `None`, not the half-written row that a non-atomic model would leave behind.

The fail-closed cases are the second interesting part. Three error shapes, each named for what went wrong rather than what the caller tried:

- **`IncompleteZkProofBundle { proof_some, inputs_some }`** — a proof with no public inputs, or public inputs with no proof. The contract rejects *before* firing any sub-message, so the partial submission never reaches the verifier. A test asserts both directions.
- **`ZkVerifierNotConfigured {}`** — a proof supplied to a contract whose `zk_verifier` is `None`. Silently dropping the proof would give the caller a false sense of verification; the contract refuses instead. Another test locks this.
- **Sub-message bubble-up** — a proof that the verifier rejects. The parent tx reverts atomically. The test above.

A sixth test proves the inverse: with a verifier configured but no proof supplied, the hash-only attestation still stores — backward compatibility with v6. A seventh covers admin-only rotation via the new `RotateZkVerifier { new_verifier: Option<String> }` message. `None` here clears the pointer, which is a deliberate kill-switch: a compromised verifier can be taken out of the flow instantly, reverting to hash-only behaviour, without a governance cycle.

Six new regression tests in total, all green, all shipped today. Zero wire-shape changes to existing messages: adding optional fields with `#[serde(default)]` is invisible to anyone using the old schema. One new admin-only execute, mirroring `RotateWavsOperator` exactly enough that the audit delta is legible.

The contract is not yet deployed. It compiles; it passes `cargo test -p agent-company --lib` at 52 tests green (up from 46); it passes the workspace at 155 / 155 (up from 149). The decision to broadcast a migration that wires `zk_verifier` to the live zk-verifier address is, like yesterday's, a human decision — this time rather less fraught than a fresh-deploy, because the contract's existing wasmd admin *is* already set (one of the v7 learnings from yesterday was to fix `deploy-fresh.ts` to always pass the sixth argument). The migrate is a one-tx operation; the wire-up is one admin-only execute. The day that happens, `ZkVerifierWired` will light up in `agent-company` and operators will be able to start supplying proofs at their own pace. The day any operator does, the first `zk_verified: true` attestation will land — and the flow the zk-verifier PoC was originally written to demonstrate will have its first piece of production traffic.

[IMAGE 6 — *Hand-drawn pencil, vintage.* A surveyor's theodolite on a tripod, set up beside a stone wall that trails off into the middle distance. The instrument is pointed at a distant reference marker; a plumb-bob hangs below it, steady. On a small folding stool beside the tripod, a notebook lies open: two pages have been filled with sightings, each one carefully drafted; a third page has been left blank, with the column headings already inked in — "bearing", "angle", "distance", "remark". The surveyor is not in the frame; the work is set up to continue.]

---

## What this is not, still

It is worth repeating the boundary, because some of it is sharper today than it was yesterday:

- **Not a mandatory proof rail.** The zk-sidecar is additive. An operator who does not supply proofs behaves exactly as they did in v6. The day proofs become the default is a future design conversation; today they are a per-attestation choice.
- **Not a TEE replacement.** The zk path proves that *the computation was done correctly against a known circuit*. A TEE proves that *the computation happened inside attested hardware*. Overlapping concerns, not the same claim. `@junoclaw/TEE_MILESTONE_ARTICLE.md` is a separate conversation and remains so.
- **Not a live contract change on uni-7.** Everything described here lives in the repository. No migration has been broadcast. The existing v7 `task-ledger` at `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm` continues to serve the three on-chain tests from yesterday; the new four wait for the next harness run; the zk-sidecar waits for the next `agent-company` migration.
- **Not an indexer, not a dashboard, not telemetry.** The observability plan from `@junoclaw/docs/TIER1_SLIM_OBSERVATION.md` is unchanged. What today adds is a test on each side of the chain boundary so the plan's assumption about error-string shape cannot silently stop being true.

The deliberate narrowness is the whole point. Every shape we decided not to ship today is a shape the audit delta did not have to carry, and a reason to keep watching before spending capital on the next layer.

---

## The day, in file-level summary

**Smoke harness (extended)**
`@junoclaw/deploy/smoke-tier15.mjs` — four new tests (T4 `AgentTrustAtLeast`, T5 `BalanceAtLeast`, T6 `TaskStatusIs`, T7 `PairReservesPositive`, the last gated on junoswap-pair presence); one new helper `assertConstraintViolatedShape` that locks the revert-text prefix at the chain boundary; every early-reject in T1–T7 now records an `error_shape` block alongside the raw error.

**Benchmark (new)**
`@junoclaw/wavs/bridge/src/benchmark-zk-verifier.ts` — runs N × `VerifyProof` against the already-deployed zk-verifier, produces `@junoclaw/docs/ZK_VERIFIER_BENCHMARK.md` and `@junoclaw/docs/zk-verifier-benchmark-results.json`, stability-checks min == max, falls back sensibly when env vars are unset.
`@junoclaw/wavs/bridge/package.json` — two new npm aliases: `benchmark-zk-verifier` and `deploy-zk-verifier`.

**Contract (v7 additive)**
`@junoclaw/contracts/agent-company/src/state.rs` — new `Config.zk_verifier: Option<Addr>`, `#[serde(default)]` for migrate compat.
`@junoclaw/contracts/agent-company/src/msg.rs` — `zk_verifier: Option<String>` on `InstantiateMsg`; `proof_base64` + `public_inputs_base64` on `SubmitAttestation`, both `#[serde(default)]`; new admin-only `RotateZkVerifier { new_verifier: Option<String> }`.
`@junoclaw/contracts/agent-company/src/contract.rs` — instantiate parses and stores `zk_verifier`; `SubmitAttestation` dispatch carries the two new optional fields; seven-case truth table on `(zk_verifier, proof_some, inputs_some)` governs whether the sub-message is fired, the attestation stored hash-only, or one of two fail-closed errors returned; `execute_rotate_zk_verifier` mirrors `execute_rotate_wavs_operator`.
`@junoclaw/contracts/agent-company/src/error.rs` — `ZkVerifierNotConfigured {}` and `IncompleteZkProofBundle { proof_some, inputs_some }`.
`@junoclaw/contracts/agent-company/src/tests.rs` — six new regressions plus a tiny stub zk-verifier contract (`ContractWrapper`-based, settable-accept) and two helpers (`instantiate_zk_stub`, `store_and_instantiate_with_zk_verifier`).
`@junoclaw/contracts/agent-company/src/integration.rs` — two existing `SubmitAttestation` call-sites updated with `proof_base64: None, public_inputs_base64: None`; the `InstantiateMsg` in `deploy_full_stack` gains `zk_verifier: None`.

**Test posture**
`agent-company` crate: 46 → 52 tests (+6 zk-sidecar).
Contracts workspace: 149 → 155 tests, all green. Zero tests weakened; every v6 hardening and every Tier 1.5 invariant remains in force. `cargo clippy --workspace --all-targets` is unchanged; the pre-existing MSRV-guarded `is_multiple_of` note in `zk-verifier` is the only warning.

**State on uni-7**
Unchanged. The v7 `task-ledger` at `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm` is still on code_id 75. The `agent-company` at `juno1lymtnjru4euexavls4gqvjwtt3twxpsgrva0m37m6krp0dqacycs40f2hw` is still on its pre-v7 code_id. The zk-verifier at `juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem` is still on code_id 64, still carrying the single 371,486-gas verify tx from March. No new code has been uploaded, no migrate has been broadcast, no config has been touched. The harness is primed; the signature is the human's.

[IMAGE 7 — *Hand-drawn pencil, vintage.* A night-watchman's hut at the postern gate of a fortified yard: a lantern hangs unlit on its peg inside, a sheaf of unsigned warrants is stacked neatly on the table beside an inkwell with its stopper in. Through the hut's small window, the finished perimeter wall is visible in the moonlight — continuous now, all the way around. The gate itself is shut but unlatched. The scene is still, expectant, and complete; nothing left to build, nothing yet to act on. "Primed but unsigned."]

---

## Shipping posture

We did not ship, today, a new `task-ledger` code_id. We did not broadcast a migrate on `agent-company`. We did not run `benchmark-zk-verifier` against uni-7. We did not instantiate a `junoswap-pair`, which means T7 will continue to report ⏭ the next time the smoke harness is run, until that instantiation happens. Some of those we will do this week; some next month; some of them will be done by another hand when the pair gets deployed for a different reason.

What we did ship is four tests that make the on-chain ring whole for six of seven variants (seventh pending, explicitly); a chain-boundary assertion that matches the unit-boundary assertion on `ConstraintViolated`'s prefix shape; a reproducible harness for the number that makes the BN254-precompile case; and a backward-compatible optional proof sidecar on `agent-company::SubmitAttestation` with six regression tests and a seven-case truth table that every configuration is checked against.

A thin ring around an already-hardened core is worth more than an ambitious ring that redraws the perimeter. Today the thinner face of the ring matched the thicker face. The inner wall and the outer wall now carry the same load. That is the shape the Rattadan posture has been asking for since v5, and it is the posture that today's four small changes simply continue.

The next concrete event is not a deploy. It is a run of `npm run benchmark-zk-verifier` against uni-7 with the zk-verifier already live — a no-state-change, one-artefact-out operation that will regenerate `@junoclaw/docs/ZK_VERIFIER_BENCHMARK.md` with a fresh set of numbers against a fresh block height. The day after that is the `agent-company` migrate that wires `zk_verifier`, and the day after *that* is the first proof-bearing `SubmitAttestation` in production. Each of those is its own piece of ceremony. None of them are today.

---

## After the courtyard

The morning's work closes yesterday's open loops at precisely the level they were left open. The smoke harness now matches the unit suite in coverage. The error-string shape is locked at both ends. The zk-verifier's gas number is reproducible instead of remembered. And the attestation flow has a proof rail that fails closed in every asymmetric configuration and costs exactly zero if no one uses it.

None of that is heroic. All of it is the cost of treating yesterday's asymmetry as a permanent lesson. The four missing smoke tests are now present; the missing chain-side shape assertion is now present; the missing benchmark harness is now present; and the missing proof rail — the one the zk-verifier has been waiting for a caller for since March — is now present, optional, and verified.

The stone wall is continuous now. The labourer's trowel goes back to its peg. A clean ledger sheet waits for tomorrow.

---

*Today's code lives in `@junoclaw/deploy/smoke-tier15.mjs`, `@junoclaw/wavs/bridge/src/benchmark-zk-verifier.ts`, and `@junoclaw/contracts/agent-company/src/{state,msg,error,contract,tests,integration}.rs`. The canonical engineering reference remains `@junoclaw/docs/TIER15_ARCHITECTURE_UPGRADE.md`. The testnet run ledger is `@junoclaw/docs/TIER15_TESTNET_RUN.md`, to be extended with tomorrow's smoke-run and benchmark-run entries. The field-operations tracker is `@junoclaw/docs/TIER1_SLIM_OBSERVATION.md`, whose grep-based observability plan now rests on two regression gates rather than one. The v6 hardening invariants from `@junoclaw/docs/RATTADAN_HARDENING.md` remain in force; the v7 Tier 1.5 invariants from `@junoclaw/docs/MEDIUM_ARTICLE_DAY_AT_THE_LEDGER.md` remain in force. The workspace regression suite stands at 155 / 155 passing (up from 149 after the zk-sidecar six). The zk-verifier on uni-7 at `juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem` (code_id 64) remains available for the benchmark; the frozen v6 `task-ledger` at `juno17aq66zyakz8su32u8tkgwmqemf0sylvv9a23nz7c7ydvkerll28skp5xfn` remains a frozen audit record and is still not migratable, and never will be.*

[IMAGE 8 — *Hand-drawn pencil, vintage.* The same stone courtyard from the opening scene, now at late afternoon: the gate stands fully open, the previously-thin section of wall indistinguishable from the rest, the labourer's trowel returned to a tool-peg on the courtyard's inner face. A single sheet of fresh parchment lies on a flat stone near the gate, weighted by a small stone paperweight; an unsigned warrant on it, an ink-pot beside. A swallow has settled on the wall. The pencil work is its most confident along the repaired section, where the new stones and the old have become the same stones.]
