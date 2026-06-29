# Build Aegis Juno Binary — Runbook

Build the Juno v29 binary wired to the Project Aegis forks:
- **CometBFT fork** (`aegis-phase-cf-hybrid`): ML-KEM-768 hybrid transport
- **Cosmos SDK fork** (`aegis-phase-d3-hybrid`): secp256k1+ML-DSA-44 hybrid keys
- **IBC-Go fork** (`aegis-phase-g-hybrid-client`): 07-tendermint hybrid client (optional, now default in CI)

Run this on the **VirtualBox Linux VM** (dragonmonk111@dragonmonk111-VirtualBox).

---

## Prerequisites

### 1. Go 1.24+ (required — CometBFT fork uses stdlib crypto/mlkem)

```bash
go version 2>/dev/null || true
```

If below 1.24 or missing:

```bash
cd /tmp
wget https://go.dev/dl/go1.24.4.linux-amd64.tar.gz
sudo rm -rf /usr/local/go
sudo tar -C /usr/local -xzf go1.24.4.linux-amd64.tar.gz
echo 'export PATH=/usr/local/go/bin:$PATH' >> ~/.bashrc
export PATH=/usr/local/go/bin:$PATH
go version   # must print go1.24.x
```

### 2. Build tools

```bash
sudo apt-get install -y build-essential git gcc
```

### 3. Disk space — need ~3 GB free

```bash
df -h ~
```

---

## Step 0 — Set environment

```bash
export AEGIS_BUILD_DIR="$HOME/aegis-build"
export JUNO_TAG="v29.0.0"
export SDK_BRANCH="aegis-phase-d3-hybrid"
export CMT_BRANCH="aegis-phase-cf-hybrid"
export IBC_BRANCH="aegis-phase-g-hybrid-client"  # set blank to skip ibc-go and match the two-fork build
mkdir -p "$AEGIS_BUILD_DIR"
```

---

## Step 1 — Clone the Aegis forks

```bash
cd "$AEGIS_BUILD_DIR"

# CometBFT fork (ML-KEM-768 hybrid transport)
git clone --branch "$CMT_BRANCH" --depth 1 \
  https://github.com/Dragonmonk111/cometbft.git aegis-cometbft
echo "CometBFT tip: $(git -C aegis-cometbft rev-parse --short HEAD)"

# Cosmos SDK fork (secp256k1 + ML-DSA-44 hybrid keys)
git clone --branch "$SDK_BRANCH" --depth 1 \
  https://github.com/Dragonmonk111/cosmos-sdk.git aegis-sdk
echo "SDK tip: $(git -C aegis-sdk rev-parse --short HEAD)"

# IBC-Go fork (07-tendermint hybrid client) — optional, now default in CI
if [ -n "$IBC_BRANCH" ]; then
  git clone --branch "$IBC_BRANCH" --depth 1 \
    https://github.com/Dragonmonk111/ibc-go.git aegis-ibc-go
  echo "IBC-Go tip: $(git -C aegis-ibc-go rev-parse --short HEAD)"
fi
```

**Deterministic outcome:** All tips match the progress.txt records:
- CometBFT: `aegis-phase-cf-hybrid` (6 commits ahead of v0.38.x)
- SDK: `aegis-phase-d3-hybrid` tip `4e6f315`
- IBC-Go: `aegis-phase-g-hybrid-client` (when cloned)

---

## Step 2 — Clone Juno v29

```bash
cd "$AEGIS_BUILD_DIR"
git clone --branch "$JUNO_TAG" --depth 1 \
  https://github.com/CosmosContracts/juno.git aegis-juno
cd aegis-juno
echo "Juno tag: $(git describe --tags)"
```

---

## Step 3 — Add replace directives to Juno's go.mod

```bash
cd "$AEGIS_BUILD_DIR/aegis-juno"

# Backup original
cp go.mod go.mod.orig

# Check what versions Juno v29 pins
grep -E "cosmos-sdk|cometbft" go.mod | head -6
```

Now add the replace block. The exact existing require lines will be shown above.
Append to the end of `go.mod`. For the **three-fork build** (default in CI, includes IBC Phase G), use:

```bash
cat >> go.mod << 'EOF'

// Project Aegis: local fork overrides (ML-KEM transport + hybrid consensus keys + IBC hybrid client)
replace (
	github.com/cometbft/cometbft => ../aegis-cometbft
	github.com/cosmos/cosmos-sdk => ../aegis-sdk
	github.com/cosmos/ibc-go/v8 => ../aegis-ibc-go
)
EOF
```

> If you skipped the ibc-go fork, drop the `github.com/cosmos/ibc-go/v8` line and match the earlier
> two-fork build. The CI default is the three-fork build.

Verify:

```bash
tail -8 go.mod
```

---

## Step 4 — Tidy and verify the dependency graph

