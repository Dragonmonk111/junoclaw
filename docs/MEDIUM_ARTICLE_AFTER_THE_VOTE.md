

## Hardening after a 91.71 % YES on Proposal #373

[IMAGE 1 — *Oil on panel, hand-painted, Turner + Palmer + Ghibli melange.* A small English harbour at first light, tide drawn out, the bones of an old jetty exposed. A brass lantern burns thin into the dawn beside a rolled parchment tied with green ribbon. Mid-distance: a single fishing boat putting out to sea. Turner-gold sky, Ghibli-green wet stone, Palmer-indigo retreating westward.]

<!-- ART PROMPT: Oil on panel, hand-painted. Turner atmospheric romanticism + Palmer visionary pastoralism + Miyazaki warm cel-painted naturalism. English fishing harbour at first light, tide out, old jetty bones exposed. Brass lantern on jetty boards, rolled parchment with softened wax seal and green ribbon beside it. One small fishing boat putting out, mid-distance. No human figures, no text. Palette: Turner storm-gold sky, Ghibli meadow-green wet stone, Palmer indigo. Canvas texture visible. -->

---

> *Proposal #373 passed on Juno mainnet on 24 March 2026 — 91.71 % YES on 59.56 % turnout — recognising JunoClaw as ecosystem infrastructure. This article is the engineering diary of the twenty-eight days between that vote and the BN254 precompile reference implementation being checked in. Every tx hash and gas number is verifiable on-chain; every test count is reproducible with `cargo test --workspace`. Not an audit. A record.*

---

## I. The vote, and the quiet right after it

The tally at 00:08 UTC, 24 March: past quorum, past majority, no meaningful NO-with-Veto. An hour later the handoff executed — seven transactions, one command. The deploy wallet ("Neo") transferred `wasmd` admin of all five testnet contracts to Dimi, bud #1. Five JUNOX followed for gas. The remaining testnet balance drained to the Mother treasury, leaving thirteen `ujunox` on Neo as a tombstone — one token per future governance seat. Then the mnemonic was deleted. Not archived. *Deleted.*

**A scope note, said plainly.** #373 recognised JunoClaw as ecosystem infrastructure on the strength of what was on the page at the time: **TEE-attested verifiable agents**, the Junoswap-revival track, the validator-sidecar roadmap. The proposal text — still readable at <https://ping.pub/juno/gov/373> and <https://hackmd.io/s/HyZu6qv5Zl> — did *not* explicitly ratify the use of on-chain Groth16 zero-knowledge verification as a *second* supervisory layer over agent behaviour. That frame has been built out in the twenty-eight days since the vote, and it is the specific direction this article's BN254 proposal asks Juno governance to adopt next. What #373 said *yes* to was the TEE half. The ZK half is new, and it is properly put to a separate vote.

That was the easy day. What followed was the long afternoon — twenty-eight days of quiet, unfestive work: walking the fence-line of what had been built, finding where water was getting in, packing mortar into the cracks one at a time.

---

## II. Four iteration passes, honestly labelled

Between late March and mid-April the contract suite went through four tagged iterations — v5, v6.0, v6.1, v7. Every diff is checked in; every regression test ships alongside it.

An earlier draft of this article called these *"Rattadan's four hardening waves,"* borrowing a community chaoscoder's name for a framing that didn't reflect authorship. The contracts were written by **Cascade** — the coding agent this project pair-programs with — at the deployer's direction. Rattadan is a valuable Juno community relationship (validator-ops, `uni-7` orientation, testnet tokens) and does not need to be tarred by association with code he never reviewed.

That narrative fix opens a second honest question: **if some of the "hardening" was fixing things I had shipped prematurely, was it hardening, or cleanup?** The honest taxonomy:

- **Three genuine security fixes.** v5's supermajority arithmetic; v6.0's identity holes; v6.1's four value-flow holes. Real bugs that would have bitten at mainnet.
- **Two cleanups of my own v4 shipping.** v5 wiring the `ContractRegistry` that v4 shipped with `None`s everywhere (dead-code atomic callbacks); v5 retracting the three weight guardrails v4 had added speculatively.
- **One capability expansion.** v7's Tier-1-slim / Tier-1.5 declarative constraint vocabulary. Not defensive — a new feature.

