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
cargo component build --release
# Output: target/wasm32-wasip1/release/junoclaw_wavs_component.wasm (352 KB)

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
