# v30 Upgrade Handler Draft

This directory contains the draft Go files for the Juno v30 chain upgrade
handler, as specified in [`docs/V30_UPGRADE_HANDLER_DESIGN.md`](../../docs/V30_UPGRADE_HANDLER_DESIGN.md).

## Files

| File | Purpose |
|------|---------|
| `constants.go` | `Upgrade` struct registration (name, handler, empty store upgrades) |
| `upgrade.go` | `CreateUpgradeHandler` — registers `bn254` capability, runs `mm.RunMigrations` |
| `app.go.snippet` | Paste-into-`app.go` snippet for handler registration + store loader |

## Scope

The handler does **exactly two things**:

1. Registers the `bn254` wasmd capability so contracts compiled with the
   BN254 precompile feature can be instantiated.
2. Runs module migrations (`mm.RunMigrations`) — a no-op for v30 since no
   module versions change, but mandatory boilerplate.

Explicitly **out of scope**: Cosmos SDK bumps, IBC bumps, new modules, param
changes outside wasmd capabilities, genesis modifications, state migrations,
Tendermint/CometBFT bumps.

## Integration

These files are a **draft** intended to be copied into the Juno chain repo
(`CosmosContracts/juno`) at `app/upgrades/v30/` once the upstream wasmvm
release containing BN254 host functions is available.

Before finalising:
- ✅ Verified: wasmd v0.61.11 (Jake's v30 PR #1202) uses `--wasm.accept_list`
  node flag for capabilities, NOT param-state. `upgrade.go` documented accordingly.
- Update `go.mod` to the wasmvm version that includes the merged BN254 PR.
- Run `make install` and the 3 local rehearsals documented in the design doc.
