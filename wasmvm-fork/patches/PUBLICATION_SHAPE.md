# Publication shape — `cosmwasm-bn254` fork + patches (P1 + keep patches)

*Decision recorded 2026-05-14. Anchor: [`memory/SESSION_PROTOCOL.md`](../../memory/SESSION_PROTOCOL.md) §T2c.*

## TL;DR

Two artefacts, one source-of-truth:

1. **Canonical authoring source-of-truth:** [`wasmvm-fork/patches/v3.0.x/`](./v3.0.x/) — the patch series. Edit here. Re-baseline here.
2. **Generated consumer convenience:** `Dragonmonk111/cosmwasm-bn254` GitHub repo, tagged `v3.0.6-bn254`. Produced by [`make-cosmwasm-bn254-fork.ps1`](./make-cosmwasm-bn254-fork.ps1). Consumers depend on this via a one-line `[patch.crates-io]`.

This is the **P1 + keep patches** option from the T2c decision: belt-and-braces. Consumers get the easy integration path (the fork tag), and we keep the patch series as the authoring artefact (which is what we'd PR upstream and what we'd use to re-baseline against future cosmwasm tags).

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

In their `Cargo.toml`:

```toml
[patch.crates-io]
cosmwasm-std = { git = "https://github.com/Dragonmonk111/cosmwasm-bn254", tag = "v3.0.6-bn254" }
cosmwasm-vm  = { git = "https://github.com/Dragonmonk111/cosmwasm-bn254", tag = "v3.0.6-bn254" }
cosmwasm-crypto-bn254 = { git = "https://github.com/Dragonmonk111/cosmwasm-bn254", tag = "v3.0.6-bn254" }
```

That's the entire integration. No patch application required on the consumer side.

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
