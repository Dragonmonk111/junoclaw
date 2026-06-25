# We Hardened a Live Cosmos Validator Against Quantum Computers — Without Touching the Chain

*2026-06-23 · Dragonmonk / VairagyaNodes*

---

<!-- IMAGE: Hero — "The Dragon and the Lighthouse" (16:9) — see Appendix prompt #1
     Studio Ghibli 2D hand-painted illustration, golden dusk over a dramatic
     headland. An ancient stone lighthouse stands at the cliff's edge, its lamp
     burning warm amber. Coiled protectively around the lighthouse base is an
     enormous dragon made entirely of geometric lattice patterns — its scales
     are hexagonal crystalline plates that refract the lighthouse beam into
     prismatic violet and gold across the darkening sea. The dragon is NOT
     threatening; it is sleeping, one eye half-open, guardian-still. Around
     the lighthouse: four small lanterns on iron poles at the four compass
     points of the headland, all glowing the same warm amber (the four
     localnet validator nodes). The cottage below has a glowing window,
     a hedgehog in a waistcoat visible inside at a terminal. Wildflowers
     in the cliff grass, fishing boats far below. Color palette: dusk
     indigo, amber gold, lattice violet, sea-glass teal, stone grey.
     Epic Miyazaki scale, hand-painted, watercolor edges, serene and
     powerful in equal measure. -->

---

## TL;DR

We ran a four-validator Juno localnet today on the live VairagyaNodes production machine — the same machine, the same disk, the same process — while the mainnet validator was safely offline. We measured the classical bandwidth baseline. We shut it down. We checked the production signing key checksum. It was untouched.

And along the way, we built the cryptographic foundation to make that key — and every key like it on every Cosmos validator — resistant to a quantum computer.

The production validator's checksum: `6ae8882e97d48be35c69ceb08bf6520ff0cd3bf7edbf556987ae385a63c0a6aa`. Before. After. Identical.

---

## The Problem No One Wants to Talk About

The secp256k1 elliptic curve secures every Cosmos validator key. Every Juno vote. Every IBC channel. Every staking withdrawal. It is broken by Shor's algorithm on a large-enough quantum computer in polynomial time.

NIST finalised ML-DSA (FIPS 204) and ML-KEM (FIPS 203) in August 2024. These are the replacements. The question is not whether you need them. The question is whether you migrate the chain you have or build a new one from scratch.

<!-- IMAGE: Section — "Two Keys on the Workbench" (3:2) — see Appendix prompt #2
     Studio Ghibli 2D hand-painted illustration, a cluttered cottage workshop
     lit by a single brass desk lamp. On the workbench: two keys side by side.
     The left key is a classical elliptic curve shape — smooth, elegant,
     flowing lines of polished bronze, worn from long use. The right key is
     the same shape but engraved with a hexagonal lattice pattern that glows
     faint violet (ML-DSA-44). A badger in spectacles holds both keys
     by their bows, studying them. They are being forged together into a
     single key: the hybrid — the two bows fused, both bit-patterns
     engraved into one shaft. On the wall behind: a chalkboard with the
     equation "VALID = secp256k1 AND ML-DSA". A cat sleeps on a stack
     of cryptography papers. Through the window: the lighthouse beam.
     Color palette: lamp amber, bronze gold, lattice violet, chalkboard
     grey-green, warm cream. Kiki's Delivery Service domestic warmth,
     hand-painted, Beatrix Potter detail. -->

The greenfield camp builds new chains with PQC at genesis. That is valid. But it does not help the 20 existing Cosmos chains with 20 sets of validators, 20 governance histories, and 20 communities of users. Migration is the harder engineering problem. It is also the one that actually matters.

---

## What We Built

Project Aegis is three interlocking upgrades. Not theoretical. Not planned. Written, tested, and green.

---

### Hybrid Consensus Keys — Cosmos SDK Fork

Every validator key becomes a fused pair:

| Component | Algorithm | Size |
|-----------|-----------|------|
| Classical half | secp256k1 | 33 B pubkey, 64 B sig |
| Post-quantum half | ML-DSA-44 (FIPS 204) | 1,312 B pubkey, 2,420 B sig |
| **Hybrid combined** | both required | **1,345 B pubkey, 2,484 B sig** |

A signature is valid only when **both halves verify**. An attacker who breaks the elliptic curve gains nothing. An attacker who somehow breaks ML-DSA-44 still faces the classical curve. The classical path is wire-identical to today — existing explorers, wallets, IBC relayers, and tooling see no change. The upgrade is purely additive.

Nine tests pass. All green. The most important one:

```
TestBothHalvesRequired — tamper either half, signature is rejected.
```

<!-- IMAGE: Section — "The Fused Lock" (1:1, social card) — see Appendix prompt #3
     Studio Ghibli 2D illustration, a single extraordinary padlock hanging
     on a mossy stone gate. The padlock is clearly two locks fused: the left
     half is classical brass, smooth and worn, with an elliptic curve
     engraved; the right half is crystalline lattice-work, glowing soft
     violet, mathematical and new. The keyhole is in the dead centre —
     both halves must be satisfied at once. A small hedgehog in a red
     knit waistcoat crouches at eye level with the lock, examining it
     with a magnifying glass. Behind the gate: a cottage garden, the
     lighthouse headland in the distance, the sea beyond. Golden hour
     light. Color palette: brass gold, lattice violet, stone grey, garden
     green, amber sunset. Precise Beatrix Potter detail on the hedgehog,
     Miyazaki grandeur on the background. -->

---

### Quantum-Safe Transport — CometBFT Fork

Validator gossip today runs on X25519 key exchange. Harvest-now-decrypt-later attacks — record the encrypted traffic today, decrypt with a quantum computer later — are real against this scheme.

We layered ML-KEM-768 (FIPS 203) on top of the existing handshake:

```
Classical:  X25519 DH → deriveSecrets → ChaCha20 framing
Hybrid:     X25519 DH + ML-KEM-768 → combiner (SHA-256) → deriveSecrets → ChaCha20 framing
```

The combiner is `SHA-256(X25519_secret || ML-KEM_shared_secret)`. Breaking the combined key requires breaking **both** the classical and post-quantum components simultaneously. The existing ChaCha20 framing is untouched. The existing golden test vectors pass unchanged.

An environment variable gates the upgrade: `AEGIS_HYBRID_TRANSPORT=1`. Operators can enable it per-node without a chain halt.

**We measured it.** Two ways — the pure handshake CPU cost over an in-memory pipe, and the wall-clock cost over real TCP sockets at simulated link latencies (both in the CometBFT fork, `p2p/conn/secret_connection_hybrid_{bench,rtt}_test.go`, AMD Ryzen 5 5600H):

| Handshake | Classical (X25519) | Hybrid (X25519 + ML-KEM-768) | Delta |
|-----------|-------------------:|-----------------------------:|------:|
| CPU per connection | 580 µs | 895 µs | **+315 µs (1.54×)** |
| Heap per connection | 25.3 KB | 65.6 KB | +40.4 KB |
| Bytes on wire (both peers) | 2,158 B | 5,623 B | **+3,465 B** |

And the round-trip cost over real sockets, median of 9, with injected one-way delay:

| Link RTT | Classical | Hybrid | Delta |
|---------:|----------:|-------:|------:|
| 0 (loopback) | 682 µs | 889 µs | +207 µs |
| 10 ms | 11.18 ms | 16.77 ms | +5.59 ms |
| 50 ms | 51.27 ms | 77.02 ms | +25.74 ms |

Two things matter here. First, **all of this is paid once per connection, at setup** — not per block, not per vote. A validator dials a handful of persistent peers and then talks to them for days. Second, the latency delta scales at roughly **half the link RTT**: the hybrid handshake adds one extra one-way leg (the ML-KEM-768 ciphertext), so it costs one additional message flight, not a multiplicative blow-up.

