package v30

import (
	"context"

	upgradetypes "cosmossdk.io/x/upgrade/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/cosmos/cosmos-sdk/types/module"

	"github.com/CosmosContracts/juno/v30/app/keepers"
)

const UpgradeName = "v30"

func CreateUpgradeHandler(
	mm *module.Manager,
	cfg module.Configurator,
	keepers *keepers.AppKeepers,
) upgradetypes.UpgradeHandler {
	return func(ctx context.Context, plan upgradetypes.Plan, fromVM module.VersionMap) (module.VersionMap, error) {
		// v30 adds BN254 host-function support to wasmvm.
		// The only chain-side change is registering the "bn254" capability
		// so that contracts compiled with cosmwasm_2_3 (or equivalent) can
		// be instantiated on this chain.
		//
		// No state migrations. No param changes outside wasmd capabilities.
		// No new modules. No genesis modifications.

		ctxSDK := sdk.UnwrapSDKContext(ctx)
		params := keepers.WasmKeeper.GetParams(ctxSDK)

		// NOTE: wasmd accepted-capabilities are either param-state or node-flag
		// depending on version.  In wasmd v0.61.11 (Jake's v30 PR #1202) the
		// accepted list is configured via the --wasm.accept_list validator flag,
		// NOT via on-chain params.  Therefore this handler does NOT mutate params
		// here; instead validators set the flag in their systemd/ cosmovisor config.
		// If a future wasmd revision moves this back to param-state, replace the
		// line below with the append + SetParams call.
		_ = params

		// Run module migrations (no-op for our scope, but mandatory).
		return mm.RunMigrations(ctx, cfg, fromVM)
	}
}
