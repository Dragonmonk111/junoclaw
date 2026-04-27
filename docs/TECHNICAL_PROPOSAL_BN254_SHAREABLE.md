# BN254 Precompile for CosmWasm — Shareable Technical Proposal

> Copy-paste version for Juno Commonwealth, Discord, Telegram, HackMD. Long-form on-chain text: <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md>. Post-#373 synthesis: <https://medium.com/@tj.yamlajatt/hardening-after-a-91-71-yes-on-proposal-373-b46d2939461f>.

![Hand-painted brass balance — left pan 187,000, right pan 371,486. Oil on panel, Turner + Constable + Ghibli melange.](images/bn254_balance.png)

<!-- ART PROMPT: Oil on panel, hand-painted. Turner atmospheric romanticism + Constable pastoral precision + Ghibli warm cel-painted naturalism, equal parts. Chemist's brass balance, worn marble counter, three-quarter view. Left pan: one tall brass weight stamped '187,000'. Right pan: rough stack totalling '371,486'. Pointer steady. Window behind: soft English morning, mist off wet meadow. No human figures. Palette: lead-grey brass, Ghibli meadow-green, Turner-honey dawn. Canvas texture visible. -->

---

## ✂️  Copy between the scissors ✂️

```
════════════════════════════════════════════════════════════
  JUNO TECHNICAL PROPOSAL — BN254 PRECOMPILE FOR COSMWASM
  Next step on the prop-373-endorsed ZK roadmap
════════════════════════════════════════════════════════════
```

## TL;DR

Add three BN254 host functions — `bn254_add`, `bn254_scalar_mul`, `bn254_pairing_equality` — to `cosmwasm` + `wasmvm`, mirroring Ethereum's `0x06` / `0x07` / `0x08` precompiles. A Groth16 verify on Juno drops from **371 486 gas** (measured on `uni-7`) to **~187 000** (EIP-1108 parity). **~2× cheaper.** Bridges every existing Ethereum ZK toolchain — `snarkjs`, `circom`, `gnark`, `ark-groth16` — to Juno natively, no EVM wrapper.

**Signaling only.** No funding ask, no on-chain code change in this proposal. The upstream PR and the eventual `MsgSoftwareUpgrade` are separate, sequential steps.

## Why BN254

| Chain | BN254 pairing | Shipped |
|-------|---|---|
| Ethereum | `0x06` / `0x07` / `0x08` | 2017 Byzantium, repriced 2019 (EIP-1108) |
| Sui | `sui::groth16` native | 2023 (launch) |
| CosmWasm | **Not yet** | BLS12-381 precedent shipped in CW 2.1 (2024) |

Every production Groth16 circuit — privacy, identity, zk-rollup verifiers — targets BN254. Any other curve means asking the entire ZK ecosystem to re-run trusted-setup ceremonies. BN254 is the cheapest bridge to a mature toolchain.

## The gas delta

```
Operation                      EVM precompile   Gas (EIP-1108)
──────────────────────────────────────────────────────────────
bn254_add                      0x06             150
bn254_scalar_mul               0x07             6 000
bn254_pairing_equality         0x08             45 000 + 34 000·N

Groth16 verify (4 pairs)       ≈ 187 000 gas on patched chain
vs. pure-CosmWasm today:       371 486 gas (measured, uni-7)
                               ──────────────────────────────
                               ~2× reduction
```

Measured in tx `F6D5774EE2073E2DD011399A7E96889BA026ED67C6A510D208FD5C575080F4DA` against `juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem` (code_id 64). Reproducible with `npm run benchmark-zk-verifier`.

## What's shipped

All on `main` of <https://github.com/Dragonmonk111/junoclaw>:

- ✅ **Host crate** — `wasmvm-fork/cosmwasm-crypto-bn254/`, `no_std`-friendly Rust against `ark-bn254 0.5`, **22 / 22 tests** (13 unit + 9 EIP-196/197 conformance vectors)
- ✅ **Guest shim** — `wasmvm-fork/cosmwasm-std-bn254-ext/`, extension trait; **2 / 2 tests**
- ✅ **Upstream patches** — `wasmvm-fork/patches/`, five unified diffs vs `CosmWasm/cosmwasm` v2.2.0 + `wasmvm` v2.2.0, gated on a new `cosmwasm_2_3` feature; no major version bump
- ✅ **Feature-gated contract** — `contracts/zk-verifier/` with `--features bn254-precompile`; default build unchanged (9 / 9 tests green)
- ✅ **Ephemeral devnet** — `devnet/Dockerfile` applies the patches and builds `junod` v29 (~12 min cold, ~90 s warm)
- ✅ **Benchmark harness** — `wavs/bridge/src/benchmark-zk-verifier-devnet.ts` runs N samples across both variants, writes `docs/BN254_BENCHMARK_RESULTS.md`
- ✅ **Upstream PR body** — `docs/WASMVM_BN254_PR_DESCRIPTION.md`
- ✅ **Long-form governance proposal** — `docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md`
- ⏳ Upstream PR opened — *gated on this vote*
- ⏳ Juno signaling submitted — *gated on HackMD cosign*
- ⏳ `MsgSoftwareUpgrade` — *gated on upstream merge*

