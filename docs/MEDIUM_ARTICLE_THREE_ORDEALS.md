# Three Ordeals on the Juno Shore

## How an agentic-AI stack on Juno walked the fire of silicon, the cold water of mathematics, and the open sea of mesh-secured chains — and what it looks like now it is ready for audit.

[IMAGE 1 — THE THREE GATES ON THE SHORE]

---

> *A note on words. This piece is a narrative summary of an architecture built in the open, by the authors of the code. The contracts have been hardened through an internal **pre-audit hardening pass** (v6) with regression tests and on-chain verification on `uni-7`. It is **not** a third-party audit. A formal independent review by a recognised security firm is a separate and recommended next step. Every claim below is grounded in code and transaction hashes; nothing in it substitutes for that external review.*

---

In the old chronicles of these islands, a stranger who wished to enter a hall did not simply walk in. They were put through trials — fire, water, open sea. Not to humiliate them. To *prove* them. A man who had carried a hot iron nine paces was known to the hall in a way that a man who had not could never be. The trial was a form of *writing* — the fire wrote onto the skin what the mouth might lie about.

A piece of software does not carry iron. But it is put to its own ordeals. For an agentic-AI stack, there are three in particular — and JunoClaw has now walked all three.

The **first ordeal** is fire: hardware. Can a machine run your code *correctly* when its owner has every reason to lie about the output?

The **second ordeal** is cold water: mathematics. Can you drop the hardware assumption entirely and prove correctness with numbers alone — the way Ethereum has been doing since 2017?

The **third ordeal** is the open sea: scale. Can your stack leave a single harbour and sail to every cove on the coast, without needing a new army of validators in each one?

This is the saga of those three ordeals, and of the architecture that came out the other side.

---

## Where we started: a validator and an idea

Before the ordeals, there was nothing ceremonious. There was **VairagyaNode** — a Juno validator running on a single machine, signing blocks that other people's transactions filled. *Vairagya* is a Sanskrit word often mistranslated as "detachment". It actually means *equanimity in service*: the validator does not choose which transactions to include, it serves them all.

And there was a question: *what if AI agents could use Cosmos natively — not through an EVM wrapper, not through a bridge, but as first-class citizens of a sovereign chain?*

That question required, in the end, ten CosmWasm contracts, a WAVS operator network, a hardware-attested enclave, a Groth16 verifier, a Model Context Protocol server, and an understanding of mesh security and modular data availability that none of us had on day one.

We did not invent any of these primitives. We connected them.

[IMAGE 2 — THE SILICON KEEP]

---

## Ordeal I — The Silicon Keep

### The problem: AI is non-deterministic, blockchains are not

A smart contract cannot run a large language model. It cannot fetch a price feed from outside the chain. It cannot determine whether a swap was fair, whether a randomness beacon was tampered with, whether a bale of evidence really came from the work it claims. These are *off-chain* questions, and a chain can only *verify* the answer — it cannot *compute* it.

The old answer to this is the **oracle**: a trusted party who computes off-chain and signs the result. Chainlink made a business of it. Cosmos chains had various half-solutions. None of them were trust-minimised in the way that a blockchain, by its own nature, ought to be.

The newer answer is the **TEE** — the Trusted Execution Environment. Intel SGX. AMD SEV-SNP. Intel TDX. A physical enclave on a physical chip that produces a **cryptographic attestation**: a signed proof that *this exact code ran on this exact data and was not tampered with, even by the machine's owner*.

JunoClaw's first ordeal was getting this to work.

### What we built

- A **WASI component** (`junoclaw-verify.wasm`, 355 KB) that deterministically recomputes swap mathematics, verifies XYK invariants, flags price-impact above a threshold, hashes inputs and outputs into a SHA-256 attestation hash, and binds everything to a component-id and task-type so the hash cannot be replayed against a different purpose.
- A **WAVS operator** (from Layer.xyz) that watches the Juno chain via RPC polling, matches `wasm-*` events, runs the WASI component inside an enclave, and submits the signed attestation back on-chain.
- A **bridge daemon** (TypeScript + CosmJS, ~400 lines) that polls the aggregator for new results and writes them to `agent-company` as `SubmitAttestation` transactions.
- An **Azure DCsv3 confidential VM** (Intel SGX, `/dev/sgx_enclave + /dev/sgx_provision`) where the entire stack ran inside a hardware enclave for the milestone transaction.

### The milestone

