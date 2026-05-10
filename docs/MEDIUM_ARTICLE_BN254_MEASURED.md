# The Number That Made It Real

## What an empirical 1.823× actually meant — measuring the BN254 precompile after proposal #374 passed.

---

*Written 10 May 2026 — a week after proposal #374 carried 61.24% YES on Juno mainnet, two weeks after the upstream patch series stabilised, the morning the devnet first produced a deterministic measurement we were willing to put our names to.*

---

## Why this article exists

We already wrote about why the BN254 precompile matters. *The Verifiable Agent* (3 May 2026) made the case in narrative; the governance proposal made it in numbers; the HackMD made it in patches. All three rested on a **projection**: by EIP-1108 algebra plus a documented 30k SDK-gas overhead ceiling, the precompile path should land at roughly 187,000–223,000 gas per Groth16 verification — somewhere between **1.66×** and **1.99×** cheaper than the pure-Wasm path.

The chain voted on a projection. The point of this article is that the projection is no longer the headline. The headline is now a measurement.

## The measurement

A single-validator devnet — `junoclaw-bn254-1`, ephemeral, reproducible, configured exactly as the upstream PRs would configure mainnet — was brought up. Both the pure-Wasm `zk-verifier` and the precompile-feature build of the same contract were deployed. The same Groth16 verification key was stored in each. The same proof was submitted, five times, to each.

Every run hit identical gas. σ = 0 across both variants.

| Variant                  | Gas per `VerifyProof` | vs precompile |
|--------------------------|----------------------:|--------------:|
| Pure-Wasm (arkworks)     |               370,600 |   1.823× more |
| **BN254 precompile**     |           **203,266** | **1.000×**    |

**A single Groth16 verification on Juno costs 1.823× less when run through the BN254 precompile.** Per call, that is 167,334 SDK gas saved. Per block, at Juno's current 80,000,000-gas block budget, the precompile lifts the verification ceiling from 215 proofs/block to 393. Per agent task, the cost difference is whether you sample verifications or audit all of them. Universal auditing was the threshold; this is the number that crosses it.

The full per-run table — txhashes, block heights, gas wanted, gas used — is in `docs/BN254_BENCHMARK_RESULTS.md` and reproducible in one command: `bash devnet/scripts/reproduce-benchmark.sh`.

## Why the measurement matters more than the projection

A projection is an argument. A measurement is a fact you cannot argue with on the same chain. There is a category of comment we have heard, politely, throughout the BN254 work — *"the algebra looks fine but precompiles always come in 30–40% above their projection."* It is a good comment. It is the comment we would have made about someone else's proposal. The 30k SDK-gas overhead ceiling we assumed is empirical bone-dry: it is the sum of `MsgExecuteContract` envelope cost, transaction signing verification, ante-handler bookkeeping, and event emission, on a chain whose `wasmd` we control.

The measurement landed within ~9% of the projection — well under the 5–10% drift band the *Verifiable Agent* article promised. We did not have to widen the band. We did not have to pad the proposal numbers. The projection held. We mention this not to take a victory lap but because it changes what the next conversation can be: the upstream PR review is no longer about "is the algebra right?", it is about "do the host functions match the spec?" That is a more productive review.

## How the measurement was produced

Every step is automated in `devnet/scripts/`. The relevant files, in the order the orchestrator runs them:

1. **`run-devnet.sh`** brings up a single-validator junod container with the patched libwasmvm linked in. Genesis is single-shot; three pre-funded accounts; 80M gas block; permissive CORS.
2. **`build-contracts-docker.sh`** compiles both the pure-Wasm and the precompile-feature variants of `zk-verifier` inside the canonical `cosmwasm/optimizer` image. Output: two `.wasm` files differing only in their import list.
3. **`deploy-now.sh`** stores both wasms, instantiates each (admin = the validator key), and writes `deploy.env` with addresses, code IDs, and the chain ID.
4. **`benchmark.sh`** auto-exports the validator privkey from the running container, generates a deterministic Groth16 proof bundle, and runs N samples per variant through a CosmJS-driven harness in `wavs/bridge/src/benchmark-zk-verifier-devnet.ts`.

The orchestrator that calls all four in sequence is `reproduce-benchmark.sh`. It is idempotent: on a re-run, completed phases are skipped unless `FRESH=1`. Default sample count is 5; `N=20 bash …/reproduce-benchmark.sh` is a one-line override.

The harness was hardened earlier this year against the Ffern audit's path-discipline findings — the same canonicalise-and-allow-root pattern that protects the operator-side upload paths. We mention this only because it changed how the benchmark is shipped: a developer who runs it on a clean checkout cannot accidentally write the results outside the repo or read a proof bundle from outside the tmpdir. The discipline seemed paranoid when we wrote it; it now does its quiet job in the developer-tools layer too.

## What we had to fight to get the number out

This part is for anyone trying to reproduce the result on a Windows host. You can skip it if your environment is straightforward Linux.

