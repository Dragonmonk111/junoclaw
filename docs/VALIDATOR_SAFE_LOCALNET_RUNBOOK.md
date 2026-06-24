# Validator-safe 4-validator bandwidth localnet runbook

Last updated: 2026-06-23

## Purpose

Run the Project Aegis 4-validator bandwidth localnet on the **same ext4 host** that runs the
VairagyaNodes Juno service, **without endangering the production validator**, then restore the
validator to its exact prior state.

The localnet is fully isolated from the production node by three guarantees:

1. **Separate `--home`** — the localnet never touches the production `~/.juno` data dir.
2. **Separate ports** — the localnet uses a +100 port offset so nothing collides.
3. **Production service stopped during the run** — guarantees zero double-sign risk
   (the validator key is never loaded by two processes at once).

> Double-sign safety note: the localnet uses **freshly generated** validator keys in its own home,
> never the production `priv_validator_key.json`. Even so, we stop the production service for the
> duration so there is provably one consensus process touching the production key: zero.

## Fill these in before you start

| Variable | Meaning | Example |
|----------|---------|---------|
| `SVC` | Production systemd service name | `junod` or `vairagyanodes` |
| `PROD_HOME` | Production data dir | `/mnt/ext4/.juno` |
| `JUNOD` | Path to the PQC-enabled Juno binary | `/mnt/ext4/aegis/junod` |
| `LOCAL_HOME_BASE` | Scratch dir for the 4 localnet homes | `/mnt/ext4/aegis-localnet` |
| `ARTIFACT_DIR` | Where to save logs + bandwidth JSON | `/mnt/ext4/aegis-localnet-artifacts` |

Discover the service name deterministically:

```bash
systemctl list-units --type=service | grep -iE 'juno|vairagya'
# Outcome: prints exactly one line -> that is $SVC. If zero/many lines, STOP and inspect.
```

---

## STEP 0 — Record baseline (deterministic snapshot)

```bash
sudo systemctl is-active "$SVC"        # Expected: active
# If prod uses a non-default RPC port, add --node tcp://127.0.0.1:PORT below
$JUNOD status --home "$PROD_HOME" 2>/dev/null | jq -r '.sync_info.latest_block_height' \
  | tee /tmp/aegis_prod_height_before.txt
sha256sum "$PROD_HOME/config/priv_validator_key.json" \
  | tee /tmp/aegis_prod_pvkey_before.txt
```

Deterministic outcome:
- `is-active` prints `active`.
- A block height is written to `/tmp/aegis_prod_height_before.txt` (non-empty integer).
- A checksum is written to `/tmp/aegis_prod_pvkey_before.txt`.

If any of these are empty, **STOP** — do not continue.

---

## STEP 1 — Stop the production validator

```bash
sudo systemctl stop "$SVC"
sleep 5
sudo systemctl is-active "$SVC"        # Expected: inactive  (exit code 3)
pgrep -af "$JUNOD" || echo "NO JUNOD PROCESS RUNNING"
```

Deterministic outcome:
- `is-active` prints `inactive`.
- `pgrep` prints `NO JUNOD PROCESS RUNNING` (no stray process holds the prod home).

---

## STEP 2 — Scaffold the 4-validator localnet (isolated home + ports)

```bash
rm -rf "$LOCAL_HOME_BASE"
mkdir -p "$LOCAL_HOME_BASE"

CHAIN_ID="aegis-localnet-1"
for i in 0 1 2 3; do
  H="$LOCAL_HOME_BASE/n$i"
  $JUNOD init "node$i" --chain-id "$CHAIN_ID" --home "$H" >/dev/null 2>&1
done

# Detect the actual bond denom from genesis (Juno binary uses ujuno, not ustake)
DENOM=$(jq -r '.app_state.staking.params.bond_denom' \
  "$LOCAL_HOME_BASE/n0/config/genesis.json")
echo "Bond denom: $DENOM"   # Expected: ujuno
[ -z "$DENOM" ] && { echo "FATAL: could not read bond_denom"; exit 1; }
```

Deterministic outcome:
- Four dirs `n0..n3` exist, each with `config/genesis.json` and a freshly generated
  `config/priv_validator_key.json` (NONE equal to the production key).

Verify isolation:

```bash
for i in 0 1 2 3; do
  diff -q "$LOCAL_HOME_BASE/n$i/config/priv_validator_key.json" \
          "$PROD_HOME/config/priv_validator_key.json" \
    && echo "FATAL: n$i key equals prod key" || echo "n$i key OK (differs from prod)";
done
```

Expected: four lines `n0 key OK` .. `n3 key OK`. Any `FATAL` line -> **STOP**.

---

## STEP 3 — Build genesis with 4 validators