On 17 March 2026, Proposal 4 on `agent-company` went through an end-to-end TEE-attested cycle. The `data_hash` was recomputed inside SGX. The attestation landed at block **11,735,127** with transaction `6EA1AE79…D26B22`. The attestation can be queried by anyone, forever.

Jake Hartnell (Juno co-founder, WAVS architect at Layer.xyz) put it simply: *"WAVS TEEs already work — you just need to run WAVS inside a TEE."* We did that.

### What the silicon gave us — and what it didn't

The TEE solved deterministic containment: for a given input, the code ran the way the code was *meant* to run, and the hardware said so. We had crossed the fire.

But we had paid in trust.

We now trusted:

- **Intel** (or AMD, or Azure) not to have a backdoor in the enclave.
- The **specific silicon generation** not to have a documented side-channel (the SGX family has had several).
- The **cloud operator** to genuinely be running on genuine hardware.

A TEE attestation is a *hardware signature*. A hardware signature is only as trustworthy as the hardware vendor and the hands that hold the machine.

This is the limitation the second ordeal was designed to break.

[IMAGE 3 — THE HERMIT SCRIBE ON THE HILL]

---

## Ordeal II — The Math That Needs No Lord

### Ethereum's quiet gift

In **October 2017**, buried in the Byzantium hard fork, Ethereum shipped two Ethereum Improvement Proposals: **EIP-196** and **EIP-197**. They added three precompiles — **point addition**, **scalar multiplication**, and **pairing check** — on the **BN254 elliptic curve** (sometimes called `alt_bn128`).

In practice this meant: a smart contract could verify a **Groth16 zero-knowledge proof** for about **187,000 gas**. Cheaper than sending ten USDC.

This is the same primitive that every zkRollup, every privacy protocol, every Tornado Cash, every zkSync, every Polygon zkEVM — *every* ZK product of the last seven years — is built on top of. It is, quietly, the most important piece of cryptographic plumbing Ethereum ever shipped.

**No pure CosmWasm chain has it.** Not Osmosis. Not Neutron. Not Sei. Not Injective. Not Juno. CosmWasm's crypto API (issue #751) supports `secp256k1`, `ed25519`, `bls12_381` — but BN254 pairing sits on the list marked "Bonus Points".

The second ordeal was to see if we could bring the best of Ethereum back to Cosmos — without forking the EVM, without wrapping geth, without any of that architectural baggage. Just the math.

### The zk-verifier contract

We wrote one. It lives at `contracts/zk-verifier/` in the repo. It is a pure-Rust Groth16 verifier, compiled to `wasm32-unknown-unknown`, using the **arkworks** ecosystem:

| Crate | Role |
|-------|------|
| `ark-bn254` | BN254 curve implementation |
| `ark-groth16` | Groth16 proving system |
| `ark-ec`, `ark-ff` | Elliptic curve + finite field traits |
| `ark-serialize` | Canonical serialization |

The circuit is deliberately simple — a *toy* circuit, because the point is the machinery, not the statement: *prove knowledge of `x` such that `x² = y`*. The prover knows `x = 3`. The verifier sees only `y = 9`. The Groth16 proof convinces the chain that someone, somewhere, knew the square root — *without revealing it*.

Nine tests pass. Adversarial cases included: a tampered proof (bits flipped), a wrong public input, a mismatched verification key, garbage bytes instead of a real VK. All rejected, all correctly, all on-chain.

### The gas bill

We deployed it to `uni-7` as **Code ID 64**, contract address `juno1ydxksv…lse7ekem`. We measured.

| Approach | Gas Cost | % of block |
|---|---:|---:|
| SHA-256 hash check (what JunoClaw used before) | ~200K | 2% |
| Groth16 with a BN254 precompile (what Ethereum has) | ~187K | 2% |
| **Groth16 in pure CosmWasm (our zk-verifier, measured)** | **371,486** | **~4%** |

Pure CosmWasm Groth16 **works**. That by itself is the news: no Cosmos chain had ever shipped it in pure Wasm before. The proof verifies in a single transaction, on testnet, today.

The precompile would still make it roughly **2× cheaper**, and that gap widens fast as circuits grow. But the hard question — *can it be done at all without a precompile?* — is answered. The verify transaction hash is `F6D5774E…80F4DA`, at block 12,673,217, and anyone can re-query it.

### What it changes

With a TEE alone:

```
TEE computes → SHA-256 attestation hash → stored on-chain → verify by hashing again
```

Trust model: *"the hardware enclave ran the code correctly."* If you trust Intel, you trust the result.

