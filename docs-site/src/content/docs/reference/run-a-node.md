---
title: Run a node
description: Configure and run a slay3rd validator node — keys, node.toml, Docker, and health checks.
---

The JunoClaw node binary is **`slay3rd`**. It takes one argument: the path to a `node.toml`.

## 1. Get the binary

The chain source is on the `commonware` branch of [`Dragonmonk111/layer-sdk`](https://github.com/Dragonmonk111/layer-sdk/tree/commonware).

```bash
git clone -b commonware https://github.com/Dragonmonk111/layer-sdk.git junoclaw-chain
cd junoclaw-chain

# Option A — native build
cargo build --release -p slay3rd        # → target/release/slay3rd

# Option B — Docker image (what the devnet runs)
docker build -f docker/Dockerfile.slay3rd -t junoclaw-chain:latest .
```

For G1, accepted operators get a pinned commit and image digest so that every validator runs identical code.

## 2. Keys

Your `keys.json` holds:

- **BLS12-381 threshold share.** Signs consensus votes
- **MAYO2 key** (hybrid mode). The post-quantum half of every vote
- **Ed25519 identity.** Authenticates your P2P connections

On devnet the coordinator generates keys centrally. For G1, keys come out of the key ceremony, and each operator generates their own share locally. Back up `keys.json` offline once. If you lose it, you lose your seat until the set is changed.

## 3. `node.toml`

This mirrors the running devnet config:

```toml
validator_index = 0                 # assigned at genesis; must match your key share
chain_id = "junoclaw-1"             # devnet value — G1 chain id is issued with genesis
p2p_listen  = "0.0.0.0:7001"
grpc_listen = "0.0.0.0:9090"
bls_key_path      = "/keys/keys.json"
identity_key_path = "/keys/keys.json"
data_dir     = "/data"
genesis_path = ""                   # set from the G1 genesis bundle
mempool_max_pending      = 10000
leader_timeout_ms        = 3000
certification_timeout_ms = 5000
hybrid_consensus = true             # BLS + MAYO2 certificates

# Optional: bootstrap from a certified snapshot instead of replaying from genesis.
# The anchor must be served identically by >= min_anchor_agree peers.
[state_sync]
peers = ["<peer-a>:9090", "<peer-b>:9090"]
min_anchor_agree = 2

# One entry per *other* validator
[[peers]]
address    = "<host>:7001"
public_key = "<ed25519-pubkey-hex>"
```

**Rules**

- `validator_index` must match the index your key share was generated for.
- `chain_id`, the peer set and the binary version must be identical across the set. Mixed versions do not interoperate on P2P, so upgrades are all-or-none.
- `[[peers]]` lists the other validators.

## 4. Run

```bash
# Native
slay3rd /etc/junoclaw/node.toml

# Docker
docker run -d --name junoclaw-node --restart unless-stopped \
  -p 7001:7001 -p 127.0.0.1:9090:9090 \
  -v /etc/junoclaw/node.toml:/config/node.toml:ro \
  -v /etc/junoclaw/keys:/keys:ro \
  -v junoclaw_data:/data \
  -e RUST_LOG=info \
  junoclaw-chain:latest slay3rd /config/node.toml
```

## 5. Verify you're participating

Signs of a healthy node in the logs:

```text
Hybrid BLS+MAYO2 consensus scheme initialized
Authenticated P2P network initialized (N peers)
Block finalized with BLS threshold certificate (CONS-05) height=…
Block finalized — certificate stored … app_hash=… digest=…
```

- Heights should rise at about 1 per second.
- `app_hash` should be **identical across all validators** at the same height. It equals the certified `state_root`.

**Node up but not signing?** Check that:

1. `validator_index` matches your key share.
2. Every peer is reachable on `7001`, and their public keys are correct.
3. `chain_id` and the binary version match the set.
4. The clock is synced over NTP.
5. If you just restarted after downtime: while the node is behind it logs `verify: rejecting proposal` / `payload unavailable`. That is expected. It backfills and then rejoins.

## 6. Operations

- **Pruning.** The validator tier keeps 64k heights of sidecar data, so disk use stays bounded.
- **Restarts.** Nodes resume from persisted state. A node that falls far behind catches up with height-range backfill, or with certified state-sync if its data dir is empty.
- **Upgrades.** Coordinated: everyone switches binary/config at an agreed height.
- **Incidents.** Report in Telegram straight away: the height, the last log lines, and what changed.