**The decisive number came from the live localnet.** We ran the same 4-node `junod-aegis` build twice — classical transport, then `AEGIS_HYBRID_TRANSPORT=1` — and sampled the consensus commit at the same height:

```
classical transport   commit_bytes = 2,265
hybrid   transport   commit_bytes = 2,265   (identical, byte-for-byte)
```

The post-quantum handshake protects the *link*, not the *payload*. It adds **zero bytes to consensus** and **zero per-block cost**. Compare this with hybrid consensus *keys* (next section), which cost 6.71× per commit forever. Quantum-safe transport is the cheapest win in the entire migration — a one-time ~315 µs and ~3.5 KB per peer connection, and nothing thereafter.

---

### PQC Attestations on CosmWasm

Covered in the previous article: MAYO and ML-DSA verification as native wasmvm precompiles, cutting on-chain PQC attestation cost by up to 2.21×. The `jclaw-credential` contract dispatches all variants. Application-layer PQC is live on uni-7 testnet today.

---

## The Numbers — Live Localnet Results

We ran it today. Four validators, one machine, ports offset +100 from the production node's defaults — with the `junod-aegis` binary built from the hybrid forks. Here is what we captured:

<!-- IMAGE: Section — "Four Lanterns on the Headland" (16:9) — see Appendix prompt #4
     Studio Ghibli 2D hand-painted panoramic illustration, dusk over a wide
     headland above the sea. Four iron lantern poles are arranged in a square
     on the clifftop grass, each lantern burning warm amber — the four
     localnet validator nodes. Thin copper wires (gossip connections)
     stretch between the lanterns, glowing faintly teal. A small hedgehog
     sits at a wooden desk between them, holding a clipboard, recording
     the light levels from each (measuring bandwidth). The lighthouse stands
     in the background, its own lamp off (the production validator, safely
     paused). A crescent moon rises. Wildflowers and sea-grass. Below:
     the dark sea, one or two fishing boat lights. Color palette: dusk
     indigo, lantern amber, gossip teal, cliff grey-green, moon silver,
     sea ink-blue. Miyazaki wide panoramic composition, hand-painted,
     contemplative. -->

### Classical baseline at N=4

| Metric | Value |
|--------|-------|
| Validators | 4 / 4 signing |
| Block height sampled | ~30 |
| Commit size (JSON/RPC) | 2,267 bytes |
| P2P bytes sent / peer / block | ~2,600 bytes |
| Consensus reached in | ~15 seconds |
| Production signing key: before | `6ae8882e...` |
| Production signing key: after | `6ae8882e...` — **identical** |

### Measured Hybrid-44 at N=4 (live localnet)

| Metric | Classical | Hybrid-44 | Increase |
|--------|-----------|-----------|----------|
| Validators | 4 / 4 signing | 4 / 4 signing | — |
| Block height sampled | ~30 | ~15 | — |
| Commit size (JSON/RPC) | **2,267 B** | **15,208 B** | **6.71×** |
| Signature size | 64 B | 2,491 B (hybrid wire) | ~38.9× per sig |
| Consensus reached | ~15 s | ~15 s | no delay observed |

The dominant term is four hybrid signatures. Each hybrid signature carries both an Ed25519 classical proof and an ML-DSA-44 post-quantum proof, framed by the ADR-008 §F2 wire format. The block body itself barely changes; the commit metadata is where the bytes live.

### Projected Hybrid-44 at N=100 (aegis-bench model)

| Metric | Classical | Hybrid-44 | Overhead |
|--------|-----------|-----------|----------|
| Signature size | 64 B | 2,484 B | 38.8× per sig |
| Commit bytes / block | ~56 KB | ~248 KB | ~4.4× total |
| Annual validator bandwidth | ~0.30 TB | **1.31 TB** | +1.01 TB/yr |
| ML-DSA-44 verify time | — | ~101 μs/sig | — |
| CPU for 100-validator round | — | ~10 ms | not the bottleneck |

