#!/bin/sh
# Initialize a local Juno chain for JunoClaw development.
# This script runs inside the junod container.

set -e

CHAIN_ID="${CHAIN_ID:-junoclaw-local}"
MONIKER="${MONIKER:-junoclaw-local}"
HOME="/root/.juno"

# Check if chain is already initialized
if [ -f "$HOME/config/genesis.json" ]; then
    echo "Chain already initialized, starting..."
    exit 0
fi

echo "Initializing chain: $CHAIN_ID"

# Initialize
junod init "$MONIKER" --chain-id "$CHAIN_ID" --home "$HOME"

# Create validator key
junod keys add validator --keyring-backend test --home "$HOME" --output json > /tmp/validator.json || true

# Add genesis account
VALIDATOR_ADDR=$(junod keys show validator -a --keyring-backend test --home "$HOME")
junod add-genesis-account "$VALIDATOR_ADDR" 1000000000000ujuno --home "$HOME"

# Create gentx
junod gentx validator 100000000ujuno --chain-id "$CHAIN_ID" --keyring-backend test --home "$HOME"

# Collect gentxs
junod collect-gentxs --home "$HOME"

# Configure ports
sed -i 's/laddr = "tcp:\/\/127.0.0.1:26657"/laddr = "tcp:\/\/0.0.0.0:26657"/' "$HOME/config/config.toml"
sed -i 's/laddr = "tcp:\/\/127.0.0.1:1317"/laddr = "tcp:\/\/0.0.0.0:1317"/' "$HOME/config/app.toml"
sed -i 's/address = "0.0.0.0:9090"/address = "0.0.0.0:9090"/' "$HOME/config/app.toml"

# Enable API
sed -i 's/enable = false/enable = true/' "$HOME/config/app.toml"

echo "Chain initialized: $CHAIN_ID"
echo "Validator address: $VALIDATOR_ADDR"
