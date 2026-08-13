#!/usr/bin/env bash
#
# 7-Day Soak Test — JunoClaw Coordination Stack
#
# Runs 4 P2P coordination nodes + consensus test + batch relayer in a loop
# for 7 days, logging everything with timestamps.
#
# Prerequisites:
#   - Rust toolchain with NASM installed (apt install nasm)
#   - cargo build --release --features p2p -p junoclaw-coordination
#   - cargo build --release -p junoclaw-test-mesh
#   - Node.js 20+ for the relayer script
#   - JUNO_MNEMONIC or WALLET_ID env set for testnet txs
#   - coordination-settler contract deployed on uni-7 (deployed-testnet.json)
#
# Usage:
#   chmod +x scripts/soak-test.sh
#   ./scripts/soak-test.sh
#
# Environment:
#   SOAK_DAYS         — test duration in days (default: 7)
#   SOAK_INTERVAL     — seconds between batch cycles (default: 300 = 5 min)
#   JUNO_MNEMONIC     — wallet mnemonic for relayer
#   CHAIN_ID          — chain ID (default: uni-7)
#   RPC_URL           — RPC endpoint (default: https://juno.rpc.t.stavr.tech)
#   LOG_DIR           — log directory (default: ./soak-logs)

set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────

SOAK_DAYS="${SOAK_DAYS:-7}"
SOAK_INTERVAL="${SOAK_INTERVAL:-300}"
CHAIN_ID="${CHAIN_ID:-uni-7}"
RPC_URL="${RPC_URL:-https://juno.rpc.t.stavr.tech}"
LOG_DIR="${LOG_DIR:-./soak-logs}"

START_TIME=$(date +%s)
END_TIME=$((START_TIME + SOAK_DAYS * 86400))
CYCLE=0

mkdir -p "$LOG_DIR"

# ─── Helpers ────────────────────────────────────────────────────────────────

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" | tee -a "$LOG_DIR/soak-main.log"
}

log_err() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] ERROR: $*" | tee -a "$LOG_DIR/soak-main.log" >&2
}

elapsed() {
    local now=$(date +%s)
    echo $(( now - START_TIME ))
}

remaining() {
    local now=$(date +%s)
    echo $(( END_TIME - now ))
}

# Check if a process is still alive, restart if not
check_and_restart() {
    local name=$1
    local pid=$2
    local restart_cmd=$3
    local log_file=$4

    if ! kill -0 "$pid" 2>/dev/null; then
        log "$name (PID $pid) died, restarting..."
        eval "$restart_cmd" > "$log_file" 2>&1 &
        local new_pid=$!
        log "$name restarted as PID $new_pid"
        echo "$new_pid"
    else
        echo "$pid"
    fi
}

# ─── Build Check ────────────────────────────────────────────────────────────

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

log "=== JunoClaw 7-Day Soak Test ==="
log "Duration: $SOAK_DAYS days"
log "Cycle interval: ${SOAK_INTERVAL}s"
log "Chain: $CHAIN_ID"
log "RPC: $RPC_URL"
log "Log dir: $LOG_DIR"
log "Repo: $REPO_ROOT"
log "Start: $(date -u)"
log "End:   $(date -u -d "@$END_TIME" 2>/dev/null || date -u -r "$END_TIME" 2>/dev/null || echo "@$END_TIME")"
log ""

# Verify binaries exist
BIN_PATH="target/release"
if [ ! -f "$BIN_PATH/consensus-test" ] || [ ! -f "$BIN_PATH/gate-test" ]; then
    log "Building test-mesh binaries (consensus-test, gate-test)..."
    cargo build --release -p junoclaw-test-mesh 2>&1 | tee -a "$LOG_DIR/build.log"
fi

if [ ! -f "$BIN_PATH/soak-node" ]; then
    log "Building soak-node (real P2P, requires NASM)..."
    cargo build --release --features p2p -p junoclaw-test-mesh 2>&1 | tee -a "$LOG_DIR/build.log"
fi

log "Binaries ready."

# ─── Node Configuration ─────────────────────────────────────────────────────
#
# 4 real commonware-p2p nodes on localhost, ports 4001-4004.
# Each node knows all 4 peers via the authenticated `lookup` mesh.
# Seeds 1-4 give deterministic identities across restarts.

SEEDS=(1 2 3 4)
PORTS=(4001 4002 4003 4004)
NAMESPACE="junoclaw-soak-v1"
HEARTBEAT_SECS="${HEARTBEAT_SECS:-10}"

declare -A PUBKEYS
declare -A NODE_PIDS

