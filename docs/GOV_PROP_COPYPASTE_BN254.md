# Governance Proposal — BN254 Precompile for CosmWasm

*Signaling Proposal for Juno Network (juno-1) — post-#373*

> **DRAFT — DO NOT SUBMIT until:**
> 1. HackMD published with stable URL and the URL is pasted into FIELD 2 below (search for `<PASTE_HACKMD_URL_HERE>`).
> 2. Upstream CosmWasm PR is opened and PR number is inserted below (search for `<PASTE_PR_NUMBER_HERE>`) — *or* a sentence added confirming the PR is being opened in parallel.
> 3. Cosign decision has resolved — either Jake has reviewed the HackMD, or the 7-day solo clock has expired.
> 4. Deposit wallet has ≥ 5 000 JUNO + a few JUNO for gas.

---

## FIELD 1: TITLE

*(paste this line into the Title field — max 200 chars on juno-1)*

```
BN254 Precompile for CosmWasm — Cheap On-chain Groth16 Verification (post-#373)
```

---

## FIELD 2: DESCRIPTION

*(paste everything between the dashed lines into the Description / Summary field — keep under 10 000 chars; currently ~5 900)*

---

This proposal asks Juno governance to signal support for adding a small piece of standard cryptographic plumbing — a **BN254 pairing precompile** — to the CosmWasm virtual machine that Juno uses. The precompile makes on-chain zero-knowledge-proof verification on Juno roughly **twice as fast and twice as cheap**.

**No funds are requested. No code executes on-chain from this vote.** This is a signaling proposal only. If it passes, the code change is proposed upstream to CosmWasm — the shared VM used by dozens of Cosmos chains — with a Juno community mandate behind it. A separate future proposal would then ask validators to approve the actual software upgrade that carries it.

**Full technical HackMD (tables, risks, reproduction):** <PASTE_HACKMD_URL_HERE>

**Upstream CosmWasm PR:** <PASTE_PR_NUMBER_HERE>

