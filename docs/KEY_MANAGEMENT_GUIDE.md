# Multi-Robot Key Management — Provisioning and Rotation Guide

This document describes how to provision, manage, and rotate cryptographic keys for a fleet of robots using JunoClaw.

## Key Hierarchy

```
Governance Key (multisig)
    │
    ├── Robot Operator Key (per operator)
    │       │
    │       ├── Robot Signing Key (per robot, on controller)
    │       │       │
    │       │       └── Signs IntentMessages + ReflexBatchAttestations
    │       │
    │       └── Prover Key (per robot, on edge device)
    │               │
    │               └── Signs on-chain proof submission transactions
    │
    └── Governance Reset Key (multisig)
            │
            └── Resets circuit breakers, updates safety envelopes
```

## Key Types

| Key Type | Where It Lives | What It Signs | Rotation Policy |
|----------|---------------|--------------|-----------------|
| Governance Key | Multisig wallet (Ledger) | Safety envelope changes, breaker resets | Annual or on governance vote |
| Operator Key | Operator's wallet | Robot registration, operator-level txs | On personnel change |
| Robot Signing Key | Robot controller (TEE/HSM) | IntentMessages, ReflexBatchAttestations | On compromise suspicion or annual |
| Prover Key | Edge device (secure storage) | On-chain proof submission txs | On compromise suspicion or annual |

## Provisioning a New Robot

### Step 1: Generate Robot Signing Key

```bash
# On the robot controller (ideally in a TEE or HSM)
junoclaw keys add warehouse-bot-01 \
  --keyring-backend file \
  --home /etc/junoclaw/keys \
  --output json > robot-01-key.json
```

### Step 2: Register Robot On-Chain

```bash
# Operator submits registration transaction
junoclay tx skill-registry register \
  --dapp-name warehouse-bot-01 \
  --capability robotics \
  --metadata '{"manufacturer": "Unitree", "model": "G1", "serial": "UTG1-2026-001"}' \
  --from operator-key \
  --yes
```

### Step 3: Set Safety Envelope

```bash
# Governance approves initial safety envelope
junoclay tx safety-envelope set-envelope \
  --robot-id warehouse-bot-01 \
  --max-speed 2000 --max-force 50000 --min-distance 500 \
  --max-tilt 15000 --max-accel 3000 \
  --human-proximity-allowed false \
  --from governance-multisig \
  --yes
```

### Step 4: Configure Bridge + Prover

```bash
# On the robot
export JUNOCLAW_ROBOT_ID=warehouse-bot-01
junoclaw-ros2-bridge --robot-id warehouse-bot-01 --port 8080 &

# On the edge device
junoclaw-prover run \
  --config prover-config.toml \
  --bridge-url http://robot:8080 \
  --chain-rpc http://chain:26657
```

### Step 5: Verify Registration

```bash
junoclay query skill-registry get --dapp-name warehouse-bot-01
junoclay query safety-envelope get-envelope --robot-id warehouse-bot-01
junoclay query circuit-breaker is-locked --robot-id warehouse-bot-01
# Expected: is_locked = false
```

## Key Rotation

### Robot Signing Key Rotation

```bash
# 1. Generate new key
junoclaw keys add warehouse-bot-01-new \
  --keyring-backend file \
  --home /etc/junoclaw/keys

# 2. Update skill-registry with new key
junoclay tx skill-registry update-key \
  --dapp-name warehouse-bot-01 \
  --new-key $(junoclaw keys show warehouse-bot-01-new -a) \
  --from operator-key \
  --yes

# 3. Verify new key is active
junoclay query skill-registry get --dapp-name warehouse-bot-01

# 4. Remove old key from controller
junoclaw keys delete warehouse-bot-01 \
  --keyring-backend file \
  --home /etc/junoclaw/keys \
  --yes
```

### Prover Key Rotation

```bash
# 1. Generate new prover key
junoclaw keys add prover-bot-01-new \
  --keyring-backend file \
  --home /etc/junoclaw/prover-keys

# 2. Update prover daemon config
sed -i 's/from_account = "prover-bot-01"/from_account = "prover-bot-01-new"/' prover-config.toml

# 3. Restart prover daemon
docker compose restart prover-daemon

# 4. Verify proofs still submit
docker logs -f prover-daemon | grep "tx="
```

### Emergency Key Revocation

If a robot key is compromised:

```bash
# 1. Immediately trip circuit breaker (locks intent tier)
junoclay tx circuit-breaker trip-breaker \
  --robot-id warehouse-bot-01 \
  --reason "key compromise — emergency revocation" \
  --from governance-multisig \
  --yes

# 2. Revoke key from skill-registry
junoclay tx skill-registry revoke-key \
  --dapp-name warehouse-bot-01 \
  --from governance-multisig \
  --yes

# 3. Generate new key and re-register (see Provisioning above)
```

## Fleet Key Management Script

```bash
#!/bin/bash
# fleet-keys.sh — Manage keys for a fleet of robots

ROBOTS=("warehouse-bot-01" "warehouse-bot-02" "delivery-bot-01")

for ROBOT_ID in "${ROBOTS[@]}"; do
    echo "Provisioning $ROBOT_ID..."

    # Generate key
    junoclaw keys add "$ROBOT_ID" \
        --keyring-backend file \
        --home /etc/junoclaw/keys \
        --output json > "${ROBOT_ID}-key.json"

    # Register on-chain
    junoclay tx skill-registry register \
        --dapp-name "$ROBOT_ID" \
        --capability robotics \
        --from operator-key --yes

    # Set safety envelope
    junoclay tx safety-envelope set-envelope \
        --robot-id "$ROBOT_ID" \
        --max-speed 2000 --max-force 50000 --min-distance 500 \
        --max-tilt 15000 --max-accel 3000 \
        --from governance-multisig --yes

    echo "  $ROBOT_ID: $(junoclay keys show "$ROBOT_ID" -a --keyring-backend file --home /etc/junoclaw/keys)"
done
```

## Security Best Practices

1. **Never store keys in environment variables** — use file keyring or HSM
2. **Use TEE (SGX/SEV-SNP) for robot signing keys** when available
3. **Rotate keys annually** or on personnel changes
4. **Use multisig for governance keys** — minimum 3-of-5
5. **Log all key operations** to the moultbook contract (immutable audit trail)
6. **Test emergency revocation** before deploying to production
7. **Separate operator keys from robot keys** — operators can register robots but cannot sign intents
8. **Use different keys for different environments** — devnet, testnet, mainnet keys must never overlap

## Key Storage Options

| Option | Security | Cost | Best For |
|--------|----------|------|----------|
| File keyring | Low | Free | Development |
| Hardware HSM (YubiHSM) | High | $500+ | Production single robot |
| TEE (Intel SGX / AMD SEV-SNP) | High | Hardware cost | Production edge devices |
| Cloud KMS (AWS/GCP) | Medium | Per-use | Cloud-managed fleets |
| Ledger hardware wallet | Highest | $100+ | Governance keys |
