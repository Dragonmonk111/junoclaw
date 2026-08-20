# JunoClaw Full Stack Deployment

Docker Compose configuration for deploying the complete JunoClaw robotics trust stack.

## Architecture

```
┌──────────────────────────────────────────────────┐
│                Robot / Edge Device                │
│                                                   │
│  ┌──────────┐    ┌───────────┐    ┌───────────┐  │
│  │  ROS2    │───▶│  Bridge   │───▶│  Prover   │  │
│  │  Robot   │    │  (FastAPI)│    │  Daemon   │  │
│  └──────────┘    └───────────┘    └─────┬─────┘  │
│                                         │        │
└─────────────────────────────────────────┼────────┘
                                          │
                          ┌───────────────▼──────────────┐
                          │       Juno Chain (junod)      │
                          │                               │
                          │  ┌─────────────────────────┐  │
                          │  │   zk-verifier contract  │  │
                          │  ├─────────────────────────┤  │
                          │  │  circuit-breaker        │  │
                          │  ├─────────────────────────┤  │
                          │  │  safety-envelope        │  │
                          │  ├─────────────────────────┤  │
                          │  │  merkle-verifier        │  │
                          │  └─────────────────────────┘  │
                          └───────────────────────────────┘
```

## Quick Start

```bash
# 1. Start the full stack
docker compose up -d

# 2. Verify all services are healthy
docker compose ps

# 3. Check bridge health
curl http://localhost:8080/health

# 4. Check chain status
curl http://localhost:26657/status

# 5. Run the demo
cd simulations/gazebo-warehouse && ./demo.sh
```

## Services

| Service | Port | Description |
|---------|------|-------------|
| junod | 26657 (RPC), 1317 (LCD), 9090 (gRPC) | Local Juno chain with BN254 precompiles |
| ros2-bridge | 8080 | ROS2 HTTP bridge (simulation mode by default) |
| prover-daemon | — | Prover daemon (runs in background) |
| fleet-dashboard | 3000 | Web UI for monitoring (optional) |

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ROBOT_ID` | `robot-01` | Unique robot identifier |
| `BRIDGE_PORT` | `8080` | Bridge HTTP port |
| `BRIDGE_SIMULATE` | `true` | Run bridge in simulation mode |
| `CHAIN_RPC` | `http://junod:26657` | Chain RPC endpoint |
| `VERIFIER_ADDR` | — | zk-verifier contract address |
| `BREAKER_ADDR` | — | circuit-breaker contract address |
| `PROVER_INTERVAL` | `10` | Prover polling interval (seconds) |
| `LOG_LEVEL` | `info` | Log level for all services |

### Volumes

| Volume | Mount | Description |
|--------|-------|-------------|
| `junod-data` | `/root/.juno` | Chain state |
| `prover-keys` | `/keys` | Proving/verifying keys |

## Deployment Profiles

### Single Robot (Default)

```bash
docker compose up -d
```

### Multi-Robot Fleet

```bash
# Robot 1
ROBOT_ID=robot-01 BRIDGE_PORT=8081 docker compose up -d ros2-bridge prover-daemon

# Robot 2
ROBOT_ID=robot-02 BRIDGE_PORT=8082 docker compose up -d ros2-bridge prover-daemon
```

### Production (with real ROS2)

```bash
# On the robot
BRIDGE_SIMULATE=false docker compose up -d ros2-bridge prover-daemon
```

### With BN254 Precompiles

```bash
# Use patched junod image
JUNOD_IMAGE=junoclaw/junod-bn254:v30 docker compose up -d junod
```

## Contract Deployment

After starting the chain, deploy contracts:

```bash
# Deploy zk-verifier
junoclaw deploy zk-verifier --code ../contracts/zk-verifier/artifacts/zk_verifier.wasm

# Deploy circuit-breaker
junoclaw deploy circuit-breaker --code ../contracts/circuit-breaker/artifacts/circuit_breaker.wasm

# Deploy safety-envelope
junoclaw deploy safety-envelope --code ../contracts/safety-envelope/artifacts/safety_envelope.wasm

# Set safety envelope for robot-01
junoclaw tx safety-envelope set-envelope --robot-id robot-01 \
  --max-speed 2000 --max-force 50000 --min-distance 500 \
  --max-tilt 15000 --max-accel 3000
```

## Health Checks

```bash
# Chain
curl http://localhost:26657/status | jq '.result.sync_info.latest_block_height'

# Bridge
curl http://localhost:8080/health | jq .

# Prover daemon (check logs)
docker compose logs prover-daemon --tail 20

# Circuit breaker
junoclaw query circuit-breaker is-locked --robot-id robot-01
```

## Troubleshooting

### Bridge not connecting to ROS2

If `ros2_connected` is false in the health check:
- Ensure ROS2 daemon is running: `ros2 daemon start`
- Check `ROS_DOMAIN_ID` matches the robot's domain
- If no ROS2 installed, use `BRIDGE_SIMULATE=true`

### Prover daemon can't generate proofs

- Ensure keys are generated: `cargo run -- setup --output ./keys`
- Check keys directory is mounted: `prover-keys` volume
- Verify bridge is returning batch data: `curl http://localhost:8080/rosbag/batch_test`

### Chain not accepting proofs

- Verify zk-verifier contract is deployed
- Check gas is sufficient
- If using BN254 precompiles, ensure patched junod image is used