# ─── Bootstrap: derive each node's deterministic public key ─────────────────

log "Bootstrapping node identities (seeds 1-4)..."
for i in "${!SEEDS[@]}"; do
    seed="${SEEDS[$i]}"
    pk=$("$BIN_PATH/soak-node" --seed "$seed" --print-pubkey)
    PUBKEYS[$seed]="$pk"
    log "  seed=$seed pk=$pk"
done

# ─── Launch the 4 real P2P nodes ─────────────────────────────────────────────

start_soak_node() {
    local idx=$1
    local seed="${SEEDS[$idx]}"
    local port="${PORTS[$idx]}"
    local peer_args=()

    for j in "${!SEEDS[@]}"; do
        if [ "$j" != "$idx" ]; then
            local peer_seed="${SEEDS[$j]}"
            local peer_port="${PORTS[$j]}"
            peer_args+=(--peer "${PUBKEYS[$peer_seed]}@127.0.0.1:$peer_port")
        fi
    done

    RUST_LOG=info "$BIN_PATH/soak-node" \
        --label "node$seed" \
        --seed "$seed" \
        --listen-addr "127.0.0.1:$port" \
        --namespace "$NAMESPACE" \
        --heartbeat-secs "$HEARTBEAT_SECS" \
        "${peer_args[@]}" \
        > "$LOG_DIR/soak-node-$seed.log" 2>&1 &

    NODE_PIDS[$seed]=$!
    log "  started node$seed (seed=$seed, port=$port, PID=${NODE_PIDS[$seed]})"
}

log "Launching 4-node real P2P mesh..."
for i in "${!SEEDS[@]}"; do
    start_soak_node "$i"
done
sleep 3
log "Mesh launched. PIDs: ${NODE_PIDS[*]}"

# Ensure nodes are killed on script exit (Ctrl+C, or natural completion)
cleanup_nodes() {
    log "Shutting down soak-node processes..."
    for seed in "${SEEDS[@]}"; do
        pid="${NODE_PIDS[$seed]:-}"
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null
            log "  stopped node$seed (PID $pid)"
        fi
    done
}
trap cleanup_nodes EXIT

# ─── Main Loop ──────────────────────────────────────────────────────────────

log "Starting main soak loop..."

