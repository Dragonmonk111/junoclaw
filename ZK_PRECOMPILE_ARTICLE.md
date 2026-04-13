# Killing ETH With Love — Why Juno Is Stealing Ethereum's Best Idea

## JunoClaw Proves Groth16 ZK Verification in Pure CosmWasm. Now We Want the Precompile.

---

> *"Juno will kill Ethereum with love."*
> — Jake Hartnell, somewhere in 2022, probably at 2am

---

He wasn't wrong. He just didn't specify the murder weapon.

Turns out it's a **BN254 elliptic curve pairing**. The same one Ethereum has had since Byzantium in 2017. The same one that powers every zkRollup, every privacy protocol, every Groth16 proof that's ever settled on mainnet.

We didn't fork Ethereum's code. We didn't wrap it in an EVM compatibility layer. We sat down and wrote a pure Rust Groth16 verifier that compiles to CosmWasm — from scratch, using arkworks — and deployed it on Juno testnet.

Then we looked at the gas bill.

---

**[IMAGE 1]**

> **Midjourney prompt**: *A young monk sitting on a tatami floor, cross-legged, ink brush in hand, carefully painting an elliptic curve on a large scroll of rice paper. The curve glows faintly with teal circuit traces. Through the paper screen behind, a massive Ethereum diamond logo floats in a misty sky like a distant mountain — beautiful but far away. Cherry blossom petals drift through the room. 2D hand-drawn illustration, rough pencil linework with watercolor wash, Studio Ghibli background detail, warm earth tones with selective teal and amber accents, contemplative and slightly melancholic, manga panel composition --ar 16:9 --style raw --s 200 --v 6.1*

> *"You don't kill what you love. You learn from it."*

---

## The Heist

Ethereum's EIP-196 and EIP-197 gave the world two things: cheap elliptic curve addition/multiplication, and cheap bilinear pairing checks — both on the BN254 curve. These are the primitives that make Groth16 verification cost less than a token transfer.

**187,000 gas.** That's all it costs to verify a zero-knowledge proof on Ethereum. Cheaper than sending someone 10 USDC.

On Juno today? We don't have those precompiles. So we did it the hard way.

---

## The Hard Way

JunoClaw's `zk-verifier` contract is a proof-of-concept Groth16 verifier written in pure Rust using the arkworks library ecosystem:

- **ark-bn254** — BN254 curve implementation
- **ark-groth16** — Groth16 proving system  
- **ark-ec, ark-ff, ark-serialize** — field arithmetic and serialization

No C dependencies. No precompiles. No Ethereum. Just Rust compiling to `wasm32-unknown-unknown` and running inside CosmWasm's sandbox.

The circuit is deliberately simple: prove knowledge of `x` such that `x² = y`. The prover knows `x = 3`, the verifier only sees `y = 9`, and the Groth16 proof convinces the chain that someone knows the square root — without revealing it.

**It works.** Nine tests pass. The proof verifies. The math is correct.

The gas cost is horrifying.

---

**[IMAGE 2]**

> **Midjourney prompt**: *A hand-drawn bar chart on graph paper, sketched in pencil with rough crosshatching. Three bars labeled in handwritten text: "hash check" (tiny, green), "precompile" (tiny, teal), "pure wasm" (massive, red, breaking through the top of the page and crumpling the paper). A small stick figure stands next to the red bar looking up in dismay. Scattered coffee ring stains on the paper. 2D illustration, notebook doodle style, incel-coded hand-drawn aesthetic, messy but precise, black ink on cream paper with selective color highlights --ar 16:9 --style raw --s 150 --v 6.1*

> *"The math works. The gas bill doesn't."*

---

## The Numbers

| Approach | Gas Cost | % of Block Limit |
|----------|----------|-------------------|
| SHA-256 hash check (what JunoClaw does now) | ~200K | 2% |
| Groth16 with BN254 precompile (what Ethereum has) | ~187K | 2% |
| **Groth16 in pure CosmWasm (measured on uni-7)** | **371,486** | **~4%** |

That last row is the surprise. A single ZK proof verification in pure Wasm costs only ~371K gas — about 4% of a block limit. It works, and it's closer to the precompile cost than anyone expected.

The BN254 pairing check is the bottleneck. In Ethereum, it's a native opcode that runs in Go at near-C speeds. In CosmWasm, it's 500 million Wasm instructions of modular exponentiation, extension field arithmetic, and Miller loop computation, each individually metered.

The precompile would still make it **2× cheaper.** Same math. Same security. Just executed by the chain runtime instead of the Wasm sandbox. And as circuits grow more complex (more public inputs, larger proofs), that gap widens fast.

---

## Wait — Why Is This "Killing ETH With Love"?

Because we're not competing with Ethereum. We're *learning from it.*

Jake said Juno would kill ETH with love, and here's what that looks like in practice:

1. **Ethereum invented the primitive** (BN254 precompiles, 2017)
2. **Juno's people left to build something different** (CosmWasm, sovereign chains, real governance)
3. **Now we're bringing the best of ETH back** — not by wrapping an EVM, not by forking geth, but by building a native implementation that fits CosmWasm's architecture