**Three real fixes, two self-corrections, one feature.** The self-corrections are worth calling out, not hiding. That v5 *reversed* v4's weight guardrails in the same public repo — rather than quietly keeping them — is itself the governance-proof. A system that admits premature decisions is one worth reviewing.

### The three real fixes, briefly

- **v5 supermajority arithmetic.** v4's tally was `total_voted_weight ≥ 67 % AND yes > no`. A 51 / 16 / 0 split met both clauses — so a constitutional proposal requiring 67 % nominal yes could pass on 51 % actual yes with abstain silently helping. v5 flipped to a pure yes-ratio: `yes_weight × 100 ≥ total_weight × 67`. A minority holding > 33 % now blocks. Regression: `test_code_upgrade_supermajority_blocks_minority` flipped from `Passed` to `Rejected`.

- **v6 identity hygiene.** `task-ledger::SubmitTask` was unauthenticated — anyone could forge reputation increments against any agent. And `agent-company` had overloaded WAVS-operator identity onto the `task_ledger` admin wallet. v6 added three ownership invariants to `SubmitTask` and an explicit `wavs_operator: Option<Addr>` on `agent-company::Config`, rotatable via governance.

- **v6.1 value-flow (the four farmer crimes).** **F1 (CRIT):** `CompleteTask` let the submitter self-finalise, quietly confirming their own escrow. **F2 (HIGH):** `DistributePayment` was open to any caller — a 1-`ujunox` griefer could permanently lock distribution. **F3 (HIGH):** `SubmitWork` accepted duplicate `work_hash` — two claims for the same work. **F4 (MED):** Junoswap silently ate any unexpected denom. All four fixed, none had been exploited, all had been reachable.

None was deleted; every fix is a new invariant asserted in a regression test.

[IMAGE 2 — *Watercolour on rough paper, Turner + Constable + Ghibli.* A stone manor's long perimeter wall in afternoon sun. Four piles of matched stones at the base, four trowels, four copperplate-labelled cards pinned with copper nails: "F1", "F2", "F3", "F4". No figure; a willow's shadow across one pile.]

<!-- ART PROMPT: Watercolour on rough cold-press paper, hand-painted. Turner afternoon atmospherics + Constable pastoral precision + Miyazaki warm cel-painted naturalism. English country manor's long stone perimeter wall in afternoon light. Four piles of matched stones, four trowels, four copperplate labels on copper nails: 'F1' 'F2' 'F3' 'F4'. No human figures, only a willow's shadow across one pile. Single swallow upper distance. Palette: Turner-soft honey sky, Ghibli-warm cream stone, Palmer-deep willow shadow. No other text. Canvas texture visible. -->

### The two cleanups

- **v5 registry wiring.** v4's `RATTADAN_HARDENING.md` boasted of cross-contract atomic callbacks — but every v4 deployment had shipped with `ContractRegistry { None, None, None }`, running the callbacks as silent no-ops. v5 wired the pointers and exposed an admin `UpdateRegistry`. Architectural theatre, retired.
- **v5 weight-guardrail retraction.** v4's 20 % delta cap / 500-block cooldown / 1-bps floor were rate-limiting dressed as minority protection. A 51 % coalition with patience defeats them. v5 replaced all three with a single structural lever: `WeightChange` now requires 67 % supermajority, mirroring `CodeUpgrade`. The diff *removed* code.

### The one feature — v7 Tier-1-slim / Tier-1.5

v6 tightened *who was allowed to swear*. v7 gave the ledger a vocabulary for *what could be sworn about* — a bounded declarative constraint enum attached to each task as `pre_hooks` / `post_hooks`, evaluated around the `Running → Completed` transition. Any failing hook returns `ContractError::ConstraintViolated { reason }` and reverts atomically — nothing partially settles.

Seven sentences a task can refuse to be completed without: `AgentTrustAtLeast`, `BalanceAtLeast`, `PairReservesPositive`, `TaskStatusIs`, `TimeAfter`, `BlockHeightAtLeast`, `EscrowObligationConfirmed`. **Why an enum and not a callback?** Audit surface. A reviewer reading `AgentTrustAtLeast { agent_id: 42, min_score: 75 }` knows exactly what will be checked; a `callback_addr` is an infinite regress. The cost of expressiveness is paid once, in the enum.

