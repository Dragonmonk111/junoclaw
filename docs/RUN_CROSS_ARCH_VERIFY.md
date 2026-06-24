# Cross-Arch Determinism Verify — Run Instructions

Proves `HybridPubKey.Verify` (secp256k1 + ML-DSA-44) produces identical results
on x86_64 and ARM64. Run all commands on the **VirtualBox Linux VM** unless noted.

---

## Step 0 — Clone junoclaw and confirm the program builds

```bash
cd ~
git clone https://github.com/Dragonmonk111/junoclaw.git aegis-verify-repo
cd aegis-verify-repo/aegis-accounts

# Quick build check (x86_64 native)
go build ./cmd/verify-artifact/
echo "Build OK: $?"
```

---

## Option A — QEMU emulation (available right now, no accounts needed)

### A1. Install QEMU user-mode + binfmt support

```bash
sudo apt-get update
sudo apt-get install -y qemu-user-static binfmt-support
update-binfmts --display qemu-aarch64 | grep -q enabled \
  && echo "binfmt OK" || echo "WARNING: binfmt not enabled"
```

### A2. Cross-compile for linux/arm64

```bash
cd ~/aegis-verify-repo/aegis-accounts
mkdir -p artifacts
GOOS=linux GOARCH=arm64 go build -o artifacts/verify-artifact-arm64 ./cmd/verify-artifact/
file artifacts/verify-artifact-arm64
# expected: ELF 64-bit LSB executable, ARM aarch64
```

### A3. Run native x86_64

```bash
cd ~/aegis-verify-repo/aegis-accounts
go run ./cmd/verify-artifact/ | tee artifacts/verify-amd64.json
```

### A4. Run ARM64 under QEMU

```bash
qemu-aarch64-static ./artifacts/verify-artifact-arm64 | tee artifacts/verify-arm64.json
```

### A5. Compare — diff MUST be empty

```bash
jq -S 'del(.goos,.goarch,.go_version)' artifacts/verify-amd64.json > /tmp/a.json
jq -S 'del(.goos,.goarch,.go_version)' artifacts/verify-arm64.json > /tmp/b.json
diff /tmp/a.json /tmp/b.json \
  && echo "CROSS-ARCH DETERMINISM CONFIRMED" \
  || { echo "FAIL — determinism broken"; cat /tmp/a.json /tmp/b.json; exit 1; }
```

**Expected output:**
```
CROSS-ARCH DETERMINISM CONFIRMED
```

**Expected JSON (both files, only goos/goarch/go_version differ):**
```json
{
  "goos": "linux",
  "goarch": "amd64",          // arm64 on the other file
  "go_version": "go1.24.x",
  "input_sha256": "<same hex on both>",
  "tampered_rejected": true,
  "verify_ok": true
}
```

---

## Option B — AWS t4g (real Graviton2 ARM64 silicon, ~$0.01 total)

### B0. Prerequisites

```bash
# Must have AWS CLI configured with at least ec2:RunInstances, ec2:DescribeInstances,
# ec2:TerminateInstances, ec2:CreateKeyPair permissions.
aws sts get-caller-identity   # confirm auth works

# Choose your region — us-east-1 has the lowest t4g cost
export AWS_DEFAULT_REGION=us-east-1
```

### B1. Find the Ubuntu 24.04 ARM64 AMI for your region

```bash
AMI=$(aws ec2 describe-images \
  --owners 099720109477 \
  --filters 'Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-arm64-server-*' \
            'Name=state,Values=available' \
  --query 'sort_by(Images,&CreationDate)[-1].ImageId' \
  --output text)
echo "ARM64 AMI: $AMI"
```

### B2. Create a one-off keypair (delete after)

```bash
aws ec2 create-key-pair --key-name aegis-arm-test \
  --query 'KeyMaterial' --output text > /tmp/aegis-arm-test.pem
chmod 600 /tmp/aegis-arm-test.pem
```

### B3. Launch the instance

```bash
AID=$(aws ec2 run-instances \
  --image-id "$AMI" \
  --instance-type t4g.micro \
  --key-name aegis-arm-test \
  --count 1 \
  --query 'Instances[0].InstanceId' \
  --output text)
echo "Instance: $AID"

# Wait for it to be running (~30s)
aws ec2 wait instance-running --instance-ids "$AID"

# Get public IP
IP=$(aws ec2 describe-instances --instance-ids "$AID" \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
echo "IP: $IP"
```

