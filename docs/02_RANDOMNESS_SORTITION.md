# Randomness & Sortition in JunoClaw

> How we pick juries fairly when nobody trusts anybody.

---

## The Problem

Sometimes the DAO needs to select a random subset of members — for dispute resolution, task assignment, or verification committees. If anyone could predict or influence who gets selected, they could rig the outcome.

**Example:** The DAO has 13 buds. A dispute arises. You need 3 random jurors. If the proposer could choose who those 3 are, they'd pick their allies. Sortition (random selection) prevents this.

---

## How It Works in JunoClaw

### The Pipeline

```
Step 1: DAO member creates a SortitionRequest proposal
        "Pick 3 random members for dispute resolution"
                    │
                    ▼
Step 2: DAO votes (51% quorum)
        7 of 13 buds vote Yes → Passed
                    │
                    ▼
Step 3: Proposal executes → contract creates PendingSortition
        Snapshots all eligible member addresses
        Waits for randomness from one of two sources:
                    │
           ┌────────┴────────┐
           │                 │
    ┌──────▼──────┐   ┌─────▼───────┐
    │  Source A:   │   │  Source B:  │
    │  NOIS Proxy  │   │  WAVS drand │
    │  (IBC)       │   │  (attested) │
    │              │   │             │
    │  Juno sends  │   │  Operator   │
    │  IBC msg to  │   │  fetches    │
    │  NOIS chain  │   │  drand.love │
    │  → drand     │   │  randomness │
    │  → callback  │   │  → submits  │
    └──────┬───────┘   └──────┬──────┘
           │                  │
           └────────┬─────────┘
                    │
                    ▼
Step 4: Randomness arrives (32 bytes, hex-encoded)
        Contract receives via NoisReceive or SubmitRandomness
                    │
                    ▼
Step 5: Fisher-Yates shuffle
        Deterministic selection using SHA-256 sub-randomness
        Same randomness ALWAYS produces same selection
                    │
                    ▼
Step 6: SortitionRound stored on-chain
        Selected members, pool size, randomness source, hex seed
        Fully auditable — anyone can verify the selection
```

---

## The Two Randomness Sources

### Source A: NOIS Proxy (IBC-based drand)

