# Phase C5 — CometBFT fork: hybrid secret connection (results)

**Status:** DONE (build + tests green). **Date:** 2026-06-17.

The ADR-006 hybrid (X25519 + ML-KEM-768) secret connection is folded into a real
CometBFT fork and proven to build and pass tests. This is the C5 deliverable; C6
(devnet real-link RTT) is the remaining, devnet-gated step.

## What was done

- **Fork:** `cometbft/cometbft` branch `v0.38.x`, shallow-cloned to
  `aegis-forks/cometbft` (gitignored; the fork keeps its own git history).
- **Additive patch (no rewrite of the classical path):**
  - New `p2p/conn/secret_connection_hybrid.go` — `MakeSecretConnectionHybrid()`
    implements the hybrid handshake, reusing the package internals (`sort32`,
    `computeDHSecret`, `deriveSecrets`, `signChallenge`, `shareAuthSignature`,
    the `SecretConnection` struct, ChaCha20-Poly1305 framing, nonce handling).
  - New `p2p/conn/secret_connection_hybrid_test.go` — hybrid handshake + AEAD
    read/write + combiner-binds-both tests.
  - `go.mod`: `go 1.22.11` → `go 1.24.0` (for the stdlib `crypto/mlkem`; no new
    external dependency).
  - `p2p/transport.go`: `upgradeSecretConn` now selects the hybrid path when
    `AEGIS_HYBRID_TRANSPORT=1`, so one binary runs either path (A/B for C6).

## Design (mirrors ADR-006)

- **Key agreement:** classical X25519 ECDH **and** ML-KEM-768 encaps/decaps.
  Roles use the existing lexical sort of the X25519 pubkeys: **lo = ML-KEM
  initiator** (decapsulates), **hi = responder** (encapsulates, sends one
  ciphertext). Exactly one `ek` (1184 B) per peer in the eph message and one
  `ct` (1088 B) hi→lo on the wire.
- **Combiner:** `combineHybridSecrets(dh, ssPQ) = SHA-256(dh || ssPQ)` feeds the
  EXISTING `deriveSecrets`. So the AEAD keys depend on **both** secrets, yet
  `deriveSecrets` stays a pure function of a 32-byte input — the classical golden
  vectors are untouched.
- **Downgrade/tamper resistance:** both ML-KEM encapsulation keys and the
  ciphertext are bound into the Merlin transcript, so any tamper breaks the STS
  challenge MAC and authentication fails.
- **Auth:** unchanged ed25519 STS challenge (hybrid ML-DSA-44 node-key auth is a
  separate, documented extension point in `secretconn/PORTING.md` §2.2).

## Verification (WSL, go1.24.0)

```
cd aegis-forks/cometbft
go test ./p2p/conn/ -v     # all PASS:
  TestHybridSecretConnectionHandshake          PASS   (new)
  TestHybridSecretConnectionReadWrite          PASS   (new)
  TestHybridCombineSecretsBindsBoth            PASS   (new)
  TestMakeSecretConnection (5 adversarial)     PASS   (existing, unchanged)
  TestSecretConnectionHandshake / ReadWrite    PASS   (existing)
  TestDeriveSecretsAndChallengeGolden          PASS   (existing golden — KDF intact)
go build ./p2p/...         # BUILD_OK (transport swap compiles)
go vet   ./p2p/conn/       # clean
```

The existing **adversarial** (`evil_secret_connection_test.go`) and **golden**
KDF tests passing **unchanged** is the key evidence the patch is additive and
non-regressive.

## Remaining (C6 — devnet, gated)

1. Build the full node binary (`make build`) for the devnet image.
2. Run two patched nodes with `AEGIS_HYBRID_TRANSPORT=1`; measure handshake RTT
   and per-connection bytes vs the classical baseline (env unset).
3. Record in `PHASE_C6_RTT_RESULTS.md`. Expectation (from C3 loopback): cost is
   ~2.4 KB of handshake bytes, not CPU.