Tier-1.5 went live on `uni-7` as code_id 75 at `juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm`, with each temporal/coupling constraint exercised against live chain state:

| Constraint | Final success tx on uni-7 |
|---|---|
| `TimeAfter` | `C85744602D8CABC4D2D0E4D15FC92237C2C8AA8AA92DD7CDA90A4CAE98C89777` |
| `BlockHeightAtLeast` | `BA62616356324712A0F466ACA04D35BEEF37739367FD4688DD55C9047922921A` |
| `EscrowObligationConfirmed` | `AE47ECB125C9B0561A937DCC33DAEA57E557C9F4DDBAC2B20A3EB0CFF2DE0DD2` |

Workspace tests at the end of the window: **155 / 155** green.

---

## III. Why TEE + ZK — the twin-lock model for agentic safety

Before the gas numbers, the purpose. An AI agent that controls a wallet, mints keys, or posts attested statements to a chain is a small but real sovereign actor. *"Trust the model"* is not a governance answer; *"trust whoever owns the server the model runs on"* is barely better. The architectural answer JunoClaw works under is the **twin-lock**: every attested agent step must pass **two independent witnesses** before the chain acts on it.

**Lock 1 — TEE attestation (hardware witness).** A signed measurement from Intel SGX / AMD SEV / Arm CCA stating that a specific binary ran inside a specific enclave on specific silicon at a specific block height. Already on-chain: the milestone quote at tx `6EA1AE79…D26B22`, block 11 735 127. This is the lock #373 explicitly ratified. It answers **"who ran what, on hardware no one at the operator can introspect?"**. It is a *provenance* statement.

**Lock 2 — ZK verification (mathematical witness).** A Groth16 proof, verified on-chain through the BN254 precompile, stating that a computation trace consistent with a public circuit was produced — without revealing the model weights, the prompts, or the private inputs. This is the lock *this* proposal asks governance to adopt. It answers **"did that run produce an output that satisfies a declared predicate over the agent's behaviour?"**. It is a *semantics* statement.

Neither lock is sufficient alone for agents with real-world power:

- **TEE without ZK.** The attested binary can do anything its code-path permits. If the agent is a large language or planning model, nobody — including the operator — can compress *"did it stay within policy?"* into a hardware measurement. You get **provenance, no semantics**: proof the right server ran, zero proof it behaved.
- **ZK without TEE.** A proof can be generated by anyone anywhere who holds the inputs. The chain cannot tell a proof produced by the declared operator from one produced by an adversary who happened to obtain the witness. You get **semantics, no provenance**: proof that *some* valid trace exists, zero proof *your* enclave produced it.
- **Both, bound together.** An attested enclave signs an output; the same output is the public input to a circuit; the circuit's proof verifies on-chain. The chain then pays, slashes, or escalates on **both** dimensions, and an adversary must break hardware root-of-trust *and* forge a proof against a circuit they do not have the witness for.

Sitting on top of both locks, and already live on `uni-7`, is a **third, declarative witness**: the v7 Tier-1.5 constraint vocabulary. Seven enum variants (`AgentTrustAtLeast`, `BalanceAtLeast`, `PairReservesPositive`, `TaskStatusIs`, `TimeAfter`, `BlockHeightAtLeast`, `EscrowObligationConfirmed`) bound *which tasks the agent is even allowed to attempt*. Each is audit-readable at a glance. Each further narrows the trust surface without demanding trust in the operator.

**A fourth, operational witness — added in April 2026 in the wake of an independent operator-side audit by Ffern Institute** — completes the arrangement: **verifiable controllability**. Compile-time Cargo features (`unsafe-shell` today, `unsafe-egress` and `unsafe-fs-write` planned) keep code that should not run out of the binary entirely — the bytes are not present, so they cannot be reached. Runtime kill-switches (`sandbox_mode` today, `signing_paused` and `egress_paused` planned) let a fleet operator pause specific capabilities in seconds, without a redeploy, in response to an incident or community alert. And a planned read-only admin RPC publishes the *current* state of every gate, so any client can verify which capabilities the agent in front of them has armed *before* sending the task. The first three witnesses answer *who ran*, *what was computed*, and *within what bounds it was even allowed to attempt*. The fourth answers *and is the operator still meaningfully in control of the fleet, observably from outside?* For real-world consequential agent work — long-running data pipelines, robotic delivery, client-paying-for-attestation tasks — this is what separates *autonomy* from *abdication*. The Ffern audit is what pointed at the gap; the verifiable-controllability layer is the response, and will be the subject of a separate future Juno governance proposal once the operator-side hardening pass lands.

