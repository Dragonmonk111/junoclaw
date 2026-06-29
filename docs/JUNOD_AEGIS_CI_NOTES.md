# junod-aegis CI — status, options, and action plan

Last updated: 2026-06-29

> **2026-06-29 — Option 1 implemented + first green run.** Added
> `.github/workflows/aegis-build-junod.yml` to the main `junoclaw` repo: `workflow_dispatch` +
> nightly (`cron 0 4 * * *`), Go 1.24 on `ubuntu-24.04`. It clones Juno at the pinned tag, clones
> the cometbft + cosmos-sdk forks (ibc-go optional via input), applies local-path `replace`
> directives, runs `go mod tidy`, builds with `-ldflags=-checklinkname=0`, smoke-tests
> `version --long` + `AEGIS_HYBRID_TRANSPORT=1`, records the sha256, and uploads the binary as an
> artifact. Steps mirror `docs/BUILD_AEGIS_JUNO_BINARY.md`.
>
> **First successful run** (Juno v29.0.0, cometbft `aegis-phase-cf-hybrid`, sdk
> `aegis-phase-d3-hybrid`, ibc-go skipped): artifact `junod-aegis-v29.0.0`, binary sha256
> `53aaf4c9fc5fc70f4c7a8bf2395407fa7deee913ce9a0bae6d159da84e518c83`, `AEGIS_HYBRID_TRANSPORT=1`
> smoke ok.
>
> **Three-fork run verified** (Juno v29.0.0, cometbft `aegis-phase-cf-hybrid`, sdk
> `aegis-phase-d3-hybrid`, **ibc-go `aegis-phase-g-hybrid-client`**): artifact
> `junod-aegis-v29.0.0`, binary sha256 `661adbaf7512d20ac5d820493271f371747c09e361ad14ce706e2c2b28d3b2a5`,
> `AEGIS_HYBRID_TRANSPORT=1` smoke ok. This is now the **default** (`ibc_go_ref` defaults to the Phase
> G branch); set it blank to fall back to the two-fork build. The `setup-go` cache warning was
> silenced by setting `cache: false`; only the harmless Node.js 20 deprecation warning remains.

## What `junod-aegis` is

`junod-aegis` is the **assembled PQC node binary** — upstream Juno (`v29.0.0`) built with `replace`
directives pointing at the three Aegis forks:

- `github.com/cometbft/cometbft` → `Dragonmonk111/cometbft` `aegis-phase-cf-hybrid`
- `github.com/cosmos/cosmos-sdk` → `Dragonmonk111/cosmos-sdk` `aegis-phase-d3-hybrid`
- `github.com/cosmos/ibc-go` → `Dragonmonk111/ibc-go` `aegis-phase-g-hybrid-client`

It is the artifact that gives you `junod init --aegis-hybrid-consensus`, hybrid transport
(`AEGIS_HYBRID_TRANSPORT=1`), `tx staking rotate-cons-key`, hybrid accounts, and the hybrid IBC
07-tendermint client all in one runnable node. It was last built on the production VM at
`~/aegis-build` (binary sha256 `98e6813…`, Juno v29.0.0, go1.24.0).

## The CI question

The concern raised earlier was a **misunderstanding**: there is no broken CI we are obligated to fix.
Findings:

1. The fork CIs and the **main `junoclaw` CI are already fixed** (Go 1.24 workflow bumps across
   cometbft / cosmos-sdk / ibc-go; ARM64 determinism moved to QEMU on `ubuntu-24.04`). Done & pushed.
2. A standalone `github.com/Dragonmonk111/junod-aegis` repo **is not accessible from this workspace**
   — `git clone …/junod-aegis.git` returns *"Repository not found"*. It either does not exist yet or
   is private without local credentials. There is therefore **no junod-aegis workflow we can see or
   edit here**.

**Conclusion:** there is nothing broken to repair right now. The only open question is whether we
*want* a dedicated junod-aegis repo + CI at all, vs building the assembled binary another way.

## Options

### Option 1 — No separate repo; build the binary in `junoclaw` CI (recommended)
Add a single GitHub Actions job in **main `junoclaw`** that, on demand (or nightly), checks out
upstream Juno at the pinned tag, applies the three `replace` directives to the Aegis fork tips,
runs `go build`, and uploads the binary as a workflow artifact (+ records its sha256). This keeps
one source of truth (the forks), needs no new repo, and the CI is already on Go 1.24.

- **Pros:** zero new repo to maintain; reuses the working junoclaw CI; reproducible artifact.
- **Cons:** the assembled binary is an artifact, not a git history.

### Option 2 — Create the `Dragonmonk111/junod-aegis` repo with its own CI
Create the repo (a thin Juno fork carrying only the `go.mod` replace block + a build workflow).
Then mirror the same fixes already applied elsewhere.

- **Pros:** a stable, taggable home for the assembled node; release artifacts per tag.
- **Cons:** another repo + go.mod pin to keep in lockstep with the three forks on every fork bump.

### Option 3 — Status quo (manual build on the VM)
Keep building `~/aegis-build` by hand when a binary is needed. Fine for measurement runs; no CI.

## Action plan (when you decide)

**If Option 1 (recommended):** I can write `.github/workflows/aegis-build-junod.yml` in `junoclaw`
now — `workflow_dispatch` + nightly, Go 1.24, applies the replaces, `go build`, uploads artifact,
prints sha256. No external repo needed.

**If Option 2:** create the repo first (steps below), then I supply the `go.mod` replace block and a
`build.yml` workflow (Go 1.24; optional ARM64 via QEMU to match the determinism proof).

```bash
# Create the repo (run locally where gh is authed)
gh repo create Dragonmonk111/junod-aegis --public \
  --description "Project Aegis PQC node: Juno + hybrid cometbft/cosmos-sdk/ibc-go forks"
# Then I will provide go.mod (with the 3 replaces), Makefile, and .github/workflows/build.yml
```

**Whichever option:** when any fork tip moves, bump the corresponding `replace` pseudo-version to the
**full canonical 12-char sha** (short shas are rejected by `go mod tidy`). Current tips:
cometbft `ff4dcefdc083`, cosmos-sdk `aegis-phase-d3-hybrid@5792792`, ibc-go `aegis-phase-g-hybrid-client`.

## Recommendation

Go with **Option 1**. It gives a reproducible junod-aegis binary in CI with no extra repo to babysit,
and it slots straight into the already-green Go-1.24 `junoclaw` pipeline. Say the word and I'll add
the workflow.
