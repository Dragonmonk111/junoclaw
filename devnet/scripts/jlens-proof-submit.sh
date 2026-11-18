#!/usr/bin/env bash
# J-Lens prover service primitive for the BN254 devnet.
#
# Takes a JlensReadout disclosure JSON (the jc-mem CommitJlens payload:
# {activations:[i8], bank_vec:[i8], threshold:u64, leaf_index:u32}),
# generates a Groth16 proof via `prove-disclosure`, anchors the
# attestation, submits resolve_with_proof, and verifies the attestation
# lands `finalized` on-chain.
#
# Env:
#   DEVNET_CONTAINER  (default: junoclaw-bn254-devnet)
#   COURT_ADDR        thought-court contract address (required)
#   SIGNER            keyring name (default: admin)
#   TREE_HEIGHT       MiMC tree height for the padded tree (default: 4)
#   ATTESTATION_ID    optional; defaults to meta.suggested_attestation_id
#
# Usage: jlens-proof-submit.sh <disclosure.json> [--artifacts <dir>]
set -euo pipefail

CONTAINER="${DEVNET_CONTAINER:-junoclaw-bn254-devnet}"
COURT="${COURT_ADDR:?COURT_ADDR (thought-court address) required}"
SIGNER="${SIGNER:-admin}"
HEIGHT="${TREE_HEIGHT:-4}"
CHAIN_ID="${CHAIN_ID:-junoclaw-bn254-1}"
FEES="${FEES:-60000ujuno}"

DISCLOSURE="${1:?usage: jlens-proof-submit.sh <disclosure.json> [--artifacts <dir>]}"
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
"$BIN" --disclosure "$DISCLOSURE" --height "$HEIGHT" --out "$ARTIFACTS"

META="$ARTIFACTS/meta.json"
ROOT=$(grep -o '"activation_root": *"[^"]*"' "$META" | cut -d'"' -f4)
BANK=$(grep -o '"bank_commitment": *"[^"]*"' "$META" | cut -d'"' -f4)
SCORE=$(grep -o '"score": *[0-9]*' "$META" | grep -o '[0-9]*')
INTEGRITY=$(grep -o '"integrity": *[0-9]*' "$META" | grep -o '[0-9]*')
DETECTION=$(grep -o '"detection": *[a-z]*' "$META" | awk '{print $2}')
AID="${ATTESTATION_ID:-$(grep -o '"suggested_attestation_id": *"[^"]*"' "$META" | cut -d'"' -f4)}"

junod_tx() {
  docker exec "$CONTAINER" junod tx "$@" --keyring-backend test --output json
}
junod_query() {
  docker exec "$CONTAINER" junod query "$@" --output json
}

echo "== anchoring $AID (root ${ROOT:0:16}…, score $SCORE)"
junod_tx wasm execute "$COURT" \
  '{"anchor_attestation":{"id":"'"$AID"'","activation_root":"'"$ROOT"'","claims":{"score":'"$SCORE"',"integrity":'"$INTEGRITY"',"detection":'"$DETECTION"',"bank_commitment":"'"$BANK"'"}}}' \
  --from "$SIGNER" --amount 1ujuno --fees "$FEES" --gas 400000 --chain-id "$CHAIN_ID" -y >/dev/null
sleep 4

echo "== submitting resolve_with_proof"
TX=$(junod_tx wasm execute "$COURT" \
  '{"resolve_with_proof":{"attestation_id":"'"$AID"'","proof_base64":"'"$(cat "$ARTIFACTS/jlens-proof.b64")"'","public_inputs_base64":"'"$(cat "$ARTIFACTS/jlens-inputs.b64")"'"}}' \
  --from "$SIGNER" --fees "$FEES" --gas 800000 --chain-id "$CHAIN_ID" -y)
TXHASH=$(echo "$TX" | grep -o '"txhash":"[^"]*"' | cut -d'"' -f4)
sleep 4

STATUS=$(junod_query wasm contract-state smart "$COURT" \
  '{"get_attestation":{"attestation_id":"'"$AID"'"}}' | grep -o '"status":"[^"]*"' | cut -d'"' -f4)
echo "attestation $AID status=$STATUS (tx $TXHASH)"
[ "$STATUS" = "finalized" ] || { echo "FAIL: expected finalized" >&2; exit 1; }
echo "PASS: disclosure proven + resolved on-chain"
