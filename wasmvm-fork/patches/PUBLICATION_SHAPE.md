# Publication shape — P2 (patch series only, no public fork)

*Decision updated 2026-08-04. Supersedes the P1 decision recorded 2026-05-14. Anchor: [`memory/SESSION_PROTOCOL.md`](../../memory/SESSION_PROTOCOL.md) §T2c.*

## TL;DR

One artefact, zero fork maintenance:

1. **Canonical source-of-truth:** [`wasmvm-fork/patches/v3.0.x/`](./v3.0.x/) — the patch series. Edit here. Re-baseline here.
2. **No public fork.** No `Dragonmonk111/cosmwasm-bn254` repo. No `Dragonmonk111/wasmvm` fork tag. Consumers apply the 10 patches at build time via a build script.

This is the **P2** option from the T2c decision. Given that upstream CosmWasm#2685 is deferred to Backlog (no PR accepted until ~Q3/Q4 2026), maintaining a public fork for 3-6 months with no upstream merge path is unnecessary burden. P2 has zero fork maintenance — patches rebase automatically on `cargo update`.

## Rationale for the change from P1 to P2

- **2026-05-14:** P1 (fork + tag) was chosen for shipping speed. The assumption was that upstream would accept the PR within weeks.
- **2026-06-29:** @DariuszDepta replied on CosmWasm#2685, moved it to Backlog, stated the team will not take external proposals until ~Q3/Q4 2026. This extends the fork maintenance window from weeks to 3-6 months.
- **2026-08-04:** P2 adopted. The patch series is complete (10/10 clean, 22/22 tests pass). A build script applies the patches at build time. No fork to maintain. When upstream reopens, the same patch series becomes the PR body.

## Why this shape

| Concern | P1 (fork-tag) only | P2 (patches) only | **P1 + P2 (this choice)** |
|---|---|---|---|
| Consumer integration | One-line `[patch.crates-io]` | Apply N patches per build | **One-line** |
| Audit transparency | `git diff v3.0.6 v3.0.6-bn254` | Read N patch files | **Both** (diff *and* patch series) |
| Re-baseline cost (new cosmwasm tag) | Rebase all commits | Re-author patches | **Re-author patches, regenerate fork** |
| Upstream PR shape | Cherry-pick from fork | Apply patches in branch | **Either path** (we choose at PR time) |
| Source-of-truth ambiguity | Unclear if patches are stale | Unclear if fork is stale | **Patches are canonical; fork is generated** |

The P1+P2 combo costs nothing extra at authoring time (the fork is regenerated from the patches by the script) and adds belt-and-braces for both consumers and upstream maintainers.

## How to (re-)generate the fork

From a clean working tree:

```powershell
# Default: v3.0.6 → v3.0.6-bn254 in $env:USERPROFILE\junoclaw-build\cosmwasm-bn254-fork
.\wasmvm-fork\patches\make-cosmwasm-bn254-fork.ps1

# Re-baseline against a future tag, e.g. v3.0.7:
.\wasmvm-fork\patches\make-cosmwasm-bn254-fork.ps1 -CosmwasmTag v3.0.7 -TagName v3.0.7-bn254 -Force
```

The script will:

1. Clone `CosmWasm/cosmwasm` at the requested tag.
2. Create a branch `bn254/<tag>` from that tag.
3. Apply the v3.0.x patch series in numerical order, **one commit per patch**, so the resulting `git log` has the same shape as a feature-branch PR.
4. Tag the resulting HEAD with the requested tag name.

## How to publish the fork (one-time GitHub setup)

Once the script succeeds locally:

1. Create an **empty** repo at `https://github.com/Dragonmonk111/cosmwasm-bn254`.
   - Settings: Public, **no** README / LICENSE / .gitignore. Empty.
   - Description: "BN254 (alt_bn128) host functions for CosmWasm — patch series ports against cosmwasm v3.0.6, motivated by Juno gov #374."
2. From the fork directory:

   ```powershell
   cd "$env:USERPROFILE\junoclaw-build\cosmwasm-bn254-fork"
   git remote add origin https://github.com/Dragonmonk111/cosmwasm-bn254.git
   git push -u origin bn254/v3.0.6
   git push origin v3.0.6-bn254
   ```

3. On GitHub, set `bn254/v3.0.6` as the default branch.
4. Add a top-of-README that says: *"This is a generated convenience fork. Authoring source-of-truth is [`Dragonmonk111/junoclaw/wasmvm-fork/patches/v3.0.x/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches/v3.0.x). Re-baseline by editing the patches there and re-running `make-cosmwasm-bn254-fork.ps1`."*
5. Enable branch protection on `bn254/v3.0.6` (require linear history, require signed commits if you've set up commit signing).
6. Apply OCI-supply-chain hygiene: 2FA enforced (already a GitHub-account-level setting per the warg-registry-package memory file), Dependabot alerts enabled, SBOM available via the GitHub-native dependency graph.

## How consumers integrate

Run the build script (`build-wasmvm-bn254.sh`, to be created in Phase 2 of the Track B plan):

1. Script clones `CosmWasm/wasmvm` at tag `v3.0.4`
2. Script clones `CosmWasm/cosmwasm` at tag `v3.0.6`
3. Script applies the 10 patches from `wasmvm-fork/patches/v3.0.x/`
4. Script adds `[patch.crates-io]` to `libwasmvm/Cargo.toml` pointing to the local patched cosmwasm
5. Script builds `libwasmvm.x86_64.so`
6. Consumer swaps the `.so` into their junod build

No fork needed. No git tags to maintain. Patches rebase automatically against future cosmwasm releases.

## What stays in `wasmvm-fork/patches/v3.0.x/`

The patch series is **unchanged** by this decision:

- `00-rust-toolchain.toml.patch` through `09-cosmwasm-crypto-bn254-new-crate.patch` (10 files, 10/10 CLEAN against v3.0.6)
- `README.md` — patch manifest
- `apply-and-test-v3.ps1` (one level up) — verification harness

These remain the authoring source-of-truth. To revise the patches:

1. Apply them to a clean v3.0.6 checkout (use `apply-and-test-v3.ps1`)
2. Make changes in the working tree
3. Regenerate the patches with `git diff -p` or the existing `regen-patches-cargo-v3.ps1`
4. Verify with `apply-and-test-v3.ps1 -CosmwasmTag v3.0.6`
5. Regenerate the fork with `make-cosmwasm-bn254-fork.ps1 -Force`
6. Force-push the updated branch and re-tag

## Cross-references

- [`make-cosmwasm-bn254-fork.ps1`](./make-cosmwasm-bn254-fork.ps1) — the generator script.
- [`v3.0.x/README.md`](./v3.0.x/README.md) — patch series manifest.
- [`apply-and-test-v3.ps1`](./apply-and-test-v3.ps1) — verification harness used by `make-cosmwasm-bn254-fork.ps1`'s sanity check.
- [`memory/track-b-forward-port.md`](../../memory/track-b-forward-port.md) — Track B forward-port worklog.
- [`memory/SESSION_PROTOCOL.md`](../../memory/SESSION_PROTOCOL.md) §T2c — the decision context.
- [`docs/UPSTREAM_ISSUE_DRAFTS.md`](../../docs/UPSTREAM_ISSUE_DRAFTS.md) — upstream Issue 1 + Issue 2 (both reference the patch directories; Issue 1 will additionally reference the fork tag once published).

---

*Apache-2.0. This file documents the publication-shape decision; the script implements it.*
