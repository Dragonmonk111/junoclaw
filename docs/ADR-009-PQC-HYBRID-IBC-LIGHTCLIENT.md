# ADR-009: Hybrid-aware IBC `07-tendermint` light-client migration (Project Aegis Phase G)

**Status:** Proposed — Phase G deliverable of [PROJECT_AEGIS_JUNO_FULL_PQC](./PROJECT_AEGIS_JUNO_FULL_PQC.md) §4.4
**Date:** 2026-06-25
**Authors:** Dragonmonk / VairagyaNodes (with Cascade)
**Scope:** IBC `07-tendermint` light client — header verification, validator-set tracking, misbehaviour, the relayer path (surfaces #10 IBC client verify, #11 cross-chain trust transfer)
**Depends on:** ADR-008 (hybrid consensus keys — the thing the client must verify), ADR-006 (same ML-DSA-44 impl), the landed `VerifyCommitLight` / `VerifyCommitLightTrusting` hybrid tests in `types/consensus_hybrid_test.go`

> This ADR specifies how an IBC light client on chain **B** verifies headers
> produced by a chain **A** whose validator set is migrating to hybrid
> Ed25519+ML-DSA-44 consensus keys (ADR-008). It introduces **no new wire
> message** and **no flag day**: a counterparty that has not upgraded its client
> continues to verify the **classical half** of every hybrid signature, exactly
> as it does today, until it opts in to PQC verification.

---

## Context

IBC's `07-tendermint` client is itself a CometBFT light client embedded in a
chain's state machine. To accept a `MsgUpdateClient` carrying a header from the
counterparty chain, it runs the **same** verification primitives as a P2P light
client:

1. `VerifyCommitLight` — the **adjacent** path: the new header is at `trusted+1`,
   so the *full* trusted validator set must sign and clear 2/3.
2. `VerifyCommitLightTrusting` — the **skipping** path: the new header may be
   many blocks ahead; the client only requires that validators **carried over**
   from the trusted set contribute > `trustLevel` (default 1/3) of the trusted
   voting power.

Both resolve each signer's pubkey from the **trusted validator set snapshot**
and call that pubkey's `VerifySignature`. This is the exact dispatch property
ADR-008 §F4 relies on — the commit carries only raw signature bytes plus the
`ValidatorAddress`; the verifier resolves the key *type* from state.

### What changes when chain A goes hybrid

During chain A's migration window (ADR-008 §7), A's `ValidatorSet` is
**heterogeneous**: some validators sign classical-only 64 B signatures, some
sign 2,491 B hybrid signatures. A header's `Commit` therefore contains a **mix**.

A light client on chain B must:

- **Resolve each signer's key type** from A's validator set in the client's
  `ConsensusState` — already the dispatch model, no change.
- **Hold the hybrid pubkey** (1,344 B) for migrated validators in its tracked
  validator set — this is the storage/proto change.
- **Verify the hybrid signature** (both halves) for migrated validators — this
  is the new verify branch.
- **Tolerate validator-set churn** as A's validators rotate consensus keys via
  `MsgRotateConsKey` (ADR-008 §F6) — handled by the existing skipping path, but
  the *trust-overlap counting* must count a rotated validator's hybrid key
  toward the trusted power if and only if its **address** (classical half,
  ADR-008 F1-b invariant) is unchanged.

The F1-b address invariant is the linchpin: because `HybridPubKey.Address()`
returns the **classical** half's address, a validator that rotates from
classical to hybrid keeps the **same 20-byte address**. From the light client's
perspective the validator's *identity* (address + voting power) is continuous;
only the *key material it must fetch and the verifier it must run* change.

---

## Decision

Make the `07-tendermint` client **hybrid-aware** by extending the three points
where it touches a validator pubkey — **storage, dispatch, and trust-overlap
counting** — while leaving the IBC message grammar, the connection/channel
handshake, and the proof-verification (Merkle/ICS-23) paths **untouched**.

### G1 — `ConsensusState` / `Header` carry hybrid validator pubkeys

The client stores the trusted validator set in its `ClientState` /
`ConsensusState` lineage; `Header.ValidatorSet` and `Header.TrustedValidators`
are `cmtproto.ValidatorSet` messages. ADR-008 §F4 already added the
`aegis_hybrid_ed25519_mldsa44` oneof variant to `proto/tendermint/crypto/keys.proto`
and wired `PubKeyToProto` / `PubKeyFromProto`. **No new proto work is required
in IBC itself** — the IBC client deserializes the validator set through the same
CometBFT codec, so once the CometBFT fork's codec knows the hybrid variant, the
client transparently reconstructs hybrid pubkeys.

The only IBC-side requirement is a **size-bound bump**: the IBC client enforces
its own `MaxBytes` / header-size limits in `ClientState.Validate` and in the
relayer's chunking. A header from an all-hybrid 100-validator set carries
~134 KB of validator pubkeys + ~249 KB of commit signatures. The client's
header size ceiling must be raised to admit it (mirrors ADR-008's
`MaxCommitBytes` bump on the consensus side).

### G2 — Verification dispatch (already proven, now on the IBC surface)

`VerifyCommitLight` and `VerifyCommitLightTrusting` in `light/verifier.go` /
`types/validator_set.go` resolve each signer's pubkey from the trusted set and
dispatch to its `VerifySignature`. The landed `types/consensus_hybrid_test.go`
already proves both paths work over heterogeneous and all-hybrid sets:

- `VerifyCommitLight` (+`AllSignatures`) verifies a heterogeneous adjacent commit.
- `VerifyCommitLightTrusting` (the IBC **skipping** path) counts carried-over
  hybrid validators toward the trust level and rejects a sub-threshold overlap
  with `ErrNotEnoughVotingPowerSigned`.

**The IBC client inherits this for free** because it calls the *same* CometBFT
functions. G2 is therefore not new code in CometBFT — it is (a) confirming the
IBC `07-tendermint` module compiles against the hybrid-aware CometBFT fork, and
(b) an integration test that drives `MsgUpdateClient` with a hybrid header
end-to-end through the SDK's `x/ibc` keeper.

### G3 — Trust-overlap counting across a consensus-key rotation

The skipping path's security rests on **validator continuity**: it counts the
voting power of validators present in **both** the trusted set and the new
header's set. The identity used for the intersection is the **address**.

Because of the F1-b invariant (`HybridPubKey.Address()` == classical address), a
validator that rotates classical → hybrid via `MsgRotateConsKey`:

- keeps the **same address** → the skipping path still counts it as
  "carried over",
- but now signs with its **hybrid key** → the client must use the **new**
  (hybrid) pubkey from the header's validator set to verify that signature, not
  the stale classical pubkey from the trusted snapshot.

**Rule G3:** when counting trust overlap, match validators by **address**; when
verifying a matched validator's signature, use the pubkey from the **header's**
validator set (the set that produced the commit), not the trusted snapshot. This
is already how CometBFT's `VerifyCommitLightTrusting` is structured (it verifies
against `vals` = the commit's set, and counts overlap against `trustedVals`), so
the rotation case is covered **provided** the rotation preserves the address —
which ADR-008 guarantees.

**Failure mode if the invariant were violated:** if a rotation changed the
address, the skipping path would treat the validator as *new* (not carried
over), the trust overlap could drop below 1/3, and `MsgUpdateClient` would fail
with `ErrNotEnoughVotingPowerSigned` — a **safe** failure (liveness loss, not a
forged update). The relayer would fall back to the adjacent path. This is the
correct conservative behaviour and motivates keeping the F1-b invariant.

### G4 — Misbehaviour (`MsgSubmitMisbehaviour`) with hybrid keys

IBC freezes a client on proof of equivocation: two valid headers at the same
height with different `BlockID`s, each carrying a commit that verifies against
the (possibly overlapping) validator set. The verification reuses G2 dispatch,
so a hybrid validator's equivocation is detectable iff **both** halves of each
conflicting signature verify — strictly stronger than classical (a quantum
attacker who broke only Ed25519 cannot fabricate freezing misbehaviour against
an honest hybrid validator, mirroring ADR-008 §F5).

No new misbehaviour message; the existing `Misbehaviour` type carries two
`Header`s and runs the same hybrid-aware commit verification.

### G5 — Relayer compatibility

Relayers (Hermes, Go relayer) construct `MsgUpdateClient` by fetching headers
from chain A's RPC and submitting to chain B. They are **light clients in
user-space** and need the same three changes:

- **Header size:** raise the relayer's per-message and per-tx size limits to
  admit ~hundreds of KB headers for large hybrid sets (G1).
- **Pubkey decode:** the relayer's vendored CometBFT must know the hybrid oneof
  variant to deserialize A's validator set (satisfied by depending on the
  CometBFT fork tag).
