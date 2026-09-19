#!/usr/bin/env bash
# Hermes IBC Relayer Setup — uni-7 ↔ osmo-test-5
#
# Prerequisites:
#   1. Install Hermes:  cargo install hermes --version latest
#   2. Fund wallets:
#      - uni-7:    The Builder (juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z) — already funded
#      - osmo-test-5: Get tokens from faucet at https://faucet.osmotest5.osmosis.zone/
#        Send to a new osmo1... address you control
#
# This script:
#   1. Adds keys to Hermes keystore for both chains
#   2. Creates an IBC connection between uni-7 and osmo-test-5
#   3. Creates a transfer channel (port "transfer" ↔ "transfer")
#   4. Prints the channel ID for use in IBC transfers
#
# Usage:
#   export HERMES_HOME="$HOME/.hermes"
#   bash hermes-setup.sh

set -euo pipefail

CONFIG_FILE="$(dirname "$0")/config.toml"

echo "=== Hermes IBC Setup: uni-7 ↔ osmo-test-5 ==="
echo ""

# ── Step 1: Add keys ─────────────────────────────────────────────────────────
# You need the mnemonic for both chains. The Parliament wallet works for uni-7.
# For Osmosis testnet, create a new wallet and fund it from the faucet.

echo "1. Adding keys to Hermes keystore..."
echo "   uni-7 key: junoclaw-builder"
echo "   osmo-test-5 key: junoclaw-builder"
echo ""
echo "   Run these commands manually (they require interactive mnemonic input):"
echo ""
echo "   # Add uni-7 key (use Parliament 'The Builder' mnemonic)"
echo "   hermes keys add --chain uni-7 --mnemonic-file <(echo 'YOUR_PARLIAMENT_MNEMONIC')"
echo ""
echo "   # Add osmo-test-5 key (create new wallet, fund from faucet)"
echo "   hermes keys add --chain osmo-test-5 --mnemonic-file <(echo 'YOUR_OSMO_MNEMONIC')"
echo ""
echo "   # Or restore from an existing keyfile:"
echo "   # hermes keys add --chain uni-7 --key-file ~/keys/uni7.json"
echo "   # hermes keys add --chain osmo-test-5 --key-file ~/keys/osmo5.json"
echo ""

# ── Step 2: Verify connections ───────────────────────────────────────────────
echo "2. Verifying chain connectivity..."
hermes --config "$CONFIG_FILE" query chain uni-7 status
hermes --config "$CONFIG_FILE" query chain osmo-test-5 status
echo ""

# ── Step 3: Create connection ───────────────────────────────────────────────
echo "3. Creating IBC connection (uni-7 → osmo-test-5)..."
echo "   hermes --config \"$CONFIG_FILE\" create connection uni-7"
echo ""
echo "   This will:"
echo "   - Create a client on uni-7 tracking osmo-test-5"
echo "   - Create a client on osmo-test-5 tracking uni-7"
echo "   - Open a connection between them"
echo ""
echo "   Save the connection ID from the output."
echo ""

# ── Step 4: Create channel ──────────────────────────────────────────────────
echo "4. Creating transfer channel..."
echo "   hermes --config \"$CONFIG_FILE\" create channel uni-7 --port-a transfer --port-b transfer"
echo ""
echo "   This opens a channel on port 'transfer' (our cw-ics20-transfer contract)"
echo "   to port 'transfer' (Osmosis native ICS-20 module)."
echo ""
echo "   Save the channel ID (format: channel-N)."
echo ""

# ── Step 5: Start relaying ──────────────────────────────────────────────────
echo "5. Start relaying..."
echo "   hermes --config \"$CONFIG_FILE\" start"
echo ""
echo "   Hermes will now relay packets between uni-7 and osmo-test-5."
echo "   Any IBC transfer sent on either chain will be relayed to the other."
echo ""

# ── Step 6: Verify ───────────────────────────────────────────────────────────
echo "6. Verify channel is open..."
echo "   hermes --config \"$CONFIG_FILE\" query channels --chain uni-7"
echo ""
echo "=== Setup Complete ==="
echo ""
echo "IBC Transfer Example (uni-7 → osmo-test-5):"
echo "  junod tx ibc-transfer transfer transfer <COUNTERPARTY_PORT/channel> <osmo_recipient> 100ujunox \\"
echo "    --from the-builder --chain-id uni-7 --node https://junotestnet-rpc.kleomedes.network"
echo ""
echo "The cw-ics20-transfer contract at juno1e2zsnjl7ft37y45jcztucqlmez4m4ylge202dlmaln3aw99lu2ssdeust2"
echo "will handle the ICS-20 packet lifecycle (send, ack, timeout)."