**The CPU finding is the most important result.** At 101 μs per ML-DSA-44 verification, a full round of 100 validator signatures takes ~10 ms against a 6-second block time. The chain does not slow down. Bandwidth is the cost — 1.31 TB per year per validator. A commodity server with a 1 Gbps uplink handles this with headroom.

The "PQC consensus is too expensive" argument assumes compute is the bottleneck. It is not. The measured 6.71× commit-size increase at N=4 is the real-world bandwidth cost to defend validator consensus against a quantum computer.

---

## What Is Still Classical

Accuracy matters. This article covers consensus keys and transport. It does not cover:

- **Normal account signatures** — Juno wallets remain secp256k1 until a full key migration is coordinated
- **IBC light-client assumptions** — remain classically secured until IBC itself adopts hybrid keys
- **CosmWasm contract logic** — PQC attestations are opt-in per contract

These are the remaining phases of Project Aegis. They are planned, tracked, and follow directly from the consensus-layer foundation we proved today.

<!-- IMAGE: Section — "The Honest Map" (3:2) — see Appendix prompt #5
     Studio Ghibli 2D illustration, a hand-drawn map spread on a cottage
     table, lit by afternoon light through a small window. The map shows
     a coastline with several landmarks. Some landmarks have a glowing
     violet seal stamped on them (DONE: consensus keys, transport,
     CosmWasm precompile). Others are drawn in faint pencil, not yet
     stamped (IBC channels, account migration, batch verify). A small
     otter in a wool coat sits at the table, quill in hand, carefully
     inking in the next landmark to be stamped. The map is large, honest,
     and clearly a work in progress — the stamped sections are celebrated,
     the pencil sections are calm and expected. Through the window:
     the lighthouse. The sea beyond. Color palette: parchment cream,
     seal violet, ink brown, afternoon amber, sea-glass teal.
     Beatrix Potter precision meets Nausicaä cartographic detail. -->

---

## Why Not Just Build a New Chain?

We are sometimes asked why we pursue migration over a greenfield design. The answer is not philosophical — it is economic.

A new L1 starts with zero validators, zero liquidity, zero governance history, and zero user trust. Every existing Cosmos chain that wants PQC consensus must either migrate individually or route through the new chain via IBC — adding latency and additional trust assumptions. Juno alone has been running for four years. That is not easily reproduced.

Migration is harder to engineer. It requires surgical forks, careful port-offsetting, checksum verification, and a production runbook that proves the existing validator's key is untouched at the end. We wrote that runbook. We ran it. It works.

The greenfield and migration paths are not rivals — they are Pareto points. Different use cases. Marius's Falcon-1024 BFT design gives you PQC consensus from genesis. Project Aegis gives you PQC consensus on the chain that already exists. Both are correct. Only one of them requires Juno's 300+ validators to migrate to a new network.

---

## What Is Next

| Task | Status |
|------|--------|
| Build Aegis Juno binary (replace directives wiring forks) | **Done** — binary SHA-256 `98e6813a...bcacdb` |
| PQC localnet: repeat bandwidth test with Hybrid-44 sigs | **Done** — 6.71× commit-size increase measured |
| Run regen-mldsa.sh → emit wasmvm patches 20-28 | **Done** — 00-28 apply cleanly to cosmwasm v2.2.2 |
| Rebuild devnet with ML-DSA precompile | **Done** — `junoclaw/junod-bn254:devnet` rebuilt |
| Benchmark ML-DSA gas (closes §5.1 open number) | **Done** — ML-DSA-44/65/87 pure + precompile gas captured |
| C6: Measure hybrid transport handshake cost | **Done** — +315 µs CPU, +3,465 B/conn one-time; **0 B per block** (commit unchanged 2,265 = 2,265) |
| IBC light-client hybrid-key migration | **Design done** — ADR-009 (in-place `07-tendermint`, classical-half fallback, no flag day) |
| Normal account key PQC migration | **Design done** — ADR-010 (staged opt-in ladder, address-stable, no fund move) |
| Cross-arch determinism: ARM64 vs x86_64 verify hash | ARM hardware |

