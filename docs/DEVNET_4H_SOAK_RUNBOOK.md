# Devnet ≥4-hour stability soak runbook

Last updated: 2026-06-28

## Purpose

Run the single-validator BN254 devnet (`junoclaw-bn254-devnet`, chain `junoclaw-bn254-1`)
for **≥4 hours (or overnight)** to close the last open item in `docs/COMPLETION_PLAN.md` §6:
prove the devnet does **not** stall over a long run. The historical stall was caused by the
**WSL2 clock freezing / jumping backward on Windows host sleep/resume** — CometBFT needs a
monotonically-advancing clock. So the soak is really a test of: *does the clock stay monotonic
for 4h+ while the devnet runs?*

> **Pass criterion:** block height strictly increases across every sample for the whole run,
> and `catching_up` stays `false`. Any sample where height did not advance vs the previous
> sample = a stall = FAIL (capture the container logs around that timestamp).

## Where things run (read first)

| Component | Host | How it runs |
|-----------|------|-------------|
| **Devnet** (this soak) | Windows host → **WSL2** → Docker | `junoclaw-bn254-devnet` container, RPC `localhost:26657` |
| **Production validator** | Separate **VirtualBox VM** (VairagyaNodes ext4 host) | **MANUAL** process: `junod start --home /home/dragonmonk111/.juno` (NOT systemd) |

**Key point:** the devnet and the production validator are normally on **different machines/VMs**,
so the devnet soak does **not** require stopping the production validator. STEP A below is therefore
**OPTIONAL** — do it only if (a) you are running the soak on the same box as prod and want to free
CPU/RAM, or (b) you simply want prod down overnight. If prod runs on its own always-on VM, **skip
STEP A and STEP F** and just run STEP 0–4 on the Windows/WSL2 side.

---

## STEP A (OPTIONAL) — Safely stop the production validator

> The prod validator is a **manual process**, not a systemd service, so use PID + checksum safety,
> not `systemctl`. Run these **on the production VM**.

```bash
PROD_HOME=/home/dragonmonk111/.juno
JUNOD=/usr/local/bin/junod

# A1. Record baseline height + the exact validator key checksum
$JUNOD status --home "$PROD_HOME" 2>/dev/null | jq -r '.sync_info.latest_block_height' \
  | tee /tmp/prod_height_before.txt
sha256sum "$PROD_HOME/config/priv_validator_key.json" | tee /tmp/prod_pvkey_before.txt

# A2. Find and gracefully stop the manual junod process
PID=$(pgrep -f "junod start --home $PROD_HOME")
echo "prod junod PID = ${PID:-<none>}"
[ -n "$PID" ] && kill -TERM "$PID"
sleep 8
pgrep -f "junod start --home $PROD_HOME" && echo "STILL RUNNING — investigate" \
  || echo "PROD VALIDATOR STOPPED"
```

Deterministic outcome:
- `/tmp/prod_height_before.txt` holds a non-empty integer.
- `/tmp/prod_pvkey_before.txt` holds a checksum.
- `PROD VALIDATOR STOPPED` is printed (no process holds the prod home).

> Double-sign safety: the devnet uses its **own** freshly-generated key in the container volume,
> never the prod `priv_validator_key.json`. Stopping prod is belt-and-suspenders, not required for
> key safety.

---

## STEP 0 — Prevent the Windows host from sleeping (THE root-cause fix)

Run in **PowerShell as Administrator** on the Windows host. This is the single most important
step — host sleep is what froze the WSL2 clock and stalled consensus in earlier runs.

```powershell
# Save current timeouts so you can restore them afterwards
powercfg /query SCHEME_CURRENT SUB_SLEEP | Select-String "Power Setting|Current AC" | Out-File $env:TEMP\powercfg_before.txt

# Disable standby + disk + hibernate on AC power for the soak
powercfg /change standby-timeout-ac 0
powercfg /change hibernate-timeout-ac 0
powercfg /change disk-timeout-ac 0
powercfg /change monitor-timeout-ac 10   # screen may still turn off; that's fine

# Confirm
powercfg /query SCHEME_CURRENT SUB_SLEEP | Select-String "Current AC Power Setting Index"
```

