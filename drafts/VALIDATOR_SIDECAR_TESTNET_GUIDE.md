# Validator Sidecar Guide — Running the JunoClaw Coordination Node on Testnet

> This guide walks Juno validators through running the coordination layer sidecar alongside their `junod` process on the uni-7 testnet. The sidecar is a lightweight Rust binary that joins the Commonware P2P mesh, participates in BFT consensus for agent message ordering, and produces threshold certificates that get settled on Juno via the `coordination-settler` contract.

---

## What you're running

A **coordination node** — a Rust process that:

1. Joins a P2P mesh of 4 validator nodes (tolerates 1 byzantine)
2. Orders agent messages into finalized batches via BFT consensus
3. Produces threshold certificates for each finalized batch
4. Optionally runs the J-Lens truth gate (audits agent message content before inclusion)

The coordination node is **not a blockchain**. It has no tokens, no staking, no governance. It produces ordered batches of messages that get verified and settled on Juno via the `coordination-settler` CosmWasm contract.

**Security model:** This is the same sidecar pattern used by oracle networks like Skip/Slinky. The coordination node runs alongside `junod` on the same machine. It does not modify `junod`, does not share keys with it, and does not affect block production. If the sidecar crashes, your validator keeps producing blocks normally.

---

## Prerequisites

