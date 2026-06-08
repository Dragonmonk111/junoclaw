# BN254 Governance Wiring Plan

> Operational checklist bridging the signaling proposal (`GOV_PROP_COPYPASTE_BN254.md`) to
> on-chain execution: deposit, vote, software upgrade, and contract migration.

---

## Phase 1 — Signaling Proposal (this proposal)

**Goal:** Get a community mandate for BN254 before opening the upstream CosmWasm PR.

| Step | Command / Action | Who |
|---|---|---|
| 1. Publish HackMD | Copy `docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md` → HackMD → stable URL | VairagyaNodes |
| 2. Update `GOV_PROP_COPYPASTE_BN254.md` FIELD 2 | Paste the HackMD URL into the description | VairagyaNodes |
| 3. Verify deposit wallet | `junod query bank balances juno1qh8rgkdm77wrhlf7un20gz9gmtpxkyaeldt0pg` ≥ 5 000 JUNO | VairagyaNodes |
| 4. Submit `TextProposal` | `junod tx gov submit-proposal --type text ...` (see §CLI below) | VairagyaNodes |
| 5. Seed Commonwealth | Post link + summary in `forum.juno.network` | VairagyaNodes |
| 6. Track vote | `junod query gov proposal <id>`; Ping validators in Discord/TG | VairagyaNodes |

### §1.1 — Signaling proposal CLI

```bash
export PROPOSER=juno1qh8rgkdm77wrhlf7un20gz9gmtpxkyaeldt0pg
export TITLE="BN254 Precompile for CosmWasm — Cheap On-chain Groth16 Verification (post-#373)"
export DESC=$(cat docs/GOV_PROP_COPYPASTE_BN254.md | sed -n '/^---$/,/^---$/p')

junod tx gov submit-proposal \
  --title "$TITLE" \
  --description "$DESC" \
  --deposit 10000000ujuno \
  --from $PROPOSER \
  --chain-id juno-1 \
  --gas auto --gas-adjustment 1.4 \
  --gas-prices 0.075ujuno \
  --yes
```

*Note: signaling proposals on Juno are `TextProposal` (legacy `v1beta1`) or `MsgSubmitProposal` with empty `messages` (`v1`). Use whatever the current `junod` binary supports.*

---

## Phase 2 — Upstream CosmWasm PR (parallel, not gated by vote)

**Goal:** Get the patch set reviewed and merged into `CosmWasm/cosmwasm` and `CosmWasm/wasmvm`.

| Step | Status | Owner |
|---|---|---|
| Open PR against `cosmwasm` (v2.2.x branch) | ⏳ pending | VairagyaNodes / Cascade |
| Open PR against `wasmvm` (v2.2.x branch) | ⏳ pending | VairagyaNodes / Cascade |
| Forward-port to v3.0.x (`cosmwasm_2_3` → `cosmwasm_2_4` or equivalent) | ⏳ pending | Jake / Juno AI (external gating) |
| Address review feedback | ⏳ pending | VairagyaNodes |
| Tag release with BN254 host functions | ⏳ pending | CosmWasm maintainers |

**Track B (v3.0.x forward-port):** The `wasmvm-fork/patches/v3.0.x/` directory contains the rebased patch set. Ownership confirmation with Jake Hartnell / Juno AI is pending. This is **not** blocking the signaling proposal.

---

## Phase 3 — Software Upgrade Proposal (after upstream merge + Juno binary)

**Goal:** Upgrade Juno validators to a `junod` binary linked against the BN254-enabled `wasmvm`.

This is a `MsgSoftwareUpgrade` (or legacy `SoftwareUpgradeProposal`). It does **not** change contract state; it only adds the `bn254` capability to the VM accept-list.

### §3.1 — Upgrade handler checklist

| Step | Action | Verification |
|---|---|---|
| 1. Update Juno `go.mod` | Point to the tagged `wasmvm` with BN254 | `go mod tidy` passes |
| 2. Copy upgrade handler | `drafts/v30/` → `app/upgrades/v30/` in `CosmosContracts/juno` | Compile check |
| 3. Verify capability registration | `upgrade.go` registers `bn254` in `wasm.accept_list` | Unit test |
| 4. Local rehearsal 1 | `make install` + single-node devnet | Blocks advance |
| 5. Local rehearsal 2 | Run `StoreCode` + `Instantiate` for precompile contract | Contract responds |
| 6. Local rehearsal 3 | Run `VerifyProof` via benchmark harness | Gas ~203K |
| 7. Draft `SoftwareUpgradeProposal` | Set `upgrade-height` ≈ 2 weeks out | Validator notice period |
| 8. Submit proposal | `junod tx gov submit-proposal software-upgrade ...` | Proposal ID assigned |
| 9. Vote | Rally validators | ≥ 50 % YES + no 33 % veto |
| 10. Halt + swap binary | At upgrade height, replace `junod` binary with new build | Chain resumes |

### §3.2 — Software upgrade CLI template

```bash
export UPGRADE_NAME="v30-bn254"
export UPGRADE_HEIGHT="<TBD — target block height>"
export DEPOSIT="5000000000ujuno"

junod tx gov submit-proposal software-upgrade \
  "$UPGRADE_NAME" \
  "$UPGRADE_HEIGHT" \
  --title "Juno v30 — Enable BN254 precompile in wasmvm" \
  --description "Upgrades wasmvm to the BN254-enabled release. No state migration. See docs/V30_UPGRADE_HANDLER_DESIGN.md." \
  --deposit "$DEPOSIT" \
  --upgrade-info '{"binaries":{"linux/amd64":"https://github.com/CosmosContracts/juno/releases/download/v30.0.0/junod-v30.0.0-linux-amd64.tar.gz"}}' \
  --from $PROPOSER \
  --chain-id juno-1 \
  --gas auto --gas-adjustment 1.4 \
  --gas-prices 0.075ujuno \
  --yes
```

