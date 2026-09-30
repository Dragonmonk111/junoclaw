# Post-Quantum Accounts Are Live: The Hybrid Spend, the Catch-Up Test, State-Sync, and the Road to Testnet

*September 29–30, 2026 — consolidated status. Supersedes the September 26
draft, which was written before its "remaining proofs" got proven.*
**Final revision September 30** — snapshot state-sync and the `Simulate`
gRPC endpoint shipped and passed live tests, and the wasm gas-metering
audit closed out the "crafted inputs" question. All three are folded in
below.

Three days ago we reported that JunoClaw verified a MAYO post-quantum
signature inside a CosmWasm contract, and that hybrid secp256k1+MAYO accounts
had landed in the chain's auth module — with the honest caveat that a live
hybrid *spend* was still unproven, because the signer tooling didn't exist yet.
That proof exists now. So do the live backfill test, cert-anchored
state-sync, transaction simulation, and a gas-metering audit. This is the
consolidated status.

## The headline: a hybrid transaction, end to end

A JunoClaw account can now be secured by **two signatures that must both
verify**: secp256k1 (what every Cosmos account uses today) and MAYO (a
NIST-selected post-quantum signature scheme). If quantum computers break
secp256k1, the MAYO half still stands. If someone finds a flaw in MAYO, the
secp256k1 half still stands. Security is max(classical, PQ), not a bet on
either.

Until this week the chain could *verify* such an account but nothing could
*sign* one — the MAYO signer needs a Linux/C toolchain that doesn't exist in
our build environment. The fix was tooling, and it's all open:

- **`tools/hybrid-sign`** — a standalone signer (sriracha-mayo + k256) that
  generates a hybrid keypair, derives the JunoClaw address the same
  domain-separated way every address is derived, builds a real Cosmos
  `MsgSend` transaction (`TxBody`/`AuthInfo`/`SignDoc`), and signs the
  standard `SHA-256(SignDoc)` digest with both keys — the secp256k1 signature
  and the MAYO signature packed into one field. Ships as a Docker image
  (`junoclaw-hybrid-sign`) for the Linux toolchain.
- **`tx-sender broadcast`** — a new subcommand that takes raw signed
  transaction bytes and pushes them over gRPC, the same path every
  transaction takes.

The live result on the devnet:

| Step | Result |
|---|---|
| Deployer funded hybrid account `juno16ct0r…` | committed |
| Hybrid-signed `MsgSend` broadcast | committed @ **height 165,266**, `code=0`, gas_used **109,423** |
| Same tx with one byte corrupted in the MAYO signature | **rejected at check_tx**: `InvalidSignature` |
| Second valid spend (seq 1) | committed @ height 169,044, `code=0` |

That middle row matters more than the rest. A corrupted-signature rejection
at check_tx is the difference between "the verifier is in the codepath" and
"the verifier is *enforced*." Both secp256k1 and the vendored pure-Rust MAYO
verifier (`junoclaw-mayo-verify`) ran in consensus — ~109k gas for the whole
transaction, no precompile, no wasm metering.

For the airdrop community: the quantum-safe accounts aren't a roadmap item.
They're signing and spending on a live chain today.

## The second headline: catch-up, live-verified

The other testnet blocker was bulk backfill — the mechanism that lets a
lagging node fetch missing blocks in parallel height-ranges instead of a
lazy one-RTT-per-block crawl. The code shipped earlier; the live test was
the missing proof.

The test, run today:

- Stopped validator `node-2` at ~height 164,800.
- Let the other three keep finalizing (~1 block/second) — a ~700-block gap
  that kept growing.
- Restarted it and watched.

Node-2 did what the new path is designed to do: learned the peer tip,
requested missing payloads in batched height-ranges over P2P, executed the
finalized chain in order — roughly **1,600 heights in ~15 minutes, ~1.7× the
live block-production rate** — then rejoined consensus proposing and
certifying blocks, on the identical `app_hash` as its peers. No divergence,
no halt, no manual surgery.

What this is not — staying honest: it's not snapshot state-sync. A brand-new
validator still replays history; backfill makes that replay parallel instead
of serial. True state-sync (trust a finalized `state_root`, download state in
verified chunks, resume) was the remaining design item — *until the next
morning*. See below.

## The third headline: snapshot state-sync, live-verified (Sept 30 addendum)

The same night the article above went out, the last missing piece shipped:
a fresh node can now join **without replaying history at all**.

How it works — the trust model is the Tendermint one, with our certificates
as the anchor:

- **Donor side** (`state_sync` gRPC on every node): serves a snapshot of the
  KV state — chunked, length-prefixed records with a per-chunk SHA-256,
  plus a Merkle root computed over all sorted records.
- **Joiner side**: lists snapshots, downloads chunks, verifies every chunk
  checksum, recomputes the whole-dump Merkle root — and compares it to the
  `state_root` inside a **BLS-threshold-certified `BlockPayload`** fetched
  independently from the same peer's light-client service. The snapshot is
  only adopted if the dump hashes to what the validator set certified.
