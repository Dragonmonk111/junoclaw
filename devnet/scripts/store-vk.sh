#!/usr/bin/env bash
# Store a Groth16 VK on the zk-verifier contract
set -euo pipefail

ZK_VERIFIER="${ZK_VERIFIER:-juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p}"
ADMIN="${ADMIN:-admin}"
NODE="${NODE:-http://localhost:26657}"
GAS_PRICES="${GAS_PRICES:-0.075ujuno}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Read VK from proof artifacts
VK_FILE="${PROJECT_ROOT}/circuits/moultbook-membership/devnet/proof-artifacts/vk.b64"
if [ ! -f "$VK_FILE" ]; then
    echo "error: VK file not found at $VK_FILE" >&2
    echo "Run: cargo run --bin gen-proof" >&2
    exit 1
fi

VK_B64=$(cat "$VK_FILE" | tr -d '\n')

echo "[store-vk] Storing VK on zk-verifier: $ZK_VERIFIER"
echo "[store-vk] VK size: $(echo -n "$VK_B64" | wc -c) bytes (base64)"

# Build execute message JSON
MSG=$(printf '{"store_vk":{"vk_base64":"%s"}}' "$VK_B64")

# Write to temp file in container (-i attaches stdin)
printf '%s' "$MSG" | docker exec -i junoclaw-bn254-devnet sh -c "cat > /tmp/store_vk_msg.json"

# Sanity-check the file landed (run wc inside the container)
WROTE=$(docker exec junoclaw-bn254-devnet sh -c "wc -c < /tmp/store_vk_msg.json")
echo "[store-vk] wrote $WROTE bytes to container /tmp/store_vk_msg.json"

# Submit transaction — junod needs the JSON inline, so read the file
# inside the container via command substitution.
TX_OUT=$(docker exec junoclaw-bn254-devnet sh -c "junod tx wasm execute '$ZK_VERIFIER' \"\$(cat /tmp/store_vk_msg.json)\" \
    --from '$ADMIN' \
    --chain-id junoclaw-bn254-1 \
    --keyring-backend test \
    --gas auto \
    --gas-adjustment 1.5 \
    --gas-prices '$GAS_PRICES' \
    --broadcast-mode sync \
    --yes \
    --output json \
    --node '$NODE'" 2>&1)

echo "$TX_OUT"

TXHASH=$(echo "$TX_OUT" | grep -o '"txhash":"[^"]*"' | cut -d'"' -f4)
if [ -z "$TXHASH" ]; then
    echo "error: no txhash in response" >&2
    exit 1
fi

echo "[store-vk] txhash=$TXHASH — polling for confirmation..."
for i in $(seq 1 15); do
    sleep 2
    TX_RESULT=$(docker exec junoclaw-bn254-devnet junod query tx "$TXHASH" --node "$NODE" --output json 2>/dev/null) && break
    echo "  attempt $i..."
done

if [ -z "$TX_RESULT" ]; then
    echo "error: tx not indexed after 30s" >&2
    exit 1
fi

echo "[store-vk] VK stored successfully"
