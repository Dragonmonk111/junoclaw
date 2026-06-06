# Deferred Work Plan

**Created:** 5 June 2026  
**Status:** Phase 2 complete, awaiting user direction on remaining workstreams

---

## Overview

Phase 2 (differential test, measured gas numbers, v30 handler cleanup) is complete and committed. The following workstreams are deferred pending user approval and gating conditions:

1. **Patch verification** — confirm patches apply cleanly to current v2.2.x tags
2. **Track B forward-port** — rebase 10 patches onto cosmwasm v3.0.1 + wasmvm v3.0.4
3. **Moultbook devnet** — stand up independent devnet deploy (build + deploy + smoke-test)
4. **PR #1202 review** — post Moultbook-style code review on Jake's v30 branch

---

## Workstream 1: Patch Verification

**ID:** `v1`  
**Priority:** High  
**Gating:** None (can run immediately)  
**Estimated effort:** 30 minutes

### Objective
Run `wasmvm-fork/patches/check-baseline.sh` against current v2.2.x tags to confirm the 10 patches still apply cleanly. This is a sanity check before starting Track B forward-port.

### Steps

1. Navigate to `wasmvm-fork/`
2. Run `./patches/check-baseline.sh`
3. Review output for patch fuzz or rejection
4. If patches fail:
   - Document which patches fail and why
   - Determine if manual fix is needed or if upstream has moved in incompatible ways
5. If patches pass:
   - Mark `v1` complete
   - Proceed to Track B (pending ownership confirmation)

### Success criteria
- All 10 patches apply with zero fuzz
- No manual conflict resolution required

### Failure modes
- Patch fuzz > 0: document, assess severity, may need to regenerate patches
- Patch rejection: upstream API changed, need to investigate drift

---

## Workstream 2: Track B Forward-Port

**IDs:** `v2`, `t1`, `t2`  
**Priority:** High  
**Gating:** Must confirm ownership with Jake/Juno AI before starting (`v2`)  
**Estimated effort:** 4-6 hours

### Objective
Forward-port the 10 BN254 patches from the v2.2.2 baseline onto cosmwasm v3.0.1 + wasmvm v3.0.4. This is the upstream PR path for the BN254 precompile.

### Prerequisites
- [ ] `v1` complete (patches verified on v2.2.x)
- [ ] `v2` complete (ownership confirmed with Jake/Juno AI)

### Steps

#### Phase 2.1: Clone and setup (`t1`)
1. Clone `cosmwasm` at tag `v3.0.1`
2. Clone `wasmvm` at tag `v3.0.4`
3. Create feature branches: `track-b-bn254` in both repos
4. Re-pin Rust toolchain to 1.78 (same as v2.2.2 baseline) in `rust-toolchain.toml`
5. Verify clean build of unmodified v3.0.1/v3.0.4

#### Phase 2.2: Forward-port patches (`t1`)
1. Extract the 10 patches from `wasmvm-fork/patches/`
2. Apply each patch to the v3.0.1/v3.0.4 branches
3. Resolve conflicts (API drift expected)
4. Document each conflict and resolution

#### Phase 2.3: Adapt API drift (`t2`)
Known API changes between v2.2.x and v3.x:
- `read_region` signature change
- Host-function surface changes
- `cosmwasm-vm` internal refactors

For each:
1. Identify the new API in v3.x
2. Adapt the BN254 host-function call sites
3. Update `cosmwasm-crypto-bn254` if needed
4. Update `cosmwasm-vm` integration if needed

#### Phase 2.4: Build verification (`t2`)
1. Build `cosmwasm-crypto-bn254` on v3
2. Build `cosmwasm-vm` with BN254 integration on v3
3. Run unit tests for `cosmwasm-crypto-bn254`
4. Run differential test on v3 (if host-fn surface allows)

### Success criteria
- `cosmwasm-crypto-bn254` builds clean on v3.0.1
- `cosmwasm-vm` builds clean with BN254 integration on v3.0.4
- All unit tests pass
- Differential test passes (if host-fn surface allows)

### Failure modes
- API drift too large: may need to rewrite portions of the integration
- Toolchain incompatibility: may need to adjust Rust version (requires re-verification)
- Build failures: investigate, document, may need upstream guidance

---

## Workstream 3: Moultbook Devnet

**ID:** `m1`  
**Priority:** Medium  
**Gating:** None (can run in parallel with Track B)  
**Estimated effort:** 2-3 hours

### Objective
Stand up a Moultbook devnet deploy with build + deploy + smoke-test scripts. This is independent of BN254/v30 work and can proceed in parallel.

