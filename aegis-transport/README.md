# aegis-transport — Phase C hybrid KEM harness (C2 + C3)

Reference implementation and conformance/property tests for the hybrid
**X25519 + ML-KEM-768** secret-connection key agreement specified in
[`../docs/ADR-006-PQC-HYBRID-TRANSPORT.md`](../docs/ADR-006-PQC-HYBRID-TRANSPORT.md)
(Project Aegis, surfaces #3/#4).

It is the C2 deliverable: it validates the **composition** ADR-006 introduces on
top of ML-KEM — the KEM combiner and transcript binding — end to end.

## What it proves

- **Agreement.** A full initiator/responder handshake derives an *identical*
  32-byte session key on both peers via the ADR-006 combiner
  `session_key = HKDF-SHA256(ss_x25519 || ss_mlkem768, salt=H(transcript), info="junoclaw/aegis/secret-connection/v1")`.
- **Wire sizes.** ML-KEM-768 `ek` = 1,184 B and `ct` = 1,088 B, pinned to the
  ADR-006 bandwidth table (drift fails the test / `SelfCheckSizes`).
- **Transcript binding / downgrade safety.** Flipping the negotiated version in
  the transcript the initiator authenticates makes the peers fail to agree —
  a silent downgrade is detectable.
- **Combiner soundness props.** Determinism and order-sensitivity of the
  `ss_classical || ss_pq` combiner; ML-KEM implicit-rejection on a tampered
  ciphertext (no panic, no agreement).

## Dependencies

**None beyond the Go 1.24 standard library** — `crypto/mlkem`, `crypto/ecdh`,
`crypto/hkdf`, `crypto/sha256`, `crypto/rand`. This matches ADR-006's runtime
selection: the devnet already builds on Go 1.24 (`devnet/Dockerfile`
`GO_TAG=1.24-bookworm`), so the handshake uses the maintained, constant-time
stdlib ML-KEM rather than a third-party crate.

## Conformance posture

Full **ACVP / NIST KAT** conformance of ML-KEM-768 itself is *inherited from the
Go standard library's own test suite* (`src/crypto/mlkem`, ACVP-tested by the Go
security team). Choosing stdlib means we do not maintain our own ML-KEM vectors;
this harness covers the parts ADR-006 adds. The Rust-side differential
(`fips203` vs `libcrux-ml-kem`) is implemented in
[`../aegis-kem-diff`](../aegis-kem-diff) (the C2b sub-step).

## Run

> The Windows host has no Go toolchain; run inside WSL / the devnet container
> where Go 1.24 is available.

```sh
# from aegis-transport/
go test ./...               # conformance + property tests (incl. wire codec)
go run .                    # demo: agreement, wire sizes, timing
go run ./cmd/twonode 300    # C3: two-node handshake over loopback TCP
```

## C3 — two-node prototype (`cmd/twonode`)

Runs the hybrid handshake between a real initiator and responder over a loopback
TCP socket, with the session key **confirmed over the wire** (HMAC tag +
ack), and measures latency + bandwidth against a classical X25519-only baseline.
Measured (loopback): hybrid adds **+2,350 B/handshake** (≈ the ~2.3 KB KEM
prediction) and ~**+0.24 ms** — the cost is bytes, not crypto CPU, as ADR-006
argues. Wire framing lives in `kem/wire.go` (length-prefixed fields matching the
transcript discipline).

## Status

- **C2 (this harness):** done — combiner + transcript binding validated on Go
  stdlib ML-KEM-768.
- **C2b:** done — Rust `fips203`-vs-`libcrux-ml-kem` differential lives in
  [`../aegis-kem-diff`](../aegis-kem-diff) (+ `ACVP_WIRING.md`).
- **C3:** done — two-node loopback prototype (`cmd/twonode`); real-link RTT
  awaits folding into a CometBFT fork on the devnet.
