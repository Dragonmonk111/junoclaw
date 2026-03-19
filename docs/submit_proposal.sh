#!/bin/bash
# ============================================================
# JunoClaw Governance Proposal Submission Script
# Run this on your Juno validator node (VirtualBox Ubuntu VM)
# ============================================================
# 
# BEFORE RUNNING: Copy these files to the VM:
#   - proposal_description.txt  (full description text)
#   - proposal.json             (for SDK v0.47+ method)
#
# Choose your key name below:
KEY_NAME="vairagyanodes"   # <-- change this to your actual key name in junod keys list
CHAIN_ID="juno-1"
NODE="https://juno-rpc.polkachu.com:443"
GAS_PRICES="0.025ujuno"
DEPOSIT="1000000000ujuno"  # 1,000 JUNO

echo "============================================"
echo "  JunoClaw Proposal Submission"
echo "  Chain: $CHAIN_ID (MAINNET)"
echo "  Deposit: 1,000 JUNO"
echo "============================================"
echo ""

# ── STEP 1: Check junod version ──
echo "── Step 1: Checking junod version ──"
junod version
echo ""

# ── STEP 2: List available keys ──
echo "── Step 2: Available keys ──"
junod keys list --output json | jq '.[].name'
echo ""
echo "Using key: $KEY_NAME"
echo "If this is wrong, edit KEY_NAME at the top of this script and re-run."
echo ""

# ── STEP 3: Check balance ──
echo "── Step 3: Checking balance ──"
ADDR=$(junod keys show $KEY_NAME -a)
echo "Address: $ADDR"
junod query bank balances $ADDR --node $NODE --output json | jq '.balances'
echo ""

# ── STEP 4 (OPTIONAL): Claim validator rewards ──
# Uncomment the next 3 lines if you need to claim rewards first
# echo "── Step 4: Claiming validator rewards ──"
# VALOPER=$(junod keys show $KEY_NAME --bech val -a)
# junod tx distribution withdraw-rewards $VALOPER --commission --from $KEY_NAME --chain-id $CHAIN_ID --node $NODE --gas auto --gas-adjustment 1.3 --gas-prices $GAS_PRICES -y

echo ""
echo "── Step 5: Submit Proposal ──"
echo ""
echo "Choose a method below. Only run ONE of them."
echo ""

# ════════════════════════════════════════════════
# METHOD A: Legacy text proposal (SDK v0.45/v0.46)
# ════════════════════════════════════════════════
echo "METHOD A command (legacy - for older junod):"
echo "────────────────────────────────────────────"
cat << 'CMDA'
junod tx gov submit-proposal \
  --type Text \
  --title "JunoClaw — Verifiable AI Agents with TEE-Attested Junoswap Revival on Juno" \
  --description "$(cat proposal_description.txt)" \
  --deposit 1000000000ujuno \
  --from vairagyanodes \
  --chain-id juno-1 \
  --node https://juno-rpc.polkachu.com:443 \
  --gas auto \
  --gas-adjustment 1.5 \
  --gas-prices 0.025ujuno
CMDA
echo ""

# ════════════════════════════════════════════════
# METHOD B: New submit-proposal (SDK v0.47+)
# ════════════════════════════════════════════════
echo "METHOD B command (new SDK v0.47+ - uses proposal.json):"
echo "────────────────────────────────────────────"
cat << 'CMDB'
junod tx gov submit-proposal proposal.json \
  --from vairagyanodes \
  --chain-id juno-1 \
  --node https://juno-rpc.polkachu.com:443 \
  --gas auto \
  --gas-adjustment 1.5 \
  --gas-prices 0.025ujuno
CMDB
echo ""

# ════════════════════════════════════════════════
# METHOD C: submit-legacy-proposal (SDK v0.47+ but using legacy path)
# ════════════════════════════════════════════════
echo "METHOD C command (v0.47+ legacy fallback):"
echo "────────────────────────────────────────────"
cat << 'CMDC'
junod tx gov submit-legacy-proposal \
  --type Text \
  --title "JunoClaw — Verifiable AI Agents with TEE-Attested Junoswap Revival on Juno" \
  --description "$(cat proposal_description.txt)" \
  --deposit 1000000000ujuno \
  --from vairagyanodes \
  --chain-id juno-1 \
  --node https://juno-rpc.polkachu.com:443 \
  --gas auto \
  --gas-adjustment 1.5 \
  --gas-prices 0.025ujuno
CMDC

echo ""
echo "============================================"
echo "  HOW TO PICK THE RIGHT METHOD:"
echo "  1. Try METHOD A first"
echo "  2. If it says 'unknown command', try METHOD C"
echo "  3. If that also fails, use METHOD B with proposal.json"
echo "============================================"
echo ""
echo "AFTER SUBMISSION:"
echo "  - Copy the TX hash from the output"
echo "  - Run: junod query tx <TX_HASH> --node $NODE"
echo "  - Find proposal_id in the tx events"
echo "  - Verify: junod query gov proposal <PROPOSAL_ID> --node $NODE"
echo ""