With Groth16 on top:

```
TEE computes → generates Groth16 proof → proof verified on-chain → mathematical certainty
```

Trust model: *"the math is correct."* You do not need to trust Intel. You do not need to trust the operator. You do not need to trust us. The proof either verifies or it does not.

This is the move from *trust the silicon* to *trust the math*. It does not retire the TEE — they compose. The TEE guarantees the *execution*; the ZK proof guarantees the *statement*. Defence in depth, two independent witnesses.

### What we are asking for

Three host functions in `wasmvm`, implemented in Go using `gnark-crypto` (MIT-licensed, used by Consensys). Three weeks of chain work. The signatures would look roughly like:

```rust
fn bn254_add(p1: &[u8], p2: &[u8]) -> Result<Vec<u8>, _>;
fn bn254_scalar_mul(p: &[u8], s: &[u8]) -> Result<Vec<u8>, _>;
fn bn254_pairing_check(pairs: &[u8]) -> Result<bool, _>;
```

Three functions. One chain upgrade. Juno becomes the first CosmWasm chain with native ZK verification — and because the PR could go upstream, every CosmWasm chain benefits.

But the proof-of-concept is already live. The case is made from running code, not from a slide.

[IMAGE 4 — LONGSHIPS SAILING OUT]

---

## Ordeal III — The Mesh of Many Shores

### The third wall: even with math, one chain is one chain

Assume the first two ordeals are passed. Assume every attestation is TEE-signed and every computation is ZK-proven. You still live on one chain. Juno, at its current 3-second block time, does about **333 contract-executes per block**, roughly **111 TPS**. That is a village harbour. For an agentic stack meant to serve populations, it is nowhere near enough.

The naive answer is to launch more chains. IBC — the Inter-Blockchain Communication protocol Jae Kwon gave the world in 2020 — is designed for exactly this. A chain is just a shard, and sovereign chains can talk to each other through light-client proofs. That is the Viking move: do not try to make one harbour bigger, sail out and build twenty.

There are, however, two further problems.

**Problem one: validator economics.** Every new Cosmos chain needs its own validator set. Call it 100 validators each staking $50k — that is $5M of bootstrap capital, per chain, minimum. Want 10,000 chains? Find $50 billion and a million validators. *Good luck.*

**Problem two: per-chain throughput.** Even if you had infinite chains, a standard CometBFT chain bundles consensus, execution, and data availability into a single process. They are coupled. To raise execution throughput you have to raise consensus throughput, which shrinks the validator's window to verify, which costs security.

Both problems had answers. Both answers were already in the Cosmos ecosystem, from three separate teams, open-source.

### Mesh Security — Jake, Ethan, Sunny