```bash
CHAIN_ID="aegis-localnet-1"
# DENOM was auto-detected in STEP 2; do not hardcode here
# 3a. add a funded account + gentx per node into n0's genesis
for i in 0 1 2 3; do
  H="$LOCAL_HOME_BASE/n$i"
  KEY="val$i"
  $JUNOD keys add "$KEY" --keyring-backend test --home "$H" >/dev/null 2>&1
  ADDR=$($JUNOD keys show "$KEY" -a --keyring-backend test --home "$H")
  $JUNOD genesis add-genesis-account "$ADDR" 1000000000${DENOM} \
    --home "$LOCAL_HOME_BASE/n0" >/dev/null 2>&1
  # mirror the account into each node's own genesis for gentx signing
  # skip self-copy for i=0 (cp to itself errors silently on Linux)
  [ "$i" -ne 0 ] && cp "$LOCAL_HOME_BASE/n0/config/genesis.json" "$H/config/genesis.json"
  $JUNOD genesis gentx "$KEY" 500000000${DENOM} --chain-id "$CHAIN_ID" \
    --keyring-backend test --home "$H" >/dev/null 2>&1
done

# 3b. collect all gentxs on n0
mkdir -p "$LOCAL_HOME_BASE/n0/config/gentx"
for i in 1 2 3; do cp "$LOCAL_HOME_BASE/n$i"/config/gentx/*.json \
  "$LOCAL_HOME_BASE/n0/config/gentx/"; done
$JUNOD genesis collect-gentxs --home "$LOCAL_HOME_BASE/n0" >/dev/null 2>&1
$JUNOD genesis validate-genesis --home "$LOCAL_HOME_BASE/n0"

# 3c. distribute final genesis to all nodes
for i in 1 2 3; do cp "$LOCAL_HOME_BASE/n0/config/genesis.json" \
  "$LOCAL_HOME_BASE/n$i/config/genesis.json"; done
```

Deterministic outcome:
- `validate-genesis` prints `genesis file is valid`.
- Final `genesis.json` lists exactly 4 `gen_txs`.

---

## STEP 4 — Wire ports + peers (+100 offset, no prod collision)

```bash
# SDK v0.50.x uses 'comet' subcommand; fall back to 'tendermint' for older builds
get_id () { $JUNOD comet show-node-id --home "$1" 2>/dev/null \
  || $JUNOD tendermint show-node-id --home "$1"; }
ID0=$(get_id "$LOCAL_HOME_BASE/n0")

# n0:26756 p2p / 26757 rpc, n1:26856/26857, n2:26956/26957, n3:27056/27057
# gRPC: n0:9090, n1:9100, n2:9110, n3:9120  API: n0:1317, n1:1327, n2:1337, n3:1347
for i in 0 1 2 3; do
  H="$LOCAL_HOME_BASE/n$i"
  P2P=$((26756 + i*100)); RPC=$((26757 + i*100))
  GRPC=$((9090 + i*10)); API=$((1317 + i*10))
  # Use [^"]* to match any IP — safe even if init generates 127.0.0.1 or 0.0.0.0
  sed -i "s|laddr = \"tcp://[^\"]*:26656\"|laddr = \"tcp://0.0.0.0:$P2P\"|" "$H/config/config.toml"
  sed -i "s|laddr = \"tcp://[^\"]*:26657\"|laddr = \"tcp://0.0.0.0:$RPC\"|" "$H/config/config.toml"
  sed -i 's|^addr_book_strict = true|addr_book_strict = false|' "$H/config/config.toml"
  # REQUIRED for localhost localnet: without this CometBFT rejects all but the
  # first inbound peer from 127.0.0.1, leaving n0 with 1 peer and no quorum
  sed -i 's|^allow_duplicate_ip = false|allow_duplicate_ip = true|' "$H/config/config.toml"
  if [ "$i" -ne 0 ]; then
    sed -i "s|^persistent_peers = \"\"|persistent_peers = \"$ID0@127.0.0.1:26756\"|" "$H/config/config.toml"
  fi
  # Offset gRPC and API ports in app.toml to avoid inter-node collisions
  sed -i "s|0\.0\.0\.0:9090|0.0.0.0:$GRPC|" "$H/config/app.toml"
  sed -i "s|tcp://0\.0\.0\.0:1317|tcp://0.0.0.0:$API|" "$H/config/app.toml"
done

# Verify port substitution worked on n0 (spot-check)
grep -E '26756|26757' "$LOCAL_HOME_BASE/n0/config/config.toml" | head -4
# Expected: two lines showing :26756 (p2p) and :26757 (rpc). If empty, sed didn't match — STOP.
```

Deterministic outcome:
- Each node's `config.toml` has a unique p2p/rpc port (no `26656/26657`, so **no collision**
  with the production node's default ports).

---

## STEP 5 — Start the 4 nodes (background, logged)

```bash
for i in 0 1 2 3; do
  H="$LOCAL_HOME_BASE/n$i"
  # ports are already patched in config.toml; no --p2p.laddr override needed
  nohup $JUNOD start --home "$H" > "$H/node.log" 2>&1 &
done

# Poll until n0 reaches height ≥ 2 (consensus live) or timeout after 60s
for _ in $(seq 30); do
  BLK0=$(curl -s "http://127.0.0.1:26757/status" \
    | jq -r '.result.sync_info.latest_block_height' 2>/dev/null)
  [ "${BLK0:-0}" -ge 2 ] 2>/dev/null && echo "Consensus live at height $BLK0" && break
  sleep 2
done

# Final heights on all 4 nodes
for i in 0 1 2 3; do
  RPC=$((26757 + i*100))
  echo -n "n$i height: "
  curl -s "http://127.0.0.1:$RPC/status" | jq -r '.result.sync_info.latest_block_height'
done
```

