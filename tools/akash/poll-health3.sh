#!/bin/bash
URL="https://nju71p1bj9ccrch90uuu8tikb4.ingress.h100.wdc.hh.akash.pub/health"
for i in $(seq 1 90); do
  result=$(curl -sk --max-time 10 "$URL" 2>&1)
  echo "$i: $result"
  if echo "$result" | grep -q '"status"'; then
    echo "MODEL IS READY"
    break
  fi
  sleep 30
done
