#!/bin/bash
#
# Stop the old soak test, pull latest, rebuild with 4 layers, restart.
#
# Run on the VM:
#   chmod +x scripts/soak-restart-4layer.sh
#   ./scripts/soak-restart-4layer.sh
#
# Optional env vars:
#   MOULTBOOK_ADDR   — moultbook-v0 contract address (enables layer 4 live)
#   SETTLER_ADDR     — coordination-settler contract address
#   RELAYER_KEY      — wallet mnemonic for relayer
#   MOULTBOOK_TOPIC  — topic namespace (default: soak-test)
#   SOAK_DAYS        — duration in days (default: 7)
#   SOAK_INTERVAL    — seconds between cycles (default: 300)

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"

echo "=== Stopping old soak test ==="

# Kill any running soak-node processes
pkill -f "soak-node" 2>/dev/null && echo "  killed soak-node processes" || echo "  no soak-node running"

# Kill any running consensus-test / gate-test
pkill -f "consensus-test" 2>/dev/null && echo "  killed consensus-test" || echo "  no consensus-test running"
pkill -f "gate-test" 2>/dev/null && echo "  killed gate-test" || echo "  no gate-test running"

# Kill any running relayer
pkill -f "junoclaw-relayer" 2>/dev/null && echo "  killed relayer" || echo "  no relayer running"

# Kill any running soak-test.sh
pkill -f "soak-test.sh" 2>/dev/null && echo "  killed soak-test.sh" || echo "  no soak-test.sh running"

# Kill the node relay script if running
pkill -f "relay-batch-testnet" 2>/dev/null && echo "  killed relay script" || echo "  no relay script running"

sleep 2
echo "  all old processes stopped"

echo ""
echo "=== Pulling latest code ==="
git pull origin main

echo ""
echo "=== Building all 4 layers ==="
echo "  Layer 1-2: junoclaw-test-mesh (J-Lens gate + P2P consensus)..."
cargo build --release --features p2p -p junoclaw-test-mesh 2>&1 | tail -5

echo "  Layer 3-4: junoclaw-relayer (settlement + moultbook)..."
cargo build --release -p junoclaw-relayer 2>&1 | tail -5

echo ""
echo "  Binaries built:"
ls -la target/release/soak-node target/release/consensus-test target/release/gate-test target/release/junoclaw-relayer

echo ""
echo "=== Starting 4-layer soak test ==="

# Export env vars for layer 4 (if provided)
export SOAK_DAYS="${SOAK_DAYS:-7}"
export SOAK_INTERVAL="${SOAK_INTERVAL:-300}"
export MOULTBOOK_ADDR="${MOULTBOOK_ADDR:-}"
export MOULTBOOK_TOPIC="${MOULTBOOK_TOPIC:-soak-test}"
export SETTLER_ADDR="${SETTLER_ADDR:-}"
export RELAYER_KEY="${RELAYER_KEY:-${JUNO_MNEMONIC:-}}"

if [ -n "$MOULTBOOK_ADDR" ] && [ -n "$SETTLER_ADDR" ] && [ -n "$RELAYER_KEY" ]; then
    echo "  Layer 4: ENABLED (moultbook=$MOULTBOOK_ADDR, topic=$MOULTBOOK_TOPIC)"
    echo "  Layer 3: settler=$SETTLER_ADDR"
else
    echo "  Layer 4: moult tests will run each cycle (no live moultbook contract configured)"
    echo "  To enable live layer 4, set MOULTBOOK_ADDR + SETTLER_ADDR + RELAYER_KEY"
fi

echo "  Duration: $SOAK_DAYS days"
echo "  Cycle interval: ${SOAK_INTERVAL}s"
echo ""

# Start the soak test in the background
LOG_DIR="${LOG_DIR:-./soak-logs}"
mkdir -p "$LOG_DIR"

nohup bash scripts/soak-test.sh > "$LOG_DIR/soak-orchestrator.log" 2>&1 &
SOAK_PID=$!
echo "  soak-test.sh started (PID $SOAK_PID)"
echo "  Logs: $LOG_DIR/"
echo "  Status: $LOG_DIR/soak-status.json"
echo ""
echo "=== 4-layer soak test running ==="
echo ""
echo "  Monitor:"
echo "    tail -f $LOG_DIR/soak-main.log"
echo "    cat $LOG_DIR/soak-status.json | python3 -m json.tool"
echo ""
echo "  Stop:"
echo "    kill $SOAK_PID"
echo "    pkill -f 'soak-node|consensus-test|gate-test|junoclaw-relayer'"
