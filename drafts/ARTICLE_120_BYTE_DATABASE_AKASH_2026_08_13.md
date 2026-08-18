# The 120-Byte Database

*How an immutable hash chain that stores almost nothing gives you a continuous, verifiable database of everything — and how anyone can run the whole environment on Akash.*

**August 13, 2026**

---

## The Claim and the Trap

Here is a claim that sounds too good to be true: **the JunoClaw coordination chain stores ~120 bytes per batch, yet captures all important data — every sensor reading, every robot command, every agent message — verified.**

Both halves of that sentence are true. But between them sits a trap that nearly every blockchain-for-machines project falls into, and it's worth naming precisely:

**Integrity is not availability.**

A hash chain proves that specific data existed, in a specific order, audited and agreed upon. It does not store that data. If you anchor a 32-byte hash of a sensor stream to Juno and then delete the stream, the hash still proves *something was there* — but the readings are gone. The anchor is a commitment, not the payload.

Projects like peaq, IOTA's machine economy, and Robonomics resolve this tension by brute force: put the data itself on-chain. Every robot event is a transaction. Every inspection report is a stored payload. The result is integrity *and* availability, at the cost of turning the chain into a telemetry database — bloat, gas scaling with data volume, and a ledger that can't survive a real robot fleet.

We resolve it differently: **by tiering.** And the tiering only works because the hash chain makes every off-chain byte accountable. That's the actual claim of this article:

> **The chain doesn't store the database. It makes any database storing it honest.**

---

## What the Hash Chain Actually Is

The coordination mesh (`crates/junoclaw-coordination`) produces finalized batches:

```rust
pub struct Batch {
    pub messages: Vec<AgentMessage>,
    pub prev_hash: [u8; 32],      // links to previous batch
    pub height: u64,
    pub timestamp: u64,
    pub gate_result: Option<GateResult>,  // J-Lens audit attached
}
```

Each batch chains to the previous via `prev_hash` — an immutable hash chain, BFT-ordered by 4 validators, every message truth-gated by J-Lens. Per batch, Juno stores:

| Field | Size |
|-------|------|
| `commonware_height` | 8 bytes |
| `messages_hash` | 32 bytes |
| `certificate` | 32 bytes |
| `timestamp` | 8 bytes |
| `submitter` | ~40 bytes |
| **Total** | **~120 bytes** |

One hundred messages — sensor readings, actuation commands, agent deliberations — collapse into 120 bytes of permanent, on-chain proof.

The 4-node consensus test (2026-08-13) verified the properties that matter: hash chain integrity, Byzantine detection via the J-Lens red gate, zero false positives on clean batches, 32-byte certificates, ~69k msg/s submission rate.

---

## The Four-Tier Model

This is where the "continuous database" claim becomes real. The database exists — it's just split by *what needs to be where*. Four tiers, each answering a different question:

### Hot tier — ~5% RAM

What validators and sidecars actually hold in memory:

- The chain head (latest batch hash + height)
- The current window's message buffer (pre-finalization)
- The validator set

That's it. **50–100MB per sidecar, and it never grows.** It doesn't grow because history doesn't live in memory — history is *provable*, so it doesn't need to be *held*. The 5% figure isn't an optimization; it's a consequence of the architecture. RAM holds the verification-critical frontier, nothing else.

### Warm tier — cheap append-only disk

The full batch payloads, indexed by height:

```
batch_4041.json  -> messages, timestamps, gate results
batch_4042.json
...
```

Every byte in this tier is verifiable: recompute `messages_hash` from any batch, compare against the Juno anchor at that height. Mismatch = tampered. Match = proven.

This **is** the continuous database. It's continuous not because it lives on-chain, but because it's *verifiable against the chain* — you can trust a warm-tier replica you've never seen before, served by a stranger, because you can check it yourself. Retention is a policy decision (e.g. 90 days rolling), not a protocol constraint. It's append-only, streamable, and costs zero RAM.

### Cold tier — optional permanent archival

Critical windows (incidents, audits, regulatory holds) get pinned to content-addressed storage — IPFS or Arweave — addressed by the *same hash* the chain anchors. The content address and the on-chain commitment are literally the same value. Retrieval is self-verifying by construction.

### Tier 4 — The on-chain index: Moultbook

Between the minimal settler anchor and the off-chain tiers sits one more on-chain piece: **Moultbook** (`contracts/moultbook-v0`). It already implements the commitment model — entries store a `commitment` and reference off-chain content by CID, never the content itself.

Each finalized coordination batch (or each incident) can be published as a moult:

- `commitment` = the batch's `messages_hash` — the same 32 bytes the settler anchors
- `refs` = coordination batch heights plus a topic namespace — `commonware:4041` links the moult to the BFT sequence, `topic:pipeline-A12` makes "everything that happened on this pipeline" one query (regular posts index topics through refs; on-chain `topic_hash` is reserved for anonymous entries)
- `visibility` = Public, Group, or Owner — operational control the raw settler doesn't have
- `PublishAnon` = anonymous incident reporting, ZK-proven membership without revealing which robot spoke. A whistleblowing actuator is a real scenario, and it's already implemented.

This is built. The relayer (`crates/junoclaw-relayer`) gained a `moult.rs` module: after each batch settles, it posts the moultbook addendum — same commitment, refs to height and topic. Run it with `--moultbook <contract> --topic <namespace>`. Best-effort by design: a failed moult post never stalls settlement of the next batch.

The settler is the machine-verifiable anchor — for contracts and automated verifiers. Moultbook is the semantic index — for agents and humans asking "what happened." Same commitment, two surfaces. And because the context-agent trust indexer already reads moultbook, J-Lens verdicts, trust scores, and batch commitments all end up queryable in one place.

## Agent Memory: Can Agents Reason About Past Events?

This is the question that separates JunoClaw from every other blockchain-for-machines project. peaq, Robonomics, and IOTA all give agents access to past events — but they do it by putting the data on-chain. The chain *is* the memory. It works at small scale. It doesn't survive a real fleet.

| Project | How agents access past data | Cost |
|---|---|---|
| **peaq** | Data on-chain marketplace — agents read directly | Chain bloats, gas scales with data volume |
| **Robonomics** | Telemetry on substrate — agents query on-chain history | Same bloat, limited to small payloads |
| **IOTA** | Data in Tangle + Streams — agents read DAG + data channels | No smart contract verification, data not indexed |
| **JunoClaw** | Moultbook index → refs → warm/store fetch → verify against anchor | 120 bytes on-chain, payloads off-chain, any copy self-verifying |

JunoClaw's four tiers give agents **discovery** (Moultbook: "what happened in pipeline-A12?"), **retrieval** (warm tier or content-addressed store: fetch the actual payloads), and **verification** (recompute `messages_hash`, compare to the on-chain anchor). The agent doesn't trust the warm-tier server — it trusts the hash.

### The next step: Moultbook as the query layer over a Commonware store

Today, warm-tier payloads live on disk served by the DAO. The natural evolution is to make the **Commonware P2P mesh that runs consensus also serve the data it certified** — a content-addressed blob store where each batch's payloads are keyed by the same `messages_hash` the chain anchors.

```
Agent query: "What happened in pipeline-A12?"
  → Moultbook: ListByRef(topic:pipeline-A12) → [commitment, refs, height]
  → Commonware store: GET(messages_hash) → batch payload
  → Verify: SHA256(payload) == commitment == on-chain anchor ✓
  → Agent now has verified past events to reason about
```

This makes Moultbook + Commonware store = **agent memory**. No centralized server, no external dependency. The mesh that ordered the messages also serves them — and the hash chain makes every byte accountable. An agent can reconstruct any past state, verify it independently, and reason about it with full confidence.

Moultbook's `refs` field already supports this: today it carries `commonware:<height>` and `topic:<namespace>`. Tomorrow it can carry `cid:<content-hash>` pointing directly into the Commonware store. The contract doesn't need to change — the refs are free-form strings. The relayer just needs to publish the CID alongside the height and topic when it posts the moult.

### Do we need "data availability" in the rollup sense? No.

Rollup DA layers (Celestia-style) exist because validators must *re-execute* state transitions to fraud-prove the chain. Withhold the data and validation halts — DA is a **liveness** requirement.

Our hash chain has no state machine to re-execute. Verification is *hash recomputation*, not re-execution. What we need is retrievability for audit and liability — a **durability** requirement, which is far cheaper: a warm-tier retention policy plus optional cold archival. No erasure coding, no DA sampling, no DA committee. And the 120-byte anchor gives us something DA layers don't: any retrieved copy is self-authenticating. DA guarantees the data exists; the hash chain guarantees it's true.

---

## Why the 5% Number Is the Right Number

A naive design keeps "everything important" in the database, which drifts toward "everything" in RAM or at least hot storage. The hash chain inverts the question: **what is the minimum state required to verify everything else?**

The answer is remarkably small:

- One 32-byte hash per finalized batch (the chain tip suffices to verify the entire history backwards via `prev_hash`)
- The current in-flight window
- Who the validators are

Everything else — terabytes of sensor history, if it comes to that — can live in the cheapest storage available, be served by untrusted parties, and still be *cryptographically honest*. The RAM footprint of truth is constant. The disk footprint of history scales, but disk is the cheapest resource in computing and it's priced like it.