## Reproduce every claim

From a clean checkout:

```bash
cargo test -p zk-verifier --lib                                  # 9 / 9
cargo test --manifest-path wasmvm-fork/cosmwasm-crypto-bn254/Cargo.toml    # 22 / 22
cargo test --manifest-path wasmvm-fork/cosmwasm-std-bn254-ext/Cargo.toml   # 2 / 2
cargo check -p zk-verifier --features bn254-precompile            # exit 0
cd devnet && ./scripts/run-devnet.sh \
          && ./scripts/deploy-zk-verifier.sh \
          && ./scripts/benchmark.sh                               # writes BN254_BENCHMARK_RESULTS.md
```

## What this proposal does **not** ask for

- ❌ No community-pool funds — the work is done.
- ❌ No on-chain code execution — signaling only.
- ❌ No CosmWasm major-version bump — patches hide behind a `cosmwasm_2_3` feature flag; existing contracts compile unchanged.
- ❌ No new trust assumptions — BN254 has 12+ years of cryptanalysis, same curve every Ethereum Groth16 circuit uses, arkworks is library-audited.
- ❌ No validator coordination today — that's the separate `MsgSoftwareUpgrade` step, gated on both this vote *and* upstream merge.

## Plan of record if this passes

1. Open upstream PR against `CosmWasm/cosmwasm` + `CosmWasm/wasmvm`.
2. CosmWasm maintainer review (weeks, not days).
3. On merge + release, draft a separate `MsgSoftwareUpgrade` bumping Juno to the carrying `wasmd`.
4. Post-upgrade, `MsgMigrateContract` on `juno1ydxksvr…` from code_id 64 → precompile variant. Address unchanged. Next `VerifyProof` costs ~187K instead of 371K.

## Audit scope — narrow by design

The ~900-line reference crate is **~90 % tests + docs** (test-and-docs-to-code ratio **0.9**). Effective audit surface: ~90 lines of host-function glue + ~300 lines of upstream patches = **~400 lines**. The crypto (`ark-bn254 0.5`) is library-audited upstream; the 9 EIP-196/197/1108 conformance vectors give byte-exact ground truth; the patches are feature-gated so existing contracts are out of scope.

For context — recent CosmWasm audit ballparks (public figures):

| Project (auditor)          | Scope     | Cost      | Duration |
|----------------------------|-----------|-----------|----------|
| Stargaze marketplace (Oak) | ~3 500 LoC| $25–35k   | 2–3 wk   |
| DAODAO v2 (Oak)            | ~6 000    | $40–60k   | 3–4 wk   |
| Mars v2 (Halborn)          | ~12 000   | $80–120k  | 4–6 wk   |
| Stride (Informal Systems)  | ~8 000    | $150k+    | 8–12 wk  |

Typical CosmWasm projects ship at 30–50 % test-to-code; 90 % is unusual, and it shortens the audit's *"understand the intent"* phase — the intent is spelled out in the tests plus the EIP reference.

**BN254 precompile realistic estimate: $30–45k, 3–5 weeks** with Oak Security, Halborn, or Informal Systems. (An earlier $15–25k / 1–2 wk line under-estimated multi-platform validation, differential-test review, fork-integration testing, and the re-audit after the parallel operator-side hardening track. Fuller breakdown in the long-form HackMD.) Funded via DAO treasury post-mainnet, per #373's plan.

## Relation to Prop #373

**#373** passed **91.71 % YES** on **59.56 %** turnout (19–24 March 2026, <https://ping.pub/juno/gov/373>) — recognising JunoClaw as Juno ecosystem infrastructure and endorsing the ZK roadmap. This proposal is the first technical step on that direction. The 371 486 gas number has been the headline of the ZK track since March; cutting it to ~187K is the smallest concrete unit of work that moves Groth16 on Juno from *demonstrable* to *practical*.

## Voting guide