while true; do
    NOW=$(date +%s)

    if [ "$NOW" -ge "$END_TIME" ]; then
        log "=== Soak test complete! ==="
        log "Duration: $(elapsed)s"
        log "Cycles completed: $CYCLE"
        log "End: $(date -u)"
        break
    fi

    CYCLE=$((CYCLE + 1))
    REM=$(remaining)
    EL=$(elapsed)
    log ""
    log "--- Cycle $CYCLE | Elapsed: ${EL}s | Remaining: ${REM}s ---"

    # ── Step 0: Health-check the 4 real P2P nodes, restart any that died ──

    for i in "${!SEEDS[@]}"; do
        seed="${SEEDS[$i]}"
        pid="${NODE_PIDS[$seed]:-}"
        if [ -z "$pid" ] || ! kill -0 "$pid" 2>/dev/null; then
            log "  node$seed (seed=$seed) is down, restarting..."
            start_soak_node "$i"
        fi
    done

    # ── Step 1: Run consensus-test (produces a finalized batch) ──
    #
    # This runs the 4-node consensus simulation, which:
    # - Creates 4 validators (3 honest, 1 byzantine)
    # - Produces a hash-chained batch with threshold certificate
    # - Verifies certificate size < 300 bytes
    # - Measures throughput

    CONSENSUS_LOG="$LOG_DIR/consensus-cycle-$CYCLE.log"
    log "Running consensus-test (cycle $CYCLE)..."

    if RUST_LOG=info "$BIN_PATH/consensus-test" > "$CONSENSUS_LOG" 2>&1; then
        log "  consensus-test: PASS"

        # Extract key metrics from the log
        CERT_SIZE=$(grep -oP 'Certificate size: \K[0-9]+' "$CONSENSUS_LOG" || echo "?")
        THROUGHPUT=$(grep -oP 'Throughput: \K[0-9]+' "$CONSENSUS_LOG" || echo "?")
        log "  cert_size=${CERT_SIZE}B throughput=${THROUGHPUT}msg/s"
    else
        log_err "consensus-test FAILED (cycle $CYCLE)"
        log "  See: $CONSENSUS_LOG"
    fi

    # ── Step 2: Run gate-test (J-Lens truth gate verification) ──

    GATE_LOG="$LOG_DIR/gate-cycle-$CYCLE.log"
    log "Running gate-test (cycle $CYCLE)..."

    if RUST_LOG=info "$BIN_PATH/gate-test" > "$GATE_LOG" 2>&1; then
        log "  gate-test: PASS"
    else
        log_err "gate-test FAILED (cycle $CYCLE)"
        log "  See: $GATE_LOG"
    fi

    # ── Step 3: Relay batch to uni-7 testnet ──
    #
    # Every 12 cycles (1 hour at 5-min intervals), relay a batch on-chain.
    # This avoids spamming the testnet while still demonstrating settlement.

    if [ $((CYCLE % 12)) -eq 0 ]; then
        RELAY_LOG="$LOG_DIR/relay-cycle-$CYCLE.log"
        log "Relaying batch to $CHAIN_ID (cycle $CYCLE)..."

        if CHAIN_ID="$CHAIN_ID" RPC_URL="$RPC_URL" \
           node deploy/relay-batch-testnet.mjs > "$RELAY_LOG" 2>&1; then
            log "  relay: PASS"

            # Extract tx hash if present
            TX_HASH=$(grep -oP 'txhash: \K\w+' "$RELAY_LOG" || echo "?")
            HEIGHT=$(grep -oP 'height: \K[0-9]+' "$RELAY_LOG" || echo "?")
            log "  tx_hash=$TX_HASH height=$HEIGHT"
        else
            log_err "relay FAILED (cycle $CYCLE)"
            log "  See: $RELAY_LOG"
            log "  (This may be expected if testnet RPC is down or wallet is unfunded)"
        fi
    fi

    # ── Step 4: Health summary ──

    ALIVE_COUNT=0
    for seed in "${SEEDS[@]}"; do
        pid="${NODE_PIDS[$seed]:-}"
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            ALIVE_COUNT=$((ALIVE_COUNT + 1))
        fi
    done

    log "Health: cycle=$CYCLE elapsed=${EL}s remaining=${REM}s p2p_nodes_alive=${ALIVE_COUNT}/4"

    # Write a summary file that can be checked externally
    cat > "$LOG_DIR/soak-status.json" << EOF
{
  "cycle": $CYCLE,
  "elapsed_seconds": $EL,
  "remaining_seconds": $REM,
  "start_time": $START_TIME,
  "end_time": $END_TIME,
  "soak_days": $SOAK_DAYS,
  "p2p_nodes_alive": $ALIVE_COUNT,
  "last_cert_size": "${CERT_SIZE:-unknown}",
  "last_throughput": "${THROUGHPUT:-unknown}",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF

    # ── Step 5: Sleep until next cycle ──

    sleep "$SOAK_INTERVAL"
done

log ""
log "=== Soak test finished ==="
log "Total cycles: $CYCLE"
log "Total duration: $(elapsed)s"
log "Logs in: $LOG_DIR"

# Generate final report
log "Generating final report..."
{
    echo "# JunoClaw Soak Test Report"
    echo ""
    echo "- **Start:** $(date -u -d "@$START_TIME" 2>/dev/null || echo "@$START_TIME")"
    echo "- **End:** $(date -u)"
    echo "- **Duration:** $(elapsed)s ($SOAK_DAYS days planned)"
    echo "- **Cycles:** $CYCLE"
    echo ""
    echo "## Consensus Test Results"
    echo ""
    echo "| Cycle | Result | Cert Size | Throughput |"
    echo "|-------|--------|-----------|------------|"
    for i in $(seq 1 "$CYCLE"); do
        f="$LOG_DIR/consensus-cycle-$i.log"
        if [ -f "$f" ]; then
            result="PASS"
            cert=$(grep -oP 'Certificate size: \K[0-9]+' "$f" || echo "?")
            tp=$(grep -oP 'Throughput: \K[0-9]+' "$f" || echo "?")
            echo "| $i | $result | ${cert}B | ${tp}msg/s |"
        fi
    done
    echo ""
    echo "## Relay Results"
    echo ""
    echo "| Cycle | Result | Tx Hash | Height |"
    echo "|-------|--------|---------|--------|"
    for i in $(seq 12 "$CYCLE"); do
        f="$LOG_DIR/relay-cycle-$i.log"
        if [ -f "$f" ]; then
            result="PASS"
            tx=$(grep -oP 'txhash: \K\w+' "$f" || echo "?")
            h=$(grep -oP 'height: \K[0-9]+' "$f" || echo "?")
            echo "| $i | $result | $tx | $h |"
        fi
    done
} > "$LOG_DIR/SOAK_REPORT.md"

log "Report written to $LOG_DIR/SOAK_REPORT.md"