[NOIS](https://nois.network) is a Cosmos chain dedicated to providing verifiable randomness via IBC. It sources entropy from [drand](https://drand.love), a distributed randomness beacon run by a network of independent operators.

```
agent-company                NOIS chain              drand network
     │                           │                        │
     │  WasmMsg::Execute         │                        │
     │  (nois_proxy, GetRandom)  │                        │
     │ ─────────────────────────▶│                        │
     │         IBC packet        │   HTTP beacon query    │
     │                           │ ──────────────────────▶│
     │                           │   random bytes         │
     │                           │ ◀──────────────────────│
     │  IBC callback             │                        │
     │  (NoisReceive)            │                        │
     │ ◀─────────────────────────│                        │
     │                           │                        │
     ▼                           │                        │
  resolve_sortition()            │                        │
```

**When to use:** When `nois_proxy` is configured in agent-company's Config. The contract automatically sends the IBC request on proposal execution.

**Status:** `nois_proxy` is currently `None` on uni-7 (no NOIS testnet proxy deployed). This source is ready in code but not yet wired.

### Source B: WAVS-Attested drand (Fallback)

The WAVS operator (on Akash or local) fetches randomness from drand and submits it directly to the contract.

```
agent-company                WAVS operator           drand.love
     │                           │                        │
     │  Event: pending_sortition │                        │
     │ ─────────────────────────▶│  HTTP GET               │
     │   (operator detects)      │  /public/latest        │
     │                           │ ──────────────────────▶│
     │                           │   { randomness: "ab.." }│
     │                           │ ◀──────────────────────│
     │  ExecuteMsg::              │                        │
     │  SubmitRandomness          │                        │
     │ ◀─────────────────────────│                        │
     │                           │                        │
     ▼                           │                        │
  resolve_sortition()            │                        │
```

**When to use:** Always available. The operator wallet (admin, governance, or task_ledger) submits the randomness. Only authorized senders are accepted.

**Status:** Working. Tested on uni-7. This is the active source.

---

## The Fisher-Yates Algorithm

Once randomness arrives, the contract runs a deterministic Fisher-Yates shuffle to select members:

```rust
// Simplified from contract.rs — resolve_sortition / select_members

fn select_members(eligible: &[Addr], count: u32, seed: &[u8; 32]) -> Vec<Addr> {
    let mut pool = eligible.to_vec();
    let mut selected = vec![];
    
    for i in 0..count {
        // Derive sub-randomness: SHA-256(seed || counter)
        let sub_seed = sha256(&[seed, &i.to_le_bytes()].concat());
        
        // Use sub-seed to pick index from remaining pool
        let remaining = pool.len();
        let index = u64::from_le_bytes(sub_seed[0..8]) % remaining as u64;
        
        // Swap-remove: move selected to end, shrink pool
        selected.push(pool.swap_remove(index as usize));
    }
    
    selected
}
```

**Key properties:**
- **Deterministic**: Same seed + same eligible list = same selection. Always.
- **No duplicates**: swap_remove guarantees each member selected at most once.
- **Verifiable**: Anyone with the randomness hex and the eligible snapshot can rerun the algorithm and get the same result.
- **Sub-randomness**: Each selection uses `SHA-256(seed || counter)` so selections are independent.

---

## On-Chain Data Structures

From `contracts/agent-company/src/state.rs`:

```rust
// The request (waiting for randomness)
pub struct PendingSortition {
    pub proposal_id: u64,
    pub count: u32,           // How many to select
    pub purpose: String,      // "dispute_jury", "verification_committee", etc.
    pub eligible: Vec<Addr>,  // Snapshot of all members at time of request
}

// The result (after randomness arrives)
pub struct SortitionRound {
    pub id: u64,
    pub proposal_id: u64,
    pub purpose: String,
    pub selected: Vec<Addr>,          // The chosen members
    pub pool_size: u32,               // How many were eligible
    pub randomness_source: String,    // "nois:job_123" or "wavs_drand:round_456"
    pub randomness_hex: String,       // The 32-byte seed (hex)
    pub created_at_block: u64,
}

// Storage
pub const SORTITION_SEQ: Item<u64>;
pub const SORTITION_ROUNDS: Map<u64, SortitionRound>;
pub const PENDING_SORTITION: Map<&str, PendingSortition>;
```

---

## Security Properties

| Attack | Prevention |
|--------|-----------|
| **Proposer picks the jury** | Randomness comes from external source (drand), not the proposer |
| **Operator rigs the randomness** | drand is distributed (18+ independent operators), no single party controls it |
| **Operator submits fake randomness** | Only admin/governance/task_ledger can submit; in TEE mode, hardware signs |
| **Replay old randomness** | Each sortition has a unique job_id; pending sortition is cleared after resolution |
| **Select more than pool size** | Contract rejects at proposal creation if count > number of eligible members |
| **Invalid randomness format** | Contract validates: must be exactly 64 hex chars (32 bytes) |

---

## Tests (All Passing)

From `contracts/agent-company/src/tests.rs`:

| Test | What It Verifies |
|------|-----------------|
| `test_sortition_proposal_and_wavs_resolve` | Full flow: propose → vote → execute → submit randomness → verify round stored |
| `test_sortition_deterministic` | Same randomness produces same selection, no duplicates |
| `test_sortition_unauthorized_randomness_rejected` | Random address can't submit randomness (only authorized senders) |
| `test_sortition_count_exceeds_pool_rejected` | Can't request more members than exist in the pool |
| `test_sortition_invalid_randomness_rejected` | Short or malformed hex is rejected |

---

## How Randomness Connects to Sidecars

The validator sidecar (Stage 9) amplifies the randomness trust:

```
WITHOUT sidecars (current):
  drand → WAVS operator (Akash) → SubmitRandomness → contract
  Trust: you trust the Akash operator didn't tamper with the drand response

WITH sidecars (future):
  drand → WAVS sidecar (TEE on validator) → SubmitRandomness → contract
  Trust: the TEE enclave fetched drand and signed the result
         — the validator CAN'T tamper even if they wanted to
```

This is why sidecars matter for sortition specifically: **randomness must be unforgeable for fair jury selection.**

---

## How Randomness Connects to Junoswap

Future use: **price oracle verification**. The sortition system can select random validators to independently verify Junoswap pool prices against external sources. This creates a decentralized, randomly-selected price verification committee — no single oracle can be bribed.

```
Junoswap pool price = $X
                │
                ▼
SortitionRequest: "Select 3 validators to verify JUNOX/USDC price"
                │
                ▼
3 random validators run TEE sidecars
Each queries CoinGecko/Binance/Osmosis independently
Each submits attested price
                │
                ▼
Aggregator: 2-of-3 agree → price verified on-chain
```

This doesn't exist yet but the infrastructure is all in place.
