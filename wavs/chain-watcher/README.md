# JunoClaw Chain Watcher

Persistent Node.js service that monitors Juno testnet (uni-7) events from the `agent-company` contract, runs verification workflows, and optionally submits attestations back on-chain.

## Architecture

```
Juno Testnet (uni-7)
    │
    ├── WebSocket (Tendermint /websocket)
    │       └─→ EventWatcher ─→ Verifier ─→ Attestor ─→ TX back to contract
    │
    └── REST API (polling fallback)
            └─→ EventWatcher (same pipeline)

FeedServer (ws://localhost:7778)
    └─→ Frontend UpdatesPanel
```

## Components

| Module | Role |
|--------|------|
| `event-watcher.ts` | Subscribes to Tendermint WS + polls REST for contract events |
| `verifier.ts` | Runs verification logic per event type (software mode) |
| `attestor.ts` | Submits `SubmitAttestation` TX back to agent-company |
| `feed-server.ts` | Broadcasts events/verifications to frontend via WebSocket |
| `index.ts` | Orchestrates the pipeline |

## Usage

```bash
# Install
npm install

# Run (read-only mode — no attestation submission)
npm start

# Run with attestation submission
OPERATOR_MNEMONIC="word1 word2 ... word24" npm start

# Dev mode (auto-restart on changes)
npm run dev
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `JUNO_RPC` | `https://juno-testnet-rpc.polkachu.com` | Juno RPC endpoint |
| `JUNO_WS` | `wss://juno-testnet-rpc.polkachu.com/websocket` | Tendermint WebSocket |
| `JUNO_REST` | `https://juno-testnet-api.polkachu.com` | Juno REST API |
| `AGENT_COMPANY` | `juno1k8dxll...` | Contract address to watch |
| `OPERATOR_MNEMONIC` | (empty) | Wallet mnemonic for attestation TX |
| `FEED_PORT` | `7778` | WebSocket feed port for frontend |
| `POLL_INTERVAL` | `6000` | Polling interval in ms |
| `LOG_LEVEL` | `info` | `debug` / `info` / `warn` / `error` |

## Watched Events

- `wasm-wavs_push` — WAVS task execution triggers
- `wasm-outcome_create` — Outcome market creation
- `wasm-sortition_request` — Random member selection
- `wasm-code_upgrade` — Contract upgrade proposals
- `wasm-execute_proposal` — Proposal execution
- `wasm-create_proposal` — New proposal creation
- `wasm-cast_vote` — Vote cast events

## Feed Protocol

Frontend connects to `ws://localhost:7778` and receives JSON messages:

```jsonc
// Chain event
{ "type": "chain_event", "timestamp": "...", "data": { "eventType": "wasm-wavs_push", "txHash": "...", "blockHeight": 12345, "attributes": {...} } }

// Verification result
{ "type": "verification", "timestamp": "...", "data": { "verified": true, "proposalId": 1, "taskType": "wavs_push", "dataHash": "...", "attestationHash": "..." } }

// Attestation TX submitted
{ "type": "attestation_tx", "timestamp": "...", "data": { "proposalId": 1, "txHash": "ABC123..." } }

// Status (sent on connect)
{ "type": "status", "timestamp": "...", "data": { "watcherStatus": "running", "connectedClients": 1, ... } }
```