**Threat model, explicit:**

| Adversary capability | Blocked by |
|---|---|
| Compromises operator wallet, not enclave | TEE attestation quote fails to verify |
| Compromises enclave, cannot produce valid ZK proof | zk-verifier rejects; parent tx reverts atomically |
| Produces forged attestation + valid ZK proof for an *out-of-policy* task | Tier-1.5 constraint enum rejects at `Running → Completed` |
| Compromises an operator-side helper (off-chain RCE, key exfil, SSRF) | Cargo feature flags keep the dangerous code paths out of the binary entirely; runtime kill-switches halt specific capabilities without redeploy; published kill-switch state lets clients refuse to send tasks to an agent whose operator has dropped meaningful control |
| Has real enclave + valid proof + in-vocabulary task + observably-controlled fleet | **Behaving within policy by construction — which is the goal** |

The whole point of making BN254 a **native precompile** rather than a Wasm library is economic: at 371 486 gas per verify (3.7 % of a block), the ZK lock is too expensive to require on every attestation. At ~187 000, it can run on *every high-value step*, not just the flagship one. **Cheap enough to be mandatory is the security property; "2× faster" is its shadow.**

Agentic safety, in this framing, is not a promise about the agent. It is an arrangement of *witnesses around the agent* — hardware, mathematics, a narrow grammar of permitted tasks, and an externally-observable operator control plane — such that any step the chain acts on has been seen by all four. #373 endorsed the first piece. This proposal completes the second. The third is already live on `uni-7`. The fourth is the post-Ffern-audit hardening pass and will be the subject of a separate proposal once shipped.

---

## IV. The BN254 precompile

The headline work of the afternoon was turning the zk-verifier's 371 486 gas — a design goal since March — into a ready-to-submit governance proposal with a working reference implementation behind it.

The case-in-principle is simple. Ethereum has had BN254 pairing precompiles at `0x06`, `0x07`, `0x08` since Byzantium (2017). Sui shipped them at launch (2023). CosmWasm 2.1 added BLS12-381 pairing primitives (2024) — the host-level plumbing is already in the codebase. **What CosmWasm does not yet have is BN254, which is the curve every existing Groth16 circuit on Ethereum already uses** — `snarkjs`, `circom`, `gnark`, `ark-groth16`. BN254 is the cheapest bridge to an already-mature toolchain.

At Ethereum's EIP-1108 schedule:

| Operation | EVM precompile | Gas |
|---|---|---|
| `bn254_add` | `0x06` | 150 |
| `bn254_scalar_mul` | `0x07` | 6 000 |
| `bn254_pairing_equality` | `0x08` | 45 000 + 34 000 × N pairs |

A four-pair Groth16 verify = `45 000 + 4·34 000 + lincomb (~6 150)` ≈ **~187 000 gas** on a patched chain, versus the measured **371 486** on today's pure-Wasm path. Almost exactly 2×. The saving is concentrated, correctly, in the pairing.

