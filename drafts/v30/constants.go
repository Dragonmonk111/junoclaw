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