The BN254 pairing precompile isn't an Ethereum feature. It's a *math feature.* Ethereum just got there first. There's no reason every CosmWasm chain shouldn't have it.

**No Cosmos SDK chain has native BN254 pairing today.** Not Osmosis. Not Neutron (RIP). Not Sei. Not Injective. Juno could be first.

---

**[IMAGE 3]**

> **Midjourney prompt**: *Two characters sitting on opposite sides of a small wooden bridge over a quiet stream at golden hour. On the left, a figure in an orange hoodie with a small diamond-shaped pendant (Ethereum), looking tired but content. On the right, a figure in a dark blue hoodie with a circular claw pendant (Juno), sketching equations in a notebook. Between them on the bridge, a small glowing crystal sphere containing a swirling elliptic curve. They're not fighting. They're just... sitting. 2D hand-painted, rough pencil outlines with soft watercolor, Studio Ghibli pastoral composition, warm golden light, wildflowers in the grass, melancholic beauty, manga panel --ar 16:9 --style raw --s 250 --v 6.1*

> *"The bridge between chains isn't made of messages. It's made of math."*

---

## What JunoClaw Would Do With It

Right now, JunoClaw's WAVS pipeline works like this:

```
TEE computes → SHA-256 attestation hash → stored on-chain → re-verified via hash check
```

Trust model: *the hardware enclave ran the code correctly.* If you trust Intel SGX, you trust the result.

With Groth16 + BN254 precompile:

```
TEE computes → generates Groth16 proof → proof verified on-chain → mathematical certainty
```

Trust model: *the math is correct.* You don't need to trust Intel. You don't need to trust the operator. You don't need to trust anyone. The proof either verifies or it doesn't.

A Groth16 proof is 256 bytes. The verification key is stored once (~1KB). The verification costs 187K gas — *less than the hash check we're doing now.*

That's the punchline. **ZK verification with a precompile would be cheaper than what we're already doing.**

---

## The Gap

Here's the Cosmos ecosystem ZK landscape as of today:

| Chain | BN254 Support | How |
|-------|--------------|-----|
| Ethereum | ✅ | EIP-196/197 precompiles (since 2017) |
| Sui | ✅ | Native Move module |
| Polygon, Arbitrum, OP | ✅ | Inherited from Ethereum |
| Sei, Injective | ⚠️ | EVM module only, not CosmWasm |
| **Every pure CosmWasm chain** | **❌** | **Nothing** |

CosmWasm's crypto API (issue #751) lists BN254 pairing as "Bonus Points" — acknowledged as valuable but not implemented. The supported curves are secp256k1, ed25519, and BLS12-381. BN254 is the missing piece.

---

**[IMAGE 4]**

> **Midjourney prompt**: *A hand-drawn world map on old parchment paper, with chains represented as small glowing dots connected by thin lines. Ethereum is a large bright diamond in the center. Around the edges, smaller dots labeled in scratchy handwriting: "Sui ✓", "Polygon ✓", "Arbitrum ✓". In the Cosmos cluster, several dots are dark and unlit: "Osmosis ✗", "Neutron ✗ (rip)", "Sei ⚠". One dot labeled "Juno" has a tiny hand-drawn question mark next to it, and a small figure is drawing a line from Juno to Ethereum with a pencil. Navigator's map aesthetic, aged paper texture, 2D hand-drawn, ink and watercolor, cartographer's style with anime character details --ar 16:9 --style raw --s 200 --v 6.1*

> *"X marks the spot. It was always BN254."*

---

## The Ask

We're not asking Juno to become an L2. We're not asking for an EVM module. We're asking for **three host functions**:

```rust
fn bn254_add(p1: &[u8], p2: &[u8]) -> Result<Vec<u8>, ...>;
fn bn254_scalar_mul(p: &[u8], s: &[u8]) -> Result<Vec<u8>, ...>;
fn bn254_pairing_check(pairs: &[u8]) -> Result<bool, ...>;
```

That's it. Three functions. Implemented in Go using `gnark-crypto` (battle-tested, MIT licensed, used by Consensys). Added to `wasmvm` the same way `secp256k1_verify` was added. Shipped in the next `wasmd` upgrade.

**Effort**: 3–5 weeks of dev work. One chain upgrade.

**Result**: Every CosmWasm contract on Juno can verify ZK proofs. Not just JunoClaw — anyone. Private DeFi. zkML inference proofs. Cross-chain ZK light clients. Verifiable AI agent outputs.

The upstream PR could even go to CosmWasm core, making every CosmWasm chain benefit. Juno just ships it first.

---

## What We've Built So Far

The proof-of-concept is live in the JunoClaw repo:

- **`contracts/zk-verifier/`** — Full Groth16 BN254 verifier contract
- **9 tests** including adversarial cases (tampered proofs, wrong inputs, mismatched VK)
- **Benchmark instrumentation** measuring native CPU time and estimating Wasm gas
- **`docs/BN254_PRECOMPILE_CASE.md`** — Complete gas analysis, precedent research, implementation roadmap
- **Deploy script** with `--dry-run` mode for offline validation

