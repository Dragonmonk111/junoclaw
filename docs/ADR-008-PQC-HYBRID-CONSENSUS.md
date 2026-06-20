# ADR-008: Hybrid Ed25519 + ML-DSA-44 consensus signatures for CometBFT (Project Aegis Phase F)

**Status:** Partially implemented — **F1 (crypto types) + F2 (wire format) + F3 (`FilePV` dual-sign) LANDED & tested**; **F4 heterogeneous verification dispatch + F5 evidence/slashability LANDED & tested** (protoc-free, via in-memory hybrid validators); **consensus signature-size bounds raised** (`MaxSignatureSize`, `MaxCommitSigBytes`, `MaxCommitBytes`) for the 2,491 B hybrid signature. Still gated on protoc: F4 hybrid-pubkey **persistence** in `cmtproto.Validator` (oneof), F6 `MsgRotateConsKey`, remote signer. Phase F deliverable of [PROJECT_AEGIS_JUNO_FULL_PQC](./PROJECT_AEGIS_JUNO_FULL_PQC.md) §4.1, §5.1
**Date:** 2026-06-18
**Authors:** Dragonmonk / VairagyaNodes (with Cascade)
**Scope:** CometBFT validator consensus signing (surfaces #1 prevote/precommit, #2 commit aggregation, #14 genesis/gentx)
**Depends on:** Phase A foundations (algorithm-tagged keys), ADR-006 (same ML-DSA-44 impl), ADR-007 (same hybrid construction pattern)

> This ADR specifies the **consensus-layer** migration to hybrid post-quantum
> signatures. It is the highest-risk, highest-impact phase in Aegis: every
> validator signs every block, and a bug here halts the chain. No code change
> described here ships without the determinism checklist in §6 being fully
> satisfied.
>
> **Honest boundary:** consensus voting is the *last* classical surface in
> CometBFT. Transport (ADR-006, Phase C ✅) and accounts (ADR-007, Phase D-core ✅)
> are already proven. This ADR closes the loop on the chain's own plumbing.

---

## Context

CometBFT validators reach agreement via the Tendermint consensus algorithm:

1. A **proposer** broadcasts a `Proposal` for a block at `(height, round)`
2. Every validator broadcasts a **prevote** for that `BlockID` (or nil)
3. Every validator broadcasts a **precommit** for that `BlockID` (or nil)
4. When >2/3 precommits are seen for the same `BlockID`, the block is **committed**

Each of these three messages carries a **signature** over canonical signing bytes,
produced by the validator's consensus key. The key is held by `PrivValidator`
(`types/priv_validator.go`) — either a file on disk (`FilePV`) or a remote
signer (`tmkms`, Horcrux).

The validator's **public key** is stored in chain state (`ValidatorSet`) and
looked up by a 20-byte `Address` (hash of the pubkey). The `Vote` and `Proposal`
messages carry only the signature and the address; the pubkey comes from state.

### Current primitive: Ed25519

- **Public key:** 32 B — `ed25519.PubKeyEd25519`
- **Signature:** 64 B
- **Address:** `tmhash.SumTruncated(pubkey)` — 20 B
- **Speed:** ~1.2 ms sign, ~2.3 ms verify (per op, Go stdlib)

### Quantum threat

Ed25519 is a discrete-log primitive on Curve25519. **Shor's algorithm recovers the
private key from the public key in polynomial time.** A quantum adversary can:

- Forge votes for any validator whose pubkey is in state (all of them)
- Fake a commit by forging >2/3 precommit signatures
- Produce fake double-sign evidence that passes slashing verification
- Impersonate the proposer and fork the chain

This is a **Q-day** threat (not retroactive — signatures are one-shot). But
consensus keys are **long-lived** (months to years) and **high-exposure** (every
block, forever), so by the `exposure × lifetime` rule they are the **highest
priority** surface to migrate.

### Why ML-DSA-44 (not Falcon, not MAYO)

Per §5.1 of `PROJECT_AEGIS_JUNO_FULL_PQC.md`:

- **Determinism is non-negotiable.** Every validator must verify identically on
  every CPU/OS. ML-DSA verification is integer-only.
- **Falcon** (FN-DSA) relies on floating-point Gaussian sampling — a
  reproducibility and constant-time hazard for a heterogeneous validator set.
  FIPS 206 is still draft.
- **MAYO** is excellent for app-layer attestations (JunoClaw) but is not a
  finalized NIST standard and carries novelty risk (cf. Rainbow).
- **ML-DSA-44** is finalized (FIPS 204, NIST category 2), already vendored in
  `cloudflare/circl`, already measured on-chain (~270k gas), and is the
  **smallest** of the ML-DSA family (2,420 B sig). ML-DSA-65 saves one NIST
  category at a cost of +467 GB/year in block growth — not worth it for a
  hybrid key that still carries the Ed25519 half (§5.1).

---

## Decision

Make validator consensus keys **hybrid Ed25519 + ML-DSA-44**: a valid
vote/proposal/commit requires **both** signatures to verify. Security is the
*stronger* of the two. Un-rotated validators continue to validate with their
classical key during the transition window.

### F1 — New `crypto.PubKey` / `crypto.PrivKey` types in CometBFT

#### F1-a: Standalone `mldsa44` key type

Implement CometBFT's `crypto.PubKey` and `crypto.PrivKey` interfaces for a
pure ML-DSA-44 key, using `github.com/cloudflare/circl/sign/mldsa/mldsa44`
(already vendored and proven in D3 / ADR-007).

```
mldsa44.PubKey  = 1,312 B  // FIPS 204 public key
mldsa44.PrivKey = seed 32 B → expanded via ML-DSA.KeyGen
mldsa44.Signature = 2,420 B // FIPS 204 signature
```

`PubKey.Address()` = `tmhash.SumTruncated(pubkey)` — 20 B, **new address space**
distinct from Ed25519. This is used internally; the hybrid key (F1-b) delegates
`Address()` to the classical half so existing lookups work.

`PubKey.VerifySignature(msg, sig)` calls `mldsa44.Scheme().Verify(pub, msg, sig, nil)`.
Must pass NIST KAT vectors (embedded in tests).

#### F1-b: Hybrid `aegis-hybrid-ed25519-mldsa44` key type

Composes the existing `ed25519.PubKeyEd25519` + the new `mldsa44.PubKey`:

```
HybridPubKey {
    classical: ed25519.PubKey   // 32 B
    pqc:       mldsa44.PubKey  // 1,312 B
}

HybridPrivKey {
    classical: ed25519.PrivKey   // 64 B
    pqc:       mldsa44.PrivKey  // 32 B seed → expanded
}
```

**`VerifySignature(msg, sig)` — the critical path:**

```
sig = decodeHybridSig(rawSig)
// sig.classical = 64 B ed25519 signature
// sig.pqc       = 2,420 B mldsa44 signature

return classicalPub.VerifySignature(msg, sig.classical)
    && pqcPub.VerifySignature(msg, sig.pqc)
```

**Both halves MUST pass.** A classical break alone or a PQC break alone does
not forge.

**`Address()` — the migration invariant:**

```
func (pk HybridPubKey) Address() Address {
    return pk.classical.Address()  // IDENTICAL to pre-migration
}
```

This means all `ValidatorAddress → PubKey` lookups in state continue to work
**without a state migration**. The PQC half is carried inside `Bytes()` and
`Type()` but does not change the 20-byte address.

**`Type()`** returns `"aegis-hybrid-ed25519-mldsa44"` (registered in the codec).

---

### F2 — Wire format: how `Vote.Signature` grows

Current: `Vote.Signature` = 64 B (Ed25519).

Hybrid: `Vote.Signature` = **2,491 B** — a versioned, algorithm-tagged encoding:

```
[1 byte]  version = 0x01
[1 byte]  classical_algo_id = 0x01  // ed25519
[2 bytes] classical_sig_len = 64     // big-endian uint16
[64 B]    classical_sig
[1 byte]  pqc_algo_id = 0x02         // mldsa44
[2 bytes] pqc_sig_len = 2420         // big-endian uint16
[2420 B]  pqc_sig
```

Total: 2,491 B. Overhead: 7 bytes.

**Properties:**
- **Crypto-agile:** future schemes (ML-DSA-65, SLH-DSA, etc.) slot in by
  registering a new `algo_id` and length. Old verifiers reject unknown IDs.
- **Self-describing:** no out-of-band length table needed.
- **Backward-compatible:** a classical-only verifier sees `version=0x01` and
  rejects (it expects raw 64 B). It never mis-verifies.

**No proto schema change required.** The `Vote` and `Proposal` proto messages
already use `bytes signature = N;` — variable length. The `Commit` message
aggregates vote signatures; the size growth is the real cost (§5).

---

### F3 — `PrivValidator` signs with both halves

`PrivValidator` is the interface every validator implements
(`types/priv_validator.go`):

```go
type PrivValidator interface {
    GetPubKey() (crypto.PubKey, error)
    SignVote(chainID string, vote *cmtproto.Vote) error
    SignProposal(chainID string, proposal *cmtproto.Proposal) error
}
```

**`FilePV` modification:**

`SignVote` and `SignProposal` must:
1. Compute `signBytes` via `VoteSignBytes(chainID, vote)` / `ProposalSignBytes(chainID, proposal)`
2. Sign with the **classical** half
3. Sign with the **PQC** half
4. Encode as hybrid sig (F2 format)
5. Assign to `vote.Signature` / `proposal.Signature`

**Key storage:** `FilePV` currently stores one key in `priv_validator_key.json`:

```json
{
  "address": "ABCD...",
  "pub_key": {"type": "tendermint/PubKeyEd25519", "value": "base64..."},
  "priv_key": {"type": "tendermint/PrivKeyEd25519", "value": "base64..."}
}
```

We **add a sidecar file** `priv_validator_key_mldsa44.json` with the PQC half:

```json
{
  "address": "ABCD...",
  "pub_key": {"type": "aegis/PubKeyMlDsa44", "value": "base64..."},
  "priv_key": {"type": "aegis/PrivKeyMlDsa44", "value": "base64..."}
}
```

**Why a sidecar:** avoids breaking `tmkms`, Horcrux, and any tooling that parses
`priv_validator_key.json`. The PQC file is optional during transition; if
absent, the validator signs classical-only (§7, transition rules).

**`SignerClient` / tmkms:** the remote signer protocol (protobuf) needs to
support the new key type. The simplest path is: the remote signer holds **both**
halves and returns the full hybrid signature. A future iteration could split the
signing across two HSMs (classical in one, PQC in another).

---

### F4 — Validator set stores the hybrid pubkey

Validators are stored in `ValidatorSet` with `PubKey crypto.PubKey`. The key is
serialized via protobuf in `cmtproto.Validator`.

**Changes:**
1. New `PublicKey` proto variant `aegis_hybrid_ed25519_mldsa44` in
   `proto/tendermint/crypto/keys.proto` (oneof `PublicKey.Sum`)
2. Regenerate the protobuf and extend `PubKeyToProto` / `PubKeyFromProto` in
   `crypto/encoding/codec.go` (the codec uses a generated `pc.PublicKey` oneof,
   **not** JSON — so this step is `protoc`/`buf` gated; see §10)
3. Register the JSON type via `cmtjson.RegisterType` (for `FilePV` disk format —
   **not** protoc-gated, can land first)
4. `Validator.Address()` remains the classical half's address (F1-b invariant)

**Verification dispatch (the key safety property):** the `VerifySignature` path
in `types/vote_set.go` and `types/validator_set.go` looks up the validator's
pubkey from state and calls *that pubkey's* `VerifySignature`. The commit carries
only **raw signature bytes** plus the `ValidatorAddress`; the verifier resolves
the pubkey type from state and dispatches accordingly:
- pubkey is `HybridPubKey` → hybrid verify (both halves required)
- pubkey is `ed25519.PubKey` → classical verify (unchanged)

**Transition consequence:** during the migration window the `ValidatorSet` is
heterogeneous, so a single block's `Commit` contains a **mix** of classical-only
and hybrid signatures. Every validator can verify every signature because it
resolves each signer's registered key type from state — no validator needs to
guess. This is the exact same dispatch pattern as opt-in account migration in
ADR-007, applied at the consensus layer.

**Landed (protoc-free) — verification dispatch.** `types/consensus_hybrid_test.go`
drives the real `VerifyCommit` / `VerifyCommitExtended` path over in-memory
hybrid validators (`MockPV` wrapping `hybrid.GenPrivKey()`), proving:
- A heterogeneous set (2 classical + 2 hybrid), all signing, produces a `Commit`
  and `ExtendedCommit` that both verify — every signature dispatches on the
  signer's registered key type.
- A mid-migration block with one validator offline still commits: the surviving
  3-of-4 **mixed** quorum (always ≥1 classical and ≥1 hybrid) clears 2/3 — the
  literal §6 item-9 "upgrade without halting" scenario.
- An all-hybrid set verifies via single verification, and the batch-verify gate
  (`shouldBatchVerify`) correctly returns **false** for hybrid and mixed sets
  (ML-DSA-44 advertises no `crypto.BatchVerifier`), so the chain never enters an
  unsupported batch path.
- The **light-client / IBC** verification paths work on hybrid sets (§6 item 10):
  `VerifyCommitLight` (+`AllSignatures`) verifies a heterogeneous adjacent-header
  commit, and `VerifyCommitLightTrusting` (the IBC *skipping* path) counts
  hybrid validators carried across a validator-set rotation toward the trust
  level — and rejects a sub-threshold overlap with `ErrNotEnoughVotingPowerSigned`.

**Landed (proto regen + codec wiring).** `proto/tendermint/crypto/keys.proto`
now carries a third `PublicKey` oneof variant:
`bytes aegis_hybrid_ed25519_mldsa44 = 3` (ed25519(32) || mldsa44(1312) = 1,344 B).
`buf generate` regenerated `keys.pb.go`; `crypto/encoding/codec.go` wires
`hybrid.PubKey` into `PubKeyToProto` / `PubKeyFromProto` with size validation,
and registers the proto type for JSON encoding. Build, vet, and all tests
pass (`types/`, `crypto/...`, `privval/...`). This unblocks `Validator.ToProto()`
and state/genesis persistence of hybrid validator pubkeys.

**Consensus signature-size bounds (required for F2/F4).** A hybrid signature is
`hybrid.SignatureSize` = 2,491 B, far above the classical 64 B. Three bounds had
to be raised or they reject every hybrid `Vote`/`CommitSig`/`Proposal`/extension
in `ValidateBasic` and under-count block size:
- `types/signable.go` — `MaxSignatureSize` now `max(hybrid.SignatureSize, 64)`.
- `types/block.go` — `MaxCommitSigBytes` (2,537 B) and `MaxCommitBytes`'s
  per-sig proto overhead (2 → 3, for the 2-byte length prefix) track it, so
  `MaxDataBytes` block-size accounting stays correct. `TestMaxCommitBytes` /
  `TestBlockMaxDataBytes` assert the exact new byte budgets.

---

### F5 — Evidence / slashability with hybrid keys

`DuplicateVoteEvidence` carries two conflicting votes. The evidence verifier calls
`pubKey.VerifySignature(vote.SignBytes, vote.Signature)` using the validator's
pubkey from state.

**Security analysis:**
- If the validator has a hybrid key, both halves must verify for evidence to pass.
- A quantum attacker forges a double-sign: they must forge **both** the
  classical and PQC halves. Forging only the classical half → verification
  fails → evidence rejected → slashing prevented. This is **strictly stronger**
  than today.
- Evidence pool serialization: `Evidence` messages are gossiped rarely. The
  bandwidth impact of 2,484 B signatures in evidence is negligible.

**Test criterion:** Mock double-sign with hybrid key → evidence verified.
Classical-only forged double-sign → evidence rejected.

**Landed (protoc-free).** `evidence/verify_hybrid_test.go` exercises the real
`evidence.VerifyDuplicateVote` (the function a node calls before slashing):
- Two conflicting hybrid-signed precommits at the same H/R/S → evidence is
  **valid** (the malicious validator is slashable).
- A forged double-sign whose Ed25519 half is intact but ML-DSA-44 half is
  corrupted (a quantum attacker who broke only Ed25519 trying to *frame* an
  honest validator) → **rejected** with `ErrVoteInvalidSignature`. Fabricating
  slashable evidence now also requires breaking ML-DSA-44.

---

### F6 — `MsgRotateConsKey`: consensus key rotation

Consensus keys on Cosmos chains are **not rotatable in-place** today. The key is
set at `create-validator` or genesis and never changes.

**Path A (preferred): `MsgRotateConsensusKey`**

Add a new SDK message:

```proto
message MsgRotateConsensusKey {
    string validator_address = 1;  // bech32 operator address (valoper1...)
    google.protobuf.Any new_consensus_pubkey = 2;  // hybrid pubkey
}
```

- Validator signs with their **operator key** (already hybrid from Phase D)
- `x/staking` module updates the validator's `ConsensusPubkey` in state
- The change takes effect at `H+1` (next block)
- **Rate limit:** once per 24h per validator, to prevent DoS via repeated
  `ValidatorSet` hash changes
- **Light client impact:** the `ValidatorSet` hash changes. Light clients must
  receive a `Change` proof. This is normal for any validator set update, but
  the rate limit prevents spam.

**CometBFT ordering constraint:**

`VoteSignBytes` includes `ValidatorAddress` (20 B) but NOT the pubkey. The pubkey
is looked up from the `ValidatorSet` snapshot at the block's height. So if a
validator rotates at height `H+1`, their votes at height `H` still verify against
the old pubkey. This is the same as any validator set change — CometBFT already
handles it via `LastCommit` and the height-indexed validator set.

**Path B (fallback): genesis re-bootstrap at upgrade height**

- Chain-halt upgrade
- All validators submit new hybrid pubkeys off-chain
- Upgrade handler rebuilds validator set
- Restart

Requires coordinated downtime. Only acceptable if Path A fails.

**Decision:** implement Path A. It is the only path that works for a chain with
IBC channels that cannot pause.

---

## 5. Bandwidth — the real cost

Consensus signing is high-volume: every block, every validator signs.

| Validators | Ed25519 baseline | Hybrid-44 sigs | Δ per block | Δ per year |
|-----------:|-----------------:|---------------:|------------:|-----------:|
| 50 | 3.2 KB | 124 KB | +121 KB | ~640 GB |
| 100 | 6.4 KB | 248 KB | +242 KB | ~1.3 TB |
| 150 | 9.6 KB | 373 KB | +363 KB | ~1.9 TB |

(Assumes ~6 s blocks ≈ 14,400 blocks/day. SI units: 1 KB = 1,000 B.)

**Verify CPU is NOT the bottleneck:**

| Variant | Single verify | Per block (N=100) |
|---------|--------------:|------------------:|
| Ed25519 | ~2.3 ms | ~230 ms |
| ML-DSA-44 | ~101 µs | ~10 ms |
| Hybrid | ~2.4 ms | ~240 ms |

The PQC half adds ~10 ms per block — a sub-percent slice of a 6 s block.
**Bandwidth is the cost.**

**Mitigations (implement in Phase F, not later):**

1. **ML-DSA-44 already chosen** (not 65). Saves ~26 % signature bytes.
2. **Compress `Commit` metadata.** The `Commit` structure repeats
   `ValidatorAddress` for every signature. A compressed variant uses set-order
   indexing instead.
3. **Snapshot pruning.** Block store already prunes; growth rate matters more
   than absolute size.
4. **Lazy gossip.** Peers with the block don't need the full commit immediately.

**Test criterion:** 4-validator localnet, 1,000 blocks. Measure block size, peer
bandwidth, disk growth. Compare against classical baseline.

---

## 6. Determinism & consensus-safety checklist

Every item must have a test before Phase F ships.

| # | Requirement | Test |
|---|-------------|------|
| 1 | Bit-for-bit deterministic `VerifySignature` across x86/ARM/Linux/macOS | Run `go test` on WSL + bare metal; compare hash of `VerifySignature` results |
| 2 | Constant-time signing (side-channel resistant) | `cloudflare/circl` provides this; verify with `dudect` or `valgrind --tool=cachegrind` |
| 3 | No floating-point in verify path | Static analysis: `grep -r 'float\|Float' crypto/mldsa44/` returns nothing |
| 4 | NIST KAT vectors pass | Embed KAT JSON; test at `go test` time |
| 5 | Invalid classical half → reject, even if PQC half valid | Flip one bit in classical sig → `VerifySignature` returns false |
| 6 | Invalid PQC half → reject, even if classical half valid | Flip one bit in PQC sig → `VerifySignature` returns false |
| 7 | Old code (classical-only verifier) rejects hybrid sig | Feed hybrid sig to `ed25519.PubKey.VerifySignature` → false |
| 8 | Batch verification (optional but desired) | ⚠️ Not implemented for hybrid — ML-DSA-44 has no `crypto.BatchVerifier`. **Safe fallback verified:** `shouldBatchVerify` returns false for hybrid/mixed sets, so verification uses per-signature dispatch (`types/consensus_hybrid_test.go`) |
| 9 | Fork coordination: un-rotated validators still validate | ✅ `types/consensus_hybrid_test.go` — heterogeneous all-sign commit verifies; 3/4 mixed quorum (1 offline) reaches 2/3 |
| 10 | Light client can verify hybrid headers | ✅ `types/consensus_hybrid_test.go` — `VerifyCommitLight`(+AllSignatures) verifies a heterogeneous commit; `VerifyCommitLightTrusting` (IBC skipping) counts carried-over hybrid validators toward the trust level and rejects a sub-threshold overlap |
| 11 | Evidence verification with hybrid keys | ✅ `evidence/verify_hybrid_test.go` — hybrid double-sign verified; classical-only forgery rejected (`ErrVoteInvalidSignature`) |
| 12 | `MsgRotateConsKey` rate limit enforced | Submit two rotations within 24h → second rejected |

---

## 7. Transition rules (how the chain upgrades without halting)

1. **Hybrid validators and classical validators coexist** in the same
   `ValidatorSet` during the transition window.
2. A block's `Commit` may contain **both** classical-only and hybrid signatures.
3. Each signature is verified against the **validator's registered pubkey type**
   in state at that height.
4. **No validator is forced to rotate.** Governance sets a target threshold
   (e.g., 66 % hybrid) before Phase H (deprecate classical) is proposed.
5. **Light clients** verify the classical half only until they upgrade to hybrid
   support. This is safe because the classical half is still a valid signature.
6. **IBC channels** remain open. The classical half of hybrid signatures is
   valid under the existing `07-tendermint` client until both chains upgrade
   (Phase G).

---

## 8. What this does NOT do (honest boundaries)

- **Does NOT make the chain "fully quantum-safe" on day one.** Classical
  signatures are still present and still verify. The chain is hybrid.
- **Does NOT change block hashes or Merkle trees.** SHA-256 is kept (Grover-safe).
- **Does NOT change the validator set update mechanism** beyond adding
  `MsgRotateConsKey`. The staking module's power reduction, delegation, and
  unbonding logic is untouched.
- **Does NOT change proposer selection.** Still weighted-by-stake; the key type
  does not affect selection.
- **Does NOT make IBC quantum-safe by itself.** The counterparty must also
  upgrade (Phase G).

---

## 9. Implementation gating — what lands without protoc

Phase F splits cleanly into a **protoc-free** layer (pure Go crypto + JSON, can
land and test today) and a **protoc-gated** layer (regenerating the `pc.PublicKey`
oneof + the remote-signer protobuf). Sequencing this way means F1/F2/F3 are
verifiable on a localnet before the heavier proto regeneration.

| Task | protoc needed? | Why | Can land now? |
|------|:--------------:|-----|:-------------:|
| F1-a `mldsa44` key type | No | Pure Go + `cmtjson.RegisterType` | ✅ |
| F1-b hybrid key type | No | Composes F1-a + ed25519; JSON only | ✅ |
| F2 hybrid sig wire format | No | Hand-rolled length-prefixed `[]byte` codec | ✅ |
| F3 `FilePV` signs both halves | No | Sidecar `priv_validator_key_mldsa44.json` (JSON) | ✅ |
| F4 verification dispatch (heterogeneous commit) | No | `VerifyCommit` resolves each signer's key from the in-memory `ValidatorSet` | ✅ tested |
| F4 validator pubkey **persistence** in state | **Yes** | `pc.PublicKey` oneof is a generated proto; needs new `Sum` variant | ❌ gated |
| F5 evidence verify | No | Reuses F1-b `VerifySignature` dispatch | ✅ tested |
| F6 `MsgRotateConsKey` | **Yes** | New SDK proto message + `x/staking` handler | ❌ gated |
| Remote signer (tmkms) | **Yes** | `privval` protobuf carries the sig type | ❌ gated |

**Unblock command (run once in WSL):**

```bash
sudo apt update && sudo apt install -y protobuf-compiler
go install github.com/bufbuild/buf/cmd/buf@latest
```

The CometBFT fork regenerates protos via `make proto-gen` (uses the `buf`
toolchain pinned in `buf.gen.yaml`). The Cosmos SDK fork (for F6) uses its own
`make proto-gen`.

**Dependency add:** the CometBFT fork's C5 work used **stdlib-only** crypto
(`crypto/mlkem`). Phase F adds `github.com/cloudflare/circl` (ML-DSA-44) — the
same version pinned in the SDK fork for D3 (`v1.6.1`) to keep one vendored
ML-DSA-44 across transport, accounts, and consensus. Run `go get
github.com/cloudflare/circl@v1.6.1` in the CometBFT fork.

---

## 10. References

- `PROJECT_AEGIS_JUNO_FULL_PQC.md` §4.1, §5.1, §6 — full PQC migration plan
- `ADR-006-PQC-HYBRID-TRANSPORT.md` — same ML-DSA-44 / ML-KEM-768 primitives
- `ADR-007-PQC-HYBRID-ACCOUNTS.md` — same hybrid construction pattern
- `aegis-forks/cometbft/crypto/crypto.go` — `PubKey` / `PrivKey` interfaces
- `aegis-forks/cometbft/types/priv_validator.go` — `PrivValidator` interface
- `aegis-forks/cometbft/types/vote.go` — `Vote` signature field
- `aegis-forks/cometbft/types/proposal.go` — `Proposal` signature field
- FIPS 204 — ML-DSA standard
- `cloudflare/circl/sign/mldsa/mldsa44` — Go implementation

---

*This ADR is a specification. Implementation tasks: F1–F7 in
`docs/FRAMEWORK_PLAN_REMAINING.md` §2.*