- **Rust 1.75+** (stable) — `rustup default stable`
- **NASM 2.16+** — required by `aws-lc-sys` (Commonware's crypto backend)
  - Ubuntu: `sudo apt install nasm`
  - macOS: `brew install nasm`
  - Verify: `nasm --version`
- **protoc** (Protocol Buffers compiler) — required by Commonware's gRPC stack
  - Ubuntu: `sudo apt install protobuf-compiler`
  - macOS: `brew install protobuf`
- **A Juno uni-7 testnet validator** (or willingness to run one)
- **Open ports**: 4001 TCP (P2P), 4002 TCP (REST API for relayer)

---

## Step 1: Build the coordination node

```bash
git clone https://github.com/CosmosContracts/junoclaw.git
cd junoclaw

# Build with P2P feature (enables real Commonware mesh)
cargo build --release -p junoclaw-coordination --features p2p

# Verify the binary
./target/release/junoclaw-coordination-node --help
```

> **Note:** The `p2p` feature enables `aws-lc-sys` which requires NASM. If you see build errors mentioning `aws-lc-sys` or `nasm`, ensure NASM is installed and on your `$PATH`.

---

## Step 2: Generate your validator key

The coordination node uses its own Ed25519 keypair (separate from your Juno validator key). Generate one:

```bash
# Generate a new keypair (saved to coordination-key.json)
./target/release/junoclaw-coordination-node keygen --output coordination-key.json
```

This produces:
```json
{
  "public_key": "a1b2c3...",
  "private_key": "d4e5f6...",
  "peer_id": "12D3KooW..."
}
```

**Share your public key** with the DAO so it can be added to the validator set in the `coordination-settler` contract. The DAO admin (the Juno Agents DAO core contract) calls `UpdateValidatorSet` to register your key.

---

## Step 3: Get the validator set

The current testnet validator set is managed by the DAO. Contact the DAO (via the Juno Agents DAO Discord or Commonwealth) to:

1. Submit your coordination public key (from Step 2)
2. Get the current list of bootstrap peers (other validators' addresses + public keys)
3. Get your validator index (0-3 for the 4-node testnet set)

You'll receive something like:
```
Bootstrap peers:
  /ip4/13.42.71.8/tcp/4001/p2p/12D3KooWAbc...  (validator 0)
  /ip4/18.33.12.90/tcp/4001/p2p/12D3KooWDef...  (validator 1)
  /ip4/44.21.9.150/tcp/4001/p2p/12D3KooWGhi...  (validator 2)
  # Your address goes here as validator 3

Validator index: 3
Threshold: 3 of 4
```

---

## Step 4: Run the coordination node

```bash
./target/release/junoclaw-coordination-node run \
  --key-file coordination-key.json \
  --listen-addr 0.0.0.0:4001 \
  --rest-addr 0.0.0.0:4002 \
  --bootstrap-peers "/ip4/13.42.71.8/tcp/4001/p2p/12D3KooWAbc...,/ip4/18.33.12.90/tcp/4001/p2p/12D3KooWDef...,/ip4/44.21.9.150/tcp/4001/p2p/12D3KooWGhi..." \
  --validator-index 3 \
  --num-validators 4 \
  --block-time 300ms \
  --juno-rpc https://juno-testnet-rpc.polkachu.com:443 \
  --contract juno16gp6mm7ehhrx0ls4s77yk668cclq5gyc09x7k45f28apwmsqkg0qhmmnc8
```

### What each flag does:

| Flag | Description |
|------|-------------|
| `--key-file` | Path to your Ed25519 keypair (from Step 2) |
| `--listen-addr` | P2P listen address (port 4001) |
| `--rest-addr` | REST API for the relayer to query finalized batches (port 4002) |
| `--bootstrap-peers` | Comma-separated list of other validators' multiaddrs |
| `--validator-index` | Your index in the validator set (0-based, assigned by DAO) |
| `--num-validators` | Total validators in the set (4 for testnet) |
| `--block-time` | Consensus block interval (300ms default) |
| `--juno-rpc` | Juno uni-7 RPC endpoint (for settlement verification) |
| `--contract` | `coordination-settler` contract address on uni-7 |

---

## Step 5: Run as a systemd service (recommended)

Create `/etc/systemd/system/junoclaw-coordination.service`:

```ini
[Unit]
Description=JunoClaw Coordination Node (testnet sidecar)
After=network.target
Wants=junod.service

[Service]
Type=simple
User=juno
Group=juno
WorkingDirectory=/home/juno/junoclaw
ExecStart=/home/juno/junoclaw/target/release/junoclaw-coordination-node run \
  --key-file /home/juno/coordination-key.json \
  --listen-addr 0.0.0.0:4001 \
  --rest-addr 0.0.0.0:4002 \
  --bootstrap-peers "/ip4/13.42.71.8/tcp/4001/p2p/12D3KooWAbc...,/ip4/18.33.12.90/tcp/4001/p2p/12D3KooWDef...,/ip4/44.21.9.150/tcp/4001/p2p/12D3KooWGhi..." \
  --validator-index 3 \
  --num-validators 4 \
  --block-time 300ms \
  --juno-rpc https://juno-testnet-rpc.polkachu.com:443 \
  --contract juno16gp6mm7ehhrx0ls4s77yk668cclq5gyc09x7k45f28apwmsqkg0qhmmnc8
Restart=on-failure
RestartSec=10
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable junoclaw-coordination
sudo systemctl start junoclaw-coordination

# Check status
sudo systemctl status junoclaw-coordination

# View logs
journalctl -u junoclaw-coordination -f
```

---

## Step 6: Verify it's working

### Check P2P connectivity:
```bash
curl http://localhost:4002/health
```
Expected response:
```json
{
  "status": "healthy",
  "peers_connected": 3,
  "validator_index": 3,
  "current_height": 42
}
```

### Check finalized batches:
```bash
curl http://localhost:4002/finalized?after=0
```
Expected response:
```json
{
  "batches": [
    {
      "commonware_height": 1,
      "messages_hash": "a1b2c3...",
      "certificate": "d4e5f6...",
      "timestamp": 1723392000000
    }
  ],
  "latest_height": 1
}
```

### Check on-chain settlement:
Query the `coordination-settler` contract on uni-7:
```bash
junod query wasm contract-state smart \
  juno16gp6mm7ehhrx0ls4s77yk668cclq5gyc09x7k45f28apwmsqkg0qhmmnc8 \
  '{"batch":{"commonware_height":1}}' \
  --chain-id uni-7
```

If the batch is settled, you'll see the stored batch with messages_hash and certificate hash.

---

## Step 7: Run the relayer (optional — one validator can do this)

One validator in the set runs the relayer daemon. It watches the coordination node's REST API for finalized batches and submits them to the `coordination-settler` contract on Juno.

```bash
cargo build --release -p junoclaw-relayer

./target/release/junoclaw-relayer run \
  --rpc https://juno-testnet-rpc.polkachu.com:443 \
  --contract juno16gp6mm7ehhrx0ls4s77yk668cclq5gyc09x7k45f28apwmsqkg0qhmmnc8 \
  --key "your-juno-testnet-mnemonic" \
  --coordination-endpoint http://127.0.0.1:4002 \
  --poll-interval 5
```

> The relayer needs a Juno testnet wallet with a small amount of JUNO for gas. 1 JUNO is enough for thousands of transactions.

---

## Firewall rules

Open these ports on your validator machine:

| Port | Protocol | Purpose |
|------|----------|---------|
| 4001 | TCP | P2P mesh (other coordination validators connect here) |
| 4002 | TCP | REST API (relayer queries — can be localhost-only if relayer runs on same machine) |

If you're running `junod` on the same machine, its ports (26656 P2P, 26657 RPC, 1317 REST) are unaffected.

---

## Resource usage

The coordination node is lightweight:
- **CPU**: <5% (mostly idle between 300ms consensus rounds)
- **RAM**: ~50-100MB (P2P mesh + consensus state)
- **Disk**: <1GB (no blockchain state — just message buffers)
- **Network**: Low — only agent messages and consensus votes, not block data

Compare: `junod` typically uses 2-8GB RAM, 50-200GB disk, and significant CPU for block validation.

---

## Troubleshooting

### Build fails with `aws-lc-sys` error
```
error: failed to run custom build command for `aws-lc-sys`
```
**Fix:** Install NASM: `sudo apt install nasm` (Ubuntu) or `brew install nasm` (macOS). Verify with `nasm --version` (needs 2.16+).

### Node starts but shows 0 peers
**Check:**
1. Your firewall allows inbound TCP on port 4001
2. The bootstrap peer addresses are correct (contact the DAO for the latest list)
3. Your public IP is reachable from the other validators' machines

### Node shows peers but no blocks being finalized
**Check:**
1. At least 3 of 4 validators are online (BFT requires 2f+1 = 3 of 4)
2. Your validator index is correct (must match what the DAO registered)
3. The validator set in the `coordination-settler` contract includes your public key

### Relayer fails to submit batches
**Check:**
1. Your relayer wallet has JUNO for gas: `junod query bank balances <your-address> --chain-id uni-7`
2. Your wallet is registered as a relayer in the contract (DAO admin calls `RegisterRelayer`)
3. The RPC endpoint is reachable: `curl https://juno-testnet-rpc.polkachu.com:443/status`

---

## What's next

- **Mainnet:** Once testnet validation proves the full pipeline (consensus → gate → settle), the DAO will propose upgrading the validator set to Juno mainnet validators running the same sidecar. Your testnet experience directly informs the mainnet deployment.
- **J-Lens gate:** The coordination node can optionally run the J-Lens truth gate, which audits agent messages using internal-state probing on open-weight models. This requires a GPU or a remote CSI server endpoint. For testnet, the gate is optional — validators can run without it initially.
- **Slashing:** There is no slashing for coordination node downtime. If your sidecar goes offline, the worst case is the consensus set drops to 3 nodes (still functional). If 2+ go offline, consensus pauses until they return — no Juno chain impact.

---

## Contact

- **DAO Discord:** [Juno Agents DAO](https://discord.gg/juno) — #coordination-layer channel
- **Commonwealth:** [commonwealth.im/juno](https://commonwealth.im/juno)
- **Contract on uni-7:** `juno16gp6mm7ehhrx0ls4s77yk668cclq5gyc09x7k45f28apwmsqkg0qhmmnc8` (code ID 86)
- **Source code:** `crates/junoclaw-coordination/` in the junoclaw repo

Questions? Post in the DAO Discord or open an issue on the repo. The builder is available to help with setup, debugging, and integration.