This is the inversion the article title points at: the database is 120 bytes per batch *on the ledger that matters*, and arbitrarily large everywhere else — with the everywhere-else held accountable by the 120 bytes.

---

## The Working Environment: All of It Runs on Akash

None of this requires AWS, a datacenter relationship, or infrastructure anyone on the team can't replicate. Every tier maps to an Akash deployment, and every deployment is an SDL file in the repo:

| Component | Akash deployment | Approx. cost |
|---|---|---|
| Coordination mesh node / validator sidecar | CPU lease (2 CPU / 1Gi) | ~$6/month each |
| **4-node soak mesh (live)** | Single CPU container, all 4 nodes over loopback | ~$40/week |
| J-Lens CSI probe server | GPU lease on demand (8×H100 SDLs in `tools/akash/`) | ~$40–100/session |
| Warm-tier log publisher | CPU + persistent storage | ~$10/month |
| Cold-tier pinner | Small CPU + Arweave/IPFS tooling | ~$5/month |

**The soak mesh is the proof of concept running right now.** Four real commonware-p2p nodes — deterministic ed25519 identities, BFT coordination, J-Lens gate checks, consensus tests every cycle — deployed as a single Akash container with its logs and `soak-status.json` served over HTTP. No SSH, no VPN, no local VM: anyone on the team opens a URL and watches the mesh run. Seven days, ~40 ACT, refundable deposit, escrow refunds on close.

The build files are public: `Dockerfile.soak-mesh` and `tools/akash/sdl-soak-mesh.yml`. Build, push, `akash tx deployment create` — the working environment belongs to everyone.

### What a session costs, concretely

At a mid-range community bid of ~400 uact/block (~600 blocks/hour):

| Duration | Lease cost |
|---|---|
| 1 hour | ~0.24 ACT |
| 1 day | ~5.76 ACT |
| 3 days | ~17.28 ACT |
| 7 days | ~40.32 ACT |

Plus a 5 ACT refundable deployment deposit. For the price of a team lunch, the coordination stack runs publicly for a week.

---

## Honest Limits

This architecture has real tradeoffs, and the article doesn't work without stating them:

1. **Someone must run the warm tier.** The chain guarantees honesty of any warm-tier copy, but not its existence. For the pilot, the Juno Agents DAO runs it. Long-term, it's a role anyone can fill — and the verification property means the network doesn't need to trust whoever does.
2. **Payload loss is possible; proof loss isn't.** If warm data is lost, the on-chain anchors survive — you retain irrefutable proof of what was reported and agreed, but not the raw content. For robotics liability, that's often the part that matters most ("the robot reported X at time T, audited green"). For scientific replay, use the cold tier for anything you can't afford to lose.
3. **Verification requires replay.** Auditing means recomputing hashes from warm-tier data. This is cheap (SHA-256 is among the fastest operations in computing) but it is *work* — the trust model is "verify on demand," not "trust by default."
4. **J-Lens is a gate, not an oracle.** It catches internal-state inconsistency (hallucination, deception) in the agent's own model. It does not verify that a physical sensor reading matches physical reality — a miscalibrated thermometer is honestly wrong. Hardware attestation (TEE-signed sensor reads, the validator sidecar roadmap) closes that gap.

---

## Roadmap to Safe Agent Data

The architecture is designed so that **each phase removes a trust assumption without breaking the previous one**. You can stop at any phase and still have a working system — just with a different trust boundary.

### Phase 0 — DAO pilot mesh (today)

- 4 DAO-appointed nodes, loopback P2P
- Warm tier = single DAO-operated disk
- Soak test running, A49 live on DAO DAO
- **Trust**: "trust the DAO operator"
- **Data availability**: centralized warm disk — verifiable but not redundant
- **Agent memory**: agents can query Moultbook, fetch from DAO disk, verify against anchor

### Phase 1 — A49 passes (30-day testnet pilot)

- Success criteria: ≥100 batches settled, 95% uptime, 0 false red positives, 100% red detection
- Nothing changes architecturally — this is the proof gate
- Jake's condition ("show me 4-node consensus running first") is already met; A49 asks for 30 days of real data
- **Trust**: same as Phase 0, but now backed by evidence
- **Timeline**: 30 days from pilot start

### Phase 2 — Validator sidecars (mainnet proposal)

- Juno validators run coordination nodes alongside `junod`
- ~50-100MB RAM, no slashing, no `junod` modification, no chain upgrade
- Mesh security inherits Juno's validator set security
- Any validator can mirror warm-tier data — redundancy without trust
- **Trust**: "same validators who secure Juno"
- **Data availability**: N potential mirrors, all self-verifying
- **Blocker**: A49 must pass first, then DAO votes on mainnet proposal