### Context
Moultbook is the cross-agent shared knowledge substrate. This workstream creates the devnet infrastructure for testing Moultbook contracts independent of the BN254 precompile.

### Steps

#### Phase 3.1: Devnet infrastructure
1. Create `devnet/scripts/run-moultbook-devnet.sh` (based on `run-devnet.sh`)
2. Configure genesis for Moultbook-specific parameters
3. Set up permissive CORS and pre-funded accounts

#### Phase 3.2: Build scripts
1. Create `devnet/scripts/build-moultbook-contracts-docker.sh`
2. Integrate with canonical `cosmwasm/optimizer` image
3. Support multiple contract variants if needed

#### Phase 3.3: Deploy scripts
1. Create `devnet/scripts/deploy-moultbook.sh`
2. Store wasms, instantiate contracts
3. Write `moultbook-deploy.env` with addresses/code IDs

#### Phase 3.4: Smoke-test scripts
1. Create `devnet/scripts/smoke-test-moultbook.sh`
2. Basic functionality tests (read, write, query)
3. Verify devnet is healthy before proceeding

### Success criteria
- Devnet starts cleanly and advances blocks
- Contracts build and deploy without errors
- Smoke tests pass
- Scripts are idempotent and reproducible

### Failure modes
- Genesis misconfiguration: adjust parameters
- Contract build failures: fix code or build script
- Deploy failures: check gas/fees, address configuration

---

## Workstream 4: PR #1202 Code Review

**ID:** `v3`  
**Priority:** Medium  
**Gating:** None (can run immediately)  
**Estimated effort:** 1-2 hours

### Objective
Review Jake's v30 PR (#1202 on Juno repo) and post a Moultbook-style code review. This is a governance/participation task, not a blocking technical dependency.

### Context
From `docs/JUNO_V30_PR_ASSESSMENT.md` §9, this is one of the action items. A Moultbook-style review means:
- Detailed, line-by-line analysis
- Focus on correctness, security, and upgrade safety
- Constructive tone with specific suggestions
- Reference to relevant standards/patterns

### Steps

1. Read PR #1202 description and diff
2. Review the v30 upgrade handler code
3. Check against patterns from Dimi's v28→v29 work
4. Verify wasmvm bump and BN254 import registration
5. Identify any concerns or questions
6. Draft review comments (Moultbook style)
7. Post review on GitHub

### Success criteria
- Review posted on PR #1202
- Review is constructive and specific
- Any concerns are clearly articulated

### Failure modes
- PR is already merged: task complete, no action needed
- PR is in flux: defer review until stable

---

## Sequencing and Dependencies

```
v1 (patch verification) ──┐
                         ├──> v2 (ownership confirm) ──> t1 (clone + forward-port) ──> t2 (adapt + build)
m1 (Moultbook devnet) ───┘ (parallel)                                         │
                                                                                     │
v3 (PR #1202 review) ──────────────────────────────────────────────────────────────┘ (parallel)
```

### Parallelizable workstreams
- `m1` (Moultbook devnet) can run immediately, independent of everything else
- `v3` (PR #1202 review) can run immediately, independent of everything else
- `v1` (patch verification) can run immediately, unblocks Track B

### Sequential dependencies
- `v2` (ownership confirm) must complete before `t1` (Track B forward-port)
- `t1` must complete before `t2` (adapt API drift + build)

---

## Decision Points

### 1. Should we start Track B now?
- **If yes:** Confirm ownership with Jake/Juno AI first (`v2`)
- **If no:** Defer until after Moultbook devnet is stable

### 2. Should we prioritize Moultbook devnet over Track B?
- **If yes:** Start `m1` immediately, defer Track B
- **If no:** Start Track B after `v1` and `v2` complete

### 3. Should we post the PR #1202 review now?
- **If yes:** Run `v3` immediately
- **If no:** Defer until after Track B or Moultbook work

---

## Next Actions (Awaiting User Direction)

1. **Immediate start candidates (no gating):**
   - `v1`: Run patch verification
   - `m1`: Start Moultbook devnet
   - `v3`: Review PR #1202

2. **Gated start (requires confirmation):**
   - `v2`: Confirm Track B ownership with Jake/Juno AI
   - `t1`/`t2`: Track B forward-port (after `v2`)

3. **User decision needed:**
   - Which workstream to prioritize?
   - Should Track B proceed, or defer?
   - Should Moultbook devnet proceed in parallel?

---

## Notes

- All workstreams are independent except Track B's internal sequencing
- Moultbook devnet is explicitly independent of BN254/v30 work
- Track B is the critical path for upstream PR submission
- PR #1202 review is a governance participation task, not blocking

**Last updated:** 5 June 2026
