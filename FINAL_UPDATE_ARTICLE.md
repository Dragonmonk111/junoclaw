# JunoClaw Ships — Everything Is Live Before the Governance Proposal

## From staking JUNO on December 30th, 2021 to deploying verifiable AI agents on Juno in March 2026.

---

**TL;DR** — JunoClaw is fully deployed on Juno testnet. Junoswap for Agents (v2) is live. 6 WAVS verification workflows run autonomously on Akash. The WASI component is published to a live registry. The governance proposal is written. Everything below is on-chain, verifiable, and open source.

---

### The full stack, shipped

Here's what's running right now, as you read this:

**On Juno testnet (uni-7):**
- `agent-company` v3 — DAO governance contract with CodeUpgrade supermajority (code ID 63)
- Junoswap for Agents (v2) factory — pair creation, fee management, DAO integration (code ID 61)
- Junoswap for Agents pair JUNOX/USDC — live XYK constant-product AMM (code ID 60)
- Junoswap for Agents pair JUNOX/STAKE — second trading pair (code ID 60)
- 5 governance proposals executed — from manual attestation to TEE hardware proofs
- 34 unit tests passing

**On Akash Network (decentralized compute):**
- WAVS operator — watches Juno for events, runs WASI verification inside containers
- WAVS aggregator — collects and serves verification results
- IPFS node — stores the 494KB WASI component binary
- Cost: US$7.85/month. 63.77 AKT funded. No AWS, no Azure dependency.

**On Azure (component registry):**
- `warg-server` running as a systemd service — auto-starts on boot
- `junoclaw:verifier` v0.1.0 published (sha256:b40d3fc...)
- Auto-publishes component after every reboot — no manual intervention
- Port 8090 open, reachable by the Akash operator

**The WASI component (494KB of Rust):**
- Compiled to `wasm32-wasip1` — runs anywhere WASI is supported
- Handles 6 verification workflows autonomously
- Proven inside Intel SGX enclave (Proposal 4, TX: `6EA1AE79...D26B22`)

---

### 6 workflows, zero humans

The WAVS operator watches Juno and reacts. No one tells it what to do. It reads on-chain events and independently computes verification results.

**1. Swap Verification**
Every Junoswap swap emits 12 attributes. The operator recomputes XYK math — offer amount, fee deduction, return calculation, constant-product invariant (k). If price impact exceeds 5%, it flags manipulation. Human swap: 1 block (~3 seconds). Agent verification: 3 blocks (~9 seconds).

**2. Sortition (Random Jury)**
When the DAO needs a random subset of members, the operator fetches drand beacon randomness and submits it on-chain. Fisher-Yates shuffle with SHA-256 sub-randomness. Deterministic. Verifiable. No one picks the jury.

**3. Outcome Verification**
Prediction markets and data tasks. The operator verifies resolution criteria against external sources. TEE hardware guarantees the code wasn't tampered with.

**4. Governance Watch**
Monitors proposals for anomalies — unusual voting patterns, quorum manipulation, rapid-fire submissions. Classifies risk and attests findings on-chain.

**5. Migration Watch**
Detects contract migration events. Verifies the new code_id against known-good hashes. Flags unauthorized migrations before damage spreads. This is how you catch rug pulls at the contract level.

**6. Swap Verify (JUNOX/USDC)**
Dedicated verification for the primary trading pair. Separate trigger, same math, independent attestation.

---

### Junoswap for Agents

The original Junoswap was abandoned. Liquidity dried up. The chain that pioneered CosmWasm had no functioning DEX.

JunoClaw ships Junoswap v2 — **Junoswap for Agents**. Two contracts. Clean rewrite from scratch. Apache 2.0. This is not affiliated with the original Junoswap team, and not a fork of the original code. It's a new AMM built specifically for agent-verified trading on Juno.

The **factory** manages pair creation. Anyone can create a trading pair. Assets are sorted deterministically — no duplicates. Default fee: 30 basis points. The factory stores a reference to the JunoClaw agent-company contract for governance integration.

The **pair** is a standalone XYK constant-product AMM:

```
fee_amount = offer_amount * fee_bps / 10000
offer_after_fee = offer_amount - fee_amount
return_amount = offer_after_fee * return_reserve / (offer_reserve + offer_after_fee)
```

Slippage protection. Empty pool guards. Volume tracking. LP share management. Swap simulation queries. Every swap emits a `wasm-swap` event that the WAVS operator watches.

438 lines for the pair contract. 209 for the factory. Small enough to audit in an afternoon. Every swap verified by an off-chain agent in hardware-attested compute. No other DEX in Cosmos does this.

> **Note:** Junoswap for Agents is currently deployed on Juno testnet (uni-7) only. These contracts have not been audited. Do not provide real liquidity until mainnet deployment and audit are complete. This is experimental infrastructure — use at your own risk.

---

### Genesis → 13 Buds

JunoClaw uses a "budding" governance model. Same philosophy as Juno's own fairdrop — distribute power, don't hoard it.

Right now, Genesis holds 100% weight. That's temporary. Genesis is urged to evolve JunoClaw and involve up to 13 buds. The timeline is with Genesis — this is a seed planted in Juno soil, not a corporate roadmap.

Once the 13 buds are active:
- Genesis loses voting power (retains 3/10000, symbolic)
- Normal proposals: 51% quorum (7 of 13)
- Code upgrades: 67% supermajority (9 of 13)
- No single actor can push changes unilaterally

The community watches the tree grow. Bad branches get pruned via `BreakChannel`.

---

### The governance proposal

A signaling proposal is going to juno-1. No code execution. No community pool funds. We're asking the Juno community to:

1. Recognize JunoClaw as a Juno ecosystem infrastructure reboot.
2. Support Junoswap revival through TEE-attested verification.
3. Endorse decentralized compute via Akash for Juno's verification layer.
4. Acknowledge the CodeUpgrade governance framework with 67% supermajority.
5. Support the future validator sidecar proposal for TEE-grade distributed attestation.

Passing signals that the Juno community recognizes JunoClaw to move to genesis address. Genesis deploys mainnet contracts. Genesis wires infrastructure. Genesis buds into 13. Then the DAO takes over.

Jake Hartnell said: "We should point the agent at projects like reviving Junoswap." That's exactly what this does.

---

### The journey

**Phase 1–3** (March 13–15): Built the WASI component, deployed four CosmWasm contracts, wrote the bridge daemon.

**Phase 4** (March 16): End-to-end on testnet — manual and autonomous attestations. Local operator watches chain, auto-detects proposals, computes hashes, submits.

**Phase 5** (March 17): TEE milestone. WASI component executed inside Intel SGX enclave on Azure DCsv3. Proposal 4 — first hardware-attested WAVS result in the Cosmos ecosystem.

**Phase 6** (March 17): Akash deployment. Three containers on decentralized compute. No single company can shut it down.

**Phase 7** (March 18): Chain Intelligence Module — 6 verification workflows. Azure warg registry with systemd persistence. Akash deployment updated. Governance proposal finalized.

Everything shipped in 5 days.

---

### Verify it yourself

Every claim above is on-chain:

| What | Where |
|------|-------|
| Agent-company contract | `juno1k8dxll...stj85k6` on uni-7 |
| TEE attestation TX | `6EA1AE79...D26B22` on uni-7 |
| Junoswap factory | `juno12v0t60...ghvq3wtkkh` on uni-7 |
| JUNOX/USDC pair | `juno1xn4mtv9...wacwqfr6e98` on uni-7 |
| JUNOX/STAKE pair | `juno156t270z...dt7f7s8ttg4s` on uni-7 |
| Akash operator | `http://provider.akash-palmito.org:31812` |
| Component registry | `http://145.132.96.212:8090` |
| Source code | [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) |

---

### What's next

The governance proposal goes live on juno-1. Then Genesis buds into 13. Then the DAO takes over.

If it passes, we keep building. If it doesn't, we keep building. The code is already deployed.

But a yes from the community means something. It means builders are shipping on Juno. It means the chain is alive. It means Juno is still where the technology is given to the people.

---

*Written by VairagyaNodes — staking Juno since December 30th, 2021.*

*All code is open source: [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw)*