- **No signing change:** the relayer does not sign consensus material; it only
  *relays*. Its own account signature to submit the tx on B is governed by
  ADR-007/ADR-010 (account keys), independent of this ADR.

A relayer that has **not** upgraded can still relay between two **classical**
chains and can still verify the **classical half** of a hybrid chain's commit if
pointed at a classical-only verification build — but it cannot construct a valid
`MsgUpdateClient` once chain A's set is majority-hybrid, because it cannot
deserialize the hybrid pubkeys. Practically, relayer operators upgrade in
lockstep with the chains they serve.

---

## Migration sequence (how two chains cross the gap without breaking the channel)

The channel between A and B stays **open** throughout. The transition is driven
entirely by **client upgrades**, not channel re-handshakes.

| Stage | Chain A consensus | Chain B's client of A | Channel A↔B |
|------:|-------------------|-----------------------|-------------|
| 0 | all classical | classical client | open (today) |
| 1 | A starts rotating (heterogeneous set) | **classical** client still verifies the **classical half** of every signature (ADR-008 §7 item 6) | open |
| 2 | A majority-hybrid | B's client **upgraded** to hybrid-aware (this ADR); verifies both halves for migrated validators | open |
| 3 | A all-hybrid; classical deprecated (Phase H) | B's client **must** be hybrid-aware or it can no longer verify | open iff B upgraded |

