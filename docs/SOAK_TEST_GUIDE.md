# Finished-Product Soak Test — Setup Guide

This guide describes how to run a full-stack soak test with the patched `junod` (BN254 precompiles), ZK proof generation, and on-chain verification in a continuous loop.

## What This Tests

Unlike the coordination soak (which tests P2P mesh + consensus), the finished-product soak tests the **full ZK loop**:

1. Prover daemon generates ZK proofs every N seconds
2. Proofs are submitted to the zk-verifier contract on-chain
3. Circuit breaker state is checked
4. Merkle roots are anchored
5. All on a patched `junod` with BN254 precompiles

## Prerequisites

- Linux machine (or WSL2 with Docker)
- Docker + Docker Compose
- 4 vCPU / 8 GB RAM minimum
- 50 GB disk (for chain state)

## Setup

### Step 1: Build the patched junod image

```bash
cd wasmvm-fork
docker build -t junoclaw/junod-bn254:v30 -f Dockerfile.junod .
```

This builds a Docker image with:
- Juno v30.0.0
- Patched wasmvm v3.0.4 (17 BN254 symbols)
- 10 BN254 precompile patches applied

### Step 2: Build the prover daemon image

```bash
cd prover-daemon
docker build -t junoclaw/prover:v0.1 .
```

### Step 3: Build the bridge image

```bash
cd plugins/plugin-ros2/bridge
docker build -t junoclaw/bridge:v0.1 .
```

### Step 4: Create soak test config

```bash
cat > soak-config.toml << 'EOF'
robot_id = "soak-bot-01"
bridge_url = "http://ros2-bridge:8080"
chain_rpc = "http://junod:26657"
keys_dir = "/keys"
verifier_addr = ""  # filled after contract deployment
circuit_breaker_addr = ""  # filled after contract deployment
safety_envelope_addr = ""  # filled after contract deployment
merkle_verifier_addr = ""  # filled after contract deployment
poll_interval_secs = 30
log_level = "info"
EOF
```

### Step 5: Start the stack

```bash
cd deploy
JUNOD_IMAGE=junoclaw/junod-bn254:v30 docker compose up -d junod

# Wait for chain to start
sleep 10
curl http://localhost:26657/status | jq .result.sync_info

# Deploy contracts
./scripts/deploy_contracts.sh

# Update config with deployed addresses
source scripts/contract_addresses.env
sed -i "s/verifier_addr = \"\"/verifier_addr = \"$VERIFIER_ADDR\"/" soak-config.toml
sed -i "s/circuit_breaker_addr = \"\"/circuit_breaker_addr = \"$BREAKER_ADDR\"/" soak-config.toml

# Start bridge + prover
docker compose up -d ros2-bridge prover-daemon
```

### Step 6: Generate proving keys

```bash
docker compose exec prover-daemon junoclaw-prover setup --output /keys --tree-height 7
```

### Step 7: Set safety envelope

```bash
junoclay tx safety-envelope set-envelope \
  --robot-id soak-bot-01 \
  --max-speed 2000 --max-force 50000 --min-distance 500 \
  --max-tilt 15000 --max-accel 3000 \
  --from validator --yes
```

### Step 8: Start the soak

```bash
# Monitor prover logs
docker compose logs -f prover-daemon

# In another terminal, monitor chain
watch -n 5 'curl -s http://localhost:26657/status | jq .result.sync_info.latest_block_height'

# Check breaker state periodically
junoclay query circuit-breaker is-locked --robot-id soak-bot-01
```

## What to Monitor

| Metric | How to Check | Expected |
|--------|-------------|----------|
| Chain block height | `curl localhost:26657/status` | Increasing every ~2.8s |
| Prover batches | `docker logs prover-daemon` | New batch every 30s |
| Proof generation time | Prover logs | ~80-187ms per proof |
| On-chain verify gas | `junoclay query tx --events...` | ~203K gas (precompile) |
| Circuit breaker | `junoclay query circuit-breaker` | Closed (no violations) |
| Bridge health | `curl localhost:8080/health` | OK |
| Container uptime | `docker compose ps` | All services up |

## Success Criteria (7-Day Soak)

| Criterion | Target |
|-----------|--------|
| Duration | 7 days (168 hours) |
| Chain uptime | 100% (no missed blocks > 5 min) |
| Proofs generated | > 20,000 (one every 30s for 7 days) |
| On-chain verifications | > 20,000 |
| Proof generation failures | 0 |
| On-chain verify failures | 0 |
| Circuit breaker false trips | 0 |
| Container restarts | 0 |
| Memory growth | < 20% over 7 days |

## Troubleshooting

### Prover can't connect to bridge
- Check `docker compose ps` — bridge must be healthy
- Check network: `docker compose exec prover-daemon curl http://ros2-bridge:8080/health`

### On-chain verify fails
- Check verifier address in config
- Check proof format (must be uncompressed)
- Check gas limit (may need to increase for large proofs)

### Chain out of disk
- Prune old blocks: `junoclay prune --keep-recent 10000`
- Or reduce block retention in config

### Memory growth
- Check `docker stats` for each container
- Prover daemon should be < 500MB
- Bridge should be < 200MB
- junod may grow to several GB (normal for full node)
