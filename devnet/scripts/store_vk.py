#!/usr/bin/env python3
"""Store a Groth16 VK on the zk-verifier contract."""

import json
import subprocess
import sys
import time

ZK_VERIFIER = "juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p"
ADMIN = "admin"
NODE = "http://localhost:26657"
GAS_PRICES = "0.075ujuno"

VK_FILE = "C:/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/circuits/moultbook-membership/devnet/proof-artifacts/vk.b64"

def run(cmd, capture=True):
    result = subprocess.run(cmd, capture_output=True, text=True, shell=True)
    if capture:
        return result.stdout.strip(), result.stderr.strip(), result.returncode
    return None, None, result.returncode

def main():
    # 1. Read VK
    with open(VK_FILE, "r") as f:
        vk_b64 = f.read().strip()
    print(f"[store_vk] VK size: {len(vk_b64)} chars (base64)")

    # 2. Build JSON message
    msg = {"store_vk": {"vk_base64": vk_b64}}
    msg_json = json.dumps(msg, separators=(',', ':'))
    print(f"[store_vk] Message size: {len(msg_json)} bytes")

    # 3. Write message to temp file in container
    msg_b64 = json.dumps(msg_json)  # double-quote escaped for sh -c
    write_cmd = (
        f'docker exec junoclaw-bn254-devnet sh -c '
        f'"cat > /tmp/store_vk_msg.json"'
    )
    # Use stdin via docker exec -i
    proc = subprocess.run(
        ["bash", "-c", f'printf "%s" {json.dumps(msg_json)} | docker exec -i junoclaw-bn254-devnet sh -c "cat > /tmp/store_vk_msg.json"'],
        capture_output=True, text=True
    )
    if proc.returncode != 0:
        print(f"error writing msg file: {proc.stderr}", file=sys.stderr)
        sys.exit(1)

    # 4. Execute the transaction
    tx_cmd = (
        f'docker exec junoclaw-bn254-devnet sh -c "'
        f'MSG=$(cat /tmp/store_vk_msg.json); '
        f'junod tx wasm execute {ZK_VERIFIER} \"$MSG\" '
        f'--from {ADMIN} --chain-id junoclaw-bn254-1 --keyring-backend test '
        f'--gas auto --gas-adjustment 1.5 --gas-prices {GAS_PRICES} '
        f'--broadcast-mode sync --yes --output json --node {NODE}"'
    )
    stdout, stderr, rc = run(tx_cmd)
    print(stdout)
    if rc != 0:
        print(f"error: tx failed: {stderr}", file=sys.stderr)
        sys.exit(1)

    # 5. Parse txhash
    try:
        tx_resp = json.loads(stdout)
        txhash = tx_resp.get("txhash") or tx_resp.get("tx_response", {}).get("txhash")
    except json.JSONDecodeError:
        # Try grep
        import re
        m = re.search(r'"txhash":"([^"]+)"', stdout)
        txhash = m.group(1) if m else None

    if not txhash:
        print("error: no txhash in response", file=sys.stderr)
        print(stdout, file=sys.stderr)
        sys.exit(1)

    print(f"[store_vk] txhash={txhash} — polling...")

    # 6. Poll for confirmation
    for i in range(15):
        time.sleep(2)
        query_cmd = (
            f'docker exec junoclaw-bn254-devnet junod query tx {txhash} '
            f'--node {NODE} --output json'
        )
        out, err, rc = run(query_cmd)
        if rc == 0:
            print(f"[store_vk] TX confirmed on chain")
            # Check for success
            try:
                result = json.loads(out)
                code = result.get("tx_response", {}).get("code", 0)
                if code == 0:
                    print("[store-vk] VK stored successfully")
                else:
                    print(f"[store-vk] TX failed with code={code}")
                    print(result.get("tx_response", {}).get("raw_log", ""))
            except Exception:
                pass
            return
        print(f"  attempt {i+1}...")

    print("error: tx not indexed after 30s", file=sys.stderr)
    sys.exit(1)

if __name__ == "__main__":
    main()
