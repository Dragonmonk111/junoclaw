#!/bin/bash
URL="https://nju71p1bj9ccrch90uuu8tikb4.ingress.h100.wdc.hh.akash.pub/health"
for i in $(seq 1 30); do
  code=$(curl -sk --max-time 5 -o /tmp/r.txt -w '%{http_code}' "$URL")
  body=$(cat /tmp/r.txt 2>/dev/null | head -c 200)
  echo "$i: $code $body"
  if echo "$body" | grep -q '"status"'; then
    echo "READY"
    break
  fi
  sleep 3
done
