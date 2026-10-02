# Consensus Goes Hybrid: Post-Quantum Certificates on a Live Chain

*October 2, 2026 — the G0 gate is closed. Every item below is verified on a running devnet.*

A week ago the honest claim was: *JunoClaw verifies post-quantum signatures on-chain, and hybrid accounts can spend.* The consensus layer still ran purely classical BLS.

That gap is now closed — and hardened. Validators carry a **hybrid identity** (ed25519 consensus key + MAYO2 keypair, bound in the participant table). Every vote is a **hybrid signature** — a BLS threshold partial *and* a 186-byte MAYO2 signature over the same namespaced message. Every certificate requires a quorum on **both** halves: `max(classical, PQ)` security, not a bet on either.

All four devnet validators finalize blocks carrying `0x01` hybrid certificates — BLS threshold cert + signer bitmap + MAYO2 quorum (`cert_len=616`) — at ~1s finality.

## The live run caught what unit tests didn't

Two real bugs surfaced only under a running network — exactly why a live gate exists:

- **Randomized-signature equivocation.** MAYO draws a fresh salt per sign, so a re-sent vote produced different bytes for an identical subject. The consensus engine read it as a conflicting vote and stalled. Fixed: signatures are memoized per message — deterministic per vote, fresh salt per distinct message.
- **Unanchorable genesis snapshot.** A state-sync donor cached a height-0 export the joiner could never certify. Fixed on both sides: donors refuse height-0, joiners skip it and take the highest certified snapshot.

## What's verified on the running devnet

**Consensus and crypto**

- 4 validators finalizing hybrid certificates (BLS + MAYO2 quorum on every block)
- Negative gates: killing 2 of 4 validators stalls *both* signature halves; a corrupted MAYO key table refuses to boot; a Byzantine state-sync donor serving mismatched certified payloads is refused, not adopted
- Hybrid secp256k1+MAYO account spends committed on-chain; a corrupted MAYO signature is rejected at `check_tx`
- MAYO-2/3/5 verified inside a contract: 309k / 400k / 726k gas
- MAYO keygen emits validator PQ identity; the Phase A DKG ceremony carries `mayo_public_hex` and binds both keys

**Mempool and transaction serving**

- Per-sender ordering and fairness (k-way merge on sequence, FIFO across senders)
- Per-sender pending cap (64) with an 8 MiB `MsgStoreCode` exception
- Persistent transaction index — results survive node restart and cache eviction
- `GetTx` returns the decoded transaction body, recovered from the committed block payload
- Relayer-grade polling: `get-tx --wait` until committed

**Performance**

- Block execution under load: p99 = **1195 ms** (bank), **1048 ms** (MAYO-2 bud), **953 ms** (MAYO-5 bud) — all under the 1500 ms gate (50% of leader timeout)

**Catch-up, sync, and safety**

- Multi-peer state-sync anchors requiring quorum agreement on certified payloads
- Snapshot adoption and ~1,600-height bulk backfill at ~1.7× live rate
- `Simulate` gRPC with real gas on a scratch overlay; wasm metering kills `CpuLoop` on the meter
- `timeout_height` enforced in both check and deliver paths
- Leader-forwarding mempool gossip over authenticated P2P
- Byte-identical deterministic genesis across a full wipe
- BLS light client verifying on a live IBC counterparty (local Osmosis)
- Fuzz coverage on MAYO inputs, public-key hashing, and Bud weight arithmetic (512-step conservation invariant)

## What remains before public testnet

The engineering is done; what remains is operational proof and ceremony:

- A 24-hour multi-node soak with a Byzantine proposer
- `app_hash` agreement monitoring across validators
- The real-entropy key ceremony and a genesis rehearsal by fresh eyes
- External review of `junoclaw-mayo-verify` against the NIST reference — the gate that matters most

## What this actually is

A chain where every finalized block carries a threshold-BLS certificate *and* a post-quantum signature quorum — at ~1s finality — that has survived live partition, backfill, regenesis, full-loss state-sync, and adversarial negative tests. Nothing else running today does all of that.

The remaining work is verification and ceremony: proving the boring claims too.

*Every claim above cites a height, tx hash, or test name in the repo — that discipline is the point.*
