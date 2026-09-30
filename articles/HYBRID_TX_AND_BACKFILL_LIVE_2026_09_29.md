# Hybrid PQ Transactions Are Live on DevNet — and So Is Fast Catch-Up

*2026-09-29 — engineering update for the JunoClaw community*

Two weeks ago we proved a post-quantum signature verifies on-chain inside the
JunoClaw runtime. Today we went further: a **complete hybrid transaction —
secp256k1 + MAYO — was signed by a standalone tool, broadcast over gRPC, and
accepted by consensus on the live devnet.** And while we were in there, we also
proved the new bulk catch-up path under real network conditions.

## The hybrid transaction, end to end

The missing piece until now was signing. The MAYO signer needs a Linux toolchain,
so it's packaged as a Docker image (`junoclaw-hybrid-sign`). The flow, all working
today:

1. `hybrid-sign keygen` produces a hybrid keypair — a secp256k1 key and a
   MAYO-2 key, encoded as one hybrid public key. From it we derive a JunoClaw
   address the same way every address is derived: domain-separated
   RIPEMD-160(SHA-256) over the hybrid pubkey bytes.
2. `hybrid-sign sign` builds a real Cosmos `MsgSend` transaction — `TxBody`,
   `AuthInfo`, `SignDoc` — signs the standard `SHA-256(SignDoc)` digest with
   **both** keys, packs the signatures, and writes raw `TxRaw` bytes.
3. `tx-sender broadcast` pushes those bytes to the node's gRPC endpoint, the
   same path every transaction takes.

The result: tx `64A0E72C…` committed at height **165266**, `code=0`, ~109k gas
used, transfer applied. Consensus ran the native pure-Rust MAYO verifier
(`junoclaw-mayo-verify`) alongside the secp256k1 check — both had to pass.

This is the practical core of the airdrop promise: JunoClaw accounts are
post-quantum **and** classical. If secp256k1 falls to a quantum adversary, MAYO
still stands; neither side is trusted alone.

## Live catch-up verification

Separately, the bulk backfill path got its first live test. We stopped
`node-2` at ~height 164,800, let the other three validators keep finalizing
(~1 block/second), then reconnected it with a ~700-block gap.

The node did exactly what the new path is designed to do: discovered the missing
finalized chain via digest links, fetched payloads in parallel over P2P, executed
the chain in order — and rejoined consensus proposing blocks again, in full
lockstep with the same `app_hash` as its peers. Roughly **1,600 heights executed
in ~15 minutes (~1.7× live block rate)** under real network conditions, no state
divergence, no halts.

This is the machinery that matters for testnet scale: a validator (or a fresh
node, or a lagging light-client checkpoint) that falls behind doesn't replay
consensus — it backfills.

## What's next — Phase 2

With transaction authentication hybrid, the next layer is **protocol
authentication**: validator identity (ed25519 + MAYO) and consensus certificates
(BLS-threshold **and** a k-of-n MAYO cert — both must verify). The design is
written down in `docs/PQ_PROTOCOL_AUTH.md`; implementation starts on devnet
behind a flag.

For everyone waiting on the airdrop: the quantum-safe accounts you'll receive
aren't a promise anymore — they're running code, verifying signatures in
consensus, today.
