#!/usr/bin/env bash
export PATH="/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
curl -s http://localhost:26657/status 2>/dev/null | python3 << 'EOF'
import sys, json
try:
    d = json.load(sys.stdin)
    print("height:", d["result"]["sync_info"]["latest_block_height"])
except:
    print("chain dead")
EOF