**Medium synthesis (post-#373 engineering diary):** https://medium.com/@tj.yamlajatt/hardening-after-a-91-71-yes-on-proposal-373-b46d2939461f

**Source (Apache-2.0):** https://github.com/Dragonmonk111/junoclaw

---

**Why this matters, beyond the numbers**

Cheap on-chain zero-knowledge verification is a precondition for two directions the Juno community is actively discussing:

1. **Secure cross-chain bridges.** If JUNO tokens flow to Base and Ethereum — as the recently proposed 10M JUNO liquidity incentive envisages — the bridges securing those flows are either multi-signature (historically fragile: Ronin, Wormhole, and Nomad together lost over a billion dollars) or zero-knowledge-verified (provably secure). Cheap BN254 makes the secure option affordable. Without it, Juno defaults to the fragile kind.

2. **AI agents that can prove what they did without leaking data.** Today, agent attestation is all-or-nothing: publish prompts, model weights, and outputs — or trust the operator. With cheap ZK verification, an agent can prove on-chain that its output complies with policy, without revealing the private inputs that produced it.

At 371,486 gas today, zero-knowledge verification on Juno consumes about 3.7 % of a whole block. That is too expensive to require on every important action. At ~187,000 gas it becomes affordable to require on every one. Cheap enough to be mandatory is the security property; the 2× speed-up is its shadow.

---

**Scope relative to Proposal #373**

Proposal #373 (91.71 % YES, 24 March 2026) recognised JunoClaw as Juno ecosystem infrastructure on the strength of TEE-attested verifiable agents, the Junoswap revival, and the validator-sidecar track. #373 did not explicitly ratify on-chain zero-knowledge verification as a second supervisory layer over agent behaviour. That frame has been built out since, and is what this proposal asks Juno governance to endorse next.

**#373 said yes to the TEE half. The ZK half is new, and is properly put to a separate vote.**

---

**The measured gas delta**

| Path | Gas per proof verification |
|---|---|
| Pure-CosmWasm (live on uni-7 today) | **371 486 gas** (tx F6D5774E…5080F4DA) |
| BN254 precompile (Ethereum EIP-1108 schedule) | **~187 000 gas** |
| Reduction | **~1.99×** |

Measured against the live `zk-verifier` contract at `juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem` (code_id 64). Reproducible from a clean checkout with the benchmark harness in the source repository.

---

**Precedent — this is not new technology**

| Chain | BN254 precompile | Since |
|---|---|---|
| Ethereum | Yes (0x06 / 0x07 / 0x08) | 2017 Byzantium, repriced 2019 |
| Sui | Yes (`sui::groth16`) | 2023 launch |
| CosmWasm | Not yet | — |

BN254 has 12+ years of cryptanalysis, is the curve every Ethereum zk-rollup and privacy protocol uses, and its reference library `ark-bn254` is audited upstream.

---

**What is already shipped (origin/main, commit 6d92067, 22 April 2026)**

- Host-function crate in Rust — 22 of 22 tests passing (unit + EIP-196/197 conformance vectors).
- Guest-side shim so contracts can call the host functions without a cosmwasm-std fork.
- Five unified diffs against CosmWasm/cosmwasm v2.2.0 and wasmvm v2.2.0, feature-gated behind a new `cosmwasm_2_3` flag — no major version bump, existing contracts compile unchanged.
- Feature-gated variant of the live zk-verifier contract (code_id 64) for A/B comparison.
- Ephemeral single-validator devnet (Dockerfile) that applies the patches and builds junod v29.
- Reproducible benchmark harness writing to `docs/BN254_BENCHMARK_RESULTS.md`.
- Upstream PR body drafted and ready at `docs/WASMVM_BN254_PR_DESCRIPTION.md`.

---

**Plan of record if this passes**

1. Open the upstream PR against CosmWasm/cosmwasm and CosmWasm/wasmvm.
2. CosmWasm maintainer review (weeks, not days).
3. On merge, a separate `MsgSoftwareUpgrade` proposal asks validators to adopt a junod build carrying the patch.
4. Post-upgrade, one `MsgMigrateContract` swaps the live zk-verifier from its current backend to the precompile variant. Same contract address, same interface, same admin, same verification key — only the internal dispatch changes.

---

**What this proposal does not ask for**

- No community-pool funds — the work is done.
- No on-chain code execution from this vote.
- No CosmWasm major-version bump.
- No new trust assumptions.
- No validator coordination today — that is the separate software-upgrade step, gated on both this vote and the upstream merge.

---

**Risks and mitigations**

- Consensus break on upgrade — host functions are new imports; existing contracts unchanged; standard upgrade-review gate applies.
- Denial-of-service via malicious pairing input — input capped at 64 pairs (~2.2M SDK gas) at the VM boundary.
- Pure-Wasm vs precompile divergence — differential test over 1,000 random proofs asserts identical accept/reject behaviour.
- Upstream rejects the PR — the host crate is vendorable into a Juno-specific `wasmd` fork; upstream merge is preferred, not required.

---

**Audit**

Effective audit surface is small by design: ~400 lines (host-function glue + upstream patches). The reference crate is ~90 % tests and documentation. The cryptographic library beneath (`ark-bn254`) is audited upstream. Rough audit estimate: **$15–25k, 1–2 weeks** with Oak Security, Halborn, or Informal Systems. To be funded via DAO treasury post-mainnet, per #373's plan.

---

**Voting guide**

- YES — ship native BN254, in line with Ethereum precedent and the #373-endorsed ZK track.
- NO — prefer a different curve. Please comment so the alternative can be addressed before any resubmission.
- ABSTAIN — counted for quorum, no view.
- VETO — harmful to Juno's direction. Please leave an on-chain comment so it can be addressed.

**Bottom line for non-developer voters:** A YES vote tells the Juno community "yes, go pursue the upstream code change." Nothing happens on-chain from this vote itself. If accepted upstream, a separate future proposal asks validators to approve the actual software upgrade.

---

**Authorship**

Proposed by VairagyaNodes — Juno staker since December 2021, validator candidate (unbonded). Reference implementation, patches, and documentation written by Cascade (pair-programming AI agent) at the proposer's direction.

Cosignature pending per #373 precedent — see the HackMD.

License — Apache-2.0 throughout.

---

## FIELD 3: DEPOSIT

**Recommended: 5 000 JUNO** (same shape as #373, which returned in full on pass; 5 000 recycled from that return).

Juno-1 minimum deposit is **250 JUNO** as of the last check — verify on https://ping.pub/juno/gov against any live proposal before submitting. A deposit ≥ minimum starts the voting period immediately; a deposit below minimum opens a 14-day deposit window that can be topped up by any wallet.

Same split as #373 is fine if preferred: 1 000 JUNO initial submission + 4 000 JUNO top-up from the proposer wallet within minutes.

---

## Pre-submission checklist

- [ ] HackMD published at stable `hackmd.io/s/...` URL.
- [ ] HackMD URL pasted into FIELD 2 above (replace `<PASTE_HACKMD_URL_HERE>`).
- [ ] Upstream CosmWasm PR opened; PR number / URL pasted into FIELD 2 (replace `<PASTE_PR_NUMBER_HERE>`).
- [ ] Cosign decision resolved: either Jake's edits landed on the HackMD, or the 7-day solo clock expired with documentation in the HackMD footer.
- [ ] Forum thread seeded at <https://commonwealth.im/juno> (link HackMD at top, one-paragraph summary body).
- [ ] Discord `#governance` announcement ready (lead paragraph + HackMD link + forum link).
- [ ] Telegram one-liner ready (headline number + HackMD link).
- [ ] Proposer wallet funded: ≥ 5 000 JUNO + a few JUNO for gas.
- [ ] Validators given ~24 h heads-up via Telegram that submission is imminent.

---

## Submission

Same pattern as #373 — three fields on a web frontend, no CLI required:

**Primary:** <https://station.juno.network/gov> → Submit Proposal → Text Proposal.
**Alternative:** <https://ping.pub/juno/gov> → Submit Proposal.
**Backup:** <https://app.juno.network> if either frontend is down.

Keplr / Leap signs the transaction; deposit deducts from the proposer wallet on confirmation.

---

## After submission

1. Capture the **proposal number** (e.g. #374) and the **submission TX hash** from the signer prompt.
2. Update these files with the live URL `https://ping.pub/juno/gov/<NUMBER>`:
   - `docs/HACKMD_BN254_PROPOSAL.md` (add a banner at top)
   - `docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md` (Links section)
   - `docs/TECHNICAL_PROPOSAL_BN254_SHAREABLE.md` (Links section)
   - `docs/BN254_PRECOMPILE_INDEX.md` (status table)
3. Update the Medium article (Medium allows post-publish edits) with the proposal link near the top and in the closing Links section.
4. Cross-post to Commonwealth, Discord `#governance`, and Telegram with the `ping.pub` URL + voting-period dates.
5. Commit + push the doc updates together with a single message such as *"docs: BN254 prop is live at #<NUMBER>, tx <HASH>".*
6. Open the validator-DM round asking for explicit YES intents before the voting period ends — same list used for #373.

---

## Expected timeline

| Step | Elapsed |
|---|---|
| HackMD publish + Commonwealth thread seed | Day 0 |
| Jake DM with 7-day cosign clock | Day 0 |
| Upstream CosmWasm PR opened | Day 0–1 |
| Forum feedback window | Day 0 → Day 3 |
| Cosign resolved (Jake landed OR solo) | Day 7 max |
| On-chain submission | Day 7 |
| 5-day voting period | Day 7 → Day 12 |
| Tally + deposit return | Day 12 |

Total: ~12 days from first HackMD publish to tally, dominated by the cosign clock and forum feedback window rather than on-chain timings.

---

## Rollback if the deposit burns (threshold not met)

Signaling-proposal deposits burn only on NO-with-VETO ≥ 33.4 %. In that case:

1. Read the veto comments on-chain and in the Commonwealth thread.
2. Rewrite the HackMD addressing each substantive objection — do not resubmit quickly.
3. Consider restructuring to a non-text proposal form (e.g. a `CommunityPoolSpend` for the audit portion as a separate ask) if the objection is "cart before horse."
4. Do not re-submit the same text within the same governance cycle.
