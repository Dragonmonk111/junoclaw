#!/bin/bash
URL="https://ca3aij1lodehd3u6j3obe0a1u4.ingress.a100.dal2.aes.akash.pub/health"
for i in $(seq 1 80); do
  result=$(curl -sk --max-time 10 "$URL" 2>&1)
  echo "$i: $result"
  if echo "$result" | grep -q '"status"'; then
    echo "MODEL IS READY"
    break
  fi
  sleep 30
done
