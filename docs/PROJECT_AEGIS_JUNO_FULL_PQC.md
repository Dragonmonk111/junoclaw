# Project Aegis — Full-Stack Post-Quantum Migration for Juno

> A deterministic plan to make the **Juno chain itself** post-quantum safe —
> every signature, every key exchange, every hash — by **migrating an existing
> Cosmos/CometBFT chain** instead of rebuilding one from scratch.
>
> Companion to the application-layer work (`PROJECT_FABLE_MAYO5_PLAN.md`,
> `MAYO_PRECOMPILE_PLAN.md`, `PQC_COMPETITIVE_ANALYSIS.md`). Fable made
> JunoClaw *attestations* quantum-safe. Aegis is about the *chain*.

**Status:** Plan / RFC. No consensus-layer code changes yet.
**Date:** 2026-06-14
**Author:** Dragonmonk / VairagyaNodes (with Cascade)
**Target stack:** `CosmosContracts/juno` v29 → vNN, CometBFT, Cosmos SDK, ibc-go v8

---

## 0. The thesis (why this can ship before a greenfield L1)

Marius is solving post-quantum security the **greenfield** way: a custom Rust
BFT stack with Falcon-1024 at the protocol layer, rebuilt from zero. That is the
right way to get a *perfect* PQC chain, and it is a 12–18 month project before
mainnet, with no IBC, DEX, wallets, or indexers on day one.

Aegis takes the **migration** way: Juno already has validators, accounts, IBC
connections, tooling, and an upgrade mechanism that has shipped 29 coordinated
upgrades. The bet is that **a hybrid, layer-by-layer migration of a live chain
can deliver meaningful quantum safety to real users faster than a from-scratch
chain can reach mainnet** — because most of the layers can be shipped
independently, each behind a normal governance upgrade, without rebuilding
consensus on day one.

Three decisions make this realistic and are the spine of the whole plan:

1. **Hybrid, not flag-day.** Every migrated primitive is `classical + PQC`
   composed together. Security becomes the *stronger* of the two; nothing
   breaks for un-upgraded peers, wallets, or counterparties during transition.
2. **Standardized algorithms, not bleading-edge bets — at the consensus root.**
   The chain's trust root migrates to **ML-DSA (FIPS 204)** and **ML-KEM
   (FIPS 203)**, which are *finalized* NIST standards with integer-only,
   deterministic verification. MAYO stays where it already wins (small
   app-layer attestation signatures in JunoClaw). This is a deliberate contrast
   to Falcon, whose floating-point signer is a determinism/constant-time hazard
   for a multi-operator validator set (see §6).
3. **Crypto-agility as a first-class feature.** Every key and signature is
   self-describing (algorithm-tagged), so the chain can rotate schemes by
   governance without another hard migration. We are not betting the chain on
   one algorithm being unbroken forever.

And because the whole point is migrating an *existing* ecosystem, Aegis is open
source and written as a reusable blueprint for **any** Cosmos/CometBFT chain —
Juno is where we prove it first, not the only place it can run.

---

## 1. What "full PQC" actually means (threat model, made deterministic)

A quantum adversary changes exactly two things, and we should never hand-wave a
third:

- **Shor's algorithm breaks public-key crypto based on factoring / discrete
  log.** That is *every* elliptic-curve primitive in the stack: Ed25519
  (validators, P2P identity), secp256k1 (accounts), X25519 (transport key
  exchange). These are **catastrophic** — forge votes, steal funds, impersonate
  peers — and they are the whole job.
- **Grover's algorithm gives a quadratic speedup against hash preimage/search.**
  SHA-256 drops to ~128-bit effective security; RIPEMD-160 to ~80-bit. SHA-256
  is **still safe**; we keep it (see §4.7). This is the part people over-rotate
  on — Grover does *not* break our Merkle trees.
- **Everything symmetric / hash-based stays fine** at current sizes (AES-256,
  SHA-256/512, SHA-3). No change required.

**Prioritization rule (deterministic):** order the migration by
`exposure × lifetime`, not by layer prettiness.

- A key is at risk if it is *still in use* on Q-day. So **long-lived keys are
  the highest priority**: validator consensus keys, treasury/multisig,
  vesting/escrow, governance authorities. They must go hybrid first.
- **"Harvest-now, decrypt-later" (HNDL)** applies only to *confidentiality*,
  i.e. the P2P transport key exchange (X25519). For a public chain most data is
  already public, so HNDL is real but secondary to signature forgery.
- Signature forgery is a **Q-day** threat (not retroactive), but any key that
  will still authorize value or consensus at Q-day must be migrated *before*
  Q-day, and rotation across a validator set is slow — so we start now.

"Full PQC" = no ECC primitive remains as a *sole* root of trust for consensus,
accounts, transport, or IBC verification. Hybrid (classical AND PQC) satisfies
this because breaking it requires breaking the PQC half too.

---

## 2. Complete cryptographic surface inventory (the deterministic matrix)

Every place a key, signature, KEM, or security-critical hash appears in a Juno
v29 / CometBFT / Cosmos SDK / ibc-go node, the threat, the chosen PQC primitive,
and the migration mechanism. **This table is the contract for the rest of the
plan — if a surface is not here, the plan is incomplete.**

