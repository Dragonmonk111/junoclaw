#!/usr/bin/env bash
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
JUNOD="/root/junoclaw-build/juno/bin/junod"

sleep 15
"${JUNOD}" query tx 82A7A3CC83B74B4B51A009A78DE1C46AEF1177E9F199EB0E53A8D0853A76ECBE --type=hash -o json 2>&1 | python3 << 'EOF'
import sys, json
try:
    d = json.load(sys.stdin)
    print(f"gas_used: {d.get('gas_used','?')}")
    print(f"code: {d.get('code','?')}")
    print(f"raw_log: {str(d.get('raw_log',''))[:400]}")
except Exception as e:
    print(f"error: {e}")
EOF
