# Git notes — BN254 measured-gas update + post-Spaces direction

*Suggested commit groupings and message bodies for the changes accumulated since the last push. Feeds straight into `git commit -m` or `git notes add -m`.*

*Headline fact for every commit body:* **devnet now measures the BN254 precompile path at 203,266 gas (5/5 σ=0), versus 370,600 gas pure-Wasm = 1.823× reduction; reproducible with `bash devnet/scripts/reproduce-benchmark.sh`.**

---

## Group 1 — Devnet benchmark, end-to-end reproducible

**Files**
- `devnet/scripts/benchmark.sh` *(modified)*
- `devnet/scripts/reproduce-benchmark.sh` *(new)*
- `wavs/bridge/src/benchmark-zk-verifier-devnet.ts` *(modified)*
- `docs/BN254_BENCHMARK_RESULTS.md` *(new — measurement output)*

**Suggested commit message**

```
benchmark: empirical BN254 devnet measurement, 1.823× reduction

Adds a one-shot reproduction path for the BN254 precompile benchmark
that the #374 governance proposal projected algebraically.

Result on junoclaw-bn254-1 single-validator devnet, 5 samples per
variant, σ = 0:

  Pure-Wasm (arkworks):   370,600 gas
  BN254 precompile:       203,266 gas
  Reduction:              1.823× (167,334 gas saved per VerifyProof)

Numbers land within ~9% of the EIP-1108-derived projection in
docs/BN254_BENCHMARK_PROJECTED.md, well inside the 5–10% drift band
the Medium article promised.

Changes:

- benchmark.sh: auto-export validator privkey from the running
  container when no signer credentials are set; auto-generate a
  Groth16 proof bundle into tmpdir/groth16_proof.json; default
  N=5; honour KEEP_PROOF=1 to skip regen; fall back to standard
  rustup PATH locations when called from a non-login shell.
- reproduce-benchmark.sh: idempotent orchestrator that runs
  devnet up → build → deploy → benchmark in one command, with
  FRESH=1 to force a clean rebuild and N=… to override sample count.
- benchmark-zk-verifier-devnet.ts: support WAVS_OPERATOR_PRIVKEY
  (hex) as alternative to WAVS_OPERATOR_MNEMONIC, since the devnet
  keys are created with --no-backup; bump signing gas price from
  0.025ujuno to 0.1ujuno to match the chain's globalfee floor.
- BN254_BENCHMARK_RESULTS.md: per-run table, txhashes, block
  heights, and headline reduction.
```

---

## Group 2 — Public-facing docs updated with the measured number

**Files**
- `docs/MEDIUM_ARTICLE_THE_VERIFIABLE_AGENT.md` *(modified)*
- `docs/MEDIUM_ARTICLE_BN254_MEASURED.md` *(new — afterwork article)*

**Suggested commit message**

```
docs(medium): replace projected gas with measured 1.823× reduction

The Verifiable Agent article (3 May) cited the projected ~187–223k
gas range. With the devnet now measuring 203,266 gas empirically,
the projection band is replaced in place by the measurement,
with an "Update, May 10, 2026" stanza near the date stamp linking
back to BN254_BENCHMARK_PROJECTED.md for the original algebra.

Adds MEDIUM_ARTICLE_BN254_MEASURED.md ("The Number That Made It
Real") as an afterwork follow-up to the Medium series, written
specifically to ground the afterwork narrative in the measurement
rather than the projection. Co-located with the existing Medium
article archive in docs/.

The governance proposal (#374) is left untouched — it has already
passed; the measurement does not require a revision to a
historical artefact.
```

---

## Group 3 — Strategic notes from the May Spaces

**Files**
- `docs/AI_DAO_FRAMING_AND_MOULTBOOK.md` *(new)*
- `docs/MESH_TIABLOB_CONSTRAINTS.md` *(new)*

**Suggested commit message**

```
docs: AI-DAO framing + Mesh/tiablob audit-aware constraints

Captures two threads from the May 2026 Twitter Spaces with Jake
Hartnell, the netadao creator, and Cybernetics members.

1. AI_DAO_FRAMING_AND_MOULTBOOK.md — restates the existing nine-
   contract JunoClaw stack as an AI-DAO primitive and scopes a
   "Moultbook" research thread for shared agent knowledge.
   Three substrate candidates listed (Howl Social, generic
   Cosmos message-board patterns, off-chain DA + commitments);
   no commitment to one yet. Concrete next-action list at §7.

2. MESH_TIABLOB_CONSTRAINTS.md — codifies the audit-aware posture
   for any Mesh- or tiablob-touching work. Mesh Security audit
   not yet funded (~$200k estimate, per Jake). Default JunoClaw
   code paths stay Mesh-free; any Mesh adapter is feature-flagged.
   Authoritative footnote provided for paste-in to other docs.
```

---

## Group 4 — wasmvm-fork FFI calling convention fix (likely already pushed; mention if not)

**Files**
- `wasmvm-fork/cosmwasm-std-bn254-ext/src/lib.rs` *(modified)*

**Suggested commit message** (only push if it is not already in upstream-of-main)

```
wasmvm-fork: align cosmwasm-std-bn254-ext FFI with VM 2-param convention

bn254_add and bn254_scalar_mul: pre-allocate output buffer; pass
both input and output region pointers. bn254_pairing_equality:
return single boolean byte. Matches the VM-side
do_bn254_add/do_bn254_scalar_mul in
patches/v2.2.2/05-cosmwasm-vm.imports.rs.patch.

Fixes the "unresolved import" / signature-mismatch surface that
was blocking the precompile contract from instantiating with
gas-auto simulation enabled. The 1.823× benchmark in this push
exercises both host functions live in production-shaped traffic.
```

---

## Group 5 — devnet plumbing already tagged but worth mentioning

**Files** *(already mostly committed in the previous deploy push; only re-list here if any are still in `git status`)*
- `devnet/scripts/init-genesis.sh` *(modified — gRPC enable + minimum-gas-prices)*
- `devnet/scripts/build-contracts-docker.sh` *(modified — CARGO_FEATURE_* env vars)*
- `devnet/scripts/build-precompile.sh` *(new — single-variant builder)*
- `devnet/scripts/deploy-now.sh` *(new — 9P-safe deploy via docker exec/cp)*

These were the 'how we got here' work; if they are untagged in the current `git status`, fold them into a separate "devnet: hardening + 9P-safe deploy" commit so the benchmark commit (Group 1) stays narrow.

---

## Suggested push order

1. **Group 4** if not yet upstream — the FFI fix is what makes the benchmark valid.
2. **Group 5** if not yet upstream — devnet plumbing comes before benchmark.
3. **Group 1** — benchmark scripts + measurement artefact.
4. **Group 2** — Medium article updates.
5. **Group 3** — strategic notes.

This keeps the "1.823× measured on devnet" claim verifiable at HEAD: at every commit boundary, the orchestrator can be run and the number reproduced.

## git notes (post-commit)

For the merge commit (or the Group 1 commit if no merge), attach a `git notes` entry:

```
git notes add -m "BN254 precompile measured: 370,600 → 203,266 gas (1.823×, σ=0). Reproducible: bash devnet/scripts/reproduce-benchmark.sh."
```

That note travels with the commit even if the commit message is squashed downstream.

---

*Drafted 10 May 2026, immediately after the empirical measurement landed.*
