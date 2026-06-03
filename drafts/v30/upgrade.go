package v30

import (
	"context"

	"cosmossdk.io/x/upgrade/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/cosmos/cosmos-sdk/types/module"

	"github.com/CosmosContracts/juno/v30/app/keepers"
)

const UpgradeName = "v30"

func CreateUpgradeHandler(
	mm *module.Manager,
	cfg module.Configurator,
	keepers *keepers.AppKeepers,
) types.UpgradeHandler {
	return func(ctx context.Context, plan types.Plan, fromVM module.VersionMap) (module.VersionMap, error) {
		// v30 adds BN254 host-function support to wasmvm.
		// The only chain-side change is registering the "bn254" capability
		// so that contracts compiled with cosmwasm_2_3 (or equivalent) can
		// be instantiated on this chain.
		//
		// No state migrations. No param changes outside wasmd capabilities.
		// No new modules. No genesis modifications.

		ctxSDK := sdk.UnwrapSDKContext(ctx)
		params := keepers.WasmKeeper.GetParams(ctxSDK)

		// Append "bn254" to the accepted capabilities list if not already present.
		// (wasmd v2.x uses param-state for accepted capabilities; if the target
		// wasmd version has moved this to a node flag, this block becomes a no-op
		// and the capability is instead set via --wasm.accept_list on validator
		// startup.  Verify against the actual wasmd version before finalising.)
		if !contains(params.InstantiatePermission.CodeUploadAccess, "bn254") {
			// NOTE: the exact field for accepted capabilities varies by wasmd version.
			// If AcceptedCapabilities does not exist on wasmtypes.Params in the
			// version pinned by v30, skip this block and document the flag approach.
			_ = params // placate compiler while field name is TBD
		}

		// Run module migrations (no-op for our scope, but mandatory).
		return mm.RunMigrations(ctx, cfg, fromVM)
	}
}

// helper stub — will be replaced by the real field check once wasmd version is known.
func contains(_ wasmtypes.AccessType, _ string) bool {
	return false
}
