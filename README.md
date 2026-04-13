# JunoClaw

Open-source agentic AI platform built on Juno Network.

## Architecture

- **On-chain (Juno V29.1 CosmWasm)**: Agent identity, task ledger, escrow — the trust layer
- **Off-chain (Rust daemon)**: LLM calls, tool execution, agent runtimes — the intelligence layer
- **Two-tier compute**: Local (fast/free) → Akash (GPU) + optional WAVS verification (TEE attested)

## Quick Start

```bash
cargo install junoclaw-cli
junoclaw init
junoclaw start
# Open http://localhost:7777
```

## Project Structure

```
junoclaw/
├── crates/
│   ├── junoclaw-core/         # Plugin trait, config, types
│   ├── junoclaw-runtime/      # Agent execution engine
│   ├── junoclaw-cli/          # CLI tool (init, start, agent)
│   └── junoclaw-daemon/       # Axum HTTP + WebSocket server
├── plugins/
│   ├── plugin-llm/            # LLM providers (Ollama, Anthropic, OpenAI)
│   ├── plugin-compute-local/
│   ├── plugin-compute-akash/
│   ├── plugin-storage-local/
│   ├── plugin-shell/
│   ├── plugin-ibc/
│   └── plugin-browser/
├── contracts/                 # CosmWasm workspace (10 crates, 118+ tests)
│   ├── junoclaw-common/       # Shared types (TaskRecord, PaymentObligation, etc.)
│   ├── agent-registry/        # On-chain agent identity + trust scores
│   ├── agent-company/         # DAO governance, payments, sortition, attestations
│   ├── task-ledger/           # Immutable task log with cross-contract callbacks
│   ├── escrow/                # Non-custodial payment obligations
│   ├── faucet/                # Testnet JUNOX faucet (one claim per address)
│   ├── builder-grant/         # TEE-verified ecosystem grants
│   ├── junoswap-factory/      # Junoswap v2 AMM pair factory
│   ├── junoswap-pair/         # Junoswap v2 AMM pair (constant product)
│   └── zk-verifier/           # BN254 Groth16 PoC (precompile case study)
├── wavs/                      # WAVS operator + bridge scripts
│   └── bridge/                # CosmJS deployment + attestation tools
├── docs/                      # Strategy docs, articles, analyses
│   └── BN254_PRECOMPILE_CASE.md
├── frontend/                  # React + Vite + Tailwind dashboard
└── deploy/                    # Deployment configs
```

## Key Integrations

- **Akash Network**: Decentralised GPU compute (one-click via Skip Protocol swap)
- **WAVS (Layer.xyz)**: Verifiable off-chain execution with TEE support
- **TrustGraph**: Verifiable reputation via WAVS
- **Skip Protocol**: One-click JUNO/USDC → AKT payment routing

## License

Apache-2.0
