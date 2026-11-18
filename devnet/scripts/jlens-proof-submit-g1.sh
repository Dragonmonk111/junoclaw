#!/usr/bin/env bash
# J-Lens prover service primitive for LIVE junoclaw-g1.
#
# Same flow as jlens-proof-submit.sh but submits through
# junoclaw-chain's tx-sender over the public gRPC gateway
# (https://api.junoclaw.xyz:443) instead of dockerized junod.
#
# Env:
#   TX_SENDER         path to tx-sender binary (required)
#   PK                ceremony proving key (default: ~/Downloads/zk-prod-r4/jlens-pk-prod.bin)
#   GRPC              gateway URI (default: https://api.junoclaw.xyz:443)
#   COURT_ADDR        thought-court contract address (required)
#   BOND              operator bond (default: 1000000ujclaw)
#   CHAIN_ID          default junoclaw-g1
#   TREE_HEIGHT       MiMC tree height (default: 4)
#   ATTESTATION_ID    optional; defaults to meta.suggested_attestation_id
#
# Usage: jlens-proof-submit-g1.sh <disclosure.json> [--artifacts <dir>]
set -euo pipefail

TXS="${TX_SENDER:?TX_SENDER (path to tx-sender binary) required}"
PK="${PK:-$HOME/Downloads/zk-prod-r4/jlens-pk-prod.bin}"
GRPC="${GRPC:-https://api.junoclaw.xyz:443}"
COURT="${COURT_ADDR:?COURT_ADDR (thought-court address) required}"
BOND="${BOND:-1000000ujclaw}"
HEIGHT="${TREE_HEIGHT:-4}"
export TX_SENDER_CHAIN_ID="${CHAIN_ID:-junoclaw-g1}"

DISCLOSURE="${1:?usage: jlens-proof-submit-g1.sh <disclosure.json> [--artifacts <dir>]}"
shift
ARTIFACTS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --artifacts) ARTIFACTS="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done
ARTIFACTS="${ARTIFACTS:-$(mktemp -d)/artifacts}"

BIN="$(dirname "$0")/../../circuits/target/release/prove-disclosure"
[ -x "$BIN" ] || BIN="${BIN}.exe"
"$BIN" --disclosure "$DISCLOSURE" --height "$HEIGHT" --pk "$PK" --out "$ARTIFACTS"

META="$ARTIFACTS/meta.json"
ROOT=$(grep -o '"activation_root": *"[^"]*"' "$META" | cut -d'"' -f4)
BANK=$(grep -o '"bank_commitment": *"[^"]*"' "$META" | cut -d'"' -f4)
SCORE=$(grep -o '"score": *[0-9]*' "$META" | grep -o '[0-9]*')
INTEGRITY=$(grep -o '"integrity": *[0-9]*' "$META" | grep -o '[0-9]*')
DETECTION=$(grep -o '"detection": *[a-z]*' "$META" | awk '{print $2}')
AID="${ATTESTATION_ID:-$(grep -o '"suggested_attestation_id": *"[^"]*"' "$META" | cut -d'"' -f4)}"

AMOUNT="${BOND%ujclaw}"

tx_execute() {
  "$TXS" execute --grpc "$GRPC" --contract "$COURT" --msg "$1" ${2:+--amount "$2" --denom ujclaw} 2>&1
}

echo "== anchoring $AID on junoclaw-g1 (root ${ROOT:0:16}…, score $SCORE, bond $BOND)"
tx_execute \
  '{"anchor_attestation":{"id":"'"$AID"'","activation_root":"'"$ROOT"'","claims":{"score":'"$SCORE"',"integrity":'"$INTEGRITY"',"detection":'"$DETECTION"',"bank_commitment":"'"$BANK"'"}}}' \
  "$AMOUNT" | tail -5
sleep 4

echo "== submitting resolve_with_proof"
tx_execute \
  '{"resolve_with_proof":{"attestation_id":"'"$AID"'","proof_base64":"'"$(tr -d '\r\n' < "$ARTIFACTS/jlens-proof.b64")"'","public_inputs_base64":"'"$(tr -d '\r\n' < "$ARTIFACTS/jlens-inputs.b64")"'"}}' | tail -5
sleep 4

STATUS=$("$TXS" query --grpc "$GRPC" --contract "$COURT" \
  --msg '{"get_attestation":{"attestation_id":"'"$AID"'"}}' | grep -o '"status":"[^"]*"' | cut -d'"' -f4)
echo "attestation $AID status=$STATUS"
[ "$STATUS" = "finalized" ] || { echo "FAIL: expected finalized" >&2; exit 1; }
echo "PASS: disclosure proven + resolved on junoclaw-g1"