| # | Surface | Where it lives | Current primitive | Quantum threat | PQC choice | Migration mechanism |
|---|---------|----------------|-------------------|----------------|-----------|---------------------|
| 1 | Validator consensus signatures (prevote / precommit / proposal) | CometBFT `PrivValidator`, `types.Vote/Proposal` | Ed25519 | Shor → forge votes, fake commits | **Hybrid Ed25519 + ML-DSA-44** (44 recommended; see §5.1) | New `crypto.PubKey`/`PrivKey` type in CometBFT; validator key rotation via upgrade |
| 2 | Commit / `CanonicalVote` aggregation, evidence | CometBFT `types.Commit`, `evidence` | Ed25519 | Shor → forge double-sign evidence or hide it | inherits #1 | same key type; evidence verifies hybrid sig |
| 3 | P2P node identity / authenticated handshake | CometBFT `p2p` secret connection (station-to-station) | Ed25519 node key | Shor → impersonate peer, eclipse | **ML-DSA-44** (or 65) node key | node-key format + handshake auth update; hybrid during transition |
| 4 | P2P transport confidentiality (session key agreement) | CometBFT secret connection ECDH | X25519 → ChaCha20-Poly1305 | **HNDL** → decrypt captured traffic later | **Hybrid X25519 + ML-KEM-768** | concatenated/`KEM-combiner` shared secret in handshake |
| 5 | Account / transaction signatures | Cosmos SDK `x/auth` ante, `cryptotypes.PubKey` | secp256k1 (also ed25519, secp256r1) | Shor → steal funds, forge txs | **Hybrid secp256k1 + ML-DSA-44** | new `PubKey` impl + `SignMode` + AnteHandler decorator; new account type or composite key |
| 6 | Multisig accounts | SDK `multisig.LegacyAminoPubKey` | secp256k1 set | Shor (per member) | composite PQC members | multisig already key-type agnostic — register PQC member keys |
| 7 | HD wallet derivation | keyring, BIP32/BIP44 | secp256k1 + BIP32 | Shor (resulting keys) | **seed → ML-DSA keygen** (no float) | derivation convention: HKDF(seed,path) → ML-DSA keygen; keyring backend |
| 8 | Address derivation | SDK `address.Hash` | RIPEMD-160(SHA-256(pk)) (secp256k1); SHA-256(pk)[:20] (ed25519) | Grover (mild; preimage) | keep SHA-256-based; define addr for PQC pk | `address.Hash("pqc", pqc_pubkey)[:20]`; pubkey hidden until first use is a *bonus* mitigation |
| 9 | Tx / block / state / app hashing | SDK store, CometBFT Merkle, tx hash | SHA-256 | Grover → 128-bit eff. | **keep SHA-256** (optional SHA-512/SHA3 margin) | no change required; documented as Grover-safe |
| 10 | IAVL / ICS-23 Merkle proofs | `iavl`, `cosmos/ics23` | SHA-256 | Grover (mild) | keep SHA-256 | no change |
| 11 | IBC light client verification (07-tendermint) | ibc-go `07-tendermint` | verifies counterparty Ed25519 set + ICS-23 | **inherits counterparty** security | **PQC-aware tendermint client** verifying ML-DSA validator sets | new/upgraded client type; both chains must support; hybrid era verifies hybrid sets |
| 12 | IBC relayer keys | relayer signs txs on each chain | secp256k1 | Shor | inherits #5 | no IBC-specific change beyond account migration |
| 13 | Interchain Accounts (ICS-27) host/controller auth | ibc-go `ica` | inherits channel + account auth | inherits #5/#11 | inherits | no extra primitive |
| 14 | Genesis / `gentx` validator onboarding | SDK `genutil` | secp256k1 + Ed25519 | Shor | hybrid gentx | gentx carries hybrid validator key from day one of the upgrade |
| 15 | CosmWasm contract crypto host functions | wasmvm crypto API | secp256k1/ed25519/bls12-381 verify + (our) `bn254`, `mayo_verify` | Shor (for ECC ones) | add `ml_dsa_verify` host fn; keep `mayo_verify` | extend the wasmvm-fork pattern already proven for BN254 + MAYO |
| 16 | Governance / authority keys (treasury, upgrade authority, params) | on-chain accounts / module authorities | secp256k1 multisig | Shor (long-lived = top priority) | **Hybrid + optional SLH-DSA (FIPS 205) hedge** for cold keys | migrate first; SLH-DSA's hash-based conservatism for keys that must never fall |
| 17 | Light clients on the *other* side (peggo/bridges, CEX deposit verifiers, explorers) | external verifiers of Juno headers | Ed25519 header verify | Shor / inherits | must consume hybrid headers | publish hybrid header format + verification lib; SDK/relayer release |

Surfaces explicitly judged **safe / no migration**: #9, #10 (SHA-256 under
Grover), all symmetric encryption (ChaCha20/AES-256), and the *content* hashing
inside JunoClaw (already SHA-256).

---

## 3. Algorithm selection, per surface (and why — deterministic)

The choice is **not** "pick one PQC algorithm." It is "pick the right
standardized primitive per surface, by the constraints that surface imposes."