---

## Phase 4 — Contract Migration Proposal (after software upgrade is live)

**Goal:** Migrate the live `zk-verifier` contract from pure-Wasm (`code_id A`) to precompile (`code_id B`).

The `deploy/migrate-zk-verifier.mjs` script handles this. In `gov` mode it **does not broadcast** the migrate; it writes a ready-to-submit proposal JSON.

### §4.1 — Migration proposal workflow

```bash
# 1. Ensure the precompile wasm is already stored on-chain (permissionless StoreCode)
#    If not stored yet, the script uploads it first:
export MODE=gov
export CHAIN_ID=juno-1
export RPC_URL=https://juno-rpc.polkachu.com
export ZK_VERIFIER_ADDR=juno1<live-address>
# Optional: skip upload if code_id already known
# export SKIP_UPLOAD=true
# export ZK_PRECOMPILE_CODE_ID=<N>

node deploy/migrate-zk-verifier.mjs
# → writes deploy/proposal-migrate-zk-verifier.json

# 2. Submit the proposal
junod tx gov submit-proposal deploy/proposal-migrate-zk-verifier.json \
  --from $PROPOSER \
  --chain-id juno-1 \
  --gas auto --gas-adjustment 1.4 \
  --gas-prices 0.075ujuno \
  --yes
```

### §4.2 — What the migration proposal contains

```json
{
  "messages": [{
    "@type": "/cosmwasm.wasm.v1.MsgMigrateContract",
    "sender": "juno10d07y265gmmuvt4z0w9aw880jnsr700jss8g0a",
    "contract": "juno1<zk-verifier-address>",
    "code_id": "<precompile-code-id>",
    "msg": "e30="
  }],
  "metadata": "ipfs://<proposal-metadata-cid>",
  "deposit": "10000000ujuno",
  "title": "Migrate zk-verifier to the BN254 precompile build",
  "summary": "..."
}
```

*Critical safety:* The contract's `wasmd`-level admin must be the governance module account (`juno10d07y265gmmuvt4z0w9aw880jnsr700jss8g0a` on Juno). If the admin is a multisig or EOA, use `MODE=admin` in the script instead.

### §4.3 — Pre-migration sanity checklist

- [ ] Software upgrade (Phase 3) is **executed** and chain height > upgrade height
- [ ] `junod query wasm contract-state smart $ZK_VERIFIER_ADDR '{"vk_status":{}}'` returns `has_vk: true`
- [ ] Precompile wasm is stored on-chain; `code_id` known
- [ ] `junod query wasm contract $ZK_VERIFIER_ADDR` shows `admin: juno10d07y265gmmuvt4z0w9aw880jnsr700jss8g0a` (gov module)
- [ ] `DRY_RUN=true` passed on script; output inspected
- [ ] `SKIP_UPLOAD=true` used if wasm already stored (saves gas)

### §4.4 — Post-migration verification

```bash
# 1. Confirm migration succeeded
junod query wasm contract $ZK_VERIFIER_ADDR
# → code_id should equal the precompile code_id

# 2. State preservation check
junod query wasm contract-state smart $ZK_VERIFIER_ADDR '{"vk_status":{}}'
# → has_vk: true, vk_size_bytes: 296 (same as before)

# 3. Gas reduction check — send a VerifyProof
#    (use the same proof fixture that cost 370,498 gas before)
#    Expected: ~203,164 gas
```

---

## Cosign Coordination

Proposal #373 used a 7-day cosign window. The same pattern applies here.

| Path | Condition | Action |
|---|---|---|
| Solo | No cosign within 7 days of HackMD publish | Submit as VairagyaNodes solo (valid for signaling) |
| Cosign | Jake Hartnell or another validator cosigns before 7-day mark | Update `GOV_PROP_COPYPASTE_BN254.md` FIELD 1 title to include cosigner name; both wallets co-sign the on-chain tx |

*The signaling proposal itself does not require a cosigner. A cosign only adds social weight.*

---

## Funding Requirements

| Item | Amount | Purpose |
|---|---|---|
| Signaling proposal deposit | 10 JUNO | `MsgSubmitProposal` minimum deposit |
| Software upgrade deposit | 5 000 JUNO | `SoftwareUpgradeProposal` minimum (check current params) |
| Contract migration deposit | 10 JUNO | `MsgSubmitProposal` for migration |
| Proposer wallet buffer | ~200 JUNO | Gas for broadcasts, queries, dry runs |
| **Total recommended** | **~5 300 JUNO** | — |

---

## Risk Register

| Risk | Mitigation | Owner |
|---|---|---|
| Upstream PR stalls | Signaling proposal still valid; can resubmit to next wasmvm major/minor | VairagyaNodes |
| Software upgrade vote fails | Re-submit with longer notice period; no state corrupted | VairagyaNodes |
| Contract admin is not gov module | Use `MODE=admin` in migration script; gov proposal path blocked | VairagyaNodes |
| WSL2 clock jump stalls devnet | `wsl --shutdown` before sleep; verify block height before any deploy | Cascade |
| Forward-port (v3) ownership gap | Confirm with Jake/Juno AI before opening v3 PRs | External gating |

---

*Last updated: 2026-06-07. See `BN254_TRAJECTORY_UPDATE.md` for the technical trajectory and `GOV_PROP_COPYPASTE_BN254.md` for the raw proposal text.*
