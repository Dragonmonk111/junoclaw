# Juno v30 Upgrade Handler — Design & Handoff Brief

**Status:** DRAFT — to be finalized after upstream PRs land (Phase 3 of [POST_VOTE_EXECUTION_PLAN](./POST_VOTE_EXECUTION_PLAN.md))
**Co-author target:** Dimi (validator, security-patch steward; pattern-matched on his v28→v29 work)
**Mandate:** Juno proposal [#374](https://ping.pub/juno/gov/374) — passed ~80% Yes, 2026-05-05
**Companion docs:** [`POST_VOTE_EXECUTION_PLAN.md`](./POST_VOTE_EXECUTION_PLAN.md), [`ADR-001-BN254-PRECOMPILE.md`](./ADR-001-BN254-PRECOMPILE.md)

---

## Purpose of this document

This is the brief we hand to Dimi (and any other reviewing validator) when asking for co-author sign-off on the v30 chain upgrade. It exists so the conversation starts from a written design, not from "trust us, it's small."

The design is deliberately minimal. The handler does **two things, only**. Anything not in §3 is explicitly out of scope.

---

## §1. Constraint summary

Marius (former Juno core contracts dev) said when we floated the upgrade:

> *"Be careful with the implementation, I cleaned up the code base massively and made it stable."*

This is now a binding constraint:

- **No state migrations** beyond what `RunMigrations` does automatically for module version bumps.
- **No param changes** outside the wasmd accepted-capabilities list.
- **No "while we're here" cleanups.** Cleanups that are objectively safe go in separate, post-v30 PRs to the Juno repo.
- **No new modules.** The whole change is wasmvm version + capability registration.

If we want to change anything else on the chain, that is a separate proposal, debated and voted on separately. v30 is one job.

---

## §2. Pattern reference: v28→v29

We pattern-match on Dimi's v28→v29 work because:

1. It's the most recent precedent for a CosmWasm-touching upgrade
2. It's small, well-structured, and reviewed by validators
3. It has a clear rollback story
4. v29.1 is the production tag we're building from — same code paths, same module versions

We borrow:
- Directory layout (`app/upgrades/v29/`)
- `UpgradeName` / `Plan.Info` JSON conventions
- Binary release naming (`junod-v29-linux-amd64.tar.gz` style)
- The convention of GPG-signing the SHA256 checksums file
- The rollback document structure

---

## §3. What v30 does (the entire scope)

### §3.1. Bump the wasmvm dependency

In `go.mod`:

```diff
- github.com/CosmWasm/wasmvm/v2 v2.x.y
+ github.com/CosmWasm/wasmvm/v2 v2.<NEW>.<NEW>
```

The `<NEW>` version is whichever wasmvm release contains the merged BN254 host-function PR (Phase 3.2 of the plan). Until that release exists, this section's exact numbers are placeholders.

### §3.2. Register the `bn254` capability

In `app/upgrades/v30/upgrade.go`:

```go
package v30

import (
    "context"

    "cosmossdk.io/core/appmodule"
    upgradetypes "cosmossdk.io/x/upgrade/types"
    "github.com/cosmos/cosmos-sdk/types/module"

    wasmkeeper "github.com/CosmWasm/wasmd/x/wasm/keeper"
    wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"

    "github.com/CosmosContracts/juno/v30/app/keepers"
)

const UpgradeName = "v30"

func CreateUpgradeHandler(
    mm *module.Manager,
    cfg module.Configurator,
    keepers *keepers.AppKeepers,
) upgradetypes.UpgradeHandler {
    return func(ctx context.Context, plan upgradetypes.Plan, fromVM module.VersionMap) (module.VersionMap, error) {
        // Add "bn254" to the accepted-capabilities list.
        params := keepers.WasmKeeper.GetParams(ctx)
        // The accepted-capabilities mechanism on wasmd uses node-side flags
        // rather than param state in current versions; verify this against
        // the wasmd version pinned in v30 before finalizing.
        // If the mechanism is param-based, append "bn254" to the existing
        // list and call SetParams(ctx, params).
        // If it's flag-based, document the flag in the validator-facing notes
        // (see §6) and skip the SetParams call here.
        _ = params

        // Run module migrations (no-op for our scope, but mandatory).
        return mm.RunMigrations(ctx, cfg, fromVM)
    }
}
```

**The handler body is ~15 lines.** Anything longer is scope creep. Anything that mutates state outside `wasmd` capabilities is scope creep.

> **Note for Dimi:** the wasmd accepted-capabilities surface has shifted between releases. In some versions it's a node flag (`--wasm.accept_list`); in others it's chain-state under `wasm.Params.AcceptedCapabilities`. Whichever applies in the v30-targeted wasmd version, **only one of the two paths above is right**. We pick the right one during Phase 2.2 of the plan and update this doc.

### §3.3. `app.go` registration

```go
// In app.go, in NewJunoApp's upgrade-handler registration loop:
app.UpgradeKeeper.SetUpgradeHandler(
    v30.UpgradeName,
    v30.CreateUpgradeHandler(app.mm, app.configurator, &app.AppKeepers),
)

// Store loader for any new module stores (none for v30, but the convention
// matches v29):
if upgradeInfo.Name == v30.UpgradeName && !app.UpgradeKeeper.IsSkipHeight(upgradeInfo.Height) {
    storeUpgrades := storetypes.StoreUpgrades{
        // No new stores. No deleted stores. No renamed stores.
    }
    app.SetStoreLoader(upgradetypes.UpgradeStoreLoader(upgradeInfo.Height, &storeUpgrades))
}
```

### §3.4. Constants file

`app/upgrades/v30/constants.go`:

```go
package v30

import (
    storetypes "cosmossdk.io/store/types"
    "github.com/CosmosContracts/juno/v30/app/upgrades"
)

var Upgrade = upgrades.Upgrade{
    UpgradeName:          UpgradeName,
    CreateUpgradeHandler: CreateUpgradeHandler,
    StoreUpgrades:        storetypes.StoreUpgrades{},
}
```

That is the whole change to the Juno chain repo. ~20 lines of code across 2 files. A reviewer can read it in two minutes.

---

## §4. What v30 explicitly does NOT do

These are listed because validators rightly ask. We say no, in writing, before they ask.

| Out of scope | Why |
|--------------|-----|
| Cosmos SDK version bump | Separate proposal, separate validator coordination |
| IBC version bump | Same |
| New `x/` modules | Same |
| Param changes (other than wasmd capabilities) | Same |
| Genesis modifications | Not applicable — this is a live-chain upgrade |
| State migrations beyond `RunMigrations` | None needed; the change is purely additive at the VM layer |
| Any contract migrations | Existing contracts unaffected; new contracts opt in via `cosmwasm_2_3` feature |
| Tendermint/CometBFT version bump | Separate concern |
| Re-genesis or chain-id change | Hard no — Juno mainnet is `juno-1`, stays `juno-1` |

If a validator suggests folding any of the above into v30, the answer is: "Good catch — let's discuss it as a separate proposal post-v30." We do not bundle, even when bundling looks efficient.

---

## §5. Test plan

### §5.1. Local rehearsal — 3 separate runs

Each rehearsal runs against a **different** v29.1 mainnet height. We use 3 different heights to flush out timing-dependent bugs (e.g., "the upgrade fires fine at heights ending in 0 but not in 5") even though the upgrade should be height-independent.

```bash
# For each of 3 chosen heights H:
# 1. Sync a Juno node from a v29.1 archival snapshot to height H
junod start --halt-height $H

# 2. Inspect state, snapshot it
junod export --height $H > pre-upgrade-$H.json

# 3. Drop the v30-patched binary in place
cp ~/junoclaw-build/juno-v30/build/junod ~/.juno/cosmovisor/upgrades/v30/bin/junod

# 4. Restart with the upgrade plan
junod start

# 5. Verify
#    a) Block production resumes at H+1
#    b) An existing contract (e.g., zk-verifier code_id 64) still executes
#    c) A NEW contract uploaded with the bn254 feature flag instantiates and
#       verifies a Groth16 proof at the lower gas cost
```

Logs from each rehearsal go in `_private/v30_rehearsal_logs/`. We attach them to the handoff brief for Dimi.

### §5.2. uni-7 testnet rehearsal (Phase 4)

After the 3 local rehearsals pass, we submit `MsgSoftwareUpgrade` on uni-7. We get other uni-7 validators to vote (Telegram + Discord). The upgrade fires. We verify post-upgrade behaviour the same way as §5.1.

uni-7 catches a class of bugs local rehearsals miss: real validator coordination, real consensus during the upgrade transition, real network conditions.

### §5.3. Mainnet (Phase 5)

After uni-7 succeeds, we submit `MsgSoftwareUpgrade` on `juno-1`. Pre-built binaries published, GPG-signed. Validators have ~10 days between vote-end and upgrade-height to swap binaries.

---

## §6. Mainnet upgrade proposal — `MsgSoftwareUpgrade`

### §6.1. Plan structure

```json
{
  "messages": [
    {
      "@type": "/cosmos.upgrade.v1beta1.MsgSoftwareUpgrade",
      "authority": "juno10d07y265gmmuvt4z0w9aw880jnsr700jvss730",
      "plan": {
        "name": "v30",
        "time": "0001-01-01T00:00:00Z",
        "height": "<CURRENT_HEIGHT_PLUS_432000>",
        "info": "{\"binaries\":{\"linux/amd64\":\"https://github.com/Dragonmonk111/junoclaw/releases/download/v30-upgrade/junod-v30-linux-amd64.tar.gz?checksum=sha256:<HASH>\",\"linux/arm64\":\"https://github.com/Dragonmonk111/junoclaw/releases/download/v30-upgrade/junod-v30-linux-arm64.tar.gz?checksum=sha256:<HASH>\",\"darwin/amd64\":\"https://github.com/Dragonmonk111/junoclaw/releases/download/v30-upgrade/junod-v30-darwin-amd64.tar.gz?checksum=sha256:<HASH>\",\"darwin/arm64\":\"https://github.com/Dragonmonk111/junoclaw/releases/download/v30-upgrade/junod-v30-darwin-arm64.tar.gz?checksum=sha256:<HASH>\"}}",
        "upgraded_client_state": null
      }
    }
  ],
  "metadata": "ipfs://<HASH>",
  "deposit": "5000000000ujuno",
  "title": "Juno v30 — BN254 precompile (Groth16 verification at ~2x lower gas)",
  "summary": "Bumps wasmvm to v2.<NEW>.<NEW> and registers the bn254 chain capability. Implements the upstream-merged CosmWasm BN254 host functions, motivated by passed signaling proposal #374. Pure-Wasm Groth16 verification gas drops from ~371k to ~200k SDK gas, enabling cheap mandatory verification on every on-chain agent task. Handler is ~20 lines across 2 files; no state migrations; rollback plan published. Co-authored with Dimi."
}
```

### §6.2. Height calculation

`<CURRENT_HEIGHT_PLUS_432000>` ≈ +15 days at 3-second blocks (3s/block × 28,800 blocks/day × 15 days = 432,000).

Breakdown:
- 5 days = voting period
- 10 days = validator binary-swap window after vote ends

If voting begins at block H, vote ends at H+144,000, upgrade fires at H+432,000.

### §6.3. Deposit

5,000 JUNO from mother wallet `juno1scpm8wukdq52lqs2g9d9ulcza4yeyy5qxct7g2` (matches Juno gov `min_deposit` of 5,000 JUNO discovered during prop #374 submission).

If the prop fails (vetoed) or doesn't reach quorum (33.4%), the deposit is returned. If `burn_vote_veto = true` triggers, the deposit is burned. We accept that risk as the price of participation.

---

## §7. Pre-built binaries

Published on `Dragonmonk111/junoclaw/releases/tag/v30-upgrade`:

```
junod-v30-linux-amd64.tar.gz       (~50 MB)
junod-v30-linux-amd64.tar.gz.sha256
junod-v30-linux-arm64.tar.gz
junod-v30-linux-arm64.tar.gz.sha256
junod-v30-darwin-amd64.tar.gz
junod-v30-darwin-amd64.tar.gz.sha256
junod-v30-darwin-arm64.tar.gz
junod-v30-darwin-arm64.tar.gz.sha256
checksums.txt                       (concatenation of all .sha256s)
checksums.txt.asc                   (GPG-signed by VairagyaNodes maintainer key)
```

GPG fingerprint of the maintainer key is published in repo `SECURITY.md`.

Binaries are reproducible from source — the release also contains a `BUILD.md` showing exact compiler version, build command, and the git commit being released.

---

## §8. Rollback plan

Published as a separate doc: `docs/V30_ROLLBACK_PLAN.md` (to be drafted in Phase 5.4).

Outline:

### §8.1. Detecting a failed upgrade

- Block height stops advancing past `Plan.Height`
- Validators report panic in logs at `Plan.Height` block
- Consensus fails to reach 2/3+ on the post-upgrade state

### §8.2. Reverting

```bash
# Stop the failing v30 binary
systemctl stop junod  # or supervisor stop, or kill -SIGTERM

# Replace with v29.1 binary
cp ~/.juno/cosmovisor/upgrades/v29/bin/junod /usr/local/bin/junod

# Remove the upgrade plan that triggered the failure
junod tx upgrade cancel-software-upgrade ...    # (if needed)

# Restart from the last known good height
junod start --halt-height <PRE_UPGRADE_HEIGHT>
```

In practice, validators who ran cosmovisor with the v30 binary pre-staged can downgrade by symlinking to the v29 binary and restarting.

### §8.3. Communications during a rollback

- Telegram `#juno-validators`: immediate alert, downgrade instructions, ETA for fixed binary
- Discord `#general`: status update, no panic
- A new proposal with the fix gets submitted within 7 days; we do **not** rush it

The rollback plan exists so validators vote yes with confidence. A clear rollback story is what separates "experimental upgrade" from "responsible upgrade." Validators read this section more carefully than any other.

---

## §9. Co-author handoff to Dimi

After Phase 4 (uni-7 rehearsal) succeeds, we send Dimi:

1. This document, in its current state at that point
2. Phase 4.3 logs from uni-7
3. Phase 5.1 local rehearsal logs (3 runs)
4. The full `app/upgrades/v30/` directory as a draft commit
5. The draft `MsgSoftwareUpgrade` JSON

Ask:

> "Dimi — we've finished local rehearsals (3 runs, different heights, all clean) and the uni-7 upgrade fired without halt. Logs attached. The handler is 20 lines across 2 files; same pattern as your v29 work. Would you co-sign as the chain-side author of record? If yes, I'd add `Co-Authored-By:` on the commit and name you as co-proposer in the gov text. If no rush or no time, I'm happy to proceed solo and credit you wherever you actually contributed."

If Dimi accepts: he gets `Co-Authored-By: Dimi <his-email>` on the v30 commit. The gov proposal title becomes "Juno v30 — BN254 precompile (co-proposed by VairagyaNodes & Dimi)."

If Dimi declines or is busy: we proceed solo. The gov proposal still says "Reviewed by Dimi where bandwidth allowed" if that reflects reality, or omits him entirely if he asks us to. **We do not push.**

---

## §10. Validator-facing summary (what we'd post in `#juno-validators`)

Short version, for the Telegram channel — no jargon, all plain:

> **Juno v30 upgrade — what it does and why**
>
> v30 is a single-purpose upgrade: it adds Ethereum-compatible BN254 elliptic-curve precompiles to CosmWasm. Concretely, this means contracts that verify Groth16 zero-knowledge proofs (today: 371k gas per verification) will verify them at ~200k gas after v30. Same proofs, same cryptography, lower cost.
>
> The upgrade handler is 20 lines across 2 files. No state migrations, no param changes outside wasmd capabilities, no module bumps. The mandate comes from passed governance proposal [#374](https://ping.pub/juno/gov/374) (~80% Yes).
>
> Process:
> - Local rehearsal × 3 ✓
> - uni-7 testnet upgrade ✓
> - Mainnet `MsgSoftwareUpgrade` proposed → 5d voting → 10d swap window → upgrade fires
>
> Pre-built binaries (linux-amd64, linux-arm64, darwin-amd64, darwin-arm64) are GPG-signed and published at `github.com/Dragonmonk111/junoclaw/releases/v30-upgrade`. Source-built reproducibly via `BUILD.md`. Rollback plan: `V30_ROLLBACK_PLAN.md`.
>
> Co-proposed with Dimi (validator). Reviewed upstream by the CosmWasm core team prior to this submission (see PR links in proposal text).
>
> Questions welcome here or in Discord. Apache-2.0 throughout.

This text gets copied into the on-chain proposal description verbatim.

---

## §11. Open questions for Dimi

(For when we actually hand this brief to him, not for now.)

1. The wasmd accepted-capabilities mechanism — flag vs param-state — depends on the v30-targeted wasmd version. Can you confirm which it is in your build, or is that a Phase 2.2 discovery for us?
2. Is +432,000 blocks (15 days) the right buffer between proposal-submission and upgrade-height, or do you prefer a longer/shorter window based on past v28→v29 experience?
3. Do you want to be on the GPG-signature for the release binaries (multi-sig the checksums file), or only on the commit?
4. Any test you'd want us to add to the rehearsal recipe that we haven't included?

---

## §12. Status checklist

- [ ] Patches rebased onto latest cosmwasm tag (Phase 0.1)
- [ ] Precompile gas measured on devnet (Phase 0.3)
- [ ] Upstream issues published (Phase 1)
- [ ] Maintainer feedback addressed (Phase 2)
- [ ] zk-verifier feature-flagged (Phase 2.1)
- [ ] v30 handler drafted (Phase 2.2) — **this document fully populates**
- [ ] Local rehearsal × 3 (Phase 2.3)
- [ ] Upstream PRs opened (Phase 3)
- [ ] Upstream PRs merged
- [ ] Wasmvm release containing BN254 published
- [ ] uni-7 upgrade proposal submitted (Phase 4.1)
- [ ] uni-7 upgrade fires successfully (Phase 4.3)
- [ ] Dimi handoff sent (Phase 5.1)
- [ ] Dimi accepts/declines co-sign (Phase 5.1)
- [ ] Pre-built binaries published (Phase 5.3)
- [ ] V30_ROLLBACK_PLAN.md drafted (Phase 5.4)
- [ ] Mainnet `MsgSoftwareUpgrade` submitted (Phase 5.2)
- [ ] Mainnet vote passes
- [ ] Mainnet upgrade fires successfully
- [ ] Post-upgrade BN254 verification gas measured & published

---

*Apache-2.0. Comments welcome via PR against this file.*
