#!/bin/bash
# ─── Full 6-Layer Local Soak — All On-Chain Layers Enabled ──────────────────
#
# Runs the 7-day soak test with all 6 layers enabled:
#   Layer 1: P2P BFT consensus (always on)
#   Layer 2: J-Lens gate (always on)
#   Layer 3: Coordination-settler (on-chain via relayer)
#   Layer 4: Moultbook posting (on-chain via relayer)
#   Layer 5: Executor bridge — task-ledger + agent-registry
#   Layer 6: Truth market — verdict submission + epoch finalization
#
# Prerequisites:
#   - Cargo build --release --features p2p -p junoclaw-test-mesh
#   - Cargo build --release -p junoclaw-relayer
#   - A funded uni-7 testnet wallet for the relayer
#   - Set RELAYER_KEY below (or export it before running)
#
# Usage:
#   bash scripts/soak-full-6layer.sh
#
# Or with custom key:
#   RELAYER_KEY="your mnemonic here" bash scripts/soak-full-6layer.sh
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# ─── Contract Addresses (uni-7 testnet) ──────────────────────────────────────
export MOULTBOOK_ADDR="juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4"
export MOULTBOOK_TOPIC="soak-test-full-6layer"
export SETTLER_ADDR="juno16gp6mm7ehhrx0ls4s77yk668cclq5gyc09x7k45f28apwmsqkg0qhmmnc8"
export TASK_LEDGER_ADDR="juno1cp88zj8vn5mdszjee8cu753eczjg9krtsmz0v65apzhp89y392mqwnehfm"
export AGENT_REGISTRY_ADDR="juno15683x0sa06yr4ejuwenxszclkvpjekxmldlxe8qsltfkhm3qpm5sy0vuep"
export TRUTH_MARKET_ADDR="juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p"
export EXECUTE="1"

# ─── Relayer Key (REQUIRED for Layers 3+4) ───────────────────────────────────
# Export a disposable testnet wallet mnemonic here, or set it as an env var
# before running this script.
# RELAYER_KEY="${RELAYER_KEY:-your testnet mnemonic here}"
# export RELAYER_KEY

if [ -z "${RELAYER_KEY:-}" ]; then
    echo "⚠️  RELAYER_KEY is not set."
    echo "    Layer 4 (Moultbook posting) and Layer 3 (settler) will NOT start."
    echo "    Layers 1, 2, 5, 6 cargo tests will still run."
    echo "    To enable all 6 layers on-chain, export RELAYER_KEY with a funded uni-7 wallet mnemonic."
    echo ""
fi

# ─── Soak Parameters ─────────────────────────────────────────────────────────
export SOAK_DAYS="${SOAK_DAYS:-7}"
export SOAK_INTERVAL="${SOAK_INTERVAL:-300}"

echo "═══ Full 6-Layer Soak Test ═══"
echo "Duration:   ${SOAK_DAYS} days"
echo "Interval:   ${SOAK_INTERVAL}s"
echo "Moultbook:  ${MOULTBOOK_ADDR}"
echo "Settler:    ${SETTLER_ADDR}"
echo "Task Ledger: ${TASK_LEDGER_ADDR}"
echo "Agent Reg:  ${AGENT_REGISTRY_ADDR}"
echo "Truth Market: ${TRUTH_MARKET_ADDR}"
echo "Execute:    ${EXECUTE}"
echo "Relayer:    $([ -n "${RELAYER_KEY:-}" ] && echo 'enabled' || echo 'DISABLED — set RELAYER_KEY')"
echo ""

# ─── Launch ──────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec bash "${SCRIPT_DIR}/soak-test.sh"
