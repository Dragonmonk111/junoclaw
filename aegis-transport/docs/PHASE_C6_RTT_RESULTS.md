# Phase C6 — hybrid-transport real-link RTT (results)

**Status:** DONE (measured). **Date:** 2026-06-17.

Quantifies the cost of the ADR-006 post-quantum-hybrid (X25519 + ML-KEM-768)
secret-connection handshake vs the classical X25519 one, using the **real
CometBFT fork code** (`MakeSecretConnectionHybrid` from `p2p/conn`) over **real
TCP sockets** (not the in-memory pipe used by the functional tests).

## Method

- Harness: `p2p/conn/secret_connection_hybrid_rtt_test.go` (`TestHybridHandshakeRTT`).
- Each trial opens a real `127.0.0.1` TCP socket pair and runs both peers'
  handshakes concurrently (`libs/async.Parallel`), exactly as `transport.go` does.
- A symmetric one-way delay is injected on every `Write` to model link
  propagation, so the wall-clock includes the handshake's round trips at a given
  RTT. Reported value is the **median of 9** trials per cell.
- Bytes-on-wire are latency-independent (counted once at zero delay).
- Reproduce: `go test ./p2p/conn/ -run TestHybridHandshakeRTT -v`

## Results (WSL2, go1.24.0)

| link RTT | classical | hybrid  | delta (hybrid − classical) |
|---------:|----------:|--------:|---------------------------:|
| 0 (CPU)  | 707 µs    | 1.078 ms| **+371 µs**                |
| 10 ms    | 11.27 ms  | 16.76 ms| +5.49 ms                   |
| 50 ms    | 51.28 ms  | 76.99 ms| +25.7 ms                   |

**Bytes on wire (both peers, full handshake):** classical **2158** →
hybrid **5623** → **+3465 bytes**.

## Interpretation

- **CPU is negligible.** The pure-compute overhead (ML-KEM-768 keygen + encaps +
  decaps + extra transcript hashing) is **~0.37 ms** — sub-millisecond, one-time,
  per connection. ML-KEM is fast; this matches ADR-006's "cost is in bytes, not
  crypto CPU."
- **One extra half-RTT.** The delta tracks the injected one-way delay almost
  exactly (+5.49 ms at 5 ms one-way; +25.7 ms at 25 ms one-way). That is **one
  additional one-way flight**: the responder (lexical-hi peer) must receive the
  initiator's ML-KEM encapsulation key in the ephemeral exchange before it can
  encapsulate and return the ciphertext, and the initiator (lo) cannot derive
  keys until that ciphertext arrives. This +0.5 RTT is inherent to adding a KEM
  on top of the simultaneous DH exchange.
- **+3.4 KB on the wire.** Breakdown: both peers send an ML-KEM ek (1184 B each =
  2368) plus one ciphertext (1088 B) hi→lo, + length-prefix framing ≈ **3465 B**.
  Both peers send an ek because neither knows its lo/hi role until the X25519
  pubkeys are compared mid-exchange; sending both eks trades one unused ek
  (1184 B) for avoiding a role-negotiation round trip — the right trade.

## So what (operational impact)

- Handshakes are per-connection at dial time, not per-block or per-message.
  Steady-state consensus/gossip throughput is **unaffected** — only the one-time
  peer dial pays +0.5 RTT and +3.4 KB.
- For a node with ~50 peers, the extra is ~170 KB of one-time handshake traffic
  total and a few ms of extra dial latency — irrelevant next to block/gossip
  bandwidth.

## Possible future optimization (not needed)

- Drop one ek by negotiating lo/hi from the dialer/listener role instead of from
  the X25519-pubkey comparison, saving 1184 B per handshake. Not worth the added
  protocol complexity at these volumes.

## Relationship to C5

C5 proved the hybrid handshake is correct and downgrade/tamper-resistant in the
real fork (functional + adversarial + golden tests). C6 proves it is cheap. Both
use the same fork code path now gated by `AEGIS_HYBRID_TRANSPORT=1` in
`p2p/transport.go`.