**Key safety property of Stage 1:** the classical half of a hybrid signature *is
a valid Ed25519 signature over the same sign bytes*. An un-upgraded client on B
verifies it correctly and never mis-verifies. The channel does not need to know
A is migrating until A deprecates classical (Stage 3) — which is governance-gated
and only proposed after counterparties have upgraded.

**Coordination:** Phase G ships the hybrid-aware client to B **before** A
proposes classical deprecation. The ordering is: (1) ADR-008 hybrid consensus
live on A, (2) ADR-009 client upgrade available and deployed by counterparties,
(3) governance on A proposes Phase H. Skipping (2) would strand B's channel.

---

## What this does NOT do (honest boundaries)

- **Does NOT make IBC packets quantum-safe in transit.** Packet commitments use
  SHA-256 (Grover-safe) and ICS-23 Merkle proofs; those are unchanged. This ADR
  is about *who signed the header*, not packet confidentiality (packets are
  public on-chain anyway).
- **Does NOT change the connection/channel handshake.** `ConnOpenInit/Try/Ack/
  Confirm` and `ChanOpen*` are untouched — they don't verify consensus
  signatures, they exchange and prove client/connection state via Merkle proofs.
- **Does NOT introduce a new IBC client type.** It extends `07-tendermint`
  in place. A `0x-pqc-tendermint` fork was considered and rejected (below).
