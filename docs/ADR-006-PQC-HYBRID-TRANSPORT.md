# ADR-006: Hybrid X25519 + ML-KEM-768 transport for CometBFT (Project Aegis Phase C)

**Status:** Proposed — Phase C / C1 deliverable of [PROJECT_AEGIS_JUNO_FULL_PQC](./PROJECT_AEGIS_JUNO_FULL_PQC.md) §4.2, §9
**Date:** 2026-06-15
**Authors:** Dragonmonk / VairagyaNodes (with Cascade)
**Scope:** CometBFT `p2p` secret connection (surfaces #3 node identity, #4 transport confidentiality)
**Depends on:** Phase A foundations (algorithm-tagged keys + KEM combiner lib)

> This ADR specifies the handshake; it does **not** change consensus, accounts,
> or IBC. It is the lowest-risk Aegis phase: node-local, hybrid (no peer
> breakage), and it closes harvest-now-decrypt-later (HNDL) on inter-node
> traffic. No consensus-layer code has been changed by this document.

---

## Context

CometBFT peers establish an authenticated, encrypted channel via the
**secret connection** handshake (`p2p/conn/secret_connection.go`). Today it is:

1. **Ephemeral key agreement** — each side generates an ephemeral **X25519**
   keypair, exchanges public keys, and computes a shared secret by ECDH.
2. **Key derivation** — the shared secret is run through HKDF-SHA256 to derive
   two ChaCha20-Poly1305 directional keys and a transcript/challenge value.
3. **Authentication** — each side signs the handshake transcript with its
   **Ed25519 node key** and exchanges the signature, binding the ephemeral
   exchange to a long-lived peer identity (station-to-station construction).
4. **Framing** — subsequent traffic is sealed with ChaCha20-Poly1305.

A quantum adversary breaks this in two distinct ways:

- **Confidentiality (HNDL, surface #4):** X25519 is a discrete-log primitive.
  An adversary that records handshake + ciphertext today can, post-Shor,
  recover the X25519 shared secret and decrypt the entire session
  retroactively. This is a **harvest-now-decrypt-later** threat and is
  *already accruing* — traffic captured in 2026 is decryptable on Q-day.
- **Authentication (surface #3):** the Ed25519 node key is forgeable post-Shor,
  letting an adversary impersonate a peer (eclipse / MITM). This is a *Q-day*
  threat, not retroactive, but rotation across a peer set is slow.

Per the Aegis `exposure × lifetime` rule, #4 (HNDL) is the urgent one because it
is retroactive; #3 rides along in the same handshake change.

---

## Decision

Make the secret connection **hybrid** — classical AND post-quantum, so security
is the *stronger* of the two and an un-upgraded peer is never locked out during
transition.

### D1 — Confidentiality: hybrid X25519 + ML-KEM-768 key agreement (#4)

Replace the single X25519 ECDH with a **dual key agreement** whose outputs are
combined:

```
ss_classical = X25519(eph_sk_local, eph_pk_remote)          # 32 B, as today
ss_pq        = ML-KEM-768 encapsulate/decapsulate            # 32 B (FIPS 203)
session_key  = HKDF-SHA256(
                 ikm  = ss_classical || ss_pq,               # KEM combiner
                 salt = H(transcript),
                 info = "junoclaw/aegis/secret-connection/v1"
               )
```

**KEM combiner (the security-critical glue).** We use the IETF/NIST-aligned
**concatenation KEM combiner**: feed `ss_classical || ss_pq` (fixed 64 B,
length-implicit because both halves are fixed-size) as IKM to HKDF-SHA256, with
the handshake transcript hash as the salt for domain separation and
transcript binding. Properties:

- **IND-CCA of the combined KEM** holds if *either* component is IND-CCA secure
  (a classical break of X25519 alone, or a PQ break of ML-KEM alone, does not
  recover the session key). This is the whole point of hybrid.
- **Transcript binding** via the salt prevents key-reuse / reflection across
  distinct handshakes.
- We do **not** invent bespoke glue: concatenate-then-KDF with domain
  separation is the construction in `draft-ietf-tls-hybrid-design` and the
  X-Wing / NIST SP 800-56C two-step lineage.

**ML-KEM-768 message flow (one extra round-trip half).** ML-KEM is a KEM, not a
DH, so the exchange is asymmetric:

- The **initiator** generates an ML-KEM-768 keypair and sends its
  **encapsulation key** `ek` (1,184 B) alongside its ephemeral X25519 pubkey.
- The **responder** runs `(ct, ss_pq) = Encapsulate(ek)` and returns the
  **ciphertext** `ct` (1,088 B) alongside its ephemeral X25519 pubkey.
- The **initiator** runs `ss_pq = Decapsulate(dk, ct)`.

Both sides now hold the same `ss_pq` (32 B) and the same `ss_classical`, and
derive `session_key` identically. The handshake stays **one round trip** — the
ML-KEM `ek`/`ct` ride on the existing two handshake messages; only their size
grows (see Bandwidth).

### D2 — Authentication: hybrid Ed25519 + ML-DSA-44 node key (#3)

The node key becomes a hybrid key `(ed25519_pk, mldsa44_pk)` (algorithm-tagged
per Phase A). Each side signs the handshake transcript with **both** halves; the
peer accepts only if **both** verify (during the hybrid era — see Migration for
the relaxation rule). ML-DSA-44 is chosen for consistency with the consensus /
account workhorse (Aegis §3, §5.1) and because its verify is cheap and
integer-only (confirmed on-chain in Phase B). This reuses the **same vendored
ML-DSA-44 implementation** as the rest of Aegis — no new signature scheme.

### D3 — Wire format & negotiation

A new handshake version tag (`secret-connection/v1-hybrid`) is exchanged in the
first message. Negotiation is **strictly downgrade-safe**:

- hybrid ↔ hybrid → full hybrid handshake (D1 + D2).
- hybrid ↔ legacy → fall back to legacy X25519 + Ed25519 (classical security),
  logged as a non-PQ peer. **No connection is refused for lack of PQ support**
  during the transition window.
- The downgrade decision is itself bound into the transcript hash, so an
  attacker cannot silently force a downgrade without breaking the (still-present)
  classical authentication.

---

## Crate / implementation selection

The secret connection lives in **Go** (CometBFT), so the runtime impl is Go.
Rust crates are needed only for the **conformance + bandwidth harness**
(`aegis-bench/`) and the Phase A combiner library used by the CosmWasm side.

### Go (runtime — the handshake itself)

| Need | Choice | Rationale |
|------|--------|-----------|
| ML-KEM-768 | **Go stdlib `crypto/mlkem`** | The devnet already builds on **Go 1.24** (`devnet/Dockerfile` `GO_TAG=1.24-bookworm`), and Go 1.24 ships `crypto/mlkem` (ML-KEM-768 + 1024) in the **standard library**. Zero external dependency, maintained by the Go security team, constant-time, FIPS 203 final. This is the pragmatic and most auditable choice. |
| X25519 | **Go stdlib `crypto/ecdh`** | Already what CometBFT effectively uses; keep it. |
| HKDF | **`golang.org/x/crypto/hkdf`** (or stdlib `crypto/hkdf`, Go 1.24+) | Standard, audited. |
| ML-DSA-44 node sig | **Aegis-vendored ML-DSA-44 (Go)** | Same impl Phase A vendors for consensus/accounts; do **not** introduce a second ML-DSA. (Go 1.24 has no stdlib ML-DSA yet; track stdlib `crypto/mldsa` if/when it lands and prefer it.) |
| Fallback ML-KEM (if stdlib unavailable) | **`cloudflare/circl`** `kem/mlkem/mlkem768` | Widely deployed, audited, used by the Go TLS hybrid ecosystem; the named fallback if a target build pins Go < 1.24. |

**Decision:** target **Go stdlib `crypto/mlkem`**; `cloudflare/circl` is the
documented fallback only for pre-1.24 builds.

### Rust (conformance + bench harness, `aegis-bench/`)

| Need | Choice | Rationale |
|------|--------|-----------|
| ML-KEM-768 | **`fips203`** (integrity-chain / Eric Schorn) | **Same author and audit posture as `fips204`**, which Aegis already uses for ML-DSA (`cosmwasm-crypto-mldsa`, contract path). Pure Rust, `no_std`, no default RNG — consistent vendor story across the whole PQC stack. |
| Formal-verification cross-check (optional) | **`libcrux-ml-kem`** (Cryspen) | HACL*/F\*-verified ML-KEM; use as a *second* implementation in the differential conformance test (mirrors the MAYO C cross-check discipline, §6). |

**Decision:** primary Rust crate **`fips203`** for vendor consistency; add
**`libcrux-ml-kem`** as the differential cross-check oracle in CI.

### ML-KEM-768 fixed sizes (FIPS 203, for the bandwidth model)

| Artifact | Size |
|----------|-----:|
| Encapsulation key `ek` (initiator → responder) | 1,184 B |
| Ciphertext `ct` (responder → initiator) | 1,088 B |
| Shared secret `ss` | 32 B |
| Decapsulation key `dk` (kept local) | 2,400 B |

---

## Bandwidth & latency impact

The cost is a **one-time per-connection handshake overhead**, *not* a per-block
or per-message cost (contrast the consensus-signature bandwidth in §5.1, which
is forever). Per secret-connection handshake, hybrid adds:

| Direction | Classical today | + Hybrid PQ adds | Driver |
|-----------|----------------:|-----------------:|--------|
| initiator → responder | 32 B (X25519 pk) | **+1,184 B** | ML-KEM `ek` |
| responder → initiator | 32 B (X25519 pk) | **+1,088 B** | ML-KEM `ct` |
| each side (auth sig) | 64 B (Ed25519) | **+2,420 B** | ML-DSA-44 sig |
| each side (node pk) | 32 B (Ed25519) | **+1,312 B** | ML-DSA-44 pk |

Total extra handshake bytes ≈ **~2.3 KB** (KEM) **+ ~3.7 KB/side** (hybrid auth)
≈ **~9.7 KB per connection**, paid **once** at connect. For a node with ~50
peers that is ~485 KB of one-time handshake traffic — negligible against block
gossip. **C3 will measure the wall-clock handshake latency delta**; ML-KEM-768
encap/decap is ~tens of µs (cheaper than X25519), so the expected latency cost
is dominated by the extra bytes on the wire, not crypto CPU.

---

## Alternatives considered

### Alternative A: ML-KEM only (drop X25519)

**Rejected.** Removing the classical half forfeits defense-in-depth: a future
ML-KEM break (or an implementation flaw) would leave transport with no fallback.
Hybrid costs ~2.3 KB/handshake to guarantee security is the *stronger* of the
two. The cost is trivial and one-time.

### Alternative B: TLS 1.3 hybrid (X25519MLKEM768) instead of secret connection

**Rejected for now.** Swapping CometBFT's bespoke secret connection for TLS 1.3
with the standardized `X25519MLKEM768` group is architecturally cleaner and
reuses `libcrux-kem`'s built-in hybrid, but it is a *much* larger change to
CometBFT's p2p stack and breaks the existing node-key authentication model.
Worth a separate ADR as a long-term direction; not the minimal Phase C win.

### Alternative C: ML-KEM-1024 instead of 768

**Rejected.** NIST L3 (ML-KEM-768) is the recommended transport parameter set
(Aegis §3) and matches the de-facto ecosystem hybrid (`X25519MLKEM768`). L5
(1024) adds bytes without a transport-relevant security need given the X25519
half is also present.

### Alternative D: XOR / naive concatenation without KDF

**Rejected.** Combining shared secrets must go through a KDF with domain
separation and transcript binding to preserve IND-CCA of the combiner. Raw XOR
or bare concatenation as the session key is a known footgun.

---

## Security & determinism considerations

- **HNDL closed for inter-node traffic** the moment both peers run hybrid — the
  retroactive-decryption window stops accruing for those links.
- **Downgrade resistance:** the negotiated version and any fallback are bound
  into the transcript hash that both node keys sign; a silent downgrade requires
  forging the classical signature too.
- **KEM combiner soundness:** concatenate-then-HKDF with transcript salt; secure
  if either component KEM is secure (draft-ietf-tls-hybrid-design lineage).
- **Constant-time:** Go stdlib `crypto/mlkem` and `crypto/ecdh` are
  constant-time; ML-KEM decapsulation includes the FIPS 203 implicit-rejection
  path (no timing oracle on decap failure).
- **Determinism boundary:** encapsulation *requires* RNG (this is correct and
  expected — it is key agreement, not consensus verification). The
  *consensus-critical* determinism rule (§6) does **not** apply to transport key
  agreement; it applies to signature *verification*. Transport secrecy needs
  good randomness, not reproducibility.
- **Conformance:** NIST ACVP / KAT vectors for ML-KEM-768 run in CI; differential
  test cross-checks Go stdlib vs `fips203` vs `libcrux-ml-kem` (three-way),
  mirroring the MAYO C cross-check discipline.

---

## Migration

### Transition window (hybrid era)

1. **Ship hybrid-capable binaries.** Nodes advertise `secret-connection/v1-hybrid`
   but accept legacy peers (classical handshake) without refusal.
2. **Both-verify during hybrid.** When both peers are hybrid, both signature
   halves and the combined KEM are mandatory.
3. **Measure coverage.** Track the fraction of connections that are full-hybrid
   (a per-node metric) before considering any tightening.
4. **Tighten by policy (Phase H), never silently.** Only once ecosystem coverage
   is high does governance/operator policy flip to *require* the PQ half on
   selected links. This is the §5 Phase H gate, not part of Phase C.

### Node-key format

Hybrid node keys are algorithm-tagged (Phase A): `(tag, ed25519_pk, mldsa44_pk)`.
Old nodes that cannot parse the tag reject the key (fail-closed) rather than
mis-parsing — the same self-describing-key discipline as the account layer (§3).

---

## Open questions (for review)

1. **KDF info string.** Proposed `"junoclaw/aegis/secret-connection/v1"`. Final
   domain-separation label before any interop?
2. **Auth scheme reuse.** Confirm ML-DSA-44 (not 65) for the node key — matches
   §5.1, but transport is lower-volume so 65 is *affordable* here if a higher
   margin on peer identity is wanted. Lean 44 for one-impl simplicity.
3. **Stdlib vs circl default.** Default to Go stdlib `crypto/mlkem` and treat
   `cloudflare/circl` purely as a pre-1.24 fallback — agreed?
4. **TLS 1.3 migration (Alt B)** — do we want a follow-up ADR to track replacing
   the bespoke secret connection with standardized TLS hybrid long-term?
5. **Negotiation tag transport** — piggyback the version tag on the existing
   first handshake frame, or add an explicit version byte? (Prefer piggyback to
   avoid an extra round trip.)

---

## What this ADR explicitly does NOT decide

- Consensus signatures (#1, #2) — Phase F, separate work.
- Account / tx signatures (#5–#8) — Phase D, `ADR-007-PQC-HYBRID-ACCOUNTS`.
- The exact CometBFT tag/fork point — depends on the Aegis fork-rebase track.
- Whether to migrate to TLS 1.3 hybrid (Alt B) — deferred to its own ADR.
- When to *require* the PQ half (deprecate classical-only) — Phase H, governance.

---

## References

- **Parent plan:** [`PROJECT_AEGIS_JUNO_FULL_PQC.md`](./PROJECT_AEGIS_JUNO_FULL_PQC.md) §4.2, §5, §6, §9
- **Phase B precedent (precompile + benchmark discipline):** [`MLDSA_PRECOMPILE_BENCHMARK_RESULTS.md`](./MLDSA_PRECOMPILE_BENCHMARK_RESULTS.md)
- **ADR house-style precedent:** [`ADR-001-BN254-PRECOMPILE.md`](./ADR-001-BN254-PRECOMPILE.md)
- **FIPS 203 (ML-KEM):** NIST FIPS 203
- **Go `crypto/mlkem`:** Go 1.24 standard library (ML-KEM-768 / 1024)
- **`fips203` crate:** integrity-chain (same author as `fips204` used by `cosmwasm-crypto-mldsa`)
- **`libcrux-ml-kem`:** Cryspen, HACL\*/F\*-verified ML-KEM (differential oracle)
- **`cloudflare/circl`:** `kem/mlkem/mlkem768` (pre-1.24 fallback)
- **Hybrid KEM construction:** `draft-ietf-tls-hybrid-design`; X-Wing; NIST SP 800-56C two-step KDF

---

*Apache-2.0. Comments and revisions welcome via PR against `docs/ADR-006-PQC-HYBRID-TRANSPORT.md`.*
