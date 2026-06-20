# Remaining Tasks — Project Aegis & Fable

> Deep framework plan for everything between today and "Juno is quantum-safe."
> Built from `PROJECT_AEGIS_JUNO_FULL_PQC.md`, `COMPLETION_PLAN.md`, and the
> actual fork code. Every task maps to a file, a test, or a commit.
>
> **Date:** 2026-06-18 | **Status:** ACTIVE — Phase F in progress

---

## 0. What is done (the foundation we stand on)

| Phase | What | Proof | Location |
|-------|------|-------|----------|
| A | ML-DSA-44/65/87 + ML-KEM-768 impls vendored; `cloudflare/circl` Go + `fips203`/`fips204` Rust | go test 20/20 PASS; Rust cross-checks bit-for-bit | `aegis-accounts/`, `aegis-kem-diff/`, `aegis-bench/` |
| B | `ml_dsa_verify` host fn on wasmvm fork; `jclaw-credential` dogfoods it | on-chain gas measured (260k–387k) | `docs/MLDSA_PRECOMPILE_BENCHMARK_RESULTS.md` |
| C | Hybrid transport (X25519 + ML-KEM-768) in CometBFT fork; RTT measured | go test ./p2p/conn/ all PASS incl. evil+golden UNCHANGED; +371 µs CPU, +3.4 KB wire | `aegis-forks/cometbft/p2p/conn/secret_connection_hybrid.go` |
| D-core | Hybrid account key (secp256k1 + ML-DSA-44) in Cosmos SDK fork | go test ./crypto/keys/hybrid/ 9/9 PASS | `aegis-forks/cosmos-sdk/crypto/keys/hybrid/` |
| P3–P5 | MAYO-1/2/3/5 on-chain; gas ladder reproduced; comms shipped | MAYO-5 <800k gas; precompile 2.21× at L5 | `docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md`, `MEDIUM_ARTICLE_AEGIS_FABLE_MILESTONE.md` |

---

## 1. The Remaining Surface (all phases)

```
D-wiring ──> E ──> F ──> G ──> H
   │          │    │     │
   └─proto    └─gov └─consensus └─IBC  └─deprecate classical
```

| Phase | Ships | Risk | Effort estimate |
|-------|-------|------|-----------------|
| **D-wiring** | Proto `Any` codec, keyring algo, CLI `--algo hybrid`, gas case | Low | 1–2 days |
| **E** | Treasury/governance → hybrid accounts (+ SLH-DSA cold hedge) | Medium | 2–3 days |
| **F** | **Consensus voting with hybrid Ed25519 + ML-DSA-44** | **Very high** | **2–4 weeks** |
| **G** | IBC `07-tendermint` light client verifies hybrid validator sets | Medium | 1–2 weeks |
| **H** | Governance flips minimums: require PQC half where safe | Low (political) | 1 day |

**The whole claim stands on Phase F.** Everything before it is the warm-up. Phase F is where we replace the signature that secures the chain's *most critical* action: validators agreeing on blocks.

---

## 2. Phase F Deep Dive — Consensus Hybrid Signatures

This is the most consequential and dangerous change in the whole plan. One bug here is a chain halt or a consensus split, not a failed transaction.