Deterministic outcome:
- Four heights printed, all > 1 and **advancing** when re-run after 10s
  (consensus is live across the 4 validators).

---

## STEP 6 — Measure bandwidth (the actual experiment)

```bash
# BLK = block to inspect (use a settled height, not 1)
RPC0=26757
BLK=10
mkdir -p "$ARTIFACT_DIR"

# Block summary
curl -s "http://127.0.0.1:$RPC0/block?height=$BLK" \
  | jq '{height: .result.block.header.height,
         num_txs: (.result.block.data.txs | length),
         num_sigs: (.result.block.last_commit.signatures | length)}' \
  | tee "$ARTIFACT_DIR/block-$BLK.json"

# Raw commit byte size
COMMIT_BYTES=$(curl -s "http://127.0.0.1:$RPC0/commit?height=$BLK" | wc -c)
echo "{\"commit_bytes\": $COMMIT_BYTES}" | tee "$ARTIFACT_DIR/commit-bytes-$BLK.json"

# Per-peer p2p byte counters (CometBFT net_info)
curl -s "http://127.0.0.1:$RPC0/net_info" \
  | jq '[.result.peers[] | {id: .node_info.id,
         recv_bytes: .connection_status.RecvMonitor.Bytes,
         send_bytes: .connection_status.SendMonitor.Bytes}]' \
  | tee "$ARTIFACT_DIR/net-info.json"

# Combine into single artifact
jq -s '{block: .[0], commit_bytes: .[1].commit_bytes, peers: .[2]}' \
  "$ARTIFACT_DIR/block-$BLK.json" \
  "$ARTIFACT_DIR/commit-bytes-$BLK.json" \
  "$ARTIFACT_DIR/net-info.json" \
  > "$ARTIFACT_DIR/localnet-bandwidth-N4.json"
echo "Artifact: $ARTIFACT_DIR/localnet-bandwidth-N4.json"
```

Record the numbers and compare against the `aegis-bench` model
(Hybrid-44 prediction: ~248.4 KB/block at N=100; scale down for N=4).

Deterministic outcome:
- `$ARTIFACT_DIR/localnet-bandwidth-N4.json` contains block summary, commit byte count, and per-peer send/recv counters.
- `num_sigs` should be 4 (all validators signed).

---

## STEP 7 — Tear down the localnet

```bash
# Save logs before removing (skip if ARTIFACT_DIR already set above)
mkdir -p "$ARTIFACT_DIR"
for i in 0 1 2 3; do
  cp "$LOCAL_HOME_BASE/n$i/node.log" "$ARTIFACT_DIR/n$i-node.log" 2>/dev/null || true
done
echo "Logs saved to $ARTIFACT_DIR"

pkill -f "$LOCAL_HOME_BASE/n" || true
sleep 3
pgrep -af "$LOCAL_HOME_BASE" && echo 'WARNING: processes still running' \
  || echo 'ALL LOCALNET NODES STOPPED'
rm -rf "$LOCAL_HOME_BASE"
```

Deterministic outcome:
- `pgrep` prints `ALL LOCALNET NODES STOPPED`.

---

## STEP 8 — Restart the production validator + verify identical state

```bash
# prod key must be byte-identical to the baseline (localnet never touched it)
sha256sum "$PROD_HOME/config/priv_validator_key.json" \
  | tee /tmp/aegis_prod_pvkey_after.txt
diff /tmp/aegis_prod_pvkey_before.txt /tmp/aegis_prod_pvkey_after.txt \
  && echo "PROD KEY UNCHANGED" \
  || { echo "FATAL: PROD KEY CHANGED — DO NOT START SERVICE; investigate immediately"; exit 1; }

# Key is confirmed unchanged; safe to restart
sudo systemctl start "$SVC"
sleep 10
sudo systemctl is-active "$SVC"        # Expected: active
# Add --node tcp://127.0.0.1:PORT if prod uses a non-default RPC port
$JUNOD status --home "$PROD_HOME" 2>/dev/null | jq -r '.sync_info.latest_block_height' \
  | tee /tmp/aegis_prod_height_after.txt
$JUNOD status --home "$PROD_HOME" 2>/dev/null | jq -r '.sync_info.catching_up'
# Re-run the last two commands every 30s until catching_up = false
```

Deterministic outcome:
- `PROD KEY UNCHANGED` (checksums match). A `FATAL` line means the prod home was touched — **investigate before trusting the validator**.
- `is-active` prints `active`.
- Height in `_after.txt` is **>=** the baseline height and rising.
- `catching_up` becomes `false` once resynced.

---

## Rollback (if anything goes wrong)

```bash
pkill -f "$LOCAL_HOME_BASE/n" || true     # kill any localnet process
sudo systemctl start "$SVC"               # bring prod back immediately
sudo systemctl status "$SVC" --no-pager
```

The production data dir is never written by any localnet step, so restart is always safe.