The consensus-layer foundation is in place, the transport cost is measured, and the two remaining migrations — IBC light clients (ADR-009) and account keys (ADR-010) — now have committed designs. What is left is the gated fork wiring and cross-architecture determinism verification on ARM hardware.

---

## Reproduce

- **Runbook:** `docs/VALIDATOR_SAFE_LOCALNET_RUNBOOK.md`
- **Transport handshake CPU benchmark:** `go test ./p2p/conn/ -run '^$' -bench BenchmarkSecretConnHandshake -benchmem` (CometBFT fork)
- **Transport handshake RTT + bytes:** `go test ./p2p/conn/ -run TestHybridHandshakeRTT -v` (CometBFT fork)
- **aegis-bench model:** `cargo run --release --features timing` in `aegis-bench/`
- **SDK fork (hybrid keys):** `Dragonmonk111/cosmos-sdk` @ `aegis-phase-d3-hybrid`
- **CometBFT fork (ML-KEM transport):** `Dragonmonk111/cometbft` @ `aegis-phase-cf-hybrid`
- **wasmvm fork (MAYO + ML-DSA precompile):** patches in `wasmvm-fork/patches/v2.2.2/`

---

*Post-quantum Cosmos is not a new chain. It is this chain, hardened.*

*Attribution: Dragonmonk / VairagyaNodes. ML-DSA and ML-KEM are NIST finalized standards
(FIPS 204, FIPS 203, August 2024). MAYO is a NIST additional-signatures candidate, not yet
finalized.*

---

## Appendix: Midjourney Art Prompts

> **House style** per `docs/ART_PROMPTS.md` and the prior MAYO article:
> 2D hand-painted watercolour, Miyazaki-inspired. Seaside/cottage setting.
> Warm amber, teal, lattice violet, stone grey palette. Woodland creature
> protagonists (hedgehog, badger, otter, field mouse). Old-world warmth,
> subtle techie soul.
> **Midjourney settings:** `/imagine` prompt below, append `--v 6.1 --style raw`.
> Aspect ratios: `--ar 16:9` hero/dividers, `--ar 3:2` inline, `--ar 1:1` social.
> Global `--no`: `photorealistic, 3D render, neon, cyberpunk, text, watermark, blurry, deformed`.

---

### 1. Hero — "The Dragon and the Lighthouse" (16:9, header)

```
ancient stone lighthouse on a dramatic clifftop at golden dusk, enormous
geometric lattice dragon coiled sleeping around the lighthouse base, dragon
scales made of hexagonal crystalline plates refracting the amber beam into
prismatic violet and gold across the dark sea, guardian dragon one eye
half-open, four small iron lanterns glowing amber at the compass points of
the headland, hedgehog in a wool waistcoat visible through a warm cottage
window below, wildflowers in cliff grass, fishing boats far below, first
stars emerging, 2D hand-painted watercolour illustration, Miyazaki-inspired,
warm amber and dusk indigo palette, lattice violet accents, visible
brushstrokes, epic yet serene composition, cozy and vast
--ar 16:9 --v 6.1 --style raw --no fire breathing, threatening, 3D, neon, text
```

---

### 2. Section — "Two Keys on the Workbench" (3:2, hybrid key section)

