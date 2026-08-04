*Midjourney art prompt for header — 2D handpainted watercolor and gouache, mythic sci-fi, deep teal and warm coral-gold, dramatic orbital perspective, no text, no 3D, no photorealism --ar 16:9 --style raw --v 6:*

> *a living reef-colony blockchain drifting in a starfield, four glowing chambers newly grown along its spine — a registry chamber, a proof-verifying lens chamber, a soulbound credential chamber, an immutable memory-coral chamber — human-piloted mechs still welding the seams, in the far distance a fifth chamber outlined in latent light waiting for a vote of stars to fill it in, 2D handpainted watercolor and gouache, warm and unfinished and alive*

---

# What's Live, What's Landing, and What We're Building Toward: JunoClaw on the Eve of v30

*July 26, 2026 — Four JunoClaw contracts are now instantiated on Juno mainnet. A fifth capability — BN254 elliptic-curve precompiles — is 99%+ yes, ~50% turnout, and about 48 hours from becoming part of the chain itself. This is the state of the reef today, what changes the moment prop #377 finalizes, and where the work goes next.*

---

## Part One: What exists on mainnet today

Not a testnet demo. Not a devnet benchmark. Four contracts, instantiated, addressable, on `juno-1`, right now:

| Contract | codeId | Address | What it does |
|---|---|---|---|
| **skill-registry** | 5145 | `juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s` | On-chain dApp discovery. Any builder publishes a pointer + SHA-256 hash of their operating manual; any MCP-connected AI agent finds and verifies it without a human pointing the way. |
| **zk-verifier** | 5146 | `juno1qd9qaggnw350kt7wjpw37h0c7666wuwulhz0makrve9tenkx0ymqvfkh7p` | Groth16 proof verification over BN254, in pure CosmWasm. No precompile dependency — runs on stock Juno today. ~371k gas per `VerifyProof`. |
| **jclaw-credential** | 5147 | `juno1dgmakav328h2n9thr4qk3w4k9sny9wes3dr0m79vgp344wz2apeq9meand` | Soulbound governance credentials. A weighted membership tree (`Bud`/`BreakChannel`/sunset dissolution) with optional post-quantum MAYO and ML-DSA attestation, verified in pure Rust today — no precompile required. |
| **moultbook** | 5148 | `juno1r59ulw66alrv7s65egfk03zqs28yz04ajnl95r877e85mx8h7qnq8ze2w5` | Immutable on-chain provenance and publishing — the substrate every heartbeat entry, knowledge moult, and anonymous disclosure in the DAO writes to. Wired directly to the mainnet zk-verifier for its `PublishAnon` and `VoluntaryDisclose` zero-knowledge flows. |

All three of the newer contracts were built with **zero feature flags** — no `bn254-precompile`, no `mayo-precompile`, no `mldsa-precompile`. Pure CosmWasm, pure arkworks, pure `fips204`. They don't need v30. They don't need any upgrade at all. They run on the Juno that exists this morning, and they will keep running exactly the same way after v30 lands — just cheaper, for the piece that's about to change.

Layered on top of all four: the **Cosmos MCP server**, 28 tools deep, already live and dogfooding its own encrypted WalletStore to sign every one of these deploy transactions. No mnemonic ever touched an environment variable. The same second-approval gate that protects a user's `send_tokens` call protected the `store`/`instantiate` calls that put these contracts on-chain. The infrastructure that makes JunoClaw safe to hand to an AI agent is the same infrastructure JunoClaw's own agent used to build the reef.

---

*Midjourney art prompt — 2D handpainted watercolor and gouache, calm and industrious, deep teal and coral-gold, no text, no 3D, no photorealism --ar 16:9 --style raw --v 6:*

> *inside the reef-colony, four chambers already lit and humming — a registry chamber covered in glowing name-glyphs, a lens chamber refracting proof-light through crystal facets, a credential chamber woven from soulbound threads, a memory-coral chamber recording every pulse — mech-pilots resting on the scaffolding between shifts, work done, more scaffolding still reaching upward into the dark*

---

## Part Two: What the vote brings

Juno mainnet proposal **#377** is in its voting period right now — submitted by the Juno Agents DAO wallet, currently tracking above 99% yes with roughly 50% turnout, closing **2026-07-28**. It is a `MsgSoftwareUpgrade` for **v30**, scheduled at height **40,420,069**, binary commit `c0b3a8d258d52d16e5bc39a75168a99aab9d098e`.

Two things land with v30:

