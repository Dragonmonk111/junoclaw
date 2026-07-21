# JunoClaw WAVS Component

Off-chain verification component for JunoClaw agentic DAOs, running inside the WAVS runtime (optionally in a TEE enclave).

## Architecture

```
On-chain Event (agent-company contract on uni-7)
  → WAVS Trigger (wasm-wavs_push / wasm-sortition_request / wasm-outcome_create)
    → WASI Component (wavs/src/)
      → Verification + Attestation Hash
        → WAVS Aggregator
          → Bridge Daemon (wavs/bridge/)
            → SubmitAttestation / SubmitRandomness tx → agent-company contract
```

## Workflows

| Workflow | Trigger Event | Task Type | On-chain Callback |
|----------|--------------|-----------|-------------------|
| wavs-push-verify | `wasm-wavs_push` | `data_verify` | `SubmitAttestation` |
| sortition-randomness | `wasm-sortition_request` | `drand_randomness` | `SubmitRandomness` |
| outcome-verify | `wasm-outcome_create` | `outcome_verify` | `SubmitAttestation` |
| sign-request | `wasm-sign_request` | `store_signed_tx` | `StoreSignedTx` |

### `data_verify` SSRF defenses (Ffern H-3)

The `data_verify` workflow fetches arbitrary URLs supplied by agents. Since the bridge runs with operator credentials and network access, every outbound HTTP is routed through `bridge/src/utils/ssrf-guard.ts` which enforces:

- **Scheme allowlist** — `http` and `https` only. `file://`, `ftp://`, `gopher://`, etc. are rejected.
- **Port allowlist** — `80` and `443` by default. Databases (Redis `6379`, Postgres `5432`), admin RPCs (Cosmos `26657`), SSH `22`, and the rest are blocked.
- **DNS private-IP block** — every resolved address is checked against IPv4 private ranges (`10/8`, `172.16/12`, `192.168/16`, `127/8`, `169.254/16`, CGNAT `100.64/10`, multicast, benchmark, TEST-NETs) and IPv6 private ranges (`::1`, `fc00::/7`, `fe80::/10`, `ff00::/8`, plus IPv4-mapped forms). A request is blocked if **any** resolved address is private.
- **Timeout** — 5 seconds per request via `AbortController`.
- **Body cap** — 1 MiB, enforced via streaming read with mid-stream abort.

Deployment overrides (comma-separated env vars):

- `JUNOCLAW_SSRF_ALLOWED_SCHEMES` — default `http,https`
- `JUNOCLAW_SSRF_ALLOWED_PORTS` — default `80,443`
- `JUNOCLAW_SSRF_TIMEOUT_MS` — default `5000`
- `JUNOCLAW_SSRF_MAX_BYTES` — default `1048576` (1 MiB)

Run `npm run ssrf-guard-test` from `bridge/` to exercise the defenses against a battery of SSRF payloads (AWS/GCP metadata endpoints, RFC 1918 ranges, IPv6 literals, IPv4-mapped IPv6, DNS rebinding via mock resolver).

## Prerequisites

- Rust (nightly recommended)
- `cargo-component` (`cargo install cargo-component`)
- `wasm-tools` (`cargo install wasm-tools`)
- `wkg` (`cargo install wkg`)
- WASI target: `rustup target add wasm32-wasip2`
- Node.js 20+ (for bridge daemon)

## Build

```bash
# Configure WIT registry
wkg config --default-registry wa.dev

# Build the WASI component
# IMPORTANT: pass --target explicitly. cargo-component 0.21.x ignores the
# `target` key in .cargo/config.toml and defaults to wasm32-wasip1, whose
# preview1-compat adapter drags in unused wasi:filesystem imports. WAVS
# service.json sets `file_system: false`, so a wasip1-targeted build can
# mismatch the declared permissions even though the code never touches the
# filesystem. Always build with wasm32-wasip2 explicitly:
cargo component build --release --target wasm32-wasip2
# Output: target/wasm32-wasip2/release/junoclaw_wavs_component.wasm

# Install bridge dependencies
cd bridge && npm install
```

## Deployment

1. **Deploy agent-company contract** on uni-7 with `task_ledger` set to the bridge operator address
2. **Copy .env.example to .env** and fill in `AGENT_COMPANY_CONTRACT` and `WAVS_OPERATOR_MNEMONIC`
3. **Register the WASI component** with the WAVS operator network
4. **Start the bridge daemon**: `cd bridge && npm run bridge`

## Bridge CLI Tools

```bash
# Submit attestation manually
cd bridge
npx tsx src/submit-attestation.ts <proposal_id> <task_type> <data_hash> <attestation_hash>

# Submit randomness manually
npx tsx src/submit-randomness.ts <job_id> <randomness_hex_64chars> <attestation_hash>
```

## Configuration

- **wavs.toml** — WAVS runtime chain connection config (Juno testnet uni-7)
- **service.json** — Service manifest defining workflows, triggers, and submission targets
- **.env** — Operator wallet mnemonic, contract addresses, and API keys
- **bridge/** — TypeScript bridge daemon using CosmJS

## TEE Mode

When running WAVS inside a TEE (SGX/TDX/Nitro), the attestation is hardware-backed.
The WASI component code is identical — TEE attestation wraps the operator signature automatically.
Set `use_tee = true` in the daemon's `config.toml` `[wavs]` section.
