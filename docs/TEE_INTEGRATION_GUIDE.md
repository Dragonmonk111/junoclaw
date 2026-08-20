# TEE Integration Guide — SGX / SEV-SNP Attestation for JunoClaw

This document describes how to integrate Trusted Execution Environment (TEE) attestation into the JunoClaw trust stack.

## What TEE Does in JunoClaw

In Plan D (the aggregation pattern), the TEE verifies the three inner Groth16 proofs:

```
SensorSafety proof (128B) ──┐
IntentConsistency proof ────┤── TEE verifies 3 pairings ──┐
ConsensusMembership proof ──┘                              │
                                                           ├── On-chain:
Aggregation proof (128B) ────────────────────────────────┤── zk-verifier.VerifyProof(agg)
                                                           └── tee-verifier.VerifyAttestation(report)
```

The TEE does the pairing checks (fast, hardware-attested). The ZK circuit does the consistency checks (private, cheap). Together: one 128-byte proof + one TEE attestation report.

## Supported TEE Platforms

| Platform | Status | Attestation Format |
|----------|--------|-------------------|
| Intel SGX (DCAP) | Design complete | SGX DCAP quote (4KB) |
| AMD SEV-SNP | Design complete | SEV-SNP report (4KB) |
| ARM CCA | Future | Realm attestation |

## Architecture

### Without TEE (Current — Plan D with stub attestation)

The aggregation circuit produces a proof. The on-chain `tee-attestation-verifier` contract accepts a placeholder attestation. This is the current state — suitable for development and testing.

### With TEE (Production)

```
┌─────────────────────────────────┐
│       Prover Daemon (TEE)       │
│                                 │
│  1. Receive 3 Groth16 proofs    │
│  2. Verify each pairing (in TEE)│
│  3. If all valid:               │
│     - Generate attestation      │
│     - Sign attestation with     │
│       TEE report key            │
│  4. Output: attestation report  │
│                                 │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│  On-chain tee-verifier contract │
│                                 │
│  1. Verify TEE attestation      │
│     - Check quote signature     │
│     - Check MRENCLAVE hash      │
│     - Check report data         │
│  2. If valid: accept            │
│  3. If invalid: reject          │
│                                 │
└─────────────────────────────────┘
```

## Integration Steps

### Step 1: Build TEE-Enabled Prover

```bash
# For Intel SGX
cargo build --features sgx --release

# For AMD SEV-SNP
cargo build --features sev-snp --release
```

### Step 2: Configure TEE Attestation

```toml
# prover-config.toml
[tee]
platform = "sgx"  # or "sev-snp"
attestation_endpoint = "https://sgx-attestation.example.com"
mrenclave = "0x1234..."  # expected enclave measurement
sp_id = "..."  # service provider ID (for legacy EPID)
# For DCAP:
dcap_pccs_url = "https://pccs.example.com:8081/sgx/certification/v3/"
```

### Step 3: Deploy tee-attestation-verifier Contract

```bash
junoclay deploy tee-attestation-verifier \
  --code contracts/tee-verifier/artifacts/tee_verifier.wasm \
  --init '{"admin": "juno1...", "trusted_cpus": [...], "mrenclaves": [...]}'
```

### Step 4: Verify Attestation On-Chain

```bash
junoclay tx tee-verifier verify-attestation \
  --report attestation_report.bin \
  --robot-id warehouse-bot-01 \
  --from prover-key --yes
```

## TEE Attestation Report Format

```json
{
  "platform": "sgx",
  "quote": "base64-encoded SGX DCAP quote",
  "mrenclave": "hex-encoded enclave measurement",
  "mrsigner": "hex-encoded signer measurement",
  "report_data": "hex-encoded custom data (includes proof hashes)",
  "verification_result": true,
  "timestamp": 1724073600
}
```

## Trust Model

| Assumption | Mitigation |
|------------|------------|
| TEE hardware can be side-channel attacked | Rotate keys frequently; use hardened enclaves |
| TEE firmware can have bugs | Use attested firmware versions; pin to known-good versions |
| Physical tampering | Use hardware that detects tampering and wipes keys |
| CPU vendor backdoor | This is an irreducible assumption of TEE-based trust |

## Removing TEE Trust (Future)

See the "Removing the TEE" section in [Robot Scaling Ages](../articles/ROBOT_SCALING_AGES_2026_08_19.md) for five paths to eliminate the TEE trust assumption entirely:

1. Direct Composition (one big circuit, no TEE)
2. On-Chain Multi-Verify (3× VerifyProof, no TEE)
3. Nova Folding (no pairings in circuit)
4. PLONK Custom Gates (efficient pairing in circuit)
5. BLS12-377/BW6 2-Chain (full recursion)

## Current Status

The TEE integration is **designed but not implemented**. The current Plan D uses a stub attestation that the on-chain contract accepts without verification. This is safe for development and testing but must be replaced with real TEE attestation before production deployment to untrusted environments.

## Implementation Roadmap

1. **Q4 2026**: SGX DCAP attestation in prover daemon
2. **Q1 2027**: SEV-SNP attestation support
3. **Q2 2027**: On-chain tee-verifier contract with quote verification
4. **Q3 2027**: Path 2 (on-chain multi-verify) as no-TEE alternative
