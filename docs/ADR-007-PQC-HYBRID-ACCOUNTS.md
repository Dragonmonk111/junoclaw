# ADR-007: Hybrid secp256k1 + ML-DSA-44 accounts for the Cosmos SDK (Project Aegis Phase D)

**Status:** Proposed — Phase D / D1 deliverable of [PROJECT_AEGIS_JUNO_FULL_PQC](./PROJECT_AEGIS_JUNO_FULL_PQC.md) §4.3
**Date:** 2026-06-17
**Authors:** Dragonmonk / VairagyaNodes (with Cascade)
**Scope:** Cosmos SDK account keys + transaction signing (surfaces #5 account auth, #6 tx signatures, #7 keyring, #8 address derivation)
**Depends on:** Phase A foundations (algorithm-tagged keys), Phase B (`ml_dsa_verify` measured on-chain), ADR-006 (same ML-DSA-44 impl, transport side)

> This ADR specifies an **opt-in, per-account** hybrid key type. It does **not**
> change consensus signing (Phase F), IBC (Phase G), or any existing classical
> account. A user who never creates a hybrid account is completely unaffected.
> No consensus-layer or state-machine-breaking change is introduced by adopting
> the key type itself — it rides the existing `cryptotypes.PubKey` /
> `AnteHandler` extension points.

---

## Context

A Cosmos SDK account today authenticates a transaction with a single
**secp256k1** signature (`SIGN_MODE_DIRECT` over the `SignDoc`). The account
address is `ripemd160(sha256(pubkey))[:20]`, bech32-encoded (`juno1…`).

A quantum adversary breaks this on **Q-day** (not retroactively — a signature
is a one-shot authenticator, there is no harvest-now-decrypt-later on
authentication): secp256k1 is a discrete-log primitive, so Shor recovers the
private key from any **on-chain public key**. The moment a public key is
revealed — which happens the first time an account *sends* a transaction — that
account is forgeable by a quantum adversary. For an agent DAO whose members are
long-lived autonomous keys signing continuously, this is the headline account
risk: the keys are both **high-exposure** (constantly revealed) and
**long-lived** (an agent identity persists for years).

Per the Aegis `exposure × lifetime` rule, treasury / governance keys (§4.6) and
long-lived agent keys are the first that should go quantum-safe. They are also
the lowest-volume relative to consensus (§5.1), so the bandwidth cost of a
PQC signature is affordable here long before it is affordable at the consensus
root.

This is the **account layer** the chain already lets contracts dogfood: Phase B
shipped `ml_dsa_verify` as a host function and `jclaw-credential` verifies
ML-DSA-44/65/87 attestations on-chain today. ADR-007 takes the *same* verified
ML-DSA-44 primitive and makes it authenticate **transactions**, not just
contract messages.

---

## Decision

Introduce a **hybrid account key** — classical **AND** post-quantum — so an
account is secure if **either** primitive holds, and so an account that has not
migrated is never broken by the existence of the new type.

### D1 — Key type: `hybrid{secp256k1, ml-dsa-44}`

A hybrid public key is the algorithm-tagged pair (Phase A):

```
HybridPubKey {
    secp256k1:  33 B   // compressed SEC1 point, exactly as today
    mldsa44:  1312 B   // FIPS 204 ML-DSA-44 public key
}
```

and the matching private key holds both halves. ML-DSA-44 is chosen to match the
consensus / transport / account workhorse decided in Aegis §5.1 and used in
ADR-006: NIST category 2, integer-only deterministic verify, smallest hybrid
signature of the ML-DSA family, and **already measured on-chain** in Phase B
(~270k gas pure-Wasm, ~1.04× from the precompile — verify is cheap, bandwidth is
the cost). Reusing it means **no new signature scheme enters the trust base** —
the same vendored implementation authenticates transport (ADR-006), contract
attestations (Phase B), and now accounts.

A pure `ml-dsa-44` (non-hybrid) `PubKey` impl is **also** registered, because the
hybrid type is built from it and because a future "deprecate classical" phase
(H) may want PQC-only accounts. The default, recommended type for the hybrid era
is the **hybrid** one.

### D2 — Signature: both halves over the same `SignDoc`

```
HybridSignature {
    secp256k1:  64 B   // [R || S], low-S, over sha256(SignDoc) — as today
    mldsa44:  2420 B   // ML-DSA-44 signature over the SAME SignDoc bytes
}
```

Both signatures are computed over the **identical canonical sign bytes** (the
serialized `SignDoc` for `SIGN_MODE_DIRECT`). Verification accepts the
transaction **iff both halves verify** during the hybrid era. This is the
account-layer analogue of ADR-006's "both must verify" handshake rule.

- A classical break of secp256k1 alone does not forge a transaction (the
  ML-DSA-44 half still fails).
- A (hypothetical) break of ML-DSA-44 alone does not forge a transaction (the
  secp256k1 half still fails).

The signature carries **no algorithm negotiation of its own** — the *account's
registered pubkey type* fixes which verifier runs. There is no downgrade vector
at signing time because there is nothing to negotiate: a hybrid account is
verified by the hybrid verifier, full stop.

### D3 — Address derivation: 20-byte shape preserved

To avoid forcing explorers, indexers, and wallets onto a new address format, the
hybrid address keeps the **20-byte bech32 shape**. We derive it with the SDK's
domain-separated `address.Hash` over the concatenated, algorithm-tagged public
key material:

```
addr_bytes = address.Hash("pqc/hybrid-secp256k1-mldsa44",
                          secp256k1_pub(33) || mldsa44_pub(1312))[:20]
address    = bech32(hrp = "juno", addr_bytes)
```

where `address.Hash(typ, key) = sha256( sha256(typ) || key )` (the SDK
`crypto/address` construction). Properties:

- **Domain separation:** the `typ` string prevents a hybrid address from ever
  colliding with a classical secp256k1 address (`ripemd160(sha256(pk))`) or a
  pure-`mldsa44` address (`address.Hash("pqc/mldsa44", pk)`), because the
  pre-images and the hash trees differ.
- **Binding:** the address commits to **both** halves, so an attacker cannot
  swap in a different ML-DSA key while keeping the classical half (or vice
  versa) and land on the same address.
- **Shape:** still 20 bytes, still `juno1…` — only the *pubkey type* is new on
  the wire, not the address grammar.

The pure `ml-dsa-44` address is `address.Hash("pqc/mldsa44", pk)[:20]` per
Aegis §4.3.

### D4 — HD derivation: one mnemonic backs up both halves

ML-DSA has no BIP-32 child-key derivation. We keep BIP-39 mnemonics as the
single backup secret and derive **both** halves deterministically from the same
512-bit BIP-39 seed:

```
bip39_seed = PBKDF2(mnemonic, "mnemonic"+passphrase)        # 64 B, BIP-39

# classical half: standard Cosmos BIP-44, coin type 118
secp256k1_priv = BIP32-CKD(bip39_seed, m/44'/118'/account'/0/index)

# pq half: no BIP-32 — derive a 32-byte ML-DSA seed via HKDF, path-bound
mldsa_seed = HKDF-SHA256(
                ikm  = bip39_seed,
                salt = "",                                    # none; ikm is high-entropy
                info = "aegis/pqc/mldsa44/v1:" + path,        # e.g. m/44'/118'/0'/0/0
                L    = 32
             )
mldsa44_priv = ML-DSA-44.KeyGen(mldsa_seed)
```

Properties:

- **Same mnemonic, same hybrid account** — deterministic and reproducible, so an
  existing 24-word backup also restores the PQC half. No second secret to store.
- **Wallet compatibility for the classical half** — the secp256k1 key is the
  *exact* key any standard Cosmos wallet derives from the mnemonic at that path,
  so the classical half is recoverable even by tools that know nothing about
  ML-DSA.
- **Path binding for the PQC half** — the derivation `info` includes the full
  HD path, so two accounts from the same mnemonic at different indices get
  independent ML-DSA keys, mirroring BIP-44 account/index separation.
- **Domain separation** via the `aegis/pqc/mldsa44/v1:` label ensures the
  ML-DSA seed can never coincide with any other use of the BIP-39 seed.

### D5 — AnteHandler, SignMode, gas

- **SignMode:** reuse `SIGN_MODE_DIRECT`. The hybrid nature is a property of the
  *pubkey type*, not the sign mode — the signer serializes the standard
  `SignDoc` and signs its bytes with both halves. (A dedicated
  `SIGN_MODE_HYBRID` is unnecessary and would fragment tooling; reuse is the
  lower-risk choice.)
- **AnteHandler:** a `SigVerificationDecorator` variant recognizes the hybrid
  pubkey type and verifies **both** signatures, failing the tx if either fails.
  The decorator ordering is unchanged; only the per-signature verify branch is
  extended.
- **Gas:** meter the PQC half with a fixed, conservative constant mirroring the
  `cosmwasm-crypto` ML-DSA gas approach (Phase B measured ML-DSA-44 verify at
  ~101 µs / ~270k Wasm-equivalent; the native AnteHandler verify is far cheaper
  in wall-clock, but the gas schedule should be conservative first, tuned with
  measurement). The classical half keeps its existing gas cost.

### D6 — Keyring & CLI

- `junod keys add <name> --hybrid` generates a hybrid key from a fresh or
  imported mnemonic, storing both halves in one keyring record.
- `junod keys show <name>` displays the hybrid address and both pubkey halves
  (the ML-DSA half is large; show its hash by default, full bytes with
  `--output json`).
- `junod tx … --from <hybrid-name>` signs with both halves transparently.
- High-value accounts may use an external signer (tmkms-style) for the PQC half;
  out of scope for the harness but reserved in the design.

---

## Wire & encoding

| Field | Type URL (proposed) | Bytes |
|-------|---------------------|-------|
| Pure PQC pubkey | `/aegis.crypto.mldsa44.PubKey` | 1,312 |
| Hybrid pubkey | `/aegis.crypto.hybrid.PubKey` | 33 + 1,312 = 1,345 |
| Hybrid signature (in `TxRaw.signatures`) | — | 64 + 2,420 = 2,484 |

Both halves are length-implicit (fixed-size per FIPS 204 / SEC1), so the
encoding is a simple ordered concatenation behind the Protobuf `PubKey` /
signature bytes. These are the exact sizes from Aegis §5.1 (hybrid
Ed25519+ML-DSA-44 there; secp256k1 substitutes the 33/64 classical halves).

---

## Security & determinism

- **Hybrid soundness:** forgery requires breaking **both** secp256k1 **and**
  ML-DSA-44. Holds under the standard hybrid argument (the AND-combiner for
  signatures is unconditional: an acceptor that requires both signatures is at
  least as strong as the stronger component).
- **Deterministic verification:** ML-DSA-44 verify is integer-only with no
  verify-time RNG — the same non-negotiable as the consensus path (Aegis §6) and
  the MAYO verifier. Bit-for-bit identical across validator hardware.
- **No new address grammar:** §D3 keeps 20-byte bech32, so the attack surface of
  address parsing is unchanged.
- **Opt-in blast radius:** a bug in the hybrid path can only affect accounts
  that opted in; classical accounts use untouched code paths.
- **Signing-side key hygiene:** ML-DSA-44 signing **may** use randomness (hedged
  signatures) — this is fine, it is *signing*, not consensus verification. The
  harness uses deterministic (seeded) signing for reproducible test vectors and
  notes that production signers may hedge.

---

## Alternatives considered

- **ML-DSA-65 for accounts.** Rejected as the default (Aegis §5.1): category 2
  (ML-DSA-44) is adequate for a hot path that is *also* protected by the
  classical half throughout the hybrid era, and 44 saves ~26 % of signature
  bytes. ML-DSA-65 stays reserved for the coldest keys.
- **Pure PQC accounts now (no classical half).** Rejected for the hybrid era:
  drops defense-in-depth and breaks wallet recovery of the classical half. The
  pure `mldsa44` type is registered but not the default until Phase H.
- **A new `SIGN_MODE_HYBRID`.** Rejected: fragments tooling for no security gain;
  the pubkey type already disambiguates the verifier (§D5).
- **SLH-DSA for accounts.** Rejected for the hot path (sig 17–50 KB) — reserved
  for break-glass cold keys (§4.6).

---

## Consequences

**Positive**

- First **quantum-safe transaction-signing** path for real users on a live
  Cosmos chain, opt-in and flag-day-free.
- Reuses the Phase B / ADR-006 ML-DSA-44 implementation — no new crypto in the
  trust base.
- One mnemonic still backs up everything; classical half stays
  wallet-compatible.
- Directly protects long-lived **agent DAO** keys — the headline Junoclaw use
  case.

**Negative / costs**

- Signatures grow from 64 B to 2,484 B per signer (≈ 39×). Acceptable at account
  volume (§5.1 shows this is the *cheap* layer); it is the consensus root where
  the same growth becomes a storage headline.
- Larger pubkeys in account state (1,345 B vs 33 B) for migrated accounts.
- Keyring records are larger and the CLI must learn one new key type.

---

## Implementation plan (D2 → D3)

1. **D2 — standalone harness (`aegis-accounts/`, this repo, no SDK fork).**
   Go module, stdlib + minimal vendored crypto, implementing exactly this ADR:
   keygen, hybrid sign/verify, address derivation, HD-from-mnemonic, with a test
   suite (determinism, round-trip, tamper/forgery rejection, address collision
   resistance, HD path independence, and a known BIP-32/BIP-44 vector to prove
   the classical derivation matches the standard). This validates the spec end to
   end **before** touching a fork. ← built alongside this ADR.
2. **D3 — Cosmos SDK fork integration (gated on a fork + devnet).**
   `cryptotypes.PubKey` impls (`mldsa44`, `hybrid`), Protobuf registration in the
   address/pubkey codec, the `SigVerificationDecorator` branch, the gas constant,
   and keyring/CLI support — porting the harness logic 1:1.

The harness is the source of truth for the crypto; the fork is a wiring exercise
on top of it (the same relationship `secretconn/` + `PORTING.md` has to the
CometBFT fork in Phase C).

---

## References

- PROJECT_AEGIS_JUNO_FULL_PQC §4.3 (accounts), §4.6 (cold keys), §5.1 (44 vs 65),
  §6 (determinism).
- ADR-006 (transport) — same ML-DSA-44 primitive, "both must verify" rule.
- FIPS 204 (ML-DSA), BIP-32 / BIP-39 / BIP-44, SLIP-0044 (coin type 118 = ATOM/
  Cosmos), RFC 5869 (HKDF), Cosmos SDK `crypto/address`, `crypto/keys/secp256k1`.
