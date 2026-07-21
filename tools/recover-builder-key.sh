#!/bin/bash
set -euo pipefail

ENV_FILE="/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/.env"
TMP_MNEMONIC="/tmp/juno_builder_mnemonic.txt"

# Load .env and ensure JUNO_MNEMONIC is exported
# Read mnemonic as raw value after the first '='; do not source the .env file
# because the unquoted spaces in the mnemonic would be interpreted as commands.
JUNO_MNEMONIC="$(grep '^JUNO_MNEMONIC=' "$ENV_FILE" | cut -d= -f2- | tr -d '\r\n' | sed 's/^"//;s/"$//;s/^'"'"'//;s/'"'"'$//')"

umask 077
printf '%s' "$JUNO_MNEMONIC" > "$TMP_MNEMONIC"

/usr/local/bin/junod keys add builder --recover --source "$TMP_MNEMONIC" --keyring-backend test

rm -f "$TMP_MNEMONIC"
