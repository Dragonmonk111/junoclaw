# Porting `secretconn` into a CometBFT fork (Project Aegis Phase C)

This package is the **fork-ready** form of the ADR-006 hybrid secret
connection. It is shaped to mirror CometBFT's `p2p/conn/secret_connection.go`
so the change lands as a small, reviewable patch series rather than a rewrite.

- **Spec:** `docs/ADR-006-PQC-HYBRID-TRANSPORT.md`
- **Combiner (verified):** `aegis-transport/kem/`
- **This package:** `aegis-transport/secretconn/` (5 tests pass, `go vet` clean,
  Go 1.24 stdlib only)

---

## 1. Function-by-function map

| CometBFT `secret_connection.go` | This package | What changes |
|---------------------------------|--------------|--------------|
| `MakeSecretConnection(conn, locPrivKey)` | `MakeSecretConnection(conn, ed25519.PrivateKey)` | Same entry point and overall flow. |
| `genEphKeys()` → `(ephPub, ephPriv)` | inline X25519 gen **+ `mlkem.GenerateKey768()`** | Add an ephemeral ML-KEM-768 keypair alongside the X25519 one. |
| `shareEphPubKey(conn, locEphPub)` | `exchangeRaw` of an `ephHello` | The exchanged struct now carries `{version, x25519Pub, mlkemEk}` instead of just the pubkey. |
| `_, loEphPub, hiEphPub := sort(...)` | `orderEph(...)` | Unchanged ordering rule; reused to assign the ML-KEM initiator (lo) / responder (hi) roles. |
| `computeDHSecret(ephPub, ephPriv)` | `ephPriv.ECDH(remoteEphPub)` | Classical half unchanged. |
| — (new) | ML-KEM encapsulate/decapsulate (`ct` frame, hi→lo) | **New** PQ half: one ciphertext frame. |
| `deriveSecrets(dhSecret, loEphPub, hiEphPub)` | `deriveKeyMaterial(ssClassical, ssPQ, transcript)` | IKM becomes `ss_classical \|\| ss_pq`; salt is the transcript hash; HKDF-SHA256 expands to 2 dir keys + challenge. |
| `shareAuthSignature(sc, pubKey, transcript)` | `shareAuthSignature(privKey, challenge)` | Same station-to-station step; **hybrid ML-DSA-44 plugs in at the marked extension point.** |
| `(*SecretConnection).Write/Read` | `(*SecretConn).Write/Read` | Same AEAD-framed duplex; see divergence (2) on the cipher. |

---

## 2. Intentional divergences (and what the fork must restore)

The harness is **dependency-free** (Go 1.24 stdlib only) to stay aligned with
the rest of `aegis-transport`. The real fork should restore two CometBFT
choices; both are orthogonal to the ADR-006 security changes:

1. **AEAD cipher.** Harness uses **AES-256-GCM** (`crypto/aes`+`crypto/cipher`).
   CometBFT uses **ChaCha20-Poly1305** (`golang.org/x/crypto/chacha20poly1305`)
   with its own 1024-byte chunk framing and a per-direction nonce derived from a
   frame counter. In the fork: **keep CometBFT's existing ChaCha20 framing
   verbatim** and only swap the *key derivation* feeding it. The combiner output
   (`deriveKeyMaterial`) replaces the input to CometBFT's existing
   `hkdf`/`chacha` key setup.

2. **Node-key auth = hybrid, not Ed25519-only.** Harness signs the challenge
   with Ed25519 only. ADR-006 §D2 requires a **hybrid Ed25519 + ML-DSA-44** node
   key: sign with both, concatenate (algorithm-tagged), require **both** to
   verify. The extension point is the documented block in `shareAuthSignature`.
   Reuse the **same vendored ML-DSA-44** as consensus/accounts — do not add a
   second impl.

---

## 3. ML-KEM role rule (the one genuinely new wire step)

CometBFT's eph exchange is symmetric. ML-KEM is not (KEM, not DH), so a role
must be picked. We reuse the existing `loEphPub`/`hiEphPub` ordering:

- **lo peer = ML-KEM initiator.** Its `ek` (sent in the eph hello) is the one
  encapsulated against; it later **decapsulates** the `ct`.
- **hi peer = ML-KEM responder.** It **encapsulates** against lo's `ek` and
  sends the single `ct` frame (hi→lo).

This keeps exactly one `ek` (1,184 B) and one `ct` (1,088 B) on the wire, as in
the ADR-006 bandwidth table — no wasteful bidirectional KEM. Both peers still
send an `ek` in the hello (role isn't known until after the exchange); only the
lo peer's is used.

> Round-trip note: the `ct` rides as one extra frame in the eph phase, because
> the directional AEAD keys depend on `ss_pq` and so must be available *before*
> the encrypted auth exchange. This matches ADR-006's "one extra round-trip
> half" and does not add a full RTT.

---

## 4. Negotiation / downgrade (fork must relax the harness)

The harness is **fail-closed**: a version mismatch returns `ErrDowngrade`. The
fork must implement ADR-006 §D3 **downgrade-safe** negotiation instead:

- hybrid ↔ hybrid → full hybrid handshake.
- hybrid ↔ legacy → fall back to CometBFT's classical X25519+Ed25519 handshake,
  **log the peer as non-PQ, do not refuse**.
- Bind the negotiated version + any fallback into the transcript hash (already
  done here via `handshakeTranscript`) so a silent downgrade breaks auth.

---

## 5. Concrete patch plan against the fork

1. **Pin the fork point.** Clone the CometBFT tag Juno's target version uses
   (track the Aegis fork-rebase branch). Vendor ML-DSA-44 (the Aegis impl).
2. **Patch `genEphKeys`** to also produce an ML-KEM-768 keypair; widen the eph
   message to `ephHello` (`{version, x25519Pub, mlkemEk}`).
3. **Insert the KEM step** after `loEphPub/hiEphPub` are known (role rule §3);
   add the single `ct` frame.
4. **Replace `deriveSecrets` IKM** with `ss_classical || ss_pq` and salt with
   the transcript hash (port `deriveKeyMaterial`); keep ChaCha20 framing.
5. **Make the node key hybrid** in `shareAuthSignature` (§2.2); update
   `NodeKey` load/save to the algorithm-tagged `(ed25519, mldsa44)` format
   (Phase A key types) — fail-closed on unknown tags.
6. **Implement §4 negotiation** with legacy fallback + logging.
7. **Tests:** port `secretconn_test.go` (agreement, downgrade, tamper,
   transcript binding) into the fork's `_test.go`; add NIST ML-KEM KAT
   conformance.
8. **Devnet RTT (the C3 close-out):** run the forked binary on
   `junoclaw-bn254-1`, measure real-link handshake latency + per-connection
   bytes vs the classical baseline (the `cmd/twonode` numbers are the loopback
   reference). Confirm the ADR thesis on a real link: **cost is bytes, not CPU.**

---

## 6. Status

- [x] ADR-006 spec
- [x] `kem/` combiner + conformance (verified, cross-checked)
- [x] `cmd/twonode` loopback latency/bandwidth prototype
- [x] **`secretconn/` drop-in hybrid secret connection (this package) — 5 tests, vet clean**
- [ ] Apply patch series §5 to the CometBFT fork
- [ ] Hybrid ML-DSA-44 node key (§2.2 extension point)
- [ ] Downgrade-safe negotiation (§4)
- [ ] Devnet real-link RTT measurement (§5.8)