```
cluttered cottage workshop at night, single brass oil lamp, badger in
spectacles holding two ornate keys side by side, left key smooth worn
bronze with elliptic curve engraved, right key same shape but glowing
faint violet with hexagonal lattice pattern, small forge on the workbench
fusing both keys into one hybrid shaft, chalkboard on the wall with chalk
equation underlined twice, sleeping cat on a pile of papers, lighthouse
beam through the small window sweeping dark sea, 2D hand-painted
watercolour illustration, Beatrix Potter creature detail, Miyazaki domestic
warmth, lamp amber and bronze gold palette, lattice violet accent, cozy
scattered workshop clutter, visible pencil and brushstroke texture
--ar 3:2 --v 6.1 --style raw --no photorealistic, 3D, neon, text
```

---

### 3. Section — "The Fused Lock" (1:1, social card)

```
close-up of an extraordinary padlock on a mossy stone gate, left half aged
brass with worn elliptic curve engraving, right half crystalline hexagonal
lattice glowing soft violet, single keyhole dead centre, tiny hedgehog in
a red knit waistcoat crouching at eye level holding a magnifying glass,
calm satisfied expression, cottage garden in bloom beyond the gate,
lighthouse headland in the far distance, sea glittering strip, golden
afternoon light, 2D hand-painted watercolour, Beatrix Potter precision
on the hedgehog, Miyazaki grandeur on the background, brass gold and
lattice violet palette, garden green, stone grey, pencil and wash texture
--ar 1:1 --v 6.1 --style raw --no photorealistic, 3D, neon, text
```

---

### 4. Section — "Four Lanterns on the Headland" (16:9, localnet section)

```
panoramic clifftop headland at dusk, four iron lantern poles in a square
each burning warm amber, thin glowing teal copper wires strung between the
lanterns, small hedgehog at a wooden field desk with clipboard and quill
recording measurements, lighthouse in the background with its lamp dark
and unlit, crescent moon rising above the lighthouse, sea-grass and
wildflowers in the cliff grass, ink-blue sea far below with two distant
fishing boat lights, 2D hand-painted watercolour, Miyazaki wide panoramic
composition, dusk indigo and lantern amber palette, teal gossip-wire
glow, moon silver, contemplative and precise atmosphere, vast and cozy
--ar 16:9 --v 6.1 --style raw --no photorealistic, 3D, neon, text
```

---

### 5. Section — "The Honest Map" (3:2, what is still classical section)

```
afternoon light through a cottage window falling across a large hand-drawn
coastline map on a wooden table, some landmarks stamped with glowing violet
wax seals, other landmarks sketched in faint pencil not yet sealed, small
otter in a wool coat sitting at the table with a quill carefully inking the
outline of the next landmark, steaming teapot nearby, lighthouse and sea
visible through the window, parchment and ink map with cartographic
detail, 2D hand-painted watercolour, Beatrix Potter precision on the otter,
Nausicaä-era cartographic grandeur on the map, parchment cream and wax
seal violet palette, afternoon amber light, ink brown, sea-glass teal,
work-in-progress feeling, honest and calm
--ar 3:2 --v 6.1 --style raw --no photorealistic, 3D, neon, text, words
```

---

### Usage Notes (Midjourney)

- Paste each prompt block directly into Midjourney `/imagine`.
- `--ar` ratios are already embedded: **16:9** hero/dividers, **3:2** inline, **1:1** social.
- Use **`--v 6.1 --style raw`** for the most faithful interpretation. If MJ
  auto-upgrades to v7, re-add `--v 6.1` explicitly.
- If the dragon in prompt #1 comes out aggressive, add `--no fire, roaring, claws` and
  retry with `sleeping guardian dragon, curled protectively`.
- If MJ refuses Ghibli-adjacent style, replace `Miyazaki-inspired` with
  `2D hand-painted animation aesthetic, Isao Takahata style`.
- The map in prompt #5 should have **no legible text** — MJ handles this better
  with `--no text, words` already appended; if text bleeds in, add `illegible labels`
  to the body prompt.
- These match the established JunoClaw visual identity from `docs/ART_PROMPTS.md`
  and the prior MAYO article (`drafts/ARTICLE_L5_PQC_LIVE_COSMOS.md`).
