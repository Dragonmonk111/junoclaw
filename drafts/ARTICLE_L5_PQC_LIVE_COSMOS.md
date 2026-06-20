# NIST L5 PQC Attestations on a Live Cosmos Chain — No Fork Required

*2026-06-20 · Dragonmonk / VairagyaNodes*

---

<!-- IMAGE: Hero — "The Fifth Lock" (16:9) — see Appendix prompt #1 -->

## TL;DR

We verified NIST Level 5 post-quantum signatures on a live Cosmos chain this week. Not on a new L1 built from scratch — on **Juno's uni-7 testnet**, using a CosmWasm smart contract anyone can deploy today. MAYO-5 (964-byte signatures, NIST Level 5) costs 799k gas pure-Wasm, or 361k with our precompile fork. That's less than a complex DeFi swap.

The "you need a greenfield chain for Level 5 PQC" argument is dead.

---

## Context

Two camps are forming around post-quantum cryptography (PQC) in blockchain:

1. **Greenfield** — build a new L1 with PQC at the consensus layer (Marius's Falcon-1024 custom BFT chain, Commonware's work)
2. **Migration** — add PQC to existing chains via smart contracts and optional forks (JunoClaw's approach on Juno)

Both are valid. But the greenfield camp has been claiming that NIST Level 5 security — the highest level, equivalent to AES-256 — is only practical with native execution on a purpose-built chain.

<!-- IMAGE: "Two Paths to the Summit" (3:2) — see Appendix prompt #2 -->

We disagree. And now we have the numbers.

---

## What We Measured

### MAYO: The Algorithm

MAYO (Multivariate quAdratic hash-based signature sYstem) is a NIST additional-signatures candidate. It's based on Oil & Vinegar multivariate quadratic equations — pure integer arithmetic, no floating point, no FFT. That makes it uniquely suited to wasm32 compilation: it runs in a smart contract with no C dependency.

| Variant | NIST Level | Signature | Public Key | Best For |
|---------|-----------|-----------|------------|----------|
| MAYO-2 | 1 | 186 B | 4,912 B | High-frequency attestations |
| MAYO-3 | 3 | 681 B | 2,986 B | High security, small PK |
| MAYO-5 | 5 | 964 B | 5,554 B | Maximum security (Falcon-equivalent) |

MAYO-5 signatures are **25% smaller** than Falcon-1024's 1,280 bytes at the same NIST Level 5. The tradeoff is a larger public key — but since we hash-store PKs on-chain (32 bytes permanent state), the PK only travels in the transaction payload, not in storage.

<!-- IMAGE: "The Receipt" (1:1, social card or inline) — see Appendix prompt #4 -->

### Live Testnet Results (uni-7, 2026-06-12)

| Variant | NIST | Verify Gas (pure-Wasm) | Verify Gas (precompile) | Speedup |
|---------|------|-----------------------:|------------------------:|--------:|
| MAYO-2 | L1 | 356,368 | 310,391 | 1.15× |
| MAYO-3 | L3 | 457,221 | 257,371 | 1.77× |
| MAYO-5 | L5 | 798,803 | 360,902 | 2.21× |

For context: a `cw20::Transfer` costs ~60-80k gas. A complex DeFi swap costs ~1M gas. **MAYO-5 at NIST Level 5 costs less than a DeFi swap** — and it's quantum-resistant.

At $0.10/JUNO, a Level 5 attestation costs ~$0.006. At $0.30/JUNO, it's under 2 cents.

### Precompile Results (junoclaw-bn254-1 devnet, 2026-06-17)

Our patched Juno fork adds a native `mayo_verify` host function to the wasmvm. The win grows with the parameter set: at L5, the precompile more than halves the cost (799k → 361k). It's short of the original 5-7× projection because, once the crypto is native, fixed CosmWasm/SDK transaction overhead (~250-310k gas) dominates.

---

## How It Works

### On-Chain Verification (Pure Rust, no_std)

The verifier (`junoclaw-mayo-verify`) is a pure-Rust, `#![no_std]` crate with zero C dependencies. It compiles to wasm32-unknown-unknown and runs inside CosmWasm's sandboxed wasm runtime.

<!-- IMAGE: "Fitting the Giant into the Cottage" (3:2) — see Appendix prompt #3 -->

Key engineering challenges solved:
- **Memory**: MAYO-5's expanded public key is ~1,661 KB if materialized — far beyond CosmWasm's 512 KB wasm memory limit. We implemented streaming AES-128-CTR PK expansion (`expand_pk` row-by-row), reducing peak heap to **151 KB** for MAYO-5.
- **GF(16) arithmetic**: Row-wise accumulation in `calculate_ps` (~165 KB → ~2 KB) and per-pair bin accumulation in `calculate_sps` (~4 KB → ~256 B).
- **Cross-validation**: All four variants (MAYO-1/2/3/5) are cross-checked bit-for-bit against the C reference implementation (`sriracha-mayo`).

### Smart Contract Integration

The `jclaw-credential` CosmWasm contract exposes:
- `Bud { mayo_pk }` — register a member with a MAYO public key (stores SHA-256 hash, 32 B permanent state)
- `VerifyMayoAttestation { addr, message, signature, public_key }` — verify a MAYO signature on-chain
- `MayoPkHash { addr }` — query a member's stored PK hash

The contract dispatches all four MAYO variants (1/2/3/5) and all three ML-DSA variants (44/65/87) — callers pick the security level per attestation.

### Optional Precompile

<!-- IMAGE: "The Old Door, The New Lock" (16:9) — see Appendix prompt #5 -->

On our patched Juno fork, `mayo_verify(variant, pk, msg, sig)` is a native host function. The contract routes to it when the `mayo-precompile` feature is enabled. Same contract, same wasm, same API — just faster.

---

## What This Means

### For the PQC-in-blockchain debate

The greenfield-vs-migration debate is not either/or — they're complementary Pareto points:

| Use case | Best approach |
|----------|--------------|
| PQC validator signatures from genesis | Greenfield (Marius) |
| PQC on 20 existing chains without 20 governance votes | Migration (JunoClaw) |
| Cheapest possible PQC verify gas | Native (greenfield) |
| PQC attestations portable across Cosmos today | Smart contract (JunoClaw) |
| NIST Level 5 on an existing chain | **Both work — we proved the contract path** |

### For JunoClaw

This closes the "Level 5 requires a new chain" argument. MAYO-5 at NIST Level 5 is live on uni-7 today — 799k gas pure-Wasm, 361k with precompile. The contract is portable to any CosmWasm chain (Osmosis, Neutron, Stargaze, etc.) with no chain upgrade.

### What's still classical

Accuracy matters: this is **application-layer** PQC. Juno validator consensus signatures, normal account signatures, IBC light-client assumptions, and P2P transport remain classically secured. Project Aegis (our parallel effort) is addressing consensus and transport with hybrid Ed25519 + ML-DSA-44 signatures and hybrid X25519 + ML-KEM-768 secret connections — but that's a separate milestone.

---

## What's Next

1. **ZK-proof of MAYO verification** — Groth16/BN254 circuit wrapping the verify function. ~200k gas, portable to any BN254-enabled chain (including Ethereum L2s).
2. **IBC cross-chain MAYO attestations** — verify PQC on Juno, relay to any Cosmos chain.
3. **Batch verification** — amortize tx overhead across N signatures.
4. **Project Aegis consensus** — hybrid Ed25519 + ML-DSA-44 validator signatures (Phase F consensus-safety verification already complete, protobuf persistence in progress).

---

## Reproduce

- **Live contract (uni-7):** `juno1zj39neajvynzv4swf3a33394z84l6nfduy5sntw58re3z7ef9p4q3w4y47`
- Contract code: `jclaw-credential` in this repo
- Verifier crate: `junoclaw-mayo-verify` (pure Rust, no_std)
- Benchmark script: `node deploy/benchmark-mayo-variants.cjs`
- Results: `deploy/mayo-benchmark-results.json`
- Precompile benchmarks: `docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md`

---

*Attribution: Dragonmonk / VairagyaNodes. MAYO is a NIST additional-signatures candidate (not a finalized standard). Falcon/FN-DSA was selected by NIST for standardization. Both are valid PQC choices with different tradeoffs.*

---

## Appendix: Leonardo.ai Art Prompts

> **House style** per `docs/ART_PROMPTS.md`: 2D hand-painted, old-world warmth,
> subtle techie soul, seaside/cottage setting. Studio Ghibli meets Beatrix Potter.
> Leonardo settings: aspect ratio 16:9 for headers, 3:2 for section images.
> Model: Leonardo Vision XL or Phoenix. Style: "Illustration" preset with
> "Hand-drawn" + "Watercolor" modifiers. Negative prompt: photorealistic, 3D render,
> neon, cyberpunk excess, text, watermark.

### 1. Hero — "The Fifth Lock" (header image, 16:9)

```
Studio Ghibli 2D hand-painted illustration, dramatic golden hour over a turquoise
sea. A weathered stone cottage perched on a grassy cliff, its heavy oak front door
fitted with FIVE padlocks of increasing size: a tiny brass one, a small iron one,
a medium steel one, a large ornate copper one, and an enormous ancient-looking lock
of dark tarnished metal engraved with hexagonal lattice runes that glow faintly
violet. The fifth and largest lock is clearly the focus — a small hedgehog in a
tweed waistcoat stands on a stool, calmly fitting the key to it. The door is NOT
a new door; it is old, scarred, mossy, clearly part of an existing cottage that
has stood for generations. Through the open window behind: a cluttered workshop
with a glowing terminal showing green scroll text, dried herbs, copper tools.
A lighthouse on the headland sweeps a prismatic beam. Wildflowers grow through
cracks in the stone. Color palette: golden amber, sea-glass teal, tarnished copper,
lattice violet, warm cream. Hayao Miyazaki style, visible brushstrokes,
watercolor edges, epic but cozy composition.
```

### 2. Section — "Two Paths to the Summit" (greenfield vs migration, 3:2)

```
Studio Ghibli 2D hand-painted illustration, panoramic mountain landscape above
the clouds. Two paths lead to the same snowy summit where a flag flies. The left
path: a brand-new stone staircase carved fresh into the cliff face, wide and
straight, with a single large golden eagle crest (Falcon) carved at its gate —
the greenfield path, beautiful but still under construction, scaffolding visible.
The right path: an ancient switchback trail along an existing mountainside, worn
by generations of footsteps, with five small waystation shrines at increasing
altitudes, each bearing a tiny glowing lock (the MAYO security ladder L1 through
L5). A traveller has already reached the fifth and highest shrine, planting a
small flag. The summit is the same height from both paths. Below the clouds:
a seaside village with fishing boats and cottage chimneys. Color palette:
alpine blue-white, stone grey, golden eagle, trail green, shrine amber,
sky gradient rose to indigo. Miyazaki epic landscape, hand-painted,
watercolor wash, contemplative and balanced — neither path is wrong.
```

### 3. Section — "Fitting the Giant into the Cottage" (memory optimization, 3:2)

```
Studio Ghibli 2D illustration, cozy cottage interior, warm lamp light. A small
badger in spectacles sits at a cluttered workbench. On the bench: an enormous
unrolled scroll that would clearly fill the entire room if spread flat (the
expanded MAYO-5 public key, 1,661 KB). But the badger is feeding it through a
small hand-cranked milling device that processes it one narrow strip at a time,
each strip dissolving into a tiny glowing hexagonal tile that stacks neatly in
a small wooden box (the 151 KB peak heap). The box is labeled "512 KB" and is
only about a quarter full. Around the badger: tea cups, scattered papers with
matrix equations, a sleeping cat on a stack of notebooks. Through the window:
the seaside lighthouse beam. Color palette: lamp amber, parchment cream,
hexagonal tile teal, scroll sepia, wood brown, cat orange. Miyazaki domestic
interior, detailed cozy clutter, Kiki's Delivery Service warmth.
```

### 4. Section — "The Receipt" (benchmark proof, 1:1)

```
Studio Ghibli 2D hand-painted illustration, a harbor-side market stall at noon.
A small field mouse in a waistcoat proudly holds up a long paper receipt with
five lines of numbers, standing beside an enormous brass weighing scale. On one
side of the scale: a tiny sealed envelope with a hexagonal wax seal (a MAYO-5
attestation). On the other side: a single acorn (the gas cost). The acorn side
is lighter — the envelope outweighs it in value but costs less than expected.
A wooden sign above reads "L5" in hand-painted letters. Behind the stall:
the seaside village, fishing boats, the lighthouse. A crowd of woodland
creatures peers curiously. Color palette: noon brightness, market warm
(terracotta, saffron, jade), brass gold, wax seal violet, receipt cream.
Beatrix Potter meets Spirited Away marketplace energy, watercolor and pencil,
1990s picture book illustration.
```

### 5. Section — "The Old Door, The New Lock" (no fork required, 16:9)

```
Studio Ghibli 2D hand-painted illustration, twilight on a seaside headland.
An ancient stone lighthouse — clearly old, weathered, covered in moss and
bird nests — but at its top, where a lamp room would be, a new crystalline
mechanism glows with soft violet hexagonal light (the MAYO-5 precompile,
fitted into an existing structure). The beam it casts is prismatic, painting
structured lattice patterns across the dark sea. A keeper stands at the base,
hand resting on the old stone wall, looking up with quiet satisfaction.
No scaffolding, no construction — the new mechanism sits seamlessly in the
old architecture. Below: cottage windows glow warm amber. Fishing boats
bob at anchor. Stars emerge. Color palette: twilight indigo, stone grey,
crystal violet, cottage amber, sea foam, starlight white. Nausicaä-era
Ghibli, epic scale, hand-painted, watercolor edges, hopeful and serene.
```

### Usage Notes

- Generate at **16:9** for the hero and section dividers; **3:2** for inline
  section images; **1:1** for social media cards.
- In Leonardo, use the **Illustration** style preset with **Hand-drawn** and
  **Watercolor** modifiers enabled.
- Negative prompt: `photorealistic, 3D render, neon, cyberpunk excess, text, watermark, blurry, deformed`
- These match the established JunoClaw visual identity from `docs/ART_PROMPTS.md`
  and the prior MAYO article (`articles/JUNOCLAW_MAYO_PQC_LIVE_2026_06_12.md`).
- **Do not** claim official Studio Ghibli affiliation.
