# Upstream tracking note / ping draft — CosmosContracts/juno PR #1202 (v30)

Topic: v30 upgrade (bumps Cosmos SDK, CometBFT, IBC-Go, wasmd/wasmvm)
Where to post: https://github.com/CosmosContracts/juno/pull/1202  (or ping Jake directly)
Tone: tracking + offer. This is Jake's timeline — no pressure, just staying aligned.

---

Hi @the-frey / Juno core — tracking v30 (#1202) on our side. 👋

No action requested — just flagging that we're keeping our Project Aegis fork patches (hybrid
consensus keys, hybrid accounts, `MsgRotateConsKey`, hybrid IBC 07-tendermint) rebase-ready against
the final v30 base. Two small things that would help us time the rebase:

1. **Rough merge/tag timing** for v30 → main, so we can run our patch-applicability checks against
   the final SDK / CometBFT / IBC-Go / wasmvm versions rather than a moving target.
2. A heads-up if any of these change between now and the tag, since our patches touch them directly:
   - public-key / signature interfaces (Cosmos SDK `cryptotypes`),
   - P2P secret-connection / transport (CometBFT),
   - validator-set / consensus-pubkey handling (CometBFT + SDK `x/staking`),
   - 07-tendermint client (IBC-Go),
   - VM host-function registration (wasmvm).

We'll re-run `wasmvm-fork/patches/check-baseline.sh` and the fork applicability checks once v30 tags,
then rebase the Aegis branches onto the v30 tags. Thanks — happy to help test the upgrade on a devnet
if useful.

---

## Internal checklist (do NOT post — for us)

- [ ] On v30 merge: note the final SDK / CometBFT / IBC-Go / wasmvm versions.
- [ ] Run `wasmvm-fork/patches/check-baseline.sh` against v30 base.
- [ ] Rebase `aegis-phase-cf-hybrid` (cometbft), `aegis-phase-d3-hybrid` (cosmos-sdk),
      `aegis-phase-g-hybrid-client` (ibc-go) onto the v30 tags.
- [ ] Rebuild junod-aegis from the rebased forks; re-record binary sha256.
- [ ] Bump `docs/UPSTREAM_TRACKING.md` `Last updated` + add a dated entry.