- **Atomic import**: all KV pairs + `LAST_BLOCK` land in one storage commit,
  plus the certified tip payload sidecar — then the app re-derives its state
  and recomputes the state root.

The live test, this morning: wiped node-3's data volume, restarted it. It
attempted state-sync against node-0, hit the expected tip race (the snapshot
was at the certified tip, so `Block(H+1)` didn't exist yet — bounded retry
handled it), adopted the snapshot at **height 212,930**, backfilled the ~100
blocks since the snapshot, rejoined consensus — and **proposed block 213,035**,
which all four validators finalized.

It also caught a real bug, which is what live tests are for: the first run
adopted the snapshot fine but then stalled — the `_block_payload:` sidecar
that links the executed tip was excluded from the dump (correctly — it's
consensus metadata, not app state), so `last_digest` was unset and the
finalize walk could never reach the adopted tip. Fix: the certified tip
payload now commits atomically inside `snapshot_import`. One line of
protocol understanding, live-verified.

The honest caveat: the joiner trusts one peer's gRPC endpoint for the
certificate. The root *check* is cryptographic (dump must hash to the
certified `state_root`), but peer diversity for the anchor — same caveat as
Tendermint state-sync's trusted-node RPCs — is the hardening path.

## Recap: what shipped in the last stretch

For anyone catching up — the run-up to today:

- **In-contract MAYO verification.** MAYO-2/3/5 signatures verified inside a
  CosmWasm contract on live consensus (309k/400k/726k gas respectively —
  worst case ~18% of the tx gas limit), plus a tampered-signature negative
  control that correctly failed.
- **Hybrid accounts in auth.** `PubKey::HybridSecp256k1Mayo`: a spend
  requires both a valid secp256k1 sig AND a valid MAYO-1/2/3/5 sig over the
  same sign-doc hash. Native verification, +25k gas for the PQ half.
- **Engine hardening.** Atomic finalize-driven commit, decoupled execution,
  mempool v2, bounded peer-pushed payloads.
- **Regenesis resilience.** The devnet was wiped and regenerated on purpose:
  deterministic genesis reproduced the root contract byte-for-byte, and the
  IBC bridge to Osmosis rebuilt in ~90 seconds.
- **PQ Phase 2 design.** `docs/PQ_PROTOCOL_AUTH.md` — hybrid validator
  identity (ed25519 + MAYO) and the hard problem: consensus certificates.
  No standardized threshold PQ scheme exists, so the design is a
  BLS-threshold certificate AND a k-of-n MAYO bitmap certificate — both must
  verify.
- **Key management with teeth.** We lost a funded relayer key to process,
  not crypto — now documented (`docs/KEY_MANAGEMENT.md`) with untracked
  testnet keys, backed-up keyrings, env-var handoff, and a cold-backup
  section for the hybrid keys themselves.

## Wasm gas: the crafted-input answer (Sept 30 addendum)

The open audit question was whether a hostile contract could burn unbounded
CPU. The answer, now test-proven in `wasm/keeper.rs`:

- **Instruction metering is real.** Wasmer's metering middleware is active —
  hackatom's `CpuLoop` (an explicit infinite loop) dies with `OutOfGas`
  under a bounded meter, not a hang.
- **Memory is capped.** `AllocateLargeMemory` requesting 64MiB fails
  gracefully against the 32MiB per-instance cap — contract error, not an
  OOM.
- **Two metering fixes landed from the audit.** `check_tx` now validates
  under the same `MAX_VALIDATE_GAS` (200k) cap as `deliver_tx` — previously
  it used the block meter (100M), so a tx could be mempool-accepted but
  guaranteed to fail at deliver. And `DEFAULT_QUERY_GAS` went 500k → 4M:
  honest MAYO-5 verify costs ~726k, so PQ-verify smart queries were
  unreachable. Node-local bound, no consensus impact.
- **Sub-message and query recursion metering** already existed (sub-meter
  per `gas_limit`, shared meter through the storage backend); the audit
  confirmed the accounting is correct end-to-end.

Remaining nuance: `store_code`'s `check_wasm`/compile step is unmetered —
bounded only by `tx_byte_gas` (10/byte) — a one-time per-code cost,
same shape as wasmd.

## The comparison questions: faults, continuity, relayer recovery

Prompted by a good question in the wild — what happens across upgrades and
validator fault cases — here's the honest version:

- **Equivocation is cryptographic evidence, not a vote.** Two valid
  BLS certificates for different payloads at the same `(epoch, view)`
  constitute proof of `≥ f+1` share compromise. The light client's
  `update_state_on_misbehaviour` freezes at the lower height; conflicting
  headers at a stored height route to misbehaviour rather than overwrite.
  Caveat, honestly: the certificate proves *that* equivocation happened —
  mapping share indices back to operator identities for slashing needs the
  DKG dealer logs (tracked in the spec).