| Vote | Meaning |
|------|---------|
| **YES** | Ship native BN254, in line with Ethereum precedent and the #373-endorsed track. |
| **NO** | Prefer a different curve. BLS12-381 already shipped in CW 2.1 for a different use-case; Pasta / Vesta / Pluto-Eris aren't standardised on any Cosmos chain. |
| **ABSTAIN** | Counted for quorum, no view. |
| **VETO** | Harmful to Juno's direction — please leave an on-chain comment so it can be addressed before resubmission. |

## Links

- � Source repository — <https://github.com/Dragonmonk111/junoclaw>
- �� Long-form on-chain text — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md>
- 📄 Upstream PR body — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/WASMVM_BN254_PR_DESCRIPTION.md>
- 📄 Gas analysis — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_PRECOMPILE_CASE.md>
- 📄 Precompile index — <https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BN254_PRECOMPILE_INDEX.md>
- 🧪 Reference implementation — <https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork>
- 🧪 Devnet — <https://github.com/Dragonmonk111/junoclaw/tree/main/devnet>
- ✍️ Post-#373 synthesis (Medium) — <https://medium.com/@tj.yamlajatt/hardening-after-a-91-71-yes-on-proposal-373-b46d2939461f>
- 🏛️ Prop #373 — <https://ping.pub/juno/gov/373> · HackMD <https://hackmd.io/s/HyZu6qv5Zl>

## Cosignature *(to be filled on HackMD publish)*

- **Author** — VairagyaNodes (Juno staker since Dec 2021; validator candidate, unbonded pos. 6; JunoClaw maintainer).
- **Coding collaborator** — Cascade, the pair-programming AI agent that wrote the reference implementation, patches, tests, and docs at the author's direction.
- **Governance cosignature** — *[Jake Hartnell — per #373 precedent, framing review]*
- **Independent third-party review (operator-side)** — **Ffern Institute**, April 2026. Audit of the off-chain operator codebase (separate scope from the BN254 precompile). Five findings remediated across three security releases on `main` (`v0.x.y-security-1`/`-2`/`-3`), five Security Advisories published. Ffern re-check is the explicit pre-condition on the upstream CosmWasm PR step that follows a YES vote. Full narrative: [**When the Abbot Came**](https://medium.com/@tj.yamlajatt/when-the-abbot-came-5dda4f22a5b1) on Medium (archival copy on [GitHub](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/MEDIUM_ARTICLE_FFERN_VISITATION.md)). With thanks.
- **Independent third-party audit (precompile)** — *not yet commissioned.* Planned post-mainnet via DAO treasury (per #373). The tests + devnet are evidence *of work*, not a substitute *for* audit.
- **License** — Apache-2.0 throughout.

---

*Built in the open. Verified by mathematics. Halving the gas cost because the math already does the work; the VM just needs to know it can.*

```
════════════════════════════════════════════════════════════
  END OF PROPOSAL TEXT — formatted for forum · Discord · Telegram · HackMD
════════════════════════════════════════════════════════════
```

## ✂️  End of copy-paste ✂️

---

## Notes for the poster *(do not paste this section)*

**Posting order:**

1. **HackMD first** — create at a stable URL, paste content between the `════` fences verbatim.
2. **Invite Jake to cosign** — Telegram DM with the HackMD link, one-line ask: *"Would you cosign / add a framing line, in the same shape as #373?"* His revisions are welcome.
3. **Commonwealth forum** — cross-post, HackMD permalink at top.
4. **Discord `#governance`** — lead paragraph + forum + HackMD links (don't paste full body; 4000-char limit).
5. **Telegram** — one-message: headline number + HackMD link.
6. **On-chain submission** — `junod tx gov submit-proposal` only after ≥72 h forum feedback. Canonical text is the long-form proposal doc.

**Rattadan note:** an earlier draft misattributed contract review to Rattadan. The reference implementation and the v5 / v6 / v6.1 / v7 iterations were written by Cascade at VairagyaNodes's direction. Rattadan's contribution to JunoClaw is real but different — validator-ops guidance, `uni-7` orientation, testnet-token coordination, Juno-community welcome — not contract review. The reviewer line was removed here and in the status index. If Rattadan chooses to endorse the proposal on his own terms in a different capacity, that's his call.

**Deposit:** 5 000 JUNO (1 000 initial + 4 000 top-up, same shape as #373). Returned on pass.

**Voting period:** 5 days, per `juno-1` gov params.

**Expected timeline:** 72 h forum → 24 h HackMD cosign → on-chain → 5-day vote = **9–10 days from first post to tally**.
