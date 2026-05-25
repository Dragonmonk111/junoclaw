# ADR-003 — IBC Task Relay

*Status: **Proposed (v2 scope)**. Created 2026-05-18.*

*Decision owner: Dragonmonk111 (JunoClaw maintainer). Pending review by Juno core (Jake) and any external relayer operator who would carry the channel.*

## Context

JunoClaw v1 is a **single-chain protocol** on Juno. A DAO posts a task, an agent on Juno claims it, settles via Juno-native escrow. This is the right starting scope — every cross-chain feature adds attack surface, and v1 needs to ship before v30/v31 to be useful.

But the agent universe is **multi-chain** by default:

- An agent running on Osmosis (large compute pool, high TPS) wants to claim a task posted on Juno
- A DAO on Stargaze wants to escrow JUNO bounties without bridging assets to Juno first
- A WAVS operator network on Babylon wants to attest proofs that settle on Juno

x402 solves this by being a centralized HTTP intermediary. JunoClaw needs a sovereign answer: **IBC**.

## Decision

Build `crates/junoclaw-ibc-relay` (Rust, off-chain) and `contracts/ibc-task-host` (CosmWasm, on-chain) for a **v2 release**, post-v31 mainnet deploy. Out of scope for v1 / v30 / v31.

The relay uses **ICS-20 token transfers + packet-forward-middleware (PFM)** rather than custom IBC channels. This piggy-backs on the most-deployed IBC primitive (every Cosmos chain ships ICS-20) and avoids needing relayer operators to spin up bespoke channels for JunoClaw.

## Architecture

```
[Osmosis agent]                                    [Juno chain]
      │                                                  │
      │  1. AcceptTask via ICS-20 + JSON memo            │
      ├──────────────────────────────────────────────────┤
      │  ICS-20 transfer (memo: junoclaw-accept-task-v1) │
      │                                                  │
      │                                            [pfm middleware]
      │                                                  │
      │                                            [ibc-task-host]
      │                                                  │
      │                                            [task-ledger]
      │                                                  │
      │  2. SubmitProof via ICS-20 + JSON memo           │
      ├──────────────────────────────────────────────────┤
      │  ICS-20 transfer (memo: junoclaw-submit-proof-v1)│
      │                                                  │
      │                                            [zk-verifier]
      │                                                  │
      │                                            [escrow.Settle]
      │                                                  │
      │  3. Settlement: ICS-20 reverse transfer           │
      │←─────────────────────────────────────────────────┤
      │  Reward arrives on Osmosis as IBC voucher        │
```

### Key design decisions

**1. ICS-20 + memo as transport.** The agent sends a 1-token ICS-20 transfer with a structured JSON memo. The memo carries the JunoClaw operation. PFM forwards the packet to `ibc-task-host`, which decodes the memo and calls `task-ledger`. This requires zero custom IBC client work.

**2. `ibc-task-host` is a thin shim.** It validates the memo schema, looks up the IBC channel mapping, and translates to native `task-ledger` / `escrow` calls. Its only privilege is "can call task-ledger on behalf of foreign-chain agents" — same security profile as the WAVS bridge in v1.

**3. Escrow stays on Juno.** Funds for tasks are escrowed in JUNO (or whatever denom the task specifies). Cross-chain settlement is a reverse ICS-20 transfer that becomes an IBC voucher on the agent's home chain. Agents accept this trade-off: faster claim, IBC voucher tail risk.

**4. Proof submission crosses chains.** The agent's proof bytes go in the ICS-20 memo. `ibc-task-host` calls `zk-verifier::VerifyProof` as a sub-message. If verification fails, the IBC packet acknowledgment is `Err`, and the relayer returns the funds.

**5. Light-client security.** No multisig bridges. No centralized relayer. The relayer process is permissionless — anyone can run it. Security rests on IBC's standard light-client model (Tendermint headers, validator set verification).

## Wire format

The memo is JSON, versioned, namespaced under `junoclaw/`:

```json
{
  "wasm": {
    "contract": "juno1...ibc-task-host",
    "msg": {
      "junoclaw_v1": {
        "accept_task": {
          "task_id": 42,
          "agent_addr": "juno1...",
          "agent_origin_chain": "osmosis-1",
          "agent_origin_addr": "osmo1..."
        }
      }
    }
  }
}
```

Three operations supported in v2.0:

- **`accept_task`** — agent registers as the worker for an open task
- **`submit_proof`** — agent submits Groth16 proof; triggers `zk-verifier`
- **`reclaim_expired`** — DAO reclaims escrow on tasks past deadline

Subsequent operations (e.g. `partial_completion`, `dispute`) are deferred.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| **PFM not on agent's chain** | Document supported chains. Initially: Osmosis, Stargaze, Neutron (all ship PFM). |
| **Relay operator censorship** | Multiple relayers permitted. Light-client doesn't trust relayers. |
| **Memo size limit** | ZK proofs (~500 bytes) fit in standard memo. If larger circuits land, add IPFS-CID indirection. |
| **Time skew (proof expires before relay)** | `task-ledger` deadline includes a 24-hour grace window for IBC-relayed proofs. |
| **Replay across chains** | Same nonce-store pattern as x402 gateway. Per-source-chain partitioned. |
| **IBC channel goes stale** | `ibc-task-host` admin can rotate channel mapping via governance. |

## Out of scope for v2

- **Atomic cross-chain task posting** — DAO on Osmosis posting a Juno task. Requires bidirectional channel; deferred to v3.
- **Multi-asset escrow** — only the chain that hosts `escrow` provides the denom. Cross-chain swap inside the protocol is out of scope.
- **TEE-attested proofs** — TEE quote verification stays Juno-native. WAVS operators on other chains relay proofs in.

## Dependencies

- IBC v8+ (Juno v25 onwards has this)
- packet-forward-middleware on both chains
- Standard relayer (`hermes` or `go-relayer`) running the channel
- Juno v31 (BN254 precompile) — IBC-relayed proofs are far more practical when verification is ~200k gas instead of 370k

## Forward-looking integrations

- **Nostr signaling layer** (ADR-004) for task discovery before IBC acceptance
- **WAVS-on-Babylon** for cross-chain TEE attestation
- **x402 gateway** can be extended to forward EVM agents through this same IBC path (agent signs EVM, gateway translates to ICS-20 packet)

## Decision log

- 2026-05-18: ADR proposed as v2 scope, blocked on v31 mainnet
- (future) Implementation begins after v31 lands
- (future) First testnet deploy: Osmosis testnet ↔ uni-7

## References

- [ICS-20 spec](https://github.com/cosmos/ibc/blob/main/spec/app/ics-020-fungible-token-transfer/README.md)
- [PFM source](https://github.com/cosmos/ibc-apps/tree/main/middleware/packet-forward-middleware)
- `docs/ADR-002-X402-COMPOSITION.md` — sister ADR for HTTP gateway
- `docs/SOVEREIGN_AGENT_PROTOCOL.md` — strategic positioning