> **LANDED 2026-06-18 — F1 (crypto types) + F2 (wire format), protoc-free:**
>
> | Item | Package | Result |
> |------|---------|--------|
> | F1-a standalone ML-DSA-44 key | `aegis-forks/cometbft/crypto/mldsa44/` | `crypto.PubKey`/`PrivKey` implemented; seed-form privkey; `Address()=tmhash.SumTruncated`; verify via `cloudflare/circl` ML-DSA-44; `go test` PASS, `go vet` clean |
> | F1-b hybrid key | `aegis-forks/cometbft/crypto/hybrid/` | composes ed25519 + mldsa44; `VerifySignature` requires **both** halves; `Address()` delegates to classical (zero state migration); `go test` PASS |
> | F2 wire format | `crypto/hybrid/hybrid.go` `encode/decodeHybridSig` | versioned, algo-tagged, **2,491 B** (7 B framing + 64 + 2,420); strict decode rejects unknown version/algo/len |
> | Dep bump | `aegis-forks/cometbft/go.mod` | `cloudflare/circl v1.3.7 → v1.6.1` (same ML-DSA-44 as SDK fork D3); `go build ./crypto/...` clean |
>
> Security-property tests green: round-trip; tamper-classical→reject; tamper-PQC→reject; size assertions; classical-only verifier rejects hybrid sig (framing/version mismatch). **Still open in F1:** embed NIST FIPS 204 KAT JSON (checklist #4) and cross-platform determinism hash (checklist #1).

> **LANDED 2026-06-18 — F4 (PrivValidator signs both halves), protoc-free:**
>
> | Item | File | Result |
> |------|------|--------|
> | Sidecar key type + load/gen/save | `aegis-forks/cometbft/privval/file_pqc.go` | `FilePVKeyMlDsa44` stored at `priv_validator_key.json_mldsa44.json`; classical key file never mutated (downgrade path preserved) |
> | `FilePV.signingPrivKey()` | `privval/file.go` | returns `hybrid.PrivKey` when sidecar present, else classical — all 4 sign call sites (vote, vote-extension, proposal) route through it |
> | `FilePV.GetPubKey()` | `privval/file.go` | returns hybrid pubkey when sidecar present; `Address()` still delegates to classical half (zero state migration) |
> | Tests | `privval/file_pqc_test.go` | `TestHybridSignVote`/`SignProposal`/`PersistReload`/`ClassicalPVUnaffected` PASS; hybrid sig is 2,491 B, verifies under hybrid pubkey, rejected by classical-only verifier; address unchanged across persist+reload |
>
> `go build ./privval/...` clean; full `go test ./privval/...` PASS (21.5s). **Still open in F4:** `SignerClient` (tmkms/remote) hybrid path — only `FilePV` is wired so far.

> **LANDED 2026-06-18 — F5 (validator set holds + verifies hybrid pubkey, rotation address-invariant), protoc-free:**
>
> | Item | File | Result |
> |------|------|--------|
> | Hybrid pubkey is a first-class `crypto.PubKey` in the set | `aegis-forks/cometbft/types/validator_hybrid_test.go` | `NewValidator` + `ValidateBasic` accept it; `GetByAddress`/`GetByIndex` resolve it; address = Ed25519-half address (zero state migration) |
> | Vote verifies through the consensus path | same | hybrid-signed prevote (2,491 B sig) verifies via `vote.Verify` (= `val.PubKey.VerifySignature`); classical-only Ed25519 verifier **rejects** it |
> | **Rotation invariant (§F5 core)** | same | reuse Ed25519 half + fresh ML-DSA-44 half → **same address, different key**; `UpdateWithChangeSet` swaps the entry with no lookup disruption; post-rotation vote verifies under new pubkey but is **rejected** by pre-rotation pubkey (proves PQ trust anchor actually moved) |
>
> `go build ./types/... ./privval/...` clean; 3 F5 tests PASS; `go test ./privval/...` still PASS (no regression). **Gated on protoc/buf (folds into F7):** `Validator.Bytes()`/`ValidatorSet.Hash()`/`Validator.ToProto()` + `crypto/encoding.PubKeyToProto`/`FromProto` need the hybrid `pc.PublicKey` oneof variant for over-the-wire / genesis / state-store persistence.

### 2.1 What consensus signing actually is (CometBFT v0.38.x)

Three surfaces sign with the validator's key:

1. **`Vote`** (prevote / precommit) — `types/vote.go:52,72`
   - `Signature []byte` — signs `VoteSignBytes(chainID, voteProto)`
2. **`Proposal`** — `types/proposal.go:32`
   - `Signature []byte` — signs `ProposalSignBytes(chainID, proposalProto)`
3. **`Commit`** — `types/commit.go`
   - Aggregates precommit signatures for the block; verified by peers replaying `VoteSignBytes`

The validator's key is held by `PrivValidator` (`types/priv_validator.go:15`):

```go
type PrivValidator interface {
    GetPubKey() (crypto.PubKey, error)
    SignVote(chainID string, vote *cmtproto.Vote) error
    SignProposal(chainID string, proposal *cmtproto.Proposal) error
}
```

Concrete impls: `FilePV` (disk), `SignerClient` (tmkms/remote). Every validator runs one.

**The validator pubkey is stored in chain state** (`ValidatorSet`, `State`) and looked up by `ValidatorAddress` (20-byte hash of pubkey). The `Vote`/`Proposal` carry only the signature bytes and the address — the pubkey comes from state.

### 2.2 The change: what we actually modify

**Goal:** every validator's consensus key becomes a **hybrid Ed25519 + ML-DSA-44** key. A valid vote/proposal requires **both** signatures to verify.

This touches **7 code areas** in the CometBFT fork, in this order:

#### Step F1 — Hybrid `crypto.PubKey` / `crypto.PrivKey` in CometBFT

**File:** `aegis-forks/cometbft/crypto/keys/hybrid/` (NEW)

Implement CometBFT's `crypto.PubKey` and `crypto.PrivKey` interfaces for a hybrid key:

```go
type HybridPubKey struct {
    Classical crypto.PubKey  // ed25519.PubKeyEd25519
    PQC       crypto.PubKey  // mldsa44.PubKey (NEW, see F2)
}

type HybridPrivKey struct {
    Classical crypto.PrivKey
    PQC       crypto.PrivKey
}

func (pk HybridPubKey) VerifySignature(msg, sig []byte) bool {
    // sig = classical_sig || pqc_sig (length-prefixed or fixed-offset)
    // BOTH must pass
}

func (pk HybridPubKey) Address() Address {
    // MUST be the SAME 20-byte address as the classical half
    // during transition, so existing ValidatorAddress lookups work
    return pk.Classical.Address()
}
```

**Critical invariant:** `Address()` returns the **classical half's address** unchanged. This means all `ValidatorAddress → PubKey` lookups in state continue to work without a state migration. The PQC half is tagged inside `Bytes()` and `Type()`.

**Test criterion:** `VerifySignature` returns `false` if either half fails. `Address()` is identical to classical-only. `Type()` returns `"aegis-hybrid-ed25519-mldsa44"`.

#### Step F2 — `mldsa44` standalone `crypto.PubKey` / `PrivKey`

**File:** `aegis-forks/cometbft/crypto/mldsa44/` (NEW)

A standalone ML-DSA-44 key type implementing CometBFT's `crypto.PubKey`/`PrivKey`, using the same `cloudflare/circl/sign/mldsa/mldsa44` package already proven in D3.

```go
type PubKey []byte  // 1,312 B
func (pk PubKey) VerifySignature(msg, sig []byte) bool { ... }
func (pk PubKey) Address() Address { return AddressHash(pk) }  // NEW address space

func (privKey PrivKey) Sign(msg []byte) ([]byte, error) { ... }
```

This is needed because the hybrid key **composes** two key types. The ML-DSA half must exist independently so it can be registered, tested, and eventually used alone in Phase H.

**Test criterion:** NIST KAT vectors pass. Hybrid construction passes against `aegis-accounts/` cross-checks. `BatchVerifier` interface optionally implemented (performance boost for commit verification).

#### Step F3 — Wire format: how `Vote.Signature` grows

Currently `Vote.Signature` is 64 bytes (Ed25519). With hybrid, it becomes:

```
Signature = classical_sig(64 B) || pqc_sig(2,420 B) = 2,484 B
```

**This is a protocol-level wire change.** The `Vote` struct in `types/vote.go` uses `Signature []byte` — that's already variable-length, so no struct change is needed. But **protobuf encoding** matters:

- `types/vote.proto` — the `Vote` proto message has `bytes signature = 7;` — variable length, OK
- The **size** goes from 64 B to 2,484 B per vote
- `Commit` aggregates `N` vote signatures — see §2.5 for bandwidth math

**No proto schema change required** if we encode the concatenation as raw `bytes`. But we MUST document the wire format and add a `SignatureVersion` or algorithm tag so future verifiers know how to split it.

**Decision:** encode as `bytes` with an internal header:

```
[1 byte version = 0x01]
[1 byte classical_algo_id = 0x01] // ed25519
[2 bytes classical_sig_len = 64] // big-endian uint16
[64 bytes classical_sig]
[1 byte pqc_algo_id = 0x02]     // mldsa44
[2 bytes pqc_sig_len = 2420]
[2420 bytes pqc_sig]
```

Total overhead: 7 bytes. Total signature: 2,491 B. This is **crypto-agile** — future schemes slot in without breaking old verifiers (they reject unknown algo IDs).

**Test criterion:** Marshal/unmarshal round-trip. Old verifier (classical-only) rejects hybrid sig (too long / unknown version). Hybrid verifier rejects truncated sig. Deterministic across all platforms.

#### Step F4 — `PrivValidator` signs with both halves

**File:** `aegis-forks/cometbft/types/priv_validator.go` — modify `FilePV`, `SignerClient`

```go
func (pv FilePV) SignVote(chainID string, vote *cmtproto.Vote) error {
    signBytes := VoteSignBytes(chainID, vote)
    
    // Classical half
    classicalSig, err := pv.ClassicalKey.Sign(signBytes)
    if err != nil { return err }
    
    // PQC half
    pqcSig, err := pv.PQCKey.Sign(signBytes)
    if err != nil { return err }
    
    vote.Signature = EncodeHybridSignature(classicalSig, pqcSig)
    return nil
}
```

`SignProposal` identical pattern.

**Critical:** `FilePV` stores the private key on disk in `priv_validator_key.json`. We need to extend this JSON format to hold **both** halves without breaking tmkms or Horcrux. Or we store them side-by-side:

```
priv_validator_key.json          (classical Ed25519 — unchanged)
priv_validator_key_mldsa44.json  (new — PQC half)
```

The PQC file uses the same JSON envelope but with `"type": "aegis-hybrid-mldsa44"`.

**Test criterion:** MockPV with hybrid key signs a vote; `VoteSignBytes` + `VerifySignature` round-trip. FilePV persists and reloads both halves. SignVote never produces a classical-only sig.

#### Step F5 — State stores the hybrid pubkey

**File:** `aegis-forks/cometbft/types/validator.go`, `state/store.go`

Validators are stored in `ValidatorSet` with `PubKey crypto.PubKey`. The key is serialized via protobuf in `cmtproto.Validator`.

We need:
1. A new `PublicKey` proto variant for hybrid: `aegis_hybrid_ed25519_mldsa44`
2. Register it in the address codec (`crypto/encoding/codec.go`)
3. Ensure `Validator.Address()` (derived from pubkey) remains the **classical half's address** during transition

**The hard part:** when a validator upgrades from classical → hybrid, their `PubKey` in state changes type but `Address()` must NOT change. This is why Step F1's `Address()` delegates to the classical half.

**Test criterion:** Load a classical validator set, replace one validator's key with hybrid, `Address()` unchanged. Re-serialize via protobuf, round-trip.

#### Step F6 — Evidence / slashability

**File:** `aegis-forks/cometbft/types/evidence.go`

`DuplicateVoteEvidence` carries two conflicting votes. The evidence verifier calls `pubKey.VerifySignature(vote.SignBytes, vote.Signature)`.

Once hybrid keys are in state, this path works automatically IF the evidence verifier uses the validator's pubkey from state (it does). But we need to confirm:

- Double-sign detection still triggers on classical half forgery alone? **No** — hybrid requires both halves, so a classical-only forged vote fails verification. A quantum attacker must forge **both** halves to evade slashing. This is correct and stronger.
- Evidence pool serialization handles 2,484 B signatures. `Evidence` is gossiped — bandwidth impact per evidence event is small (rare events).

**Test criterion:** Mock double-sign with hybrid key → evidence verified. Mock classical-only forged double-sign → evidence rejected.

#### Step F7 — `MsgRotateConsKey` — the coordination problem

Consensus keys on Cosmos chains are **not rotatable in-place.** The current key is set at genesis or via `create-validator` tx, and there's no `MsgRotateConsKey` message type.

Two paths:

**Path A: `MsgRotateConsKey` governance message (preferred)**

Add a new SDK message type in `x/staking` or `x/slashing`:

```proto
message MsgRotateConsensusKey {
    string validator_address = 1;  // bech32 operator address
    google.protobuf.Any new_consensus_pubkey = 2;  // hybrid pubkey
}
```

- Validator submits the tx with their **operator** key (already hybrid from Phase D)
- `staking` module updates the validator's `ConsensusPubkey` in state
- CometBFT state sync picks it up at the next block
- Old votes signed with classical-only key are still valid until the rotation tx is included

**Problems to solve:**
1. CometBFT's `ValidatorSet` is a snapshot at block `H`. If the key rotates at `H+1`, votes at `H` must still verify with the old key. This is normal for any validator set change, but needs careful ordering.
2. Light clients track validator set by pubkey hash. A key rotation changes the set hash. The light client `Change` proof must include the rotation.
3. Rotation must be **rate-limited** (e.g., once per 24h) to prevent DoS via repeated set-hash changes.

**Path B: Genesis re-bootstrap at upgrade height**

- Governance passes a chain-halt upgrade
- At the halt height, all validators submit their new hybrid pubkey off-chain
- The upgrade handler rebuilds the validator set with hybrid keys
- Chain restarts

Simpler, but requires **coordinated downtime** — unacceptable for a live chain with IBC channels.

**Decision:** implement Path A. It's harder but it's the only path that works for a chain that doesn't stop.

**Test criterion:** Single-validator devnet: submit `MsgRotateConsKey`, verify that subsequent blocks are signed with hybrid key, prior blocks still verify with old key. Multi-validator test: rotate one validator, confirm consensus continues.

---

### 2.3 Bandwidth — the real cost (from §5.1, reproduced)

Consensus signing is high-volume: every block, every validator signs.

| Validators | Ed25519 baseline | Hybrid-44 sigs | Δ per block | Δ per day (~14,400 blocks) | Δ per year |
|-----------:|-----------------:|---------------:|------------:|--------------------------:|-----------:|
| 50 | 3.2 KB | 124 KB | +121 KB | ~1.7 GB | ~640 GB |
| 100 | 6.4 KB | 248 KB | +242 KB | ~3.5 GB | ~1.3 TB |
| 150 | 9.6 KB | 373 KB | +363 KB | ~5.2 GB | ~1.9 TB |

**This is not fatal, but it is not free.** The chain's block size limit, disk growth, and P2P gossip bandwidth must absorb this.

**Mitigations (implement in Phase F, not after):**

1. **Compress commit signatures.** Classical Ed25519 signatures are not compressible, but the `Commit` structure repeats metadata. Consider adding a `CompressedCommit` variant that strips redundant `ValidatorAddress` entries (they're implicit in validator set order).
2. **Use ML-DSA-44, not 65.** Already decided (§5.1). Saves ~26 % signature bytes.
3. **Lazy gossip of full commits.** Peers that already have the block don't need the full commit immediately — they need only their own `CommitSig` to advance. This is already partially true in CometBFT's peer catch-up.
4. **Snapshot pruning.** Block store snapshots already prune old blocks. The growth rate matters more than the absolute size.

**Test criterion:** Run a 4-validator localnet with hybrid consensus for 1,000 blocks. Measure block size, peer bandwidth, and disk growth. Compare against classical baseline under identical load.

---

### 2.4 Determinism & consensus-safety checklist

These are acceptance criteria, not nice-to-haves. Every item must have a test.

| # | Requirement | Test |
|---|-------------|------|
| 1 | Bit-for-bit deterministic `VerifySignature` across x86/ARM/Linux/macOS | Run `go test` on WSL + bare metal; compare hash of `VerifySignature` results |
| 2 | Constant-time signing (side-channel resistant) | `cloudflare/circl` already provides this; verify with `dudect` or `valgrind --tool=cachegrind` |
| 3 | No floating-point in verify path | Static analysis: `grep -r 'float\|Float\|math/big' crypto/mldsa44/` should return nothing except `big.Int` |
| 4 | NIST KAT vectors pass | Embed KAT JSON; test at `go test` time |
| 5 | Invalid classical half → reject, even if PQC half valid | Test: flip one bit in classical sig → `VerifySignature` returns false |
| 6 | Invalid PQC half → reject, even if classical half valid | Test: flip one bit in PQC sig → `VerifySignature` returns false |
| 7 | Old code (classical-only verifier) rejects hybrid sig | Test: feed hybrid sig bytes to `ed25519.PubKey.VerifySignature` → false |
| 8 | Batch verification (optional but desired) | Implement `crypto.BatchVerifier`; test 100-vote batch vs 100× single |
| 9 | Fork coordination: un-rotated validators still validate | Test: 3/4 validators rotated to hybrid, 1 classical → consensus still reaches 2/3 |

---

### 2.5 Light client impact

CometBFT light clients verify headers by checking `Commit` signatures against the validator set. External light clients (IBC `07-tendermint`, external indexers, CEX deposit verifiers) must be able to verify hybrid signatures.

**This means Phase F is not internally complete until:**

1. The hybrid header format is documented and published
2. A Go verification library is released (can reuse `aegis-forks/cometbft/crypto/keys/hybrid/`)
3. IBC `07-tendermint` client is updated (Phase G)

**Do not ship Phase F without a published verifier lib.** Otherwise Juno becomes unverifiable by anyone outside the node software.

---

## 3. Phase D Wiring (unblock today)

Blocked on: `protoc` + `buf` not installed in WSL.

**To unblock (run once):**

```bash
wsl.exe --% -e bash -lc "sudo apt update && sudo apt install -y protobuf-compiler buf"
```

Then:

| Step | File(s) | What |
|------|---------|------|
| D3-a | `aegis-forks/cosmos-sdk/crypto/keys/hybrid/hybrid.proto` | Protobuf `Any` type for `HybridPubKey` + `HybridPrivKey` |
| D3-b | `aegis-forks/cosmos-sdk/codec/codec.go` | Register hybrid types in `InterfaceRegistry` |
| D3-c | `aegis-forks/cosmos-sdk/crypto/keyring/keyring.go` | Add `SignatureAlgo` case for `"hybrid"` |
| D3-d | `aegis-forks/cosmos-sdk/client/keys/add.go` | CLI `keys add --algo hybrid` flag |
| D3-e | `aegis-forks/cosmos-sdk/x/auth/ante/sigverify.go` | Gas case `SigVerifyCostMlDsa44` in `DefaultSigVerificationGasConsumer` |
| D3-f | Tests | End-to-end: `keys add --algo hybrid` → `tx sign` → `tx broadcast` → verify on devnet |

**Effort:** 1–2 days once protoc is installed. The hard part (crypto math) is done.

---

## 4. Phase E — Governance/Treasury Migration

**Depends on:** D (hybrid accounts working end-to-end)

**What:** Move the chain's most valuable and longest-lived keys to hybrid first.

| Surface | Current key | Action |
|---------|-------------|--------|
| Treasury module authority | Module account (classical) | Create hybrid account, transfer authority via governance |
| Upgrade authority (`x/upgrade`) | Module account | Same pattern |
| Governance depositors | Classical accounts | Encourage/incentivize hybrid migration for whales |
| Cold treasury (deep storage) | Optional SLH-DSA hedge | Governance creates SLH-DSA multisig for keys that sign rarely |

**SLH-DSA (FIPS 205) decision:** for the *coldest* keys (upgrade authority, deep treasury), offer SLH-DSA as an optional break-glass. Hash-based, security reduces to SHA-2 alone. Signature size ~17–50 KB — irrelevant for keys that sign a few times per year.

**No code change required for SLH-DSA today** — just register it as a `PubKey` type and use it in a multisig. The work is governance coordination, not engineering.

---

## 5. Phase G — IBC Hybrid Client

**Depends on:** F (consensus hybrid signatures in state)

**What:** when Juno validators sign hybrid, the IBC `07-tendermint` light client on the *counterparty* chain must verify hybrid validator sets.

**Work:**

1. New IBC client type: `07-tendermint-hybrid` (or a param on existing `07-tendermint`)
2. Client state stores `hybrid = true` flag
3. Header verification checks both classical AND PQC halves when `hybrid = true`
4. During transition, client verifies only classical half (safe — no downgrade)

**Honest boundary:** IBC is only as quantum-safe as the weakest chain in the channel. Juno→Osmosis is PQC only when *both* run hybrid consensus.

**Effort:** 1–2 weeks. Mostly ibc-go changes; the crypto verification logic reuses Phase F code.

---

## 6. Sequencing & Parallelization

```
Week 1–2:  D-wiring (parallel) + F1–F2 (hybrid crypto types in CometBFT fork)
Week 2–3:  F3–F5 (wire format, PrivValidator, state storage) + tests
Week 3–4:  F6–F7 (evidence, MsgRotateConsKey) + multi-validator localnet
Week 4–5:  F multi-validator test (4+ nodes, 1,000 blocks, bandwidth benchmark)
Week 5–6:  Phase E (governance migration) — can parallel with F testing
Week 6–8:  Phase G (IBC client) — blocked until F stable
Week 8+:   Phase H (deprecate classical) — governance vote, not engineering
```

**What can ship independently:**
- D-wiring: no dependencies, just protoc
- F1–F2: standalone crypto package, tests only
- E: governance coordination, not code

**What must wait:**
- F3–F7: all need F1–F2
- G: needs F
- H: needs everything

---

## 7. Blockers & Mitigations

| Blocker | Status | Mitigation |
|---------|--------|------------|
| `protoc`/`buf` not installed | Known, trivial | `sudo apt install protobuf-compiler buf` |
| Consensus key rotation (`MsgRotateConsKey`) does not exist | Design needed | ADR + prototype; fallback is genesis re-bootstrap (ugly but functional) |
| Block size / gossip bandwidth at 100 validators | Theoretical risk | Measure on 4-node localnet first; compress Commit metadata; ML-DSA-44 already chosen |
| Light client breakage | Certain without Phase G | Do not ship F without published verifier lib + Phase G design |
| tmkms / Horcrux compatibility | Unknown | FilePV side-by-side key files; tmkms needs protobuf update for hybrid sig format |
| Upstream merge (CometBFT / Cosmos SDK) | Political, not technical | Fork works for devnet; governance upgrade can run forked binary; upstream merge is Phase H+ |

---

## 8. What to do TODAY

1. **Install protoc/buf** → unblock D-wiring (30 min)
2. **Start F1** → create `aegis-forks/cometbft/crypto/mldsa44/` package with NIST KAT tests (1 day)
3. **Start F2** → create `aegis-forks/cometbft/crypto/keys/hybrid/` with `VerifySignature(msg,sig)` that splits and checks both halves (1 day)
4. **Draft `ADR-008-PQC-HYBRID-CONSENSUS.md`** — spec for F3–F7 wire format, rotation, determinism checklist (2–3 hours)

These three tasks (protoc, F1, F2, ADR-008) are all **parallelizable** and don't need a running devnet.

---

## 9. Definition of "Phase F Done"

- [x] F1: `crypto/mldsa44/` builds, `go vet` clean, functional tests green — **NIST KAT embedding still pending**
- [x] F2: `crypto/hybrid/` builds, `VerifySignature` requires both halves, `Address()` delegates to classical, `go test` green; §F2 wire format (2,491 B) implemented
- [x] F3: Wire format documented (§2.2 F3) and encoded in `crypto/hybrid` (2,491 B framed); old classical-only verifier rejects hybrid sig — proven by `TestHybridSignVote`
- [x] F4: `FilePV` signs both halves; ML-DSA-44 sidecar persists and reloads; address unchanged; classical-only `FilePV` path unaffected — `privval/file_pqc_test.go` green. **SignerClient (tmkms/remote) still pending.**
- [x] F5: Validator set stores + verifies hybrid pubkey; `Address()` invariant across PQ-half rotation — `types/validator_hybrid_test.go` green (3 tests). **Proto-encoding path (`Validator.Bytes()`/`Hash()`/`ToProto()` via `crypto/encoding` oneof) gated with F7 protoc regen.**
- [ ] F6: Evidence verification works with hybrid keys; classical-only forgery rejected
- [ ] F7: `MsgRotateConsKey` prototype on devnet; single-validator rotation succeeds
- [ ] Bandwidth: 4-validator localnet runs 1,000 blocks; block size + disk growth measured and documented
- [ ] Determinism: cross-platform `VerifySignature` hash identical; no float in verify path
- [ ] ADR-008 committed and reviewed

**Phase F is done when a 4-validator localnet produces 1,000 consecutive blocks with hybrid signatures, and the bandwidth/determinism numbers are published.**

---

*Built from the actual fork code and `PROJECT_AEGIS_JUNO_FULL_PQC.md` §4.1, §5.1, §6.*
*Last updated: 2026-06-18*