| Need | Constraints that dominate | Choice | Sizes (pk / sig or ct) |
|------|---------------------------|--------|------------------------|
| Consensus signatures (#1–2, #14) | deterministic verify across heterogeneous validators; finalized standard; size matters at N×/block; high volume | **ML-DSA-44** recommended (FIPS 204, cat 2); 65 optional — see §5.1 | pk 1,312 B / sig 2,420 B (44) |
| P2P identity (#3) | many short-lived, frequent | **ML-DSA-44** (FIPS 204, NIST L2) | pk 1,312 B / sig 2,420 B |
| Transport secrecy (#4) | KEM, not signature; HNDL | **ML-KEM-768** (FIPS 203, NIST L3) | ek 1,184 B / ct 1,088 B / ss 32 B |
| Account / tx signatures (#5–7) | smaller is better (tx payload); standard; deterministic | **ML-DSA-44** (FIPS 204, NIST L2) | pk 1,312 B / sig 2,420 B |
| Cold / governance / treasury keys (#16) | maximum conservatism, low volume, size tolerable | **SLH-DSA** (FIPS 205) as optional hedge | pk 32–64 B / sig 17–50 KB |
| App-layer attestations (JunoClaw, #15) | smallest signature, fits tx payload, ZK-friendly | **MAYO** (already shipped) | sig 186–964 B |

### Why ML-DSA at the consensus root rather than Falcon or MAYO

- **Determinism is non-negotiable in consensus.** Every validator must verify
  identically, bit-for-bit, on every CPU/OS. **ML-DSA verification is
  integer-only.** Falcon (FN-DSA) signing relies on floating-point Gaussian
  sampling — a well-documented constant-time / reproducibility hazard — and
  FIPS 206 is still draft. That is acceptable for a single team controlling a
  greenfield stack (Marius); it is a liability for a heterogeneous, adversarial
  validator set. We pick the boring, finalized, integer-only standard.
- **MAYO** is excellent where signature size dominates and the signer is the
  application (JunoClaw attestations). It is an *additional-signatures on-ramp*
  candidate, not a finalized standard, and multivariate schemes carry more
  novelty risk (cf. Rainbow). It does not belong at the chain's trust root, and
  it does not need to — Fable already gives it the right home.
- **SLH-DSA** is the most conservative (hash-based, security reduces to SHA-2),
  but its 17–50 KB signatures make it unusable per-block. It is the right hedge
  for a handful of *cold, long-lived* keys (upgrade authority, treasury),
  exactly where size doesn't matter and "must never break" does.

### Hybrid construction (the transition primitive)

A hybrid key is `(classical_pk, pqc_pk)`; a hybrid signature is
`(classical_sig, pqc_sig)`. Verification requires **both** to pass. Properties:

- Security = strictly the stronger of the two (a classical break alone, or a
  PQC break alone, does not forge).
- Backward compatible: encode as a new algorithm-tagged key type so old code
  that doesn't understand it simply rejects rather than mis-verifies.
- Use an IETF-style **signature combiner** (domain-separated, length-prefixed
  concatenation) and a **KEM combiner** for ML-KEM/X25519 so we don't invent
  fragile glue.

---

## 4. Layer-by-layer engineering plan

### 4.1 Consensus signatures (CometBFT) — surfaces #1, #2, #14

CometBFT already abstracts keys behind `crypto.PubKey` / `crypto.PrivKey` and a
`PrivValidator` signer. The work:

1. Add an `mldsa44`/`mldsa65` (and a `hybrid{ed25519,mldsa}`) implementation of
   the crypto interfaces, with Protobuf encodings registered in the address
   codec (level chosen per §5.1).
2. Teach `types.Vote`, `types.Proposal`, `CanonicalVote`, and evidence
   verification to handle the new key type (mostly free once the interface impl
   exists).
3. Support hybrid keys in `PrivValidator` (file + KMS/tmkms signer) so a
   validator signs with *both* halves.
4. **Validator key rotation:** consensus keys are not currently rotatable
   in-place on Cosmos chains. Two options — (a) a one-time `MsgRotateConsKey`
   added in the upgrade so each validator binds a PQC half to their existing
   slot, or (b) genesis-style re-bootstrap at the upgrade height. (a) is
   strongly preferred for a live chain.

Bandwidth: a hybrid commit grows by ~3.3 KB per validator signature. With ~100
validators that is ~330 KB/block of signatures. This is the real cost and the
reason Marius talks about "slower blocks / optimistic execution." Mitigations:
sign over `BlockID` (already the case), don't gossip full sigs redundantly, and
consider ML-DSA-44 instead of 65 for the consensus half if L2 is deemed
sufficient (sig 2,420 B). **This is a parameter to benchmark, not guess.**

### 4.2 P2P transport — surfaces #3, #4

The secret connection handshake authenticates with the node key (→ ML-DSA-44,
#3) and agrees a session key via ECDH (→ hybrid X25519 + ML-KEM-768, #4). The
session key becomes `KDF(x25519_shared || mlkem_shared)` with domain separation.
This closes HNDL on inter-validator traffic. It is **independent of consensus
signing** and can ship in its own release — an early, low-risk win.

### 4.3 Accounts & transactions (Cosmos SDK) — surfaces #5, #6, #7, #8

1. New `cryptotypes.PubKey` impls: `mldsa44` and `hybrid{secp256k1,mldsa44}`.
2. New `SignMode` (or reuse `SIGN_MODE_DIRECT` with a hybrid pubkey) and an
   **AnteHandler `SetPubKeyDecorator` / `SigVerificationDecorator`** path that
   verifies the hybrid signature and meters gas for the PQC half.
3. Address derivation for PQC keys: `address.Hash("pqc/mldsa44", pk)[:20]` —
   keeps the 20-byte bech32 address shape, so explorers/wallets need only learn
   a new pubkey type, not a new address format.
4. Keyring + CLI: generate, import, and sign with hybrid keys; `tmkms`-style
   external signer support for high-value accounts.
5. HD: PQC has no BIP32; derive deterministically as
   `seed' = HKDF(bip39_seed, path)` then `ML-DSA.KeyGen(seed')`, so existing
   mnemonics still back up PQC accounts.

This whole layer is **opt-in per account**: a user creates a hybrid account and
gets quantum-safe authentication immediately, with no flag-day. High-value
holders and the treasury migrate first (the `exposure × lifetime` rule).

### 4.4 IBC — surfaces #11, #12, #13, #17

The 07-tendermint light client verifies the counterparty validator set's
signatures. Once Juno validators sign hybrid, the *client on the other chain*
must verify hybrid sets. Work:

- A PQC-aware tendermint client (or a client param) on ibc-go that understands
  hybrid validator signatures and the larger sig sizes in the header.
- During transition, both endpoints verify the *classical* half (unchanged
  security) while the PQC half rides along; the client "upgrades" to require the
  PQC half once both sides support it.
- Honest boundary: **IBC is only as quantum-safe as the weakest connected
  chain.** A Juno→Osmosis channel is post-quantum only when *both* run hybrid
  consensus and hybrid clients. This is an ecosystem coordination problem, not a
  Juno-only one — which is exactly why doing it on a real IBC chain (and
  upstreaming it) is more valuable than doing it on an island.

### 4.5 CosmWasm — surface #15

Generalize the pattern Fable already proved. We added `bn254_*` and
`mayo_verify` host functions to a wasmvm fork. Add `ml_dsa_verify` the same way.
That lets contracts (including `jclaw-credential`) verify the *same* standardized
signature the chain uses, cheaply — and lets JunoClaw be the **app-layer testbed
and dogfood** for the chain-layer choice before consensus commits to it.

### 4.6 Governance / cold keys — surface #16

Migrate the upgrade authority, treasury multisig, and any long-lived module
authority to hybrid **first** — they have the worst `exposure × lifetime` and
the lowest transaction volume.

**SLH-DSA (FIPS 205) decision — included, but scoped.** For the *coldest* keys
(upgrade authority, treasury) offer SLH-DSA as an option. Its security reduces to
the strength of SHA-2 alone — the most conservative assumption available, and
immune to any future break of a lattice or multivariate scheme. The price is
signature size (~17–50 KB) and slower signing, which is irrelevant for a key that
signs a few times a year. It is **explicitly not** a candidate for the consensus
or account hot paths, where its size would wreck the bandwidth budget (§5.1). The
rule is simple: **ML-DSA for anything frequent, SLH-DSA as the hash-based
break-glass for the handful of keys that must never fall.**

### 4.7 Hashing — surfaces #8, #9, #10 (no migration)

SHA-256 under Grover retains ~128-bit preimage resistance — above the comfort
threshold. We **keep SHA-256** for tx/block/state/Merkle/address hashing and say
so explicitly, with an option to offer SHA-512/SHA-3 variants for extra margin
if governance ever wants it. Pretending hashes are broken would be the kind of
overclaim this project exists to avoid.

### 4.8 Verifiable computation — ZK proofs & TEE attestations (the honest gap)

Two parts of the JunoClaw stack carry an integrity guarantee that **signature
hybridization alone does not rescue**, and it is important to say so plainly
rather than imply the precompile/transport work covers them.

**TEE attestations (WAVS / TEE operators).** A TEE quote proves two separable
things: *(a) submitter identity* and *(b) trustless execution integrity* —
"this code ran in hardware the operator cannot tamper with." Wrapping the
operator's submission in our own hybrid (ML-DSA/MAYO) co-signature **fully
closes (a)** post-quantum and is pure upside, so we do it. But it **does not
close (b)**: the vendor attestation root (Intel/AMD, ECDSA today) is
Shor-forgeable, and once it is, a malicious or compromised operator can mint a
valid quote for any measurement/output and co-sign it validly. The system
silently degrades from *trust the hardware* to *trust the operator*. A
co-signature can only ever assert "I vouch," never "untamperable hardware ran
this." Genuine fixes for (b) are limited to: **vendor PQC attestation**
(upstream — we track, can't ship), a **post-quantum proof of correct execution**
(below), or multi-vendor *M-of-N* attestation (an economic hedge, not a
cryptographic guarantee).

**ZK proofs (`zk-verifier`, Groth16/BN254).** Groth16 — and every pairing/KZG
SNARK — bases soundness on discrete log + knowledge-of-exponent, which Shor
breaks, so a quantum adversary can forge accepting proofs for false statements.
**The BN254 precompile (ADR-001) is a performance change and does nothing for
this.** Groth16's zero-knowledge is also *computational*, so a captured proof
encoding a secret witness is itself an HNDL target.

**The deterministic way out — and it is one fix for both.** The conservative,
post-quantum, *deterministically verifiable* answer is a **hash-based STARK/FRI**
proof system: soundness reduces only to hash collision-resistance (the safest PQ
assumption; Grover is countered by doubling the hash output), verification is a
fixed sequence of field ops + Merkle/hash checks with Fiat-Shamir challenges
derived **deterministically** from the transcript (no verify-time RNG —
consensus-safe, exactly like ML-DSA verify), and the setup is **transparent** (no
trusted SRS, hence no toxic-waste discrete log to recover). Crucially, a
**zkVM STARK proof of correct execution** *replaces* the TEE hardware root for
property (b): the proof *is* the integrity, with no vendor to trust. So the same
move closes ZK soundness **and** reconstructs TEE execution integrity. The
residual cost is **bytes and gas** (STARK proofs are large; the natural analog of
the BN254 precompile becomes a Poseidon/Keccak verification host function) — an
engineering/economics problem, **not** a cryptographic gap. Lattice SNARKs are a
smaller-proof but younger/more-assumption-laden alternative (the "newer scheme"
hedge). Witness/input *confidentiality* remains a separate problem solved by
ML-KEM (§4.2), not by the proof system.

This is flagged as a **distinct, later track** (a future ADR — "PQ verifiable
computation / STARK migration"), not part of Phases A–E. It is called out here so
the plan does not overclaim: transport (§4.2) and signatures (§4.3, §4.5) go
hybrid now; **ZK soundness and TEE execution integrity need a proof-system
migration, which signature hybridization cannot substitute for.**

---

## 5. Phased rollout (mapped to Juno upgrade handlers)

Each phase is an independent, governance-gated upgrade (`vNN.Upgrade` in
`app/app.go`, same shape as the existing v28/v29 handlers). Each ships value on
its own; none requires the next to be useful.

| Phase | Ships | Surfaces | Depends on | Why it's safe to ship alone |
|-------|-------|----------|------------|------------------------------|
| **A. Foundations** | Algorithm-tagged hybrid key types + combiners as audited libs; conformance vectors; ML-DSA/ML-KEM Rust+Go impls vendored; devnet fork | — | — | pure libraries; no chain behavior change |
| **B. CosmWasm `ml_dsa_verify`** | host function on wasmvm-fork; JunoClaw dogfoods it | #15 | A | mirrors shipped BN254/MAYO precompile pattern |
| **C. Transport hybrid KEM** | secret-connection X25519+ML-KEM-768; ML-DSA node keys | #3, #4 | A | node-local; hybrid → no peer breakage; closes HNDL |
| **D. Opt-in PQC accounts** | hybrid account type, SignMode, AnteHandler, keyring, addresses | #5–#8 | A | opt-in per account; classical accounts untouched |
| **E. Governance/treasury migration** | move authority + treasury to hybrid (+SLH-DSA cold) | #16 | D | highest-priority keys, lowest volume |
| **F. Consensus hybrid signatures** | CometBFT hybrid validator keys + `MsgRotateConsKey` | #1, #2, #14 | A, D | hybrid → un-rotated validators still validate during window |
| **G. IBC hybrid client** | PQC-aware 07-tendermint client + header format + verifier lib | #11–#13, #17 | F | verifies classical half until counterparties upgrade |
| **H. Deprecate classical-only** | governance flips minimums to require PQC half where safe | all | C–G | only after ecosystem coverage is measured |

**What can plausibly be live "before Marius launches":** Phases A–E are not
gated on rebuilding consensus and ride existing upgrade machinery — transport
secrecy, contract-level standardized PQC, and opt-in quantum-safe accounts
(including treasury) are weeks-to-a-few-months of work each, on a chain that
already exists. That is a genuine, defensible "first real PQC coverage for live
users" claim. Phase F (consensus) is the hard part and is comparable in
difficulty to Marius's core work — but even there, *migration on a live
validator set with an existing ecosystem* reaches users faster than *bootstrap a
new validator set and ecosystem from genesis.* We should **not** claim "fully
PQC consensus before him"; we **can** claim "meaningful PQC, in production, for
real value, sooner — and a credible path to the rest."

---

## 5.1 Decision input: ML-DSA-44 vs ML-DSA-65 at the consensus root

The single biggest cost of Phase F is **signature bandwidth**: every block's
commit carries one signature per validator, so the chain pays `N × sig_size`
bytes per block, forever — in block storage and in vote gossip. That makes the
44-vs-65 choice a measurable decision, not a taste one.

**Fixed sizes (spec constants — FIPS 204 / RFC 8032 / SEC):**

| Scheme | NIST cat | Public key | Signature |
|--------|---------|-----------:|----------:|
| Ed25519 (current consensus) | — classical | 32 B | 64 B |
| secp256k1 (current accounts) | — classical | 33 B | 64 B |
| ML-DSA-44 | 2 | 1,312 B | 2,420 B |
| ML-DSA-65 | 3 | 1,952 B | 3,309 B |
| Hybrid Ed25519 + ML-DSA-44 | 2 (+classical) | 1,344 B | 2,484 B |
| Hybrid Ed25519 + ML-DSA-65 | 3 (+classical) | 1,984 B | 3,373 B |

**Consensus commit signature payload per block** (signature bytes only; ignores
~33 B/`CommitSig` metadata and any compression):

| Validators | Ed25519 baseline | Hybrid-44 | Hybrid-65 | cost of 65 over 44 |
|-----------:|-----------------:|----------:|----------:|-------------------:|
| 50 | 3.2 KB | 124.2 KB | 168.7 KB | +44.5 KB |
| 100 | 6.4 KB | 248.4 KB | 337.3 KB | +88.9 KB |
| 150 | 9.6 KB | 372.6 KB | 505.9 KB | +133.3 KB |

(SI units: 1 KB = 1,000 B. Signature payload only.)

**Projected block-data growth from commits** (N=100, ~6 s blocks ≈ 14,400
blocks/day):

| | per block | per day | per year |
|--|----------:|--------:|---------:|
| Ed25519 baseline | 6.4 KB | ~92 MB | ~34 GB |
| Hybrid-44 | 248.4 KB | ~3.58 GB | ~1.31 TB |
| Hybrid-65 | 337.3 KB | ~4.86 GB | ~1.77 TB |
| **Δ choosing 65 over 44** | **88.9 KB** | **~1.28 GB** | **~467 GB** |

**Reading:** going post-quantum at consensus is a ~1.3 TB/year block-growth event
at 100 validators *no matter what* — that is the real headline, and it is why a
from-genesis design talks about slower blocks / optimistic execution. Choosing
ML-DSA-65 over ML-DSA-44 adds **~467 GB/year** on top for one extra NIST
category.

**Measured single-op timing** (release build, pure-Rust `fips204`, 200 iters,
reproduce via `cargo run --release --features timing` in `aegis-bench/`):

| variant | keygen | sign | verify |
|---------|-------:|-----:|-------:|
| ML-DSA-44 | ~155 µs | ~396 µs | **~101 µs** |
| ML-DSA-65 | ~216 µs | ~685 µs | **~149 µs** |

Verify is the consensus-relevant op. At N=100 a node verifies ~100 commit
signatures/block: **~10 ms (44) vs ~15 ms (65)** of PQC verify CPU per block — a
sub-percent slice of a 6 s block either way. **So verify CPU is *not* the
bottleneck; bandwidth is** — which reinforces ML-DSA-44. (Wall-clock on one
machine.)

**Measured on-chain gas (done 2026-06-15 — Phase B, `junoclaw-bn254-1`):** the
`jclaw-credential` `VerifyMlDsaAttestation` path was benchmarked pure-Wasm vs the
`ml_dsa_verify` host precompile (full results:
`MLDSA_PRECOMPILE_BENCHMARK_RESULTS.md`):

| variant | pure-Wasm verify gas | precompile verify gas | speedup |
|---------|---------------------:|----------------------:|--------:|
| ML-DSA-44 | 269,604 | 260,381 | 1.04× |
| ML-DSA-65 | 328,945 | 315,212 | 1.04× |
| ML-DSA-87 | 408,298 | 387,124 | 1.05× |

This **closes the open §5.1 figure** and confirms the verify-CPU conclusion from
a second direction: ML-DSA verify is cheap enough on-chain (~270k total tx gas at
L2, most of it fixed per-tx overhead) that a native precompile barely beats pure
Wasm — flat ~1.04×, the *opposite* of MAYO's 1.15×→2.21× (which grows with the
parameter set because its GF(16) matrix verify is genuinely heavy in Wasm). The
on-chain number therefore **reinforces ML-DSA-44** and tells us the contract-side
`ml_dsa_verify` precompile is a **wasm-size / standardization** win (~17% smaller
binary), not a performance one.

**Recommendation (verify-CPU *and* on-chain gas now measured):** use
**ML-DSA-44** as the consensus and account workhorse. NIST category 2
(~AES-128-equivalent post-quantum strength) is adequate for a hot path that is
*also* still protected by the Ed25519 half throughout the hybrid era, and it
saves ~26 % of signature bytes forever (2,484 vs 3,373 B per hybrid signature). Reserve **ML-DSA-65** (or SLH-DSA) for
low-volume, long-lived, high-value keys — governance/treasury — where the size
cost is paid rarely. These size/bandwidth figures are reproducible **offline**
(no dependencies) via the standalone `aegis-bench/` crate; the `timing` feature
adds real ML-DSA-44/65 keygen/sign/verify measurements (done — see above).
**End-to-end on-chain gas is now also measured** (Phase B, table above): it
confirms verify is cheap and bandwidth is the cost, locking in ML-DSA-44.

---

## 6. Determinism & consensus-safety requirements (the non-negotiables)

Consensus crypto has constraints application crypto does not. These are
acceptance criteria, not nice-to-haves:

- **Bit-for-bit deterministic verification** across all validator hardware/OS.
  → integer-only ML-DSA; no floating point anywhere in the verify path. (This is
  the same discipline behind our `no_std`, `forbid(unsafe_code)` MAYO verifier.)
- **Constant-time** signing/verification to avoid side channels on validator
  keys.
- **Fixed, audited gas schedule** for any PQC op exposed to contracts/txs
  (mirror the `cosmwasm-crypto-mayo` gas-constant approach; conservative first,
  tuned with measurements).
- **Conformance test vectors** (NIST KATs for ML-DSA/ML-KEM; our own for the
  hybrid combiner) run in CI and cross-checked against a reference impl, exactly
  like the C cross-check we already run for MAYO.
- **Reproducible builds** of the patched `junod`/`libwasmvm` so the binary that
  validators run is auditable.
- **Crypto-agility:** algorithm tag in every key/sig so a broken scheme can be
  rotated out by governance without a second migration.

---

## 7. How Aegis rides alongside JunoClaw

This is not a detour from JunoClaw — JunoClaw is the proving ground.

- **`jclaw-credential` becomes the PQC account testbed.** The trust-tree already
  binds members to key commitments; it is the natural place to pilot hybrid /
  ML-DSA account binding before the SDK ante path is finalized.
- **The MAYO precompile pattern generalizes.** `ml_dsa_verify` (Phase B) reuses
  the exact wasmvm-fork machinery we shipped for BN254 and MAYO — same patch
  series shape, same gas-schedule discipline, same benchmark harness.
- **Moultbook + attestations get a standardized option.** Agent memory can be
  signed with MAYO (small) *or* ML-DSA (standardized, chain-aligned) behind the
  existing `PqcVerifier` trait hedge — crypto-agility at the app layer mirrors
  crypto-agility at the chain layer.
- **Governance dogfooding.** Agent-company / DAO authority keys are exactly the
  long-lived keys Phase E migrates first.

So every chain-layer phase has an app-layer rehearsal in JunoClaw, and every
app-layer primitive we already shipped points at a chain-layer phase.

---

## 8. Risks & mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| Signature-size bandwidth blowup at consensus (#1) | High | benchmark ML-DSA-44 vs 65 for the consensus half; don't redundantly gossip; this is the core tradeoff, measured not guessed |
| Validator coordination for consensus migration (F) | High | hybrid window means un-rotated validators keep validating; `MsgRotateConsKey` instead of re-genesis |
| IBC counterparty dependence (#11) | High | honest scoping: a channel is PQC only when both ends are; upstream the client so others can adopt |
| Algorithm break (ML-DSA / MAYO) | Medium | hybrid (classical half still holds); algorithm-tag agility; SLH-DSA hedge for cold keys |
| State / payload bloat from large keys | Medium | store *hashes* of PQC pubkeys where possible (the trick `jclaw-credential` already uses); keys travel in tx, commitments live in state |
| Upstream divergence (maintaining a fork) | Medium | shape every change as upstreamable to CometBFT / SDK / ibc-go / wasmd; prefer interface impls over forks (cf. Juno's `x/wrappers/gov` no-fork pattern) |
| Non-deterministic verify causing a chain halt | Critical | integer-only ML-DSA, constant-time, KAT conformance in CI, reproducible builds (see §6) |
| Overclaiming "Juno is quantum-safe" before consensus migrates | Reputational | publish the surface matrix (§2) with per-surface status; never claim more than what shipped |

---

## 9. Immediate next actions (Phases A–E — greenlit 2026-06-14)

1. **Bandwidth + size model (done):** `aegis-bench/` reproduces the §5.1 tables
   offline (dependency-free); `cargo run --features timing` adds real ML-DSA-44/65
   keygen/sign/verify timing via a pure-Rust FIPS 204 impl.
2. **Phase A — foundations:** vendor an integer-only ML-DSA verifier (Rust,
   `no_std` target) + Go impl; wire NIST KAT conformance into CI alongside the
   MAYO cross-check; extract the algorithm-tagged hybrid key/sig + combiner libs.
3. **Phase B — CosmWasm `ml_dsa_verify` (the on-chain proof):**
   - **[done]** `wasmvm-fork/cosmwasm-crypto-mldsa/` — the verify-only host-fn
     crate wrapping pure-Rust `fips204` (no_std, RNG-free, integer-only),
     mirroring `cosmwasm-crypto-mayo`; supports variants 44/65/87; builds in
     `no_std` + `--release` and passes 8 unit tests.
   - **[done]** wired into the fork the way MAYO is wired — patch series
     `20-28-*` (cosmwasm-vm `Cargo.toml`/`imports.rs`/`instance.rs`/
     `compatibility.rs`, cosmwasm-std `imports.rs`/`traits.rs`/`testing mock`,
     plus the new-crate patch). Guest side: `ml_dsa_verify_call` in
     `cosmwasm-std-bn254-ext`; contract side: feature-gated dual path in
     `jclaw-credential` (`--features mldsa-precompile`).
   - **[done]** `jclaw-credential` verifies ML-DSA-44/65/87 attestations on
     `junoclaw-bn254-1`; benchmarked pure-Wasm vs precompile with the existing
     harness (`benchmark-mldsa-devnet.sh`). **Result (§5.1, full doc
     `MLDSA_PRECOMPILE_BENCHMARK_RESULTS.md`):** verify gas
     269,604/328,945/408,298 (pure) vs 260,381/315,212/387,124 (precompile) —
     a flat ~1.04× speedup, the opposite of MAYO. ML-DSA verify is cheap in Wasm,
     so the precompile's real win is a ~17% smaller binary + a shared audited
     native verifier, **not** performance.
   **"Prove on the Juno fork first" is done** — the on-chain gas number §5.1 left
   open is now measured and folded back into §5.1.
4. **Phase C — transport:**
   - **[done] C1 — handshake spec + crate selection:** [`ADR-006-PQC-HYBRID-TRANSPORT.md`](./ADR-006-PQC-HYBRID-TRANSPORT.md)
     specifies the hybrid CometBFT secret connection: confidentiality via a
     concatenate-then-HKDF **KEM combiner** over X25519 + ML-KEM-768
     (`session_key = HKDF(ss_x25519 || ss_mlkem768, salt=H(transcript))`), node
     identity via hybrid **Ed25519 + ML-DSA-44**, with strictly downgrade-safe
     negotiation (no peer is refused for lacking PQ). Runtime impl targets the
     **Go stdlib `crypto/mlkem`** (the devnet already builds on Go 1.24, which
     ships it), `cloudflare/circl` as the pre-1.24 fallback; Rust harness uses
     `fips203` (same vendor as the `fips204` we already ship) cross-checked
     against `libcrux-ml-kem`. One-time handshake overhead ≈ ~9.7 KB/connection.
   - **[done] C2 — combiner harness + conformance:** Go stdlib-only reference
     harness `aegis-transport/` implements the ADR-006 combiner + transcript
     binding and proves session-key agreement, ADR wire sizes (ek 1184 / ct 1088),
     transcript-binding downgrade detection, and implicit-rejection tamper
     handling (6 tests, `go vet` clean on Go 1.24). The Rust↔Rust differential
     `aegis-kem-diff/` cross-checks `fips203` vs `libcrux-ml-kem` byte-for-byte
     (self + both cross-directions, 256+ iters) — with the Go stdlib leg's own
     upstream NIST ACVP suite this is the committed three-way differential;
     fixed-vector ACVP wiring is specified in `aegis-kem-diff/ACVP_WIRING.md`.
   - **[done] C3 — two-node prototype:** `aegis-transport/cmd/twonode` runs the
     hybrid handshake between a real initiator/responder over a loopback socket
     (session key confirmed over the wire via HMAC) and measures latency +
     bandwidth vs a classical X25519-only baseline. Measured: hybrid adds
     **+2,350 B/handshake** (matching the ~2.3 KB KEM prediction) and ~+0.24 ms
     on loopback — confirming ADR-006's thesis that the cost is *bytes, not
     crypto CPU*. Next: fold the handshake into a CometBFT fork on the actual
     devnet to capture real-link RTT (the only piece loopback cannot show).
5. **Phase D — accounts:** write `ADR-007-PQC-HYBRID-ACCOUNTS.md` (hybrid
   `PubKey` + AnteHandler + address derivation), then implement opt-in hybrid
   accounts on the fork.
6. **Phase E — governance/treasury:** migrate long-lived authority keys to hybrid
   (SLH-DSA option for the coldest), dogfooded via JunoClaw's agent-company / DAO
   authority keys.

Fork-first throughout: prove each phase on the Juno / wasmvm fork (our
established pattern), then shape the change for upstream.

---

## 10. Decisions (resolved 2026-06-14)

1. **Consensus / account parameter set:** **ML-DSA-44** is the recommended
   workhorse; ML-DSA-65 reserved for high-value / long-lived keys. Final lock
   pending the verify-CPU + on-chain gas benchmark on the fork (§5.1, §9).
2. **Upstream strategy:** **fork-first** — prove on the Juno / wasmvm fork (our
   established pattern), then upstream each change to CometBFT / SDK / ibc-go /
   wasmd.
3. **SLH-DSA:** **included now**, scoped to cold / governance / treasury keys
   only — never the hot path (§4.6). Hash-based break-glass, not a workhorse.
4. **Framing:** **Juno-first, open source for everyone.** Aegis is a reusable
   PQC-migration blueprint for *any* Cosmos/CometBFT chain, proven on Juno first.
   The story is "we did it on a live chain and you can too," not "Juno-only
   magic."
5. **Falcon-tagged option (Marius):** **keep the slot, don't depend on it.**
   Crypto-agility means a `falcon` backend *can* slot in later behind the same
   interface, but it is **not required** for Aegis to ship — ML-DSA is a
   finalized, integer-only, deterministic standard that stands on its own. If we
   complete the migration on ML-DSA first, Falcon becomes an optional
   collaboration / interop nicety, not a blocker. (FN-DSA is still FIPS 206
   *draft*, so depending on it now would be the slower path anyway.)

---

*Companion docs: `PROJECT_FABLE_MAYO5_PLAN.md` (app-layer MAYO-5),
`MAYO_PRECOMPILE_PLAN.md` (precompile pattern), `PQC_COMPETITIVE_ANALYSIS.md`
(vs Marius), `MARIUS_ASSESSMENT.md` (Juno v29 module surface).*

*This is a plan/RFC. No consensus-layer code has been changed. Every claim about
what is "safe" is scoped to the surface matrix in §2.*