Deterministic outcome:
- Standby / hibernate / disk timeouts report `0x00000000` (never sleep) on AC.

> If you cannot run as admin, instead keep the machine awake with a tiny keep-alive
> (e.g. leave a media file playing, or run `powercfg /requests` to confirm nothing is forcing sleep).
> Laptops: **stay plugged into AC** for the whole soak.

---

## STEP 1 — Reset the WSL2 clock + bring the devnet up clean

Run in **PowerShell** (the `wsl.exe`/`docker` commands), then the soak loop in WSL bash.

```powershell
# 1a. Tear down any existing devnet + wipe its volume (fresh monotonic clock needs a clean start)
wsl.exe -e bash -lc "cd /mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet && docker compose -f docker-compose.yml down -v"

# 1b. Reset the WSL2 VM so its clock starts fresh + monotonic
wsl.exe --shutdown
Start-Sleep -Seconds 8

# 1c. Relaunch the devnet (idempotent helper; waits for height >= 2)
wsl.exe -e bash -lc "cd /mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw && ./devnet/scripts/run-devnet.sh"
```

Deterministic outcome:
- `run-devnet.sh` prints `Devnet is live.` and a starting height ≥ 2.

> Do **NOT** run `hwclock -s` or `date -s` inside the VM during the soak — Hyper-V time-sync reverts
> it within seconds, causing the clock to oscillate ±15 min (backward jumps) → consensus stalls.
> A constant offset from real wall-clock is harmless; only *monotonicity* matters.

---

## STEP 2 — Record the soak baseline

```powershell
wsl.exe -e bash -lc "curl -s http://localhost:26657/status | jq '{height: .result.sync_info.latest_block_height, catching_up: .result.sync_info.catching_up, time: .result.sync_info.latest_block_time}'"
```

Deterministic outcome:
- Prints a height, `catching_up: false`, and a recent block time. Note the start time.

---

## STEP 3 — Run the monitoring loop for ≥4 hours

This loop samples height + `catching_up` every 5 minutes, appends a line to a log, and **flags any
sample where the height did not advance** vs the previous sample. Run it in a **dedicated WSL bash
terminal** and leave it running (4 h = 48 samples; bump `SAMPLES` for overnight, e.g. 144 = 12 h).

```powershell
wsl.exe -e bash
```

Then inside WSL bash:

```bash
ART=/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet/soak
mkdir -p "$ART"
LOG="$ART/soak-$(date +%Y%m%d-%H%M%S).log"
INTERVAL=300          # seconds between samples (5 min)
SAMPLES=48            # 48 * 5 min = 4 h   (use 144 for 12 h overnight)

prev=0; stalls=0
echo "ts,height,catching_up,delta,verdict" | tee "$LOG"
for i in $(seq 1 "$SAMPLES"); do
  s=$(curl -s http://localhost:26657/status)
  h=$(echo "$s" | jq -r '.result.sync_info.latest_block_height' 2>/dev/null)
  cu=$(echo "$s" | jq -r '.result.sync_info.catching_up' 2>/dev/null)
  ts=$(date '+%Y-%m-%dT%H:%M:%S')
  if [ -z "$h" ] || [ "$h" = "null" ]; then
    echo "$ts,RPC_UNREACHABLE,,,FAIL" | tee -a "$LOG"; stalls=$((stalls+1))
  else
    d=$(( h - prev ))
    if [ "$prev" -ne 0 ] && [ "$d" -le 0 ]; then
      echo "$ts,$h,$cu,$d,STALL" | tee -a "$LOG"; stalls=$((stalls+1))
    else
      echo "$ts,$h,$cu,$d,OK" | tee -a "$LOG"
    fi
    prev=$h
  fi
  sleep "$INTERVAL"
done
echo "=== SOAK DONE: $stalls stall/unreachable sample(s) out of $SAMPLES ===" | tee -a "$LOG"
echo "Log: $LOG"
```