### B4. SSH in and run the test

```bash
# Wait for SSH to be ready (~30s after running)
sleep 35

ssh -i /tmp/aegis-arm-test.pem -o StrictHostKeyChecking=no ubuntu@$IP << 'REMOTE'
set -e
# Install Go 1.24
wget -q https://go.dev/dl/go1.24.4.linux-arm64.tar.gz
sudo tar -C /usr/local -xzf go1.24.4.linux-arm64.tar.gz
export PATH=/usr/local/go/bin:$PATH

# Install jq
sudo apt-get install -y -q jq git

# Clone and run
git clone --quiet https://github.com/Dragonmonk111/junoclaw.git
cd junoclaw/aegis-accounts
go run ./cmd/verify-artifact/
REMOTE
```

### B5. Capture the ARM64 result

```bash
ssh -i /tmp/aegis-arm-test.pem ubuntu@$IP \
  "cd junoclaw/aegis-accounts && PATH=/usr/local/go/bin:\$PATH go run ./cmd/verify-artifact/" \
  > artifacts/verify-arm64-aws.json
cat artifacts/verify-arm64-aws.json
```

### B6. Compare against x86_64

```bash
cd ~/aegis-verify-repo/aegis-accounts
go run ./cmd/verify-artifact/ > artifacts/verify-amd64.json

jq -S 'del(.goos,.goarch,.go_version)' artifacts/verify-amd64.json > /tmp/a.json
jq -S 'del(.goos,.goarch,.go_version)' artifacts/verify-arm64-aws.json > /tmp/b.json
diff /tmp/a.json /tmp/b.json \
  && echo "CROSS-ARCH DETERMINISM CONFIRMED — real Graviton2 silicon" \
  || { echo "FAIL"; exit 1; }
```

### B7. TERMINATE the instance immediately

```bash
aws ec2 terminate-instances --instance-ids "$AID"
aws ec2 wait instance-terminated --instance-ids "$AID"
echo "Instance terminated. Total cost: ~\$0.01"

# Clean up keypair
aws ec2 delete-key-pair --key-name aegis-arm-test
rm /tmp/aegis-arm-test.pem
```

---

## Step 6 — Commit both artifacts to the repo

```bash
cd ~/aegis-verify-repo
mkdir -p aegis-bench/artifacts
cp aegis-accounts/artifacts/verify-amd64.json aegis-bench/artifacts/
# Copy whichever ARM result you got:
cp aegis-accounts/artifacts/verify-arm64.json    aegis-bench/artifacts/ 2>/dev/null || true
cp aegis-accounts/artifacts/verify-arm64-aws.json aegis-bench/artifacts/ 2>/dev/null || true

git add aegis-bench/artifacts/
git commit -m "det-hash: cross-arch verify artifact — QEMU/Graviton2 determinism confirmed"
git push
```

---

## Troubleshooting

### `qemu-aarch64-static: not found`

```bash
sudo apt-get install -y qemu-user-static
# If binfmt is not auto-registered:
sudo update-binfmts --enable qemu-aarch64
```

### `go: module github.com/junoclaw/aegis-accounts: not found`

The `cmd/verify-artifact` is inside the `aegis-accounts` module — build from that directory:
```bash
cd ~/aegis-verify-repo/aegis-accounts   # NOT the repo root
go run ./cmd/verify-artifact/
```

### `verify_ok is false` on ARM

Means `circl` ML-DSA-44 verification returned false on ARM64 — genuine determinism bug.
Steps:
1. Capture both JSON files.
2. Open an issue on `cloudflare/circl` with the platform info.
3. File in `aegis-bench/artifacts/` as a FAIL artifact with explanation.

### AWS: `InvalidAMIID.NotFound`

The AMI query is region-specific. Re-run the B1 query with your actual region,
or use the [Ubuntu AMI finder](https://cloud-images.ubuntu.com/locator/ec2/)
and filter for `24.04 arm64 hvm ebs`.

---

## What this closes

| Checklist item | Status after this run |
|----------------|----------------------|
| Phase F determinism: cross-impl verify (circl vs fips204 Rust) | ✅ done (KAT) |
| Phase F determinism: cross-CPU/OS verify hash | ✅ **this run** |
| det-hash TODO item | → **completed** |