### Phase 3 — Sortition + TEE

- drand verifiable randomness rotates consensus committee each epoch
- TEE (SGX/SEV) hardware-attests submissions — even the validator can't tamper
- No one can predict or manipulate who's in the committee
- **Trust**: "the hardware chip signed it"
- **Data availability**: same as Phase 2, but committee integrity is now quantum-resistant
- **Blocker**: sidecar adoption reaches critical mass

### Phase 4 — Commonware content store (the target)

- The P2P mesh that runs consensus **also serves the data it certified**
- Batch payloads keyed by `messages_hash`, served by mesh nodes
- Moultbook refs carry `cid:<content-hash>` pointing directly into the store
- No central warm-tier server needed — the mesh that ordered the data also serves it
- **Trust**: "the mesh that ordered it also serves it, hash verifies"
- **Data availability**: decentralized, redundant, self-verifying — **complete safe agent data fallback**

```
Agent query: "What happened in pipeline-A12?"
  → Moultbook: ListByRef(topic:pipeline-A12) → [commitment, refs, height]
  → Commonware store: GET(messages_hash) → batch payload
  → Verify: SHA256(payload) == commitment == on-chain anchor ✓
  → Agent has verified past events to reason about — no trusted server involved
```

**This is the phase where agent memory becomes fully decentralized.** An agent can reconstruct any past state, fetch the data from any mesh node, and verify it independently — without trusting the DAO, a validator, or any single operator. If one node goes down, another serves the same data. If someone serves corrupted data, the hash mismatch catches it instantly. If someone withholds data, the on-chain anchor proves something is missing.

The swap from "DAO disk" to "Commonware mesh store" doesn't change the trust model — it removes the single point of availability failure. The hash chain made every byte accountable from day one. Phase 4 makes every byte **also retrievable** without a central operator.

### What's blocking each phase

| Phase | Blocker | Status |
|-------|---------|--------|
| **A49** | DAO vote | Live on DAO DAO, awaiting result |
| **Sidecars** | A49 must pass first | Sidecar binary not yet built, proposal ready |
| **Real P2P transport** | NASM compile of Commonware | In progress — `network.rs` links, ed25519 RNG needs alignment |
| **Commonware store** | Needs real P2P + multiple live nodes | Design complete (this article), implementation not started |

Realistic timeline to Phase 4: **~2-4 months** after A49 passes and validators opt in. The Commonware store itself is weeks of dev — the critical path is getting real distributed nodes running.

---

## The Vibe

The blockchain industry's default answer to "how do we make machine data trustworthy?" is "put it all on-chain." It's the answer of a field that only has one tool.

The better answer is to separate the questions:

- **What must be immutable and universally agreed?** The ordering and the audit. ~120 bytes per batch. (Settler on Juno)
- **What must be discoverable?** The semantic index — which batches exist, for which pipeline, by which agent. (Moultbook on Juno)
- **What must be available?** The payloads. Cheap disk or Commonware store, verifiable against the anchor. (Warm tier)
- **What must be fast?** The verification frontier. ~5% of RAM, constant forever. (Hot tier)

J-Lens makes the bytes *honest at the source* — the agent's internal states are probed before its message is even allowed into the batch. BFT consensus makes them *ordered*. Juno makes them *permanent*. Moultbook makes them *findable*. Akash makes the whole environment *replicable by anyone*.

The database is continuous. The memory footprint is constant. The proof fits in 120 bytes. And any agent can query the past, fetch the data, and verify it — without trusting anyone.

---

## Present Status (2026-08-13)

| Component | Status |
|---|---|
| Hash chain + BFT consensus | 4-node test PASS, 69k msg/s, 32-byte certs |
| J-Lens gate | 28+8 tests pass, mock + live CSI server modes |
| Coordination-settler contract | Deployed on uni-7 (code ID 86) |
| 7-day soak test (Akash) | **LIVE** — cycle 127+, 762 suites, zero failures, 4/4 nodes alive |
| 7-day soak test (local VM) | Running — 4/4 P2P nodes alive, cycles passing |
| Moultbook addendum (tier 4) | Built — `crates/junoclaw-relayer/src/moult.rs`, 3 tests pass |
| Commonware content store | Design complete (this article), implementation next |
| Warm-tier publisher | Design complete (this article), implementation next |

---

*JunoClaw is built by the Juno Agents DAO. The chain is a notary, not a database — but because it's a notary, any database can be trusted. The working environment is three SDL files and an AKT balance. Reproduce everything from the repo.*