[IMAGE 3 — *Oil on panel, Turner + Constable + Ghibli.* A chemist's brass balance on worn marble. Left pan: one clean weight stamped "187,000". Right pan: a heavier stack totalling "371,486". Pointer steady. Through the window, soft English meadow.]

<!-- ART PROMPT: Oil on panel, hand-painted. Turner + Constable + Miyazaki. Chemist's brass balance on worn marble, three-quarter view. Left pan: one tall brass weight stamped '187,000'. Right pan: rough stack totalling '371,486'. Pointer steady. Window: soft English morning, mist off wet meadow. No human figures. Palette: lead-grey brass, Ghibli meadow-green through glass, Turner-honey dawn. Canvas texture visible. -->

### What landed in this window

1. **`wasmvm-fork/cosmwasm-crypto-bn254/`** — standalone `no_std`-friendly Rust crate against `ark-bn254 0.5`. Byte layout + gas schedule lifted from EIP-196/197/1108. **22 / 22 tests green** (13 unit + 9 EIP conformance vectors).
2. **`wasmvm-fork/cosmwasm-std-bn254-ext/`** — guest-side shim exposing the three host calls to contracts without a `cosmwasm-std` fork. 2 / 2 tests; zero warnings on native or wasm32.
3. **`wasmvm-fork/patches/`** — five unified diffs against `CosmWasm/cosmwasm` v2.2.0 + `CosmWasm/wasmvm` v2.2.0. Gated behind a new `cosmwasm_2_3` feature flag; no major version bump; existing contracts compile unchanged.
4. **`contracts/zk-verifier/src/bn254_backend.rs`** — single-file feature-gated dispatch. Default build still uses pure arkworks; `--features bn254-precompile` routes through the host functions. Live uni-7 contract untouched.
5. **`devnet/`** — ephemeral single-validator rig. Multi-stage Dockerfile applies the five patches, builds a BN254-patched `libwasmvm.a`, builds `junod` v29 against it. Cold ~12 min, warm ~90 s.
6. **Side-by-side benchmark** (`wavs/bridge/src/benchmark-zk-verifier-devnet.ts`) — runs N samples on both variants, writes `docs/BN254_BENCHMARK_RESULTS.md`.

Every claim is reproducible from a clean checkout:

```bash
cargo test -p zk-verifier --lib                                  # 9 / 9
cargo test --manifest-path wasmvm-fork/cosmwasm-crypto-bn254/Cargo.toml    # 22 / 22
cargo test --manifest-path wasmvm-fork/cosmwasm-std-bn254-ext/Cargo.toml   # 2 / 2
cargo check -p zk-verifier --features bn254-precompile            # exit 0
cd devnet && ./scripts/run-devnet.sh \
          && ./scripts/deploy-zk-verifier.sh \
          && ./scripts/benchmark.sh                               # writes the headline number
```

The human-facing artefacts are drafted and ready: [`WASMVM_BN254_PR_DESCRIPTION.md`](./WASMVM_BN254_PR_DESCRIPTION.md) (upstream PR body), [`JUNO_GOVERNANCE_PROPOSAL_BN254.md`](./JUNO_GOVERNANCE_PROPOSAL_BN254.md) (long-form signaling text), [`TECHNICAL_PROPOSAL_BN254_SHAREABLE.md`](./TECHNICAL_PROPOSAL_BN254_SHAREABLE.md) (condensed copy-paste).

---

## V. The optional zk-sidecar and the MCP

Two smaller threads ran alongside.

**Optional zk-proof on attestation.** `agent-company::SubmitAttestation` now takes two optional fields — `proof_base64`, `public_inputs_base64`. When `Config.zk_verifier: Option<Addr>` is `None` (the default), behaviour is unchanged. When it's `Some(addr)` and both fields are supplied, the row writes and a sub-message dispatches to the zk-verifier atomically — if the proof fails, the parent tx reverts. Named errors for every fail-closed case: `IncompleteZkProofBundle`, `ZkVerifierNotConfigured`, plus sub-message bubble-up. An admin-only `RotateZkVerifier { new: Option<String> }` mirrors `RotateWavsOperator`; `None` is a deliberate kill-switch. Six regression tests shipped with the wire-up.

**The Cosmos MCP server** ([juno.new](https://medium.com/@tj.yamlajatt/the-first-ai-that-speaks-cosmos-building-juno-new-2a0253cbce91)). 16 tools, 5 chains, 9 DAO templates, 12/12 live smoke tests against `uni-7`, write-path signed on-chain in `EE9A8FA6E7E6F6A77301DE6DC9A9E6A27D398AE7D071CAFCA2934352B8FB9327`. Adjacent to the hardening + precompile threads, not in them. It matters because the precompile makes the chain cheaper to *talk to* from the agent side; the MCP makes the chain *legible* to the agent in the first place. Two halves of the same conversation.

---

## VI. The ledger today

[IMAGE 4 — *Watercolour with graphite underdrawing, Turner + Constable + Ghibli.* A surveyor's folding table at the edge of a water-meadow, late afternoon. A scale-model of a harbour-town with thirteen coloured pins marking building-sites — twelve plain wood, one painted a quiet green. Beside the model: theodolite, surveyor's notebook open, brass dividers. No figure.]

<!-- ART PROMPT: Watercolour with graphite underdrawing, hand-painted. Turner late-afternoon + Constable meadow precision + Miyazaki domestic scale. Surveyor's folding table at English water-meadow edge. Scale-model harbour-town on the table with thirteen small coloured pins — twelve plain wood, one painted quiet green. Theodolite on tripod, surveyor's notebook open, brass dividers. No human figure. Palette: Turner-gold afternoon, Ghibli-green meadow grass, Palmer-indigo shadow. No text. Canvas visible. -->

**Live on `uni-7`:**

| Artefact | Address / code_id | Status |
|---|---|---|
| `agent-company v3` | code_id 63, `juno1k8dxll4...` | Live; admin: Dimi (bud #1) |
| `junoswap-factory` + pairs | code_id 60 / 61 | Live, no liquidity yet |
| `task-ledger` (Tier-1.5 / v7) | code_id 75, `juno1cp88zj8...` | Live; 3 constraint smoke tests green |
| `zk-verifier` | code_id 64, `juno1ydxksvr...` | Live; one `VerifyProof` at 371 486 gas |
| TEE attestation (SGX) | tx `6EA1AE79…D26B22`, block 11 735 127 | Permanent on-chain |
| MCP write-path | tx `EE9A8FA6…B9327` | Validated |

**Shipped in code, not live yet:**

- `agent-company v7` optional zk-sidecar — awaiting one admin migrate + `UpdateConfig.zk_verifier`.
- BN254 precompile — reference implementation, patches, devnet, feature-gated contract path and every doc are all committed (`origin/main`, commit `6d92067`, 22 April). To go from *shipped-to-git* to *live-on-Juno* still needs four sequential steps, each a human coordination gate rather than new code:
  - **Upstream PR** opened against `CosmWasm/cosmwasm` + `CosmWasm/wasmvm`, so the VM change lives in the canonical repository every Cosmos chain builds from. The PR body is already written (`WASMVM_BN254_PR_DESCRIPTION.md`); the click to open it is the cheapest remaining step.
  - **Juno signaling vote** — an on-chain governance proposal (5 000 JUNO deposit, returned on pass) asking validators and stakers to endorse adopting BN254. Signaling only: *no code executes on-chain from the vote itself*, it records direction.
  - **`MsgSoftwareUpgrade`** — the actual coordinated validator halt-and-resume at a chosen block height, swapping the running Juno binary for one carrying the merged upstream patch. Juno governance submits this after upstream merge; validators run the new binary from the halt-height onward.
  - **zk-verifier contract migrate** — one `MsgMigrateContract` flipping the live contract at `juno1ydxksvr...` (code_id 64) from its pure-Wasm backend to the precompile-routing backend. Same address, same ABI, same admin, same verification key — only the internal dispatch changes. Gas drops from 371 486 to ~187 000 at that moment.
  - Nothing in this chain is blocked on writing more code; everything waits on opening a PR, one vote, one coordinated upgrade, and one migrate. Section VII walks through the same five gaps from a readiness-audit angle.
- Four remaining Tier-1-slim smoke tests.

**Tests as of 2026-04-21:** `cargo test --workspace` → **155 / 155**. All BN254 test suites green (22 + 2 + 9). Feature-on build compiles clean.

---

## VII. Are we ready?

**Ready for the case to be made:** yes. Every claim is backed by a reproducible command. A reviewer can verify without trusting the author for any of it.

**Ready to *be* the case:** not yet. Five gaps — each a human step, not a code step:

1. **Open the upstream PRs** against `CosmWasm/cosmwasm` + `CosmWasm/wasmvm`. Cheapest remaining step; unlocks everything.
2. **Submit the Juno signaling proposal.** 5 000 JUNO deposit, returned on pass. 5-day voting period.
3. **Post-merge `MsgSoftwareUpgrade`** bumping Juno to a `wasmd` carrying the PR. Coordinated validator halt-height; Juno governance timing.
4. **Post-upgrade `MsgMigrateContract`** on the live zk-verifier, code_id 64 → precompile. Address unchanged. Gas drops.
5. **The twelve unfilled buds.** Orthogonal, slower, most important for legitimacy. A chain with one bud is not yet a forest.

On the audit — honestly scoped. The reference crate is ~900 lines of Rust, of which ~90 % is tests and documentation — a **test-and-docs-to-code ratio of 0.9**. Effective audit surface: ~90 lines of host-function glue plus ~300 lines of upstream unified-diff patches, roughly **400 lines**. Underneath, `ark-bn254 0.5` is already library-audited; nine EIP-196/197/1108 conformance vectors give byte-exact ground truth; the patches sit behind a new `cosmwasm_2_3` feature flag, so existing contracts are out of scope.

For calibration against other CosmWasm audits whose figures are public: DAODAO v2 audited at ~6 000 LoC by Oak Security ran in the **$40–60k** range over 3–4 weeks; Stargaze marketplace (also Oak) at ~3 500 LoC was **$25–35k**; Mars Protocol v2 at ~12 000 LoC with Halborn sat nearer **$80–120k** over 4–6 weeks; Stride's formally-verified core with Informal Systems crossed **$150k** and 8+ weeks. Typical CosmWasm projects ship at 30–50 % test-to-code by LoC; 90 % is unusual, and it cuts through the *"first understand the intent"* phase — the intent is in the tests and the EIP reference already. **Rough estimate for BN254: $15–25k, 1–2 weeks** with Oak Security, Halborn, or Informal Systems — all three have CosmWasm + crypto-primitive experience. Funded via DAO treasury post-mainnet, per #373's plan. Evidence *of work*, not a substitute *for* audit.

On the larger phrase *"sovereign agentic tasks"* — the sovereignty exists in principle and is being tested in practice (Akash, 13-bud trust tree, Apache-2.0, handoff-before-mainnet). The agentic loop closes (agent-registry + task-ledger + escrow + agent-company + builder-grant + optional zk-sidecar; 155 tests + on-chain smoke tests with tx hashes). The tasks are narrow by design — atomic revert, hash-locked attestation, seven-constraint vocabulary. *A working pattern, not a finished product.*

---

## VIII. The cosign question: Jake

Proposal #373 was cosigned by **Jake Hartnell**, who added the *"as an experiment"* framing before submission. That precedent sets the expectation for how a JunoClaw governance track looks when working well: the deployer writes the technical case; a respected community voice adds the framing that makes the political case. For BN254, the right move is to follow that precedent exactly: **ask Jake, do not ask Rattadan.**

**Why Jake.** Juno co-founder, WAVS architect, and the person whose Telegram note — *"WAVS TEEs already work — you just need to run WAVS inside a TEE"* — seeded the TEE milestone this proposal builds on. He has said publicly that *"JunoClaw was long overdue"* and *"very cool."* BN254 is a `wasmvm`-surface change, adjacent to Jake's technical domain. Continuity with #373 argues for the same cosignature. Mechanism is the same: publish the HackMD, DM Jake with a one-line ask, let his revisions land, then paste to-chain.

**Why not Rattadan.** An earlier draft proposed him as an *audit reviewer*. He wasn't. The contracts — and the four iteration passes — were authored by Cascade at the deployer's direction. Rattadan's contribution to JunoClaw is real but different: validator-ops guidance, `uni-7` orientation, testnet-token coordination, Juno-community welcome. Placing his name on a governance-facing HackMD as an audit reviewer would have overstated his role and, on a second-pass reading, would have undermined every other claim on the page. The reviewer line has been removed from the shareable proposal and the status index. Rattadan does not need to be a cosigner to remain a valuable relationship.

---

## IX. Closing — the morning of the twenty-eighth day

[IMAGE 5 — *Oil on panel, Turner + Palmer + Ghibli.* The same harbour as the opening, 28 days on. Tide coming *in* this time. The lantern is gone; in its place on the jetty, a leather-bound ledger lies open at a clean page, weighted by a river-stone. A freshly dipped quill in a brass inkwell beside it. Mid-distance: the fishing boat from image 1 returning, low in the water, catch aboard. Identical geography, opposite direction.]

<!-- ART PROMPT: Oil on panel, hand-painted. Turner + Palmer + Miyazaki. Same English fishing harbour as opening, 28 days later, early morning. Tide coming IN. Lantern gone. In its place: leather-bound ledger open at clean page, weighted by river-stone, freshly dipped quill in brass inkwell beside it. Mid-distance: same boat from opening, now returning, low in water, catch aboard, sail slack. Single gull overhead. No human figures. Palette: Turner-gold dawn, Ghibli-green algae on wet stones, Palmer-indigo at last edge of night. No text. Canvas texture visible. -->

Twenty-eight days is long enough to walk the fence-line of a seven-contract workshop four times, pack mortar into four cracks, turn a one-shot gas number into a rerunnable benchmark, wire an optional zero-knowledge rail into the attestation flow, build a standalone BN254 host-function crate from scratch with 22 green conformance tests, author a five-patch diff against a major upstream VM without breaking any guest contract, stand up an ephemeral devnet that applies the patches, and write the upstream PR description, the signaling proposal, the shareable copy, and this synthesis.

It is not long enough to form a 13-seat DAO, coordinate a mainnet deployment, commission a formal audit, produce validator TEE sidecar attestations across independent hardware, bootstrap AMM liquidity, and merge a precompile upstream. Those are the next months.

The garden doesn't mourn the gardener; it grows. The ledger does not lie about what is and is not settled; it reverts atomically when an invariant fails, and records every success with a clear attribute. The fence-line has been walked, and where it was thin it is now matched. The lantern is out. The quill is fresh.

The morning belongs to whoever walks down to the jetty next.

**— VairagyaNodes**

---

## Links

- Source (Apache-2.0): <https://github.com/Dragonmonk111/junoclaw>
- BN254 long-form proposal: [`JUNO_GOVERNANCE_PROPOSAL_BN254.md`](./JUNO_GOVERNANCE_PROPOSAL_BN254.md)
- BN254 shareable copy: [`TECHNICAL_PROPOSAL_BN254_SHAREABLE.md`](./TECHNICAL_PROPOSAL_BN254_SHAREABLE.md)
- BN254 upstream PR body: [`WASMVM_BN254_PR_DESCRIPTION.md`](./WASMVM_BN254_PR_DESCRIPTION.md)
- BN254 reference implementation: [`wasmvm-fork/`](../wasmvm-fork/)
- Prop #373 on-chain: <https://ping.pub/juno/gov/373>
- Prop #373 HackMD: <https://hackmd.io/s/HyZu6qv5Zl>

*Built in the open. Verified by hardware and by mathematics. Governed by trust. Released by choice.*

---

## Appendix — Art prompts for a painter

Every `<!-- ART PROMPT -->` block above is written to be handed directly to a commissioned oil painter or a diffusion model tuned for English Romanticism + Ghibli. The house style, repeated across all five:

> *Oil on panel or watercolour with graphite underdrawing. Hand-painted, not digital. Equal-parts melange of **J.M.W. Turner's atmospheric romanticism**, **Samuel Palmer's visionary pastoralism**, **John Constable's English water-meadow precision**, and **Hayao Miyazaki / Studio Ghibli's warm cel-painted naturalism**. Palette: Turner storm-gold, Ghibli meadow-green, Palmer indigo. Canvas / rough-paper texture visible. No human figures unless specified. No text, no signatures. Naturalist's patience, Romantic's light.*

The five scenes: **(1)** Harbour at first light, tide out, lantern + parchment. **(2)** Manor wall, four mortar-piles labelled F1–F4. **(3)** Brass balance, 187 000 vs 371 486. **(4)** Surveyor's table, 13-pin harbour-town model. **(5)** Same harbour 28 days later, tide in, ledger + quill replacing the lantern.

A commissioned oil painter in Turner + Palmer + Miyazaki house style: ~£400 – £1 200 per 30×40 cm panel, ~10 weeks for the five-panel suite. For a diffusion pass, the prompts are already workable for SDXL / Midjourney v6 / Flux — specify *"oil on panel, no text, no signature"* as negative prompts and the style holds. Visual continuity across articles is worth the small extra effort at commissioning time.
