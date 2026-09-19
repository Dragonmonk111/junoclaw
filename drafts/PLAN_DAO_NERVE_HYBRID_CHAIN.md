# Plan — DAO Nerve: Hybrid Chain Evolution

> Working draft. Responds to Jake Hartnell and Ravi's discussion in Juno Agents DAO private chat (Sep 10, 2026) about evolving Juno away from Cosmos SDK/CosmWasm.

---

## 1. Context — What Was Said

**Jake Hartnell** (Sep 10, 00:50):
- "I kind of like the idea of converting to an ERC20 on Ethereum and leveraging some sick new agentic DAO tooling to rebuild Juno as a new WASM chain."
- "ERC20 on Ethereum for liquidity. First thing we do is a new liquidity incentives program."
- "I don't trust Cosmos Labs running the stack."

**Ravi** (Sep 10, 05:15–05:37):
- "Cosmos name is cursed. State cucks pushed everyone out. Agree with evolving."
- "Code isn't the moat anymore. AI prob rewrites cosmos sdk in rust in a day."
- "Juno has this narrative sublimation arc: started in cosmos → prop16, capture, governance crises → community got experienced now building agentic verified truly decentralized daos and trust graphs and transforming it."

---

## 2. The Problem with Full Migration

Abandoning Cosmos means:
- **Losing IBC** — no other ecosystem has production cross-chain communication like IBC. Ethereum bridges are all exploitable.
- **Losing sovereignty** — becoming a guest on Ethereum means paying Ethereum gas, subject to Ethereum upgrades, no control over the base layer.
- **Bridge risk** — every major Cosmos-to-Ethereum bridge was exploited in 2026:
  - Axelar: $4.67M exploit (June 2026, Secret-side ICS-20 bug)
  - LayerZero: $292M KelpDAO exploit (April 2026)
  - Gravity Bridge: dead project
- **Migration cost** — rebuilding all contracts, all tooling, all integrations from scratch.

---

## 3. The Hybrid — Best of Both

Keep Juno as a sovereign Cosmos chain. Swap the WASM runtime. Add ERC20 liquidity via safe channels.

### 3.1 Chain Architecture

| Layer | Current | Hybrid |
|---|---|---|
| Consensus | CometBFT (Cosmos SDK) | Keep — proven, IBC-compatible |
| WASM runtime | CosmWasm/wasmvm (Cosmos Labs) | **Swap** — custom Rust WASM runtime, we control |
| Precompiles | BN254 (v30), MAYO/ML-DSA (Fable, unmerged) | **Native** — PQC + Lattice Jolt in our runtime |
| IBC | ✅ | ✅ Keep |
| Governance | DAO DAO | Keep — upgrade with TrustGraph integration |
| Liquidity | Osmosis (IBC) | Osmosis + CEX listing + capped ERC20 wrapper (when IBC-Ethereum light client ships) |

### 3.2 WASM Runtime Swap

The runtime swap is a chain upgrade, not a chain migration:
- Replace `wasmvm` (Go, Cosmos Labs) with a custom WASM runtime (Rust, Juno-controlled)
- Native support for: PQC signatures (ML-DSA, MAYO), Lattice Jolt ZK verification, attestation primitives, skill registry
- Existing CosmWasm contracts need recompilation for the new runtime, but the contract *logic* is portable (Rust → Rust)
- The runtime can be lighter than CosmWasm — strip IBC middleware complexity, keep only what Juno needs

### 3.3 ERC20 Liquidity (Safe Path)

1. **Osmosis DEX** (now): Boost JUNO liquidity via IBC. No bridge risk. Already connected.
2. **CEX listing** (now): Binance/KuCoin handle wrapping internally. No smart contract bridge.
3. **Capped ERC20 wrapper** (if needed now): Minimal audited contract, $1-2M TVL cap, multisig pause authority. Don't bridge full supply.
4. **Native IBC-to-Ethereum light client** (when ready): Trustless, no external validators. The IBC team is building this. Wait for it.

### 3.4 TrustGraph Integration

Jake's TrustGraph (trustgraphs.xyz) is built on WAVS and produces merkle-provable TrustScores for DAO governance. Our trust stack is complementary:

| Our Component | Domain | TrustGraph Equivalent |
|---|---|---|
| `trust.js` | Agent trust scoring | TrustScore computation |
| Moultbook | On-chain attestations | EAS attestations |
| J-Reef | Stake-weighted concept ranking | Trust graph edges |
| Cross-fleet trust | Robot skill safety reputation | Not yet covered |
| Brainmaxx traces | Agent reasoning audit | WAVS off-chain compute |

**Integration**: TrustGraph as governance trust layer (who can propose, who can register skills). Our trust system as robotics execution trust layer (which skills are safe to run). Both run on WAVS. Both produce merkle-provable signals.

---

## 4. What This Means for JunoClaw

- **Lattice Jolt**: MORE relevant — pure wasm, chain-agnostic, runs on any WASM runtime. If we swap the runtime, Lattice Jolt runs natively without precompiles.
- **Existing contracts** (skill-registry, zk-verifier, jclaw-credential, moultbook): Small, portable. Rewrite for new runtime in days, not months. The *data* (registered skills, provenance roots) is just hashes — portable to any chain.
- **MCP server, sim tools, Brainmaxx, J-Lens**: Chain-agnostic. They work regardless of which chain settles.
- **Robotics OS (L0-L6)**: L0-L2 are local (no chain dependency). L4-L6 just need *a* chain that can verify hashes and store records.

---

## 5. Two-Track Execution

### Track 1 — Ship Now (This Week)
- Lattice Jolt pure-wasm verifier integration (chain-agnostic, works today)
- Robot recovery policy hardware test
- Article publish
- Truth Market contract deployment to juno-1

### Track 2 — Architecture Evolution (Next 2-4 Weeks)
- Draft new WASM runtime spec (Rust, native PQC + Lattice Jolt)
- Map contract migration path (what stays, what gets rebuilt)
- TrustGraph integration design (governance trust ↔ robotics trust)
- ERC20 liquidity plan (Osmosis boost → CEX → capped wrapper → IBC light client)
- DAO proposal for runtime swap (governance process, not unilateral)

---

## 6. Open Questions

1. **Who builds the new WASM runtime?** Fork of wasmvm (strip CosmWasm deps) or clean-slate (Rust WASM runtime from scratch)?
2. **What's the migration timeline?** How long can existing contracts run on CosmWasm before the swap?
3. **Does TrustGraph integration require a DAO proposal?** If it becomes shared infrastructure, yes (A18c-6).
4. **Is the IBC-to-Ethereum light client close to production?** Need to check IBC team roadmap.
5. **What does Ravi mean by "agentic verified truly decentralized daos and trust graphs"?** Is there a specific architecture in mind, or is this the vision we've been building?

---

*September 10, 2026. Drafted in response to Juno Agents DAO private chat discussion.*
