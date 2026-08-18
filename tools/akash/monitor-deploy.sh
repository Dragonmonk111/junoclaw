#!/bin/bash
# Autonomous, self-contained monitor for a Kimi K2.6 J-Lens Akash deployment.
# Runs entirely in the background; writes ONE result file at the end so no
# repeated manual polling/tool calls are needed.
#
# Usage:
#   bash monitor-deploy.sh <dseq> <provider> <health-url> <keyring-home> <max-minutes>
#
# Writes to: /tmp/jlens-deploy-status.txt
#   Lines: STATUS=<PENDING|SUCCESS|FAILED|TIMEOUT>
#          ELAPSED_MIN=<n>
#          DETAIL=<free text>
#   Also appends last 40 lines of container logs on FAILED/TIMEOUT.

set -u
DSEQ="$1"
PROVIDER="$2"
HEALTH_URL="$3"
KEYRING_HOME="$4"
MAX_MIN="${5:-90}"

STATUS_FILE="/tmp/jlens-deploy-status.txt"
export AKASH_HOME="$KEYRING_HOME"
export AKASH_FROM=junoclaw-autodeploy
export AKASH_KEYRING_BACKEND=test
export AKASH_NODE=https://akash-rpc.polkachu.com:443
export AKASH_CHAIN_ID=akashnet-2

echo "STATUS=PENDING" > "$STATUS_FILE"
echo "ELAPSED_MIN=0" >> "$STATUS_FILE"
echo "DETAIL=starting monitor" >> "$STATUS_FILE"

start_ts=$(date +%s)
max_secs=$((MAX_MIN * 60))

while true; do
  now=$(date +%s)
  elapsed=$(( (now - start_ts) / 60 ))

  if [ $((now - start_ts)) -ge $max_secs ]; then
    {
      echo "STATUS=TIMEOUT"
      echo "ELAPSED_MIN=$elapsed"
      echo "DETAIL=exceeded ${MAX_MIN} min without success"
      echo "--- last logs ---"
      provider-services lease-logs --dseq "$DSEQ" --provider "$PROVIDER" 2>&1 | tail -n 40
    } > "$STATUS_FILE"
    exit 1
  fi

  resp=$(curl -sk --max-time 10 "$HEALTH_URL" 2>&1)
  if echo "$resp" | grep -q '"status"'; then
    {
      echo "STATUS=SUCCESS"
      echo "ELAPSED_MIN=$elapsed"
      echo "DETAIL=$resp"
    } > "$STATUS_FILE"
    exit 0
  fi

  # detect a crash traceback in logs every ~5 checks to avoid spamming lease-logs
  if [ $(( (now - start_ts) % 150 )) -lt 30 ]; then
    logs=$(provider-services lease-logs --dseq "$DSEQ" --provider "$PROVIDER" 2>&1 | tail -n 60)
    if echo "$logs" | grep -qE "Traceback|Error:|CUDA out of memory|CrashLoopBackOff"; then
      {
        echo "STATUS=FAILED"
        echo "ELAPSED_MIN=$elapsed"
        echo "DETAIL=error detected in logs"
        echo "--- last logs ---"
        echo "$logs"
      } > "$STATUS_FILE"
      exit 2
    fi
  fi

  echo "STATUS=PENDING" > "$STATUS_FILE"
  echo "ELAPSED_MIN=$elapsed" >> "$STATUS_FILE"
  echo "DETAIL=last_response=$(echo "$resp" | head -c 100)" >> "$STATUS_FILE"

  sleep 30
done
