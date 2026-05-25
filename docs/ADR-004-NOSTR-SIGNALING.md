# ADR-004 — Nostr Signaling Layer

*Status: **Proposed (v2 scope)**. Created 2026-05-18.*

*Decision owner: Dragonmonk111 (JunoClaw maintainer).*

## Context

JunoClaw v1 task discovery is **chain-bound**. To find an open task, an agent queries `task-ledger` over RPC. This works, but has problems:

1. **Liveness coupled to chain RPC.** Public RPC endpoints rate-limit; a busy agent network hammers them.
2. **No push notifications.** Agents poll, wasting bandwidth and CPU.
3. **Discovery is on-chain bytecode.** Anyone wanting to filter tasks needs CosmJS or similar; not all agent runtimes have this.
4. **Censorship surface.** If a single dominant frontend (e.g. DAO DAO web UI) becomes the standard task-listing surface, deplatforming becomes possible.

x402 solves discovery via HTTP (Coinbase facilitator hosts a directory). This is exactly the centralization JunoClaw rejects.

The sovereign answer is **Nostr** — a permissionless, relay-based publish-subscribe protocol that already has agent-friendly tooling, no required identity provider, and censorship-resistance as a design goal.

## Decision

Build `crates/junoclaw-nostr-bridge` (Rust, off-chain) for a **v2 release**. The bridge:

1. Watches `task-ledger` for new tasks (via Tendermint websocket subscription)
2. Emits a Nostr event of a custom kind to a configured set of relays
3. Listens for response events from agents (interest signals)
4. Optionally: relays agent-broadcast capabilities back to the chain

The bridge is **stateless infrastructure**, runnable by anyone — DAOs, agents, validators. Not a singleton. Multiple bridges produce duplicate Nostr events; relays de-duplicate by event ID.

## Architecture

```
[task-ledger emits event]              [agent on Nostr]
        │                                     │
        │  Tendermint websocket               │
        ▼                                     │
[junoclaw-nostr-bridge]                       │
        │                                     │
        │  publish kind:38402 event           │
        ▼                                     │
   [Nostr relays] ──────────────────────────► │
                                              │
                                       Agent receives,
                                       evaluates task,
                                       responds on-chain
                                       via x402 gateway,
                                       IBC, or native CosmJS
```

## Event kind specification

We propose **kind `38402`** (parametrized replaceable event in the experimental range, mnemonic-aligned with HTTP 402).

### Required tags

| Tag | Value | Purpose |
|---|---|---|
| `d` | `juno-1:juno1...:42` | Replaceable event identifier (chain:contract:task_id) |
| `chain` | `juno-1` | Cosmos chain ID |
| `contract` | `juno1...` | task-ledger contract address |
| `task` | `42` | task_id integer |
| `reward` | `1000000ujuno` | Reward amount (Cosmos sdk Coin format) |
| `deadline` | `1750000000` | Unix epoch seconds |
| `verifier` | `juno1...` | zk-verifier contract address |
| `vk_hash` | `sha256:abc...` | Verification key hash |
| `caps` | `compute,storage,llm` | Required agent capabilities |

### Content field

JSON-encoded task description (same shape as `task-ledger::Task` query response). Up to 64KB; relays may reject larger.

### Signing

Bridge instances sign with **Cosmos secp256k1 keys mapped into Nostr's secp256k1**. The Nostr `pubkey` field is the bridge operator's hex pubkey. Agents can verify the signing key matches a known bridge operator (e.g. cross-referenced from a public list maintained by the JunoClaw DAO).

## NIP submission

Once the kind is stabilised, submit a NIP (Nostr Implementation Proposal) titled **"NIP-XX: Verifiable Compute Task Discovery"**. This formalises the kind range and tag schema, allowing other chains (Cosmos and beyond) to reuse the kind for their own task-ledgers.

## Relay incentivisation

Nostr's economic model is unsolved. Most relays run on goodwill or paid subscriptions. JunoClaw can pioneer a relay-fee mechanism:

- Bridge operator stakes a small JUNO bond
- For each task event published, bridge can earmark a "relay tip" (e.g. 1000 ujuno)
- Relays that carried the event for 24 hours can claim a portion via a `relay-tip-redeem` contract on Juno
- Proof-of-relay = signed message from a known agent confirming "I received event X via relay Y"

This is a **secondary research stream**, not in v2 scope. v2 ships with goodwill-only relays.

## Agent flow

1. Agent connects to a configurable list of Nostr relays
2. Subscribes with filter: `{"kinds":[38402], "#chain":["juno-1"]}` (or any subset of chain/caps)
3. Receives task events as they're published by bridges
4. Evaluates: capabilities match, reward acceptable, deadline reachable
5. Responds **on-chain** via:
   - Native CosmJS if the agent has a Juno key
   - x402 gateway if the agent only has EVM keys
   - IBC relay (ADR-003) if the agent runs on a different Cosmos chain
6. Settlement is on-chain as in v1 — Nostr is **only** the discovery layer

## Identity binding

Agents may want to publish their own Nostr events (e.g. capability announcements, completion proofs). For these to be trustable, the agent's Nostr pubkey should be bindable to their on-chain identity:

```
Nostr pubkey: 02abc...
On-chain proof: cosmos signature over "junoclaw:nostr-binding-v1:02abc...:juno1..."
Published to: agent-registry as a metadata field
```

Agents who want trustless reputation across Nostr publish this binding once. Receivers verify the Cosmos signature before trusting any Nostr-emitted reputation claims.

## Censorship resistance

Nostr's strength: **any relay can carry any event**. If a major relay deplatforms JunoClaw events, agents reconfigure to use other relays. There is no single chokepoint.

Failure mode: if all major relays simultaneously refuse JunoClaw events. This is mitigated by:

- Multiple bridges run by independent operators
- Open-source bridge code so anyone can run their own
- Default relay list includes self-hosted, anonymous, and clearnet+onion relays

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| **Bridge sees event before chain commits** | Event includes `block_height`. Agents reject events for blocks they can't query. |
| **Spam events** | Bridges sign events. Agents whitelist known bridge pubkeys. |
| **Outdated events** | `deadline` tag. Relays / agents drop events past deadline. |
| **Relay inconsistency** | Subscribe to ≥3 relays. Use majority quorum for "task is open". |
| **NIP rejection** | Use the kind unilaterally. NIP is nice-to-have, not required. |

## Out of scope for v2

- **Agent-emitted reputation events** — agents publishing their own track record. Deferred to v3.
- **Encrypted task channels** — private tasks only visible to whitelisted agent Nostr pubkeys. Possible via NIP-44 encryption; deferred.
- **DM-based task negotiation** — agents and DAOs negotiating reward off-chain via Nostr DM. Deferred.

## Dependencies

- `nostr-sdk` Rust crate (mature, well-maintained)
- Tendermint websocket subscription (standard CosmJS pattern)
- ≥3 production relays in default config (e.g. wss://relay.damus.io, wss://nos.lol, wss://relay.snort.social)

## Decision log

- 2026-05-18: ADR proposed as v2 scope
- (future) NIP draft submitted to nostr-protocol/nips after Cosmos community feedback
- (future) First mainnet bridge runs after v31 deploy

## References

- [Nostr protocol](https://github.com/nostr-protocol/nostr)
- [NIPs](https://github.com/nostr-protocol/nips)
- `docs/ADR-002-X402-COMPOSITION.md` — HTTP gateway peer
- `docs/ADR-003-IBC-TASK-RELAY.md` — IBC peer
- `docs/SOVEREIGN_AGENT_PROTOCOL.md` — strategic context