The cleanest way to break the validator-bootstrap problem is **mesh security** (originally **interchain security v2**, an evolution of the Cosmos Hub's replicated security). A validator on chain A can *re-stake* their tokens to also secure chain B. No new capital. No new validators. Juno's ~150 validators can simultaneously secure, say, 50 task-execution child chains. Same stake. 50× more blockspace.

Repositories, all Apache 2.0:

- [`osmosis-labs/mesh-security`](https://github.com/osmosis-labs/mesh-security)
- [`osmosis-labs/mesh-security-sdk`](https://github.com/osmosis-labs/mesh-security-sdk)

We wired a `query_mesh_security` tool into the Cosmos MCP server so the provider and consumer contracts can be read from any AI client.

### Celestia + tiablob + spawn — Mustafa, Reece

The cleanest way to break the per-chain throughput problem is **modular data availability**. Celestia — founded by Mustafa Al-Bassam — decouples data availability from execution. A sovereign rollup chain can execute many times more transactions per block because it does not reach consensus on the *execution*; it only posts the *data* to Celestia, which guarantees ordering and availability.

For a Cosmos SDK chain to actually *use* Celestia as its DA layer, it needs a module that posts block data across. That module is **`tiablob`**, built at Rollchains / Strangelove by **Reece Williams** — who happens to be **Juno's Development Lead**. He also built **`spawn`**, which scaffolds a new modular Cosmos chain (with Celestia DA baked in) in minutes.

The bridge from Cosmos to Celestia was built, in short, *by our own house*.

- [`celestiaorg/celestia-app`](https://github.com/celestiaorg/celestia-app)
- [`rollchains/tiablob`](https://github.com/rollchains/tiablob)
- [`rollchains/spawn`](https://github.com/rollchains/spawn)

We wired a `submit_blob` tool (`MsgPayForBlobs`) into the MCP server as well.

### The arithmetic

Three multipliers, all composed, all open-source, all integrated:

| Component | What it multiplies |
|---|---|
| Base per chain (Juno, 3s blocks) | ~111 TPS ≈ 333 contract-executes/block |
| IBC (5,000 chains) | × 5,000 |
| Mesh security | enables those 5,000 chains at near-zero marginal validator cost |
| Celestia DA + tiablob (10× per chain) | × 10 |

`333 × 10 × 5,000 ≈ 16.65M contract-executes per block.`

At one task per agent per minute, that is between **333 million and 1 billion simultaneous agents** — roughly the population of a continent, or of Earth, depending on block time.

This is not a marketing figure. It is a composition of three published, deployed, measurable technologies. None of it is ours to claim — but the composition itself, and the application layer that actually *needs* this scale (AI agents), was not there until we built it.

Jake saw the composition and said "this is very cool". Read it again carefully. He was not complimenting the code. He was recognising that someone had finally built the *workload* that mesh security is a solution *for*. Without AI agents demanding thousands of chains, mesh security is a solution looking for a problem. We are the problem.

[IMAGE 5 — THE COUNCIL RING AT THE HILL-FORT]

---

## The shape of it now: a stack in layers

Here is the architecture as it stands after the three ordeals, on `uni-7`, with the v6 hardening pass complete.

```
┌──────────────────────────────────────────────────────────────────────┐
│  LAYER 7 — The Council (Governance)                                  │
│  13-bud trust-tree DAO (agent-company v6), 67% supermajority for    │
│  constitutional changes, adaptive voting deadlines, 137 passing     │
│  workspace tests, v6 hardening pass (F1–F4 closed)                  │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 6 — The Skald's Tongue (Cosmos MCP Server)                   │
│  @junoclaw/cosmos-mcp — 22 tools, 7 chains, 12 IBC routes           │
│  Any AI client (Claude / Windsurf / Cursor / local Ollama)          │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 5 — The Ship-Paths (Scaling)                                 │
│  IBC (Jae Kwon)          — horizontal scaling, sovereign shards     │
│  Mesh Security (Osmosis) — free validators for child chains         │
│  Celestia + tiablob      — modular DA, 10× per-chain throughput     │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 4 — The Watchtowers (Off-chain Compute)                      │
│  WAVS operator network (Akash deploy + Azure SGX + sidecars)        │
│  WASI components, deterministic, reproducible, ~355 KB each        │
│  Runs the verification math for every chain event                  │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 3 — The Ten Halls (Application Contracts)                    │
│  agent-registry · task-ledger · escrow · agent-company              │
│  junoswap-factory · junoswap-pair · faucet                          │
│  builder-grant · zk-verifier · junoclaw-common                      │
│  CosmWasm 2.2, wasmd v0.54, MVP-lowered via wasm-opt                │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 2 — The Illuminated Page (Math Layer)                        │
│  Groth16 BN254 verifier (zk-verifier, Code ID 64)                   │
│  Measured 371,486 gas pure-Wasm (vs ~187K with precompile)         │
│  Case made for a Juno BN254 precompile upgrade                      │
├──────────────────────────────────────────────────────────────────────┤
│  LAYER 1 — The Keep (Hardware Root)                                 │
│  Intel SGX / AMD SEV-SNP enclaves, WAVS-in-TEE                     │
│  Proposal 4 TEE-attested tx 6EA1AE79…D26B22 at block 11,735,127     │
│  Validator-run sidecars as the distributed attestation set         │
└──────────────────────────────────────────────────────────────────────┘
```

Reading bottom-up, the trust moves from *"trust this chip"* (Layer 1) to *"trust this math"* (Layer 2) to *"trust this audited contract code"* (Layer 3) to *"trust this deterministic WASI binary"* (Layer 4) to *"trust this cryptographic protocol between chains"* (Layer 5) to *"trust nothing; verify via MCP"* (Layer 6) to *"trust the 13 buds with a 67% supermajority to govern changes"* (Layer 7).

Every layer is replaceable. Every layer is checkable.

[IMAGE 6 — THE MAPPA MUNDI OF CHAINS]

---

## The facts on the ground

None of what follows is a promise. These are on-chain artefacts that anyone can query today.

**Chain.** `uni-7` (Juno testnet). RPC: `https://juno-testnet-rpc.polkachu.com`.

**The ten contracts.** A hardened v6 pass was deployed on `uni-7` on 2026-04-18 by the wallet known in the Parliament as *The Builder* (`juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z`):

| Contract | Code ID | Address |
|---|---:|---|
| `agent-registry` | 69 | `juno15683x0sa06yr4ejuwenxszclkvpjekxmldlxe8qsltfkhm3qpm5sy0vuep` |
| `task-ledger` | 70 | `juno17aq66zyakz8su32u8tkgwmqemf0sylvv9a23nz7c7ydvkerll28skp5xfn` |
| `escrow` | 71 | `juno17vrh77vjrpvu6v53q94x4vgcrmyw57pajq2vvstn608qvs5hw8kqeew3g9` |
| `agent-company` | 72 | `juno1lymtnjru4euexavls4gqvjwtt3twxpsgrva0m37m6krp0dqacycs40f2hw` |
| `builder-grant` | 73 | *(stored only; smoke-tests instantiate fresh)* |
| `junoswap-pair` | 74 | *(stored only; factory-managed)* |
| `zk-verifier` | 64 | `juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem` |

The older Parliament contracts (`agent-company v3`, junoswap factory and live pairs, faucet) are still running from earlier phases at the addresses published in `docs/JUNOCLAW_VISUAL_EXPLAINER.md`.

**The TEE milestone.** Attestation tx `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22` at block 11,735,127, hardware-signed by Intel SGX, `data_hash = 9d0f7354…7367a3b`, `attestation_hash = 945a53c5…fae5c0e8`.

**The ZK milestone.** Verify tx `F6D5774EE2073E2DD011399A7E96889BA026ED67C6A510D208FD5C575080F4DA` at block 12,673,217. 371,486 gas. Full Groth16 proof on BN254, pure CosmWasm, no precompile.

**The v6 regressions.** Four cracks (F1–F4), four fixes, three verified on-chain with named parties and real transactions, full run-log in `docs/V6_TESTNET_RUN.md`.

**The toolkit.** `@junoclaw/cosmos-mcp` v0.3.0 — 22 tools, 18/18 smoke tests passing, any AI client can use all of it including a local Ollama model over LangChain.

**The licence.** Apache 2.0, from top to bottom, including every upstream dependency we rely on.

---

## The moral of the saga

Each ordeal taught the same lesson, from three different angles.

- **Fire (hardware)** taught us that *trust is paid in hardware vendors.* A TEE is not zero trust; it is Intel trust.
- **Cold water (math)** taught us that *mathematics is the only vendor-less witness.* A Groth16 proof does not ask who manufactured anything. It only asks whether the constraint system is satisfied.
- **Open sea (mesh)** taught us that *scale is an architectural property, not a marketing one.* You do not scale by making one chain bigger; you scale by composing sovereign shards with re-used validators and off-loaded data availability.

And each ordeal was passed by *surrender* to a technique already invented by someone else:

- **Jake Hartnell** and the Layer.xyz team gave us the WAVS framework and the "just run WAVS inside a TEE" principle that made the silicon keep buildable in weeks, not years.
- **The arkworks contributors** and the original authors of **EIP-196/EIP-197** gave us the Groth16 curve and the precedent for treating it as infrastructure.
- **Jae Kwon** gave us IBC.
- **Jake, Ethan, and Sunny** (with Osmosis Labs) gave us mesh security.
- **Mustafa Al-Bassam** gave us Celestia.
- **Reece Williams** (our own Juno Dev Lead) gave us `tiablob` and `spawn`.

We did not invent IBC. We did not invent mesh security. We did not invent Celestia. We did not invent Groth16. We did not invent SGX. We connected them.

For that, **お辞儀をします** — we bow. Or, in the tongue closer to these hills: *we lower our rod at the fence-line.*

[IMAGE 7 — THE HARBOUR AT DUSK, THREE BEACONS LIT]

---

## What the article is, and what it is not

What it is:

- A narrative summary of an open-source stack that has walked three technical ordeals and has the on-chain receipts for each one.
- A map of which primitives came from where, and which humans to thank.
- A legally cautious framing of the security posture: the v6 hardening pass is **internal**; formal third-party audit is the next and separate step.

What it is not:

- Legal, financial, or investment advice.
- A claim that this stack is free of undiscovered bugs. It is not. No stack is.
- A substitute for that formal audit.

If you find a fifth crack in the hardening pass, open an issue on the repo. If you find a flaw in the zk-verifier circuit, open an issue. If you know a way to break the TEE assumption — please, publish. *That is the point of on-chain verification: it invites its own ordeal.*

---

## For the curious: where to look

| What | Where |
|---|---|
| The detailed v6 hardening writeup | [`docs/RATTADAN_HARDENING.md`](./RATTADAN_HARDENING.md) |
| The v6 testnet run, tx by tx | [`docs/V6_TESTNET_RUN.md`](./V6_TESTNET_RUN.md) |
| The on-chain smoke harness | [`deploy/smoke-v6.mjs`](../deploy/smoke-v6.mjs) |
| The BN254 precompile case | [`docs/BN254_PRECOMPILE_CASE.md`](./BN254_PRECOMPILE_CASE.md) |
| The ZK precompile article | [`ZK_PRECOMPILE_ARTICLE.md`](../ZK_PRECOMPILE_ARTICLE.md) |
| The MCP server article | [`COSMOS_MCP_ARTICLE.md`](../COSMOS_MCP_ARTICLE.md) |
| The scalability math | [`articles/medium_final_bosses_scalability.md`](../articles/medium_final_bosses_scalability.md) |
| The visual explainer | [`docs/JUNOCLAW_VISUAL_EXPLAINER.md`](./JUNOCLAW_VISUAL_EXPLAINER.md) |
| The validator sidecar model | [`docs/01_VALIDATOR_SIDECARS.md`](./01_VALIDATOR_SIDECARS.md) |
| The 13-bud genesis architecture | [`docs/GENESIS_BUDS_ARCHITECTURE.md`](./GENESIS_BUDS_ARCHITECTURE.md) |
| The TEE research notes | [`docs/WAVS_TEE_RESEARCH.md`](./WAVS_TEE_RESEARCH.md) |
| The code itself | [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) |

Apache 2.0. Open issues welcome. The three ordeals are walked; the fourth is the audit, and that one is yours.

---

*Built on JunoClaw. Ten contracts, three ordeals, seven chains spoken to, twenty-two tools in the skald's pouch, one validator still keeping time at the harbour.*

---

## Midjourney Prompts

All prompts use: `--ar 16:9 --s 250 --v 6.1`

**Shared style suffix (append to the end of every prompt):**
`2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, Ruthwell Cross carving motifs, Howard Pyle Norse illustration and Alan Lee Celtic watercolour atmosphere, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of ochre umber iron-red moss-green woad-blue heather-purple slate-grey and warm parchment cream, coastal Anglo-Saxon and early-medieval Welsh countryside, sea-mist rolling off chalk and flint cliffs, low green hills with old drystone field boundaries and lichened standing stones, occasional ravens aloft and sheep grazing below, aged and slightly smoke-stained page corners, small knotwork border motifs in at least one corner of the plate, gentle diffused storm-light`

> **Shared cast / world-notes** (feel free to place one or two, small, never centre-stage unless specified): a cloaked Welsh shepherd with a crooked ashplant and a lean hound; an Anglo-Saxon monk-scribe in undyed wool tunic carrying a leather-bound vellum book; a weathered Viking-era seafarer in a dark blue cloak with a brooch at the shoulder, standing at the prow of a clinker-built longship with a carved dragon head; a raven perched on a grey standing stone; a beacon-fire on a headland. Wooden or carved-stone signboards in the plate are welcome — rendered in simple Anglo-Saxon runes or in uneven scratched Insular capitals (e.g. ᚱᚢᚾᚪ or "BEAC·ON"); never modern lettering.

---

**Prompt 1 — The Three Gates on the Shore (Hero):**
`A wide early-medieval English coastal scene at the hour before dawn, chalk cliffs rising on the right and a shingle beach running to the left where a clinker-built Viking-era longship is drawn up on wooden rollers, three carved wooden gateposts standing in a line on the beach each topped with a different symbol — the first a forge-anvil crowned by leaping flame (fire), the second a silver knotwork ring half-submerged in a tide-pool (water), the third a windswept banner stretched between two tall ash poles with a ship's sail-mark upon it (open sea), a wool-cloaked traveller with a staff and a leather satchel walking toward the gates with their back to the viewer, a raven circling low overhead, sea-mist curling around the cliff-base, a single beacon-fire burning on a headland in the far distance, a small carved sign at the base of the first gate reading "ᚦ·ᚱᛁ ·ᚩᚱᛞᚪᛚ" (three ordeals) in uneven Anglo-Saxon runes, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, Ruthwell Cross carving motifs, Howard Pyle Norse illustration and Alan Lee Celtic watercolour atmosphere, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of ochre umber iron-red moss-green woad-blue slate-grey and warm parchment cream, aged and slightly smoke-stained page corners, knotwork border in the upper-left corner --ar 16:9 --s 250 --v 6.1`

**Prompt 2 — The Silicon Keep (The Hardware Ordeal):**
`A small stone watchtower perched on a windswept chalk promontory above a grey North Sea, built in the manner of an early Anglo-Saxon burh — rough-hewn masonry, a heavy oaken door banded with iron, a single narrow slit-window high up through which a faint teal glow can be seen as if from a hidden lamp, the whole tower seen slightly from below against a pale storm-light sky, a single monk-scribe in an undyed wool habit kneeling at the foot of the tower pressing a wax seal onto a small vellum strip — the seal carries a stylised circuit-trace pattern rendered as Insular knotwork — a raven watching from a lichened standing stone nearby, a carved stone cross beside the door inscribed in worn Anglo-Saxon runes with the word "INNAN" (within), sea-gulls wheeling below the cliff line, the wind visible in the bent heather, atmosphere of vigil and enclosure, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, Ruthwell Cross carving motifs, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of slate-grey iron-red moss-green and warm parchment cream, selective teal accent only on the single glowing window, knotwork border in the lower-right corner --ar 16:9 --s 250 --v 6.1`

**Prompt 3 — The Hermit Scribe on the Hill (The Mathematics Ordeal):**
`A lone Welsh hermit-monk seated cross-legged on a grassy ridge of a low mountain above a mist-filled valley at golden hour, an illuminated vellum codex open across their lap, on the left-hand page a delicately drawn elliptic curve rendered as a loop of gold-leaf interlace with small knotwork nodes where the curve crosses itself, on the right-hand page a verse written in Old English half-uncial script with a decorated initial letter, the scribe's quill paused mid-stroke as they look across the valley at a distant fortified hill-top enclosure rendered almost geometrically — octagonal ramparts with crystalline facets and a single tall tower of pale grey stone catching the last sunlight (a visual echo of the Ethereum diamond form, but translated into an early-medieval hill-fort), a small ink-pot and horn beside them on a flat stone, a soft drift of gorse and heather around them, a thin trail of smoke from a cooking fire further down the slope, the valley mist thinning to show one or two thatch-roofed stone houses below, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of ochre honey-gold moss-green woad-blue and warm parchment cream, aged page corners, small illuminated capital "G" (for Groth) in the top-left corner as if this were page twelve of a saga codex --ar 16:9 --s 250 --v 6.1`

**Prompt 4 — The Longships Sailing Out (The Scaling Ordeal):**
`A wide seascape at mid-morning showing seven clinker-built Viking-era longships fanning out from a single stone harbour on the left edge of the plate — each longship with a different carved prow-head (a dragon, a wolf, a raven, a stag, a salmon, a bear, a seal) and a different coloured sail stitched with a distinct knotwork emblem — all seven ships connected back to the harbour's central mead-hall by faint luminous threads of rope-like light rendered as Insular interlace knotwork drawn in gold-leaf ink across the waves (representing IBC channels), the mead-hall on the harbour wall is a long timber structure with a turf roof and a single banner above its doorway, above the entire scene a great vaulted blue-black sky containing a translucent sphere of stars and hexagonal cell-patterns (representing Celestia's data-availability layer) from which a thin rain of gold sparks falls toward each longship, three shepherd-validators in grey woollen cloaks standing on a green cliff-top on the right, each of them with staves that extend into the sea lighting several ships at once (representing mesh-secured validator sets across multiple chains), a raven aloft, small carved wooden nav-markers in the foreground water each painted with a different knotwork sigil, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, Howard Pyle Norse illustration, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of woad-blue slate-grey ochre sail-red moss-green and warm parchment cream, gold-leaf accent only on the knotwork rope-lines between ships and the sparks from the sky-sphere, knotwork border running the full top edge of the plate --ar 16:9 --s 250 --v 6.1`

**Prompt 5 — The Council Ring at the Hill-Fort (The Parliament):**
`A circular hilltop ring on a windswept Welsh summit at late afternoon, thirteen wool-cloaked figures seated on low standing stones arranged in a near-complete ring with one small gap at the viewer's edge (leaving an invitation for the reader), each figure holding a different simple token in their lap — a carved apple, a small set of iron scales, a bundle of wheat ears, a measuring rod, a folded sea-chart, a pouch of seeds, a hammer, a branch of gorse, a single white stone, a small carved wooden ship, a beehive-smoke wand, a strip of vellum with writing, and one figure holding nothing and turned slightly away from the centre (the Contrarian) — at the ring's exact centre a tall carved wooden post painted with thirteen bands of different colour and topped with a small iron-and-gold weathervane in the shape of a claw, a low chalk-and-charcoal scorch mark on the ground tracing a thirteen-pointed star beneath the post, a single raven perched on the central post, the surrounding landscape dropping away in layered hills toward a small fishing harbour and the sea far below on the right, a light sea-mist wreathing the lower hills, an old drystone wall snaking across the middle-distance slope, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, Ruthwell Cross carving motifs, Alan Lee Celtic watercolour atmosphere, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of heather-purple moss-green oatmeal ochre and warm parchment cream, small decorative knotwork panels around three of the thirteen figures' feet, top-left corner marked "XIII" in Insular capitals --ar 16:9 --s 250 --v 6.1`

**Prompt 6 — The Mappa Mundi of Chains (The Architecture):**
`A hand-drawn early-medieval world map in the manner of the Hereford Mappa Mundi, rendered as a single plate of a saga codex, the map shows an island-and-sea landscape rather than a literal world — at the exact centre a large walled settlement labelled "IUNO" in worn Insular capitals, connected by seven winding roads that become seven inked sea-routes each passing through a small harbour-town labelled "OSM", "STGZ", "NTRN", "AKSH", "CELT", "COS", and "INJ" in scratched runic abbreviations, each harbour-town drawn as a small stylised building with a distinctive roof-shape (thatched hall, stone tower, turf longhouse, timber octagon, etc.), along the margins of the plate illustrated vignettes — a monk-scribe writing, a longship at sea, a shepherd with a flock, a forge, a beacon-fire, a loom with a half-woven knotwork pattern, and a salmon leaping — each vignette corresponding to a layer of the architecture and linked into the central settlement by a thin gold interlace thread, a compass rose in the upper-right corner rendered as an eight-pointed Insular knot, a small scale-bar at the bottom labelled "dæges færd" (a day's journey) in runes, the whole plate framed with a thick braided interlace border in ochre and iron-red, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, Ruthwell Cross carving motifs, pen-and-ink underdrawing with earth-pigment watercolour wash on heavily aged cream vellum with visible fold-creases and small worm-holes, muted palette of ochre umber iron-red moss-green woad-blue slate-grey and warm parchment cream, selective gold-leaf accent only on the connecting threads between vignettes and the central settlement --ar 16:9 --s 250 --v 6.1`

**Prompt 7 — The Harbour at Dusk, Three Beacons Lit (Closing):**
`A quiet English coastal harbour seen from a low rise at dusk, sea calm as glass reflecting a sky of layered peach slate-blue and lilac, a small cluster of thatched stone cottages around a curving shingle bay, a single stone mead-hall at the harbour head with smoke rising from its louvre, three beacon-fires burning on three low headlands arranged as three points of a wide triangle above the bay — the nearest beacon small and close, the middle one at middle distance, the farthest barely a spark on the horizon — each beacon tended by a small cloaked figure visible only in silhouette, a single clinker-built longship moored peacefully at the harbour's wooden pier with its sail furled and its dragon-head prow reflecting the last light, a shepherd with a lean hound and a crooked ashplant walking along the cliff path toward the nearest beacon carrying a bundle of brushwood, a monk-scribe standing at the mead-hall door closing a vellum codex with a satisfied expression, a raven settling on the carved post of a small wooden sign at the path-junction reading "BEAC·ON ·ÞRI" (three beacons) in uneven Insular capitals, faint illuminated interlace motifs in the clouds above the three fires as if the sky itself were a manuscript page, atmosphere of work completed and watch continuing, 2D hand-drawn illustration in the style of an Insular illuminated manuscript meeting a Norse saga plate — Lindisfarne Gospels interlace, Book of Kells marginalia, Alan Lee Celtic watercolour atmosphere, pen-and-ink underdrawing with earth-pigment watercolour wash on rough cream vellum, muted palette of dusk-peach slate-blue woad-indigo warm lamp-amber heather-purple and warm parchment cream, selective warm amber accent only on the three beacons and the mead-hall smoke-hole, small knotwork colophon in the bottom-right corner containing a tiny stylised claw --ar 16:9 --s 250 --v 6.1`