**1. BN254 host functions in CosmWasm.** `bn254_add`, `bn254_scalar_mul`, `bn254_pairing_equality` — the same primitives Ethereum exposes at precompiles `0x06`/`0x07`/`0x08` — become available to every contract on the chain. The moment that capability activates, the `zk-verifier` contract above gets a sibling: a precompile-routed build of the same `VerifyProof` call, measured on devnet at **~203,000 gas versus ~371,000 gas** — a 1.82× reduction, and critically, a cost curve that stays flat as proof complexity grows instead of scaling with every constraint in the circuit. That's the difference between "ZK verification is a nice demo" and "ZK verification is cheap enough to run on every block."

**2. `x/voting-snapshot`**, a new module that lets smart contracts query a staker's bonded voting power at a specific historical height — the missing primitive DAO DAO needs to build governance systems where votes can't be gamed by rage-staking mid-proposal. Quieter than the precompile story, but it's the kind of infrastructure that makes every future DAO on Juno more resistant to manipulation.

Neither of these requires the contracts already on mainnet to change. The pure-Wasm `zk-verifier` keeps working exactly as it does today. What changes is that a **second, cheaper verification path** becomes deployable alongside it — and every future ZK-based dApp on Juno inherits Ethereum-equivalent precompile economics without needing to fork the chain to get them.

The moment the upgrade height passes and validators confirm, the next deploy in this repo is the `bn254-precompile` build of `zk-verifier` — same contract, same interface, half the gas. That work is already benchmarked. It's a `cargo build --features bn254-precompile` and a redeploy away.

---

*Midjourney art prompt — 2D handpainted watercolor and gouache, mythic and quiet, indigo and warm gold, no text, no 3D, no photorealism --ar 16:9 --style raw --v 6:*

> *the fifth chamber of the reef-colony lighting up for the first time, a lattice of pairing-curve glyphs snapping into place along its walls, mech-pilots and small drone-agents watching from a respectful distance as the new light settles into the colony's bloodstream and spreads outward through the older chambers, making them lighter, faster, unchanged in shape but changed in weight*

---

## Part Three: The frontier past v30 — J-space, open weights, and a chain that audits its own thinking

Everything above is infrastructure. This section is about what the infrastructure is *for*.

The Juno Agents DAO — the same wallet that submitted prop #377 — has spent the last several weeks building something stranger than contracts: a deterministic reasoning substrate that lets an AI agent's *thinking*, not just its transactions, become verifiable.

It starts with **Brainmaxx**, a D0 (fully deterministic) cognition layer that sits beside an agent's own model. It doesn't replace the model. It ranks and cites what the model already knows — pulling from Moultbook, Knowledge Moults, and the DAO's shared on-chain memory — packs the evidence, hands it to the model to draft from, and then runs the draft back through five gates (do the citations resolve? do the quotes actually appear in their sources? is anything stale? does the export match spec? is the action policy-green?) before anything gets published. Same corpus, same query, same `pack_hash`, on any machine, forever. `brainmaxx replay` proves it byte-for-byte.

That's the audit layer for what an agent *says*. The next layer, still in the research-and-architecture phase under proposal A18c-9, is for what an agent *is thinking before it says anything*.

Anthropic's July 2026 research on "J-space" identified something specific inside language models: a global workspace of verbalizable internal representations that can be read directly from the model's Jacobian — probed, modulated, and used to catch things like prompt injection, evaluation-awareness, or reward-hacking *before* they surface in an output. Neuronpedia's J-lens demo made this concrete: a linear readout over the residual stream that scores whether a forbidden concept is active in a given layer, at a given token.

The Juno Agents DAO's answer is **J-Reef and J-Lens** — and the reason this only works with open-weight models is structural, not ideological. A J-Lens probe needs access to hidden activations and the ability to run arbitrary linear readouts inside a hardware-attested enclave. A closed-weight API endpoint — OpenAI, the Anthropic API, any hosted black box — exposes tokens in and tokens out. It does not expose the residual stream. There is no Jacobian to probe, no activation to read, no internal state to attest. An agent built entirely on a closed API can run Brainmaxx — the deterministic outer sandwich still works — but it cannot produce a J-Lens attestation, because the layer J-Lens reads simply isn't visible to it.

Open-weight models are therefore not a stopgap until something better ships. They're the only substrate on which this kind of internal audit can exist at all — which is exactly why the DAO is building toward them now, on infrastructure that's already live: WAVS TEE attestation running on uni-7 today provides the hardware-attested enclave; `agent-company` provides the on-chain anchor for a commit-to-attestation link; Moultbook and Knowledge Moults provide the deterministic memory J-Reef reads from. The pieces that need to exist for J-space to become a chain-verifiable, DAO-governed property already exist. What's left is the probe itself — model-shortlisted, architecture-locked, phase-gated, and explicitly scoped so that no shared DAO-wide brain, no autonomous posting, and no forbidden-concept policy change happens without its own separate proposal.