- **Does NOT require both chains to upgrade simultaneously.** The classical-half
  fallback (Stage 1) makes the upgrade asynchronous.

---

## Alternatives considered

- **A brand-new `08-pqc-tendermint` client type.** Rejected: would force every
  counterparty to open a *new connection and channels* (full handshake, new
  channel IDs, liquidity migration) — a flag day for the entire IBC graph. The
  in-place extension keeps existing channel IDs and balances.
- **Verify only the classical half forever (never adopt PQC in the client).**
  Rejected: once chain A deprecates classical (Phase H), a classical-only client
  can no longer verify A's headers at all — the channel dies. Phase G is the
  prerequisite that *prevents* that.
- **Verify only the PQC half (drop classical in the client early).** Rejected
  during the hybrid era: loses defense-in-depth and breaks verification of
  not-yet-rotated validators in A's heterogeneous set.
- **Aggregate/compress commit signatures before relaying.** Deferred to a
  bandwidth optimization (ADR-008 §5 mitigation 2); not required for correctness.

---

## Consequences

**Positive**

- IBC channels survive a counterparty's consensus-key migration with **no
  channel re-handshake** and **no flag day**.
- Reuses the already-landed, already-tested hybrid `VerifyCommitLight*` paths —
  the IBC surface inherits the proof rather than re-deriving it.
- Misbehaviour detection becomes strictly stronger (G4) on hybrid validators.

**Negative / costs**

- IBC headers for large hybrid sets are **hundreds of KB** (G1) — higher relayer
  gas and per-update tx size; motivates the commit-compression optimization.
- Relayers must upgrade their vendored CometBFT to the fork tag (G5).
- Light clients on B store ~1,344 B per migrated validator pubkey instead of
  32 B — larger `ConsensusState`.

---

## Implementation plan (G1 → G5)

1. **G-harness (no SDK/IBC fork):** an integration test in the CometBFT fork that
   drives `VerifyCommitLightTrusting` across a **simulated rotation** (validator
   keeps address, swaps classical→hybrid pubkey between trusted snapshot and new
   header) and asserts (a) overlap counts it, (b) the hybrid signature verifies,
   (c) an address-changing rotation safely drops to `ErrNotEnoughVotingPowerSigned`.
   Proves G3 before any fork wiring.
2. **G-fork (gated on ibc-go fork + devnet):** depend `ibc-go` on the CometBFT
   fork tag; raise the client header size bound (G1); add an end-to-end
   `MsgUpdateClient` test through `x/ibc` with a hybrid header; confirm
   `MsgSubmitMisbehaviour` freezes on a hybrid double-sign (G4).
3. **Relayer note:** document the relayer's required fork-tag bump and size-limit
   config in `RUN_*` ops docs (G5); no relayer code change beyond the vendored
   CometBFT version.

The CometBFT fork (ADR-008) is the source of truth for the verification
primitives; Phase G is a **wiring + bounds + integration-test** exercise on the
IBC surface, exactly as ADR-008 §F4 is on the consensus surface.

---

## References

- `PROJECT_AEGIS_JUNO_FULL_PQC.md` §4.4 (IBC), §5.1 (sizes), §6 (determinism)
- `ADR-008-PQC-HYBRID-CONSENSUS.md` §F1-b (address invariant), §F4 (dispatch +
  landed `VerifyCommitLight*` tests), §F5 (evidence), §F6 (`MsgRotateConsKey`), §7
- `ADR-006-PQC-HYBRID-TRANSPORT.md` — same ML-DSA-44 primitive
- `aegis-forks/cometbft/types/validator_set.go` — `VerifyCommitLight`,
  `VerifyCommitLightTrusting`
- `aegis-forks/cometbft/light/verifier.go` — adjacent vs skipping dispatch
- ICS-07 (`07-tendermint`), ICS-02 (client), ICS-23 (commitment proofs)
- `cosmos/ibc-go` `modules/light-clients/07-tendermint`