- **Partitions and delayed commits are the tested cases.** The backfill
  test was a live partition: node-2 alone, 700+ blocks behind, parallel
  catch-up, identical `app_hash` on rejoin. The state-sync test was total
  loss: wiped volume, cert-anchored snapshot, proposing again at height
  213,035. Commits land via atomic finalize — there's no partial-commit
  window for a delayed commit to exploit.
- **Packet/ack continuity across upgrades.** The regenesis test was the
  extreme version: chain wiped and regenerated, deterministic genesis
  reproduced the root contract byte-for-byte, and the ICS20 bridge to
  Osmosis rebuilt in ~90 seconds. Per-upgrade channel continuity is the
  next dry-run item — same codepath, lighter version.
- **Relayer recovery.** The light client keeps no per-block valset — the
  genesis group key plus BLS certs are the whole trust chain, so a relayer
  restart is stateless: fetch latest certified payload, submit, done.
  `update_state` is idempotent on re-submitted headers.

## Honest scorecard, updated

**Genuinely demonstrated:**
- Hybrid secp256k1+MAYO spend: signed, broadcast, committed, and the
  corrupted-signature variant rejected at check_tx. Live, on finalized
  blocks — not a unit test.
- MAYO-2/3/5 verification on-chain (contract layer) at viable gas.
- Bulk backfill under real network conditions: ~1.7× production rate,
  clean rejoin, identical state root.
- Snapshot state-sync: cert-anchored, chunk-verified, live-verified —
  fresh node adopted at height 212,930 and proposed a finalized block.
- `Simulate` gRPC: `gas_used=18,119` success path, real error log on the
  failure path, zero state writes.
- Wasm metering: instruction-level enforcement proven against an
  intentional infinite loop and an oversized memory alloc.
- Deterministic genesis verified across a full wipe.

**Still not proven:**
- **No external audit.** `junoclaw-mayo-verify` is our Rust port of the NIST
  reference. KAT vectors pass; "ported it and it matches" is a milestone,
  not a security property.
- **KATs, not adversarial inputs.** Nobody has fuzzed the verifier or the
  hybrid pk-hash/sig-packing path with malformed bytes.
- **One negative test variant.** MAYO-corruption rejection is proven live;
  truncated-signature and wrong-variant cases use the same codepath but
  weren't broadcast.
- **Single-peer anchor for state-sync.** The root *check* is cryptographic
  but the certificate itself comes from one endpoint; multi-peer anchor
  comparison is the hardening item.
- **Per-upgrade channel continuity.** Regenesis proved full rebuild;
  in-place upgrade with live IBC channels is the remaining dry-run.
- **PQ consensus.** The hybrid-cert design is written; nothing is in
  consensus yet. That's Phase 2.

## What stands between here and testnet

Ordered roughly by embarrassment potential. ~~Struck~~ items shipped.

1. **~~`Simulate`~~** — shipped *and* live-verified Sept 30 (see scorecard):
   real execution on a scratch overlay, real gas numbers, real error logs,
   zero state writes. Integrators can dry-run.
2. **~~wasm gas limits + metering audit~~** — closed Sept 30. Instruction
   metering proven against `CpuLoop`/`AllocateLargeMemory`; two fixes
   landed (check_tx cap consistency, query gas above MAYO-5's 726k).
3. **Fuzz the PQ stack.** The verifier, the hybrid pack/parse path, Bud
   weight arithmetic. Now the top item.
4. **External eyes on the verifier.** Even a focused review against the
   NIST reference.
5. **~~Consensus atomicity + mempool + peer bounds~~** — shipped.
6. **~~Bulk catch-up + snapshot state-sync~~** — both shipped and
   live-verified: parallel backfill Sept 29, cert-anchored snapshot
   adoption Sept 30. A fresh validator no longer replays history.
7. **~~PQ auth at account level~~** — hybrid accounts live-verified.
   Remaining: hybrid validator identities and the hybrid consensus
   certificate (Phase 2, designed in `docs/PQ_PROTOCOL_AUTH.md`).
8. **Key ceremony.** Seeded devnet keys are a documented choice; testnet
   needs real-entropy DKG and keyring-held operator keys.
9. **Genesis ceremony + onboarding dry-run** by someone who didn't write it.
10. **A block explorer** — height, peers, last finality. "Check the node
    logs" is not a vibe.

## Is it novel?

Individually every piece has prior art. The composition — a threshold-BLS
consensus chain that natively speaks wasmd, runs a BLS light client *on* a
counterparty for IBC, verifies NIST post-quantum signatures in-contract, and
now has hybrid classical+PQ accounts spending on live consensus — does not
exist elsewhere, as far as we know.

The summary line for this week: **the quantum-safe account is real, spends,
and enforces both halves of its security — nodes that fall behind catch up
fast or skip replay entirely, transactions can be dry-run before broadcast,
and hostile wasm dies on the meter.** The remaining work is the boring kind:
fuzzing, external review, ceremony, onboarding. That's exactly where a
chain wants to be before testnet.