```bash
cd "$AEGIS_BUILD_DIR/aegis-juno"
export GOTOOLCHAIN=go1.24.4   # CometBFT fork requires 1.24

go mod tidy 2>&1 | tail -20
```

> **If `go mod tidy` fails:** The most likely cause is a new dependency in the
> CometBFT fork that isn't reflected in Juno's go.sum. Run:
> ```bash
> go mod download && go mod tidy
> ```
> If cloudflare/circl is missing (SDK fork dep): it will be auto-fetched.

---

## Step 5 — Build

```bash
cd "$AEGIS_BUILD_DIR/aegis-juno"
export GOTOOLCHAIN=go1.24.0

# NOTE: -checklinkname=0 is required for Go 1.24.
# bytedance/sonic/loader uses //go:linkname runtime.lastmoduledatap which
# Go 1.23+ blocks by default (-checklinkname=1). Disabling the check is safe
# here: it only affects sonic's JIT loader, not our PQC code paths.
go build -ldflags="-checklinkname=0" -o build/junod-aegis ./cmd/junod/
```

**Deterministic outcome:** `build/junod-aegis` binary produced, no errors.

> **Verified 2026-06-24:** 158 MB ELF 64-bit, sha256 `0809331d83aae0473ace982a7e9129e8358f4321f905ab4bc2ca49674b9586f1`

```bash
ls -lh build/junod-aegis
file build/junod-aegis   # ELF 64-bit LSB executable, x86-64
./build/junod-aegis version
```

---

## Step 6 — Smoke-test the binary

```bash
AEGIS_BIN="$AEGIS_BUILD_DIR/aegis-juno/build/junod-aegis"

# Confirm it links the Aegis forks
"$AEGIS_BIN" version --long 2>/dev/null | head -10

# Confirm hybrid transport env var is recognised (should not crash)
AEGIS_HYBRID_TRANSPORT=1 "$AEGIS_BIN" version 2>/dev/null | head -4
```

---

## Step 7 — Copy to a safe path

```bash
cp "$AEGIS_BUILD_DIR/aegis-juno/build/junod-aegis" \
   /home/dragonmonk111/aegis-localnet-artifacts/junod-aegis
chmod +x /home/dragonmonk111/aegis-localnet-artifacts/junod-aegis
echo "Binary saved: $(sha256sum /home/dragonmonk111/aegis-localnet-artifacts/junod-aegis)"
```

Record the sha256 in `progress.txt`.

---

## Step 8 — PQC localnet with the Aegis binary

Once the binary exists, re-run the localnet runbook
(`docs/VALIDATOR_SAFE_LOCALNET_RUNBOOK.md`) substituting:

```bash
export JUNOD=/home/dragonmonk111/aegis-localnet-artifacts/junod-aegis
export AEGIS_HYBRID_TRANSPORT=1   # enable ML-KEM-768 transport on all nodes
```

Then repeat the bandwidth measurement from Step 6 of the runbook.
The commit size should increase from **2,266 bytes** (classical) to
approximately **~14 KB** (Hybrid-44 at N=4), confirming the 248 KB/block
projection at N=100.

---

## Troubleshooting

### `go mod tidy` complains about incompatible go versions

The CometBFT fork bumped `go 1.22.11` → `go 1.24.0` to access stdlib
`crypto/mlkem`. Juno v29 may pin an earlier toolchain. Fix:

```bash
export GOTOOLCHAIN=go1.24.4
go mod tidy
```

Or add `toolchain go1.24.4` to the top of Juno's `go.mod` (after the `module` line).

### `replace` directive version mismatch

If Juno's `go mod tidy` rejects the local replace because the module version
doesn't match the pinned require version, run:

```bash
# Find what version Juno pins for cometbft
grep cometbft go.mod | head -4
# e.g. github.com/cometbft/cometbft v0.38.12

# The local replace overrides it regardless of version — this is expected.
# If tidy removes the replace, re-add it and run:
GOFLAGS=-mod=mod go build ./cmd/junod/
```

### cloudflare/circl not found

The SDK fork added `cloudflare/circl` as a direct dependency for ML-DSA-44.
`go mod tidy` should fetch it automatically. If network access is restricted:

```bash
go get github.com/cloudflare/circl@v1.6.1
go mod tidy
```

---

## What this unlocks

| Measurement | Requires Aegis binary |
|-------------|----------------------|
| C6: hybrid transport RTT (ML-KEM handshake latency) | ✅ |
| PQC localnet: Hybrid-44 bandwidth (actual, not modelled) | ✅ |
| D3 full wiring: proto/keyring/CLI in Juno | ✅ |
| Commit size vs 2,266B classical baseline | ✅ |

Once the binary is built and the PQC localnet runs, the 248 KB/block
projection becomes a measured result and the article can be posted to
technical forums (Juno governance, HackMD, ethresear.ch).