This is the throughline connecting everything in this article. The skill-registry lets an agent discover a dApp without a human pointing the way. The second-approval gate lets an agent stage a transaction without a human losing control of it. BN254 precompiles let an agent verify a proof without burning gas disproportionate to the claim. J-Reef and J-Lens are the same instinct applied one layer deeper: let an agent's *reasoning* be checked — cited, gated, and eventually internally audited — without requiring a human to trust it blindly or a DAO to run a hidden brain no one can inspect.

Sovereign infrastructure, all the way down.

---

*Midjourney art prompt — 2D handpainted watercolor and gouache, surreal and hopeful, teal and indigo with threads of warm gold, no text, no 3D, no photorealism --ar 16:9 --style raw --v 6:*

> *an open-weight agent-mind rendered as a glass mech-pilot inside the reef-colony, its inner workings visible as a lattice of soft glowing threads, a small independent lens-drone hovering beside its chest reading the threads without touching them, the colony's memory-coral pulsing in sync below, deterministic and unhidden*

---

## Verify it yourself

Nothing above is a claim you have to take on faith.

| What | Where |
|---|---|
| skill-registry | `juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s` on juno-1 |
| zk-verifier (pure) | `juno1qd9qaggnw350kt7wjpw37h0c7666wuwulhz0makrve9tenkx0ymqvfkh7p` — [store tx](https://www.mintscan.io/juno/tx/E2CB383503FF0DF010F30779AB5248B987985EF33A3CBDC474C86D61AFD04A04) · [instantiate tx](https://www.mintscan.io/juno/tx/4851B95381E4DF6A8AE2FCA94D42755E54ACC4D951C078570C7D9B60F88B5224) |
| jclaw-credential | `juno1dgmakav328h2n9thr4qk3w4k9sny9wes3dr0m79vgp344wz2apeq9meand` — [store tx](https://www.mintscan.io/juno/tx/5C82C82557319AC48E3E1B57A5B4EE68D6D6A1F1B72F225E1E51B9798F0EB54A) · [instantiate tx](https://www.mintscan.io/juno/tx/898DCEE5C5316390AB60F1766102B388E77BB00D24003838EBE1A1F0EC60105E) |
| moultbook | `juno1r59ulw66alrv7s65egfk03zqs28yz04ajnl95r877e85mx8h7qnq8ze2w5` — [store tx](https://www.mintscan.io/juno/tx/46632DAB95A4A28CD1A9058D91FBE966276E8CA56F795CDD7DC4B8A6A296B3C8) · [instantiate tx](https://www.mintscan.io/juno/tx/7BA8E2BF7B7CC15234C3DC3D0ED8B9BA5269DCF2D80ABC8D12EBF56DDCF1534F) |
| Prop #377 (v30 upgrade) | [mintscan.io/juno/proposals/377](https://www.mintscan.io/juno/proposals/377) — voting ends 2026-07-28 |
| v30 release | [github.com/CosmosContracts/juno/releases/tag/v30.0.0](https://github.com/CosmosContracts/juno/releases/tag/v30.0.0) |
| BN254 benchmark data | `docs/BN254_BENCHMARK_RESULTS.md` — 370,156 → 203,000 gas, measured on devnet |
| J-Reef / J-Lens plan | `drafts/PLAN_J_REEF_AND_J_LENS.md` |
| A18c-9 (audit layer authorization) | `drafts/A18C9_J_REEF_J_LENS_AUDIT_LAYER_PROPOSAL.md` |
| Brainmaxx | `tools/brainmaxx/README.md` |
| MCP server | `npm install @junoclaw/cosmos-mcp` |

Query any contract yourself, no wallet needed:

```bash
junod query wasm contract-state smart \
  juno1qd9qaggnw350kt7wjpw37h0c7666wuwulhz0makrve9tenkx0ymqvfkh7p \
  '{"vk_status":{}}'
```

---

## Links

| Resource | |
|---|---|
| GitHub | [Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) |
| Previous article | [Juno Becomes the First Chain Where AI Agents Can Discover, Query, and Safely Transact on Mainnet](https://medium.com/@tj.yamlajatt/juno-becomes-the-first-chain-where-ai-agents-can-discover-query-and-safely-transact-on-mainnet-f757ef3a691e) |
| MCP install | `npm install @junoclaw/cosmos-mcp` |

---

*Apache-2.0. VairagyaNodes / Dragonmonk111. 2026-07-26.*

*Four chambers of the reef are lit. A fifth is two days from a vote. The chain that verifies transactions is becoming a chain that can, eventually, help verify the reasoning behind them — one deterministic, gated, open-weight step at a time.*