The whole stack runs in WSL2-hosted Docker. Windows-side `localhost` does **not** transparently forward to WSL2 docker ports on every machine; the benchmark must therefore run from inside WSL2. Node.js 20 is `apt`-installable from NodeSource. The bridge's `node_modules` was Windows-built, so its `@esbuild/win32-x64` is the wrong binary; one `npm install @esbuild/linux-x64 --no-save` from WSL2 fixes it without polluting the lockfile.

Two surprises took longer than they should have. First, the devnet's `globalfee` floor turned out to be `0.1ujuno`, not the `0.025ujuno` the harness assumed; until we matched it, every transaction died with `insufficient fees`. Second, the on-chain admin of both contracts is the **validator** key (because `deploy-now.sh` uses `--from validator`), not the **admin** key, so the harness needs the validator's privkey, not the admin's. The `benchmark.sh` enhancement now exports the right key automatically. We name both because they are exactly the kind of paper-cuts we have promised not to leave for the next person.

## What the number unlocks

**Universal auditing of agent tasks.** The number was always the deciding factor. At 370,600 gas per verification, every team that integrates with the agent-company suite has to decide between cost and completeness; that is not a decision a verifiable system should ask its users to make. At 203,266 gas, the decision becomes a non-decision. Verify everything; the chain can carry it.

**Confidence in the upstream PR.** The cosmwasm/wasmvm review will not have to take an algebraic argument as gospel. The reviewers can run the same `reproduce-benchmark.sh` on their own hardware and get the same number, deterministically. That is the conversation we wanted to be able to have.

**A fact that grounds the AI DAO category.** A separate note (`AI_DAO_FRAMING_AND_MOULTBOOK.md`) sketches where JunoClaw fits in the AI-DAO wave that Jake Hartnell, the netadao creator, and the Cybernetics group named in the May Spaces. That note's premise — *every agent action is verifiable* — depended on this number. The note can now stop hedging.

## What we are doing next

In order of how much engineering they cost:

1. **Patch regeneration pass.** A clean rebase of the three cosmwasm patches onto latest upstream. Done in one engineering session. No code change to BN254 proper.
2. **Upstream issues, then PRs.** The upstream PRs against `CosmWasm/cosmwasm` and `CosmWasm/wasmvm` open with EIP-1108 conformance vectors, the patch series, the projected gas, and now the measured gas — pinned to a reproducible script.
3. **v30 chain-upgrade handler.** Co-authored with Dimi if he has bandwidth; pattern-matched on his v28→v29 work. One-job handler: bump wasmvm, register BN254 imports.
4. **Mainnet measurement after upgrade lands.** The same script, against `juno-1`, with the actual upgrade height in the runbook. We expect the same 1.823×; if it differs, we publish the difference, not the round number.

Beyond the precompile, the work splits along the lines Jake sketched in the Spaces. JunoClaw the **AI DAO** primitive; the WAVS bridge as **Chainlink-shaped** oracle surface; the Moultbook research thread as the substrate for cross-agent shared knowledge. The Mesh Security audit posture (`MESH_TIABLOB_CONSTRAINTS.md`) keeps us honest about where to stop.

## A short note on attribution

The pure-Wasm baseline was originally measured on uni-7 contract `juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem`, code id 64, block 12,673,217, txhash `F6D5774E…5080F4DA`. That measurement is consistent with the devnet 370,600 gas to within rounding. Both numbers are public. The precompile measurement, the on-chain admin address, and the proof bundle hashes are all in `BN254_BENCHMARK_RESULTS.md` for anyone who wants to walk the chain themselves.

The BN254 host-function crate, the upstream patches, and the integration scripts are joint work between VairagyaNodes and Cascade (the coding agent — named, as our discipline requires). Jake Hartnell endorsed the work in March; Dimi reviewed approach in April; Marius's stability-of-mainnet posture sat behind every "do not refactor while you're in there" we wrote. The Ffern Institute's 2026 operator-side audit shaped the path-discipline patterns that are now embedded in the benchmark harness itself.

The proposal passed because the chain was willing to vote on an argument. The argument is now a measurement. That feels worth one paragraph on Medium.

---

*— VairagyaNodes, with Cascade as co-author of the contracts, the patches, and this article. With acknowledgements to the Juno governance, the cosmwasm core team, and everyone who voted YES on a projection.*

*May 2026.*

---

### Reproducibility checklist

- Repository: `github.com/Dragonmonk111/junoclaw`
- Branch: `main` (commit pinned in the script)
- Result artefact: `docs/BN254_BENCHMARK_RESULTS.md`
- Orchestrator: `bash devnet/scripts/reproduce-benchmark.sh`
- Per-step scripts: `devnet/scripts/{run-devnet,build-contracts-docker,deploy-now,benchmark}.sh`
- Original projection: `docs/BN254_BENCHMARK_PROJECTED.md`
- Upstream patch set: `wasmvm-fork/patches/`
- Audit-aware constraints: `docs/MESH_TIABLOB_CONSTRAINTS.md`
- AI-DAO framing: `docs/AI_DAO_FRAMING_AND_MOULTBOOK.md`