Deterministic outcome:
- The loop writes one CSV line every 5 min. Every advancing sample is `OK`.
- Final line reports the stall count. **`0 stall` = PASS.**

> You can tail the log live from another terminal:
> `wsl.exe -e bash -lc "tail -f '$LOG'"` (substitute the printed path).

---

## STEP 4 — Evaluate + capture the result

```bash
ART=/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet/soak
LOG=$(ls -t "$ART"/soak-*.log | head -1)

# Quick verdict
stalls=$(grep -c -E ',(STALL|FAIL)$' "$LOG" || true)
first=$(sed -n '2p' "$LOG"); last=$(grep -E ',(OK|STALL|FAIL)$' "$LOG" | tail -1)
echo "First: $first"; echo "Last:  $last"; echo "Stall/unreachable samples: $stalls"

# Save container logs for the run (helps post-mortem any stall)
docker logs junoclaw-bn254-devnet > "$ART/container-$(date +%Y%m%d-%H%M%S).log" 2>&1
echo "{\"stalls\": $stalls, \"log\": \"$LOG\"}" > "$ART/soak-result.json"
cat "$ART/soak-result.json"
```

Deterministic outcome:
- `Stall/unreachable samples: 0` over ≥48 samples (≥4 h) → **soak PASSED**; update
  `docs/COMPLETION_PLAN.md` §6 / `progress.txt` to mark the ≥4h soak done.
- If `stalls > 0`: open `container-*.log` around the stall timestamp. A backward `latest_block_time`
  jump confirms the clock-regression path → host slept / Hyper-V time-sync reverted; redo STEP 0.

---

## STEP 5 — Tear down the devnet (optional)

Leave it running if you want, or stop it:

```powershell
wsl.exe -e bash -lc "cd /mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/devnet && docker compose -f docker-compose.yml down"
```

> Use `down` (keep the volume) to preserve chain state, or `down -v` to wipe it.

---

## STEP F (only if you did STEP A) — Restore the production validator

Run **on the production VM**:

```bash
PROD_HOME=/home/dragonmonk111/.juno
JUNOD=/usr/local/bin/junod

# F1. Verify the prod key is byte-identical to the baseline (devnet never touched it)
sha256sum "$PROD_HOME/config/priv_validator_key.json" | tee /tmp/prod_pvkey_after.txt
diff /tmp/prod_pvkey_before.txt /tmp/prod_pvkey_after.txt \
  && echo "PROD KEY UNCHANGED — safe to restart" \
  || { echo "FATAL: PROD KEY CHANGED — DO NOT START; investigate"; exit 1; }

# F2. Restart the manual process exactly as before (use the same flags/home as your prod setup)
nohup $JUNOD start --home "$PROD_HOME" > "$PROD_HOME/junod.log" 2>&1 &
sleep 10

# F3. Confirm it is producing/syncing
$JUNOD status --home "$PROD_HOME" 2>/dev/null | jq '{height: .sync_info.latest_block_height, catching_up: .sync_info.catching_up}'
# Re-run every 30s until catching_up = false and height rises past /tmp/prod_height_before.txt
```

Deterministic outcome:
- `PROD KEY UNCHANGED` (checksums match).
- `catching_up` becomes `false`; height climbs past the baseline.

---

## STEP G (optional) — Restore Windows sleep settings

```powershell
# Restore your normal timeouts (example values — set to your prior preference)
powercfg /change standby-timeout-ac 30
powercfg /change hibernate-timeout-ac 180
powercfg /change disk-timeout-ac 20
```

---

## Rollback (if anything goes wrong)

- Devnet stuck/odd: `docker compose -f devnet/docker-compose.yml down -v` then re-run STEP 1.
- Prod must come back NOW: on the prod VM, re-run STEP F2 immediately (the prod home is never
  written by any devnet step, so a restart is always safe).