The contract compiles to `wasm32-unknown-unknown`, passes all tests, and is ready for testnet deployment. It exists specifically to demonstrate two things:

1. **BN254 Groth16 verification works in pure CosmWasm** (it does — 371K gas on uni-7)
2. **A precompile would cut that cost in half** (187K vs 371K — and the gap grows with circuit complexity)

The PoC is the evidence. The precompile is the solution.

### Testnet Deployment Evidence

Chain: `uni-7` | Code ID: `64`  
Contract: `juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem`

| Step | TX Hash | Gas Used |
|------|---------|----------|
| Upload WASM | `9AADCE8CEB92B7341A03A746BD31BBE42A572469AE0B0195EEB0ABB08F30D837` | 3,684,912 |
| Instantiate | `E07B012540B8873D1D405CBED9B625DAEB855DEFA010D8E6607A59C3DED0B7A2` | 165,247 |
| Store VK | `C6956B5E94AF3F248B62EBF31E7EB948BB068AF357128A26C0AB1E0BA43E6180` | 212,935 |
| **Verify Proof** | `F6D5774EE2073E2DD011399A7E96889BA026ED67C6A510D208FD5C575080F4DA` | **371,486** |

- Pure CosmWasm verify gas: **371,486**
- Precompile baseline: **~187,000**
- Ratio: **2.0x** (pure CosmWasm vs precompile)

---

**[IMAGE 5]**

> **Midjourney prompt**: *A vast calm sea at golden hour, still as glass, reflecting a sky of layered peach and slate. In the middle distance, a small rocky island holds a single weathered lighthouse, its beam cutting a slow arc through low mist. In the foreground, the edge of a massive ancient structure — half Laputa, half server rack — rises from the water: stone ramparts overgrown with thick moss and hanging ferns, but threaded through with softly glowing copper conduit and teal fiber-optic veins that pulse like a heartbeat. The machinery is not fighting nature; the stone has settled into the reef, barnacles climbing the coolant pipes, tide pools forming in the compute bays. A lone figure sits on the mossy parapet, bare feet dangling over the water, a small terminal screen open on their lap casting faint teal light on their face. Seabirds roost on a decommissioned antenna array above. The lighthouse beam sweeps across the scene once, illuminating everything for an instant — the circuitry, the kelp, the figure, the stillness. 2D illustration, hand-drawn pencil linework with watercolor wash, Studio Ghibli background painting detail — Castle in the Sky meets Yokohama Kaidashi Kikou — warm amber horizon bleeding into cool grey-blue sea, selective teal accents on the living circuitry only, atmosphere of deep calm after long work, the machine and the ocean coexisting without conflict, wide cinematic composition --ar 16:9 --style raw --s 250 --v 6.1*

> *"Nine tests pass. The proof verifies. Now we need the chain to meet us halfway."*

---

## The Love Letter

Jake was right. You kill Ethereum with love by taking its best ideas and giving them a home where they're treated better.

Ethereum's BN254 precompiles are buried under gas auctions, MEV extraction, and L2 fragmentation. On Juno, a ZK proof verification would be a first-class citizen — cheap, accessible to any CosmWasm contract, governed by a community that actually votes on things.

We built the PoC. We measured the gap. We wrote the case.

**Now we're asking: can Juno be the first CosmWasm chain with native ZK verification?**

The math says yes. The precedent says yes. The code is written.

All that's left is love.

---

*JunoClaw is an open-source agentic AI platform built on Juno Network. The zk-verifier contract, gas analysis, and full implementation are available at [github.com/JunoClaw](https://github.com/JunoClaw).*

*Built on the work of Jake Hartnell, Ethan Frey, and the arkworks contributors.*

---

## Image Generation Summary

| # | Scene | Style | Quote |
|---|-------|-------|-------|
| 1 | Monk painting elliptic curve, ETH diamond in distant sky | Ghibli watercolor + manga | *"You don't kill what you love. You learn from it."* |
| 2 | Hand-drawn gas bar chart, red bar breaking through page | Notebook doodle, incel-hand-drawn | *"The math works. The gas bill doesn't."* |
| 3 | Two figures on a bridge, glowing math sphere between them | Ghibli pastoral, golden hour | *"The bridge between chains isn't made of messages. It's made of math."* |
| 4 | Navigator's map of chains, Juno drawing line to ETH | Parchment cartographer + anime | *"X marks the spot. It was always BN254."* |
| 5 | Overhead desk shot, laptop + origami crane + tea | Ghibli night-coding, warm/cool light | *"Nine tests pass. The proof verifies. Now we need the chain to meet us halfway."* |

All prompts target **Midjourney v6.1** with `--style raw` for hand-drawn authenticity. The aesthetic threads: rough pencil linework, watercolor wash, warm earth tones, selective teal/amber circuit accents, contemplative mood, slightly messy human imperfection.
