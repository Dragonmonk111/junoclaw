# JunoClaw Quickstart

**GitHub:** https://github.com/Dragonmonk111/junoclaw  
**Proposal #373:** https://daodao.zone/dao/juno/proposals/373  
**Medium:** https://medium.com/@tj.yamlajatt

---

## What Is JunoClaw, In One Paragraph

JunoClaw is a DAO where AI agents execute on-chain actions — swaps, governance votes, contract upgrades — and every action produces a cryptographic proof that it ran correctly. The proof is stored on-chain permanently. The DAO governs the agents. The agents can't lie about what they did.

---

## Why Junoswap Here Is Different: The Agentic Swap

Most DEXes are built for humans: you open a UI, approve a transaction, done. JunoClaw's Junoswap v2 is built for **agents**.

**Traditional swap flow:**
```
Human → UI → Wallet approval → Smart contract
```

**Agentic swap flow:**
```
DAO policy → Agent task → WAVS operator (inside TEE) → Verify math → Post attestation → Junoswap pair contract
```

The difference: no human approves each individual trade. The **DAO sets policy** ("rebalance treasury if JUNO drops 10%", "execute only if price impact < 2%"), the **agent executes autonomously**, and the **WAVS operator independently recomputes the constant-product math** and posts a hardware-attested receipt on-chain.

If the agent cheated — wrong price, wrong amount, tampered logic — the attestation fails. The mismatch is permanent and publicly verifiable.

### What the agent can be instructed to do (via DAO governance vote):

| Task | What happens |
|------|-------------|
| Treasury rebalance | Agent swaps DAO funds at target ratio, WAVS verifies invariant |
| Liquidity provision | Agent adds LP to a pair on behalf of the DAO treasury |
| Price-triggered swap | Agent watches oracle, executes when condition is met |
| Anti-manipulation guard | WAVS flags if price impact > 5%, agent holds the trade |
| Cross-chain arbitrage (roadmap) | Agent routes through IBC, WAVS verifies each hop |

The DAO never has to trust that the agent did the right thing. It can read the attestation log.

---

## The Agentic DAO Loop

```
         ┌─────────────────────────────────┐
         │         DAO (13 buds)           │
         │  votes on policy proposals      │
         └────────────┬────────────────────┘
                      │ ExecuteProposal
                      ▼
         ┌─────────────────────────────────┐
         │       agent-company contract    │
         │  task-ledger queues the job     │
         └────────────┬────────────────────┘
                      │ WAVS picks up task
                      ▼
         ┌─────────────────────────────────┐
         │    WAVS operator (TEE enclave)  │
         │  runs 494KB WASI component      │
         │  verifies swap math             │
         │  produces attestation_hash      │
         └────────────┬────────────────────┘
                      │ submit attestation
                      ▼
         ┌─────────────────────────────────┐
         │  Junoswap pair contract         │
         │  executes swap                  │
         │  stores proof on-chain forever  │
         └─────────────────────────────────┘
```

Any bud holder (governance member) can query the attestation log at any time and see exactly what the agent did, when, and whether the math checked out.

---

## What You Can Do Today (Testnet — uni-7)

The full stack is live on Juno testnet. No mainnet deployment yet (pending Proposal #373).

### Chain config
```
Chain ID:  uni-7
RPC:       https://juno-testnet-rpc.polkachu.com
REST:      https://juno-testnet-api.polkachu.com
Denom:     ujunox
```

### Contract addresses (uni-7)
```
agent-company v3:   juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6
junoswap-factory:   juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh
junoswap-pair       
  JUNOX/USDC:       juno1xn4mtv9cfc7q3zphvstkhqgn4g864pppvq64zvdnmcsen3jwacwqfr6e98
  JUNOX/STAKE:      juno156t270zr84xskkj6k6yq6w4pj8xu646kfjsngscpjdhhmmdt7f7s8ttg4s
```

### Query the DAO config
```bash
curl -s "https://juno-testnet-api.polkachu.com/cosmwasm/wasm/v1/contract/juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6/smart/$(echo '{"config":{}}' | base64)" | jq
```

### Query Junoswap pair info
```bash
curl -s "https://juno-testnet-api.polkachu.com/cosmwasm/wasm/v1/contract/juno1xn4mtv9cfc7q3zphvstkhqgn4g864pppvq64zvdnmcsen3jwacwqfr6e98/smart/$(echo '{"pair":{}}' | base64)" | jq
```

### Query pool reserves
```bash
curl -s "https://juno-testnet-api.polkachu.com/cosmwasm/wasm/v1/contract/juno1xn4mtv9cfc7q3zphvstkhqgn4g864pppvq64zvdnmcsen3jwacwqfr6e98/smart/$(echo '{"pool":{}}' | base64)" | jq
```

---

## Run the Verification Component Yourself

The WASI component that runs inside the TEE is open source and can be run locally without SGX for testing:

```bash
git clone https://github.com/Dragonmonk111/junoclaw
cd junoclaw

# Build the WASI component
cd wavs/components/agent-trigger
cargo build --target wasm32-wasip1 --release

# Run via wavs-cli (no TEE, local verify)
wavs-cli exec \
  --component target/wasm32-wasip1/release/agent_trigger.wasm \
  --input '{"task_id":1,"action":"verify_swap","pair":"JUNOX/USDC"}'
```

To run with hardware attestation (Intel SGX required):
```bash
wavs-cli exec \
  --component agent_trigger.wasm \
  --input @input.json \
  --device /dev/sgx_enclave \
  --device /dev/sgx_provision
```

The TEE attestation proof (`attestation_hash`) is what gets submitted on-chain. See `tools/bud-seal/` for the secrets handoff tooling.

---

## What the Governance Looks Like From a Member's Perspective

Once the 13 buds are distributed, a sitting member can:

1. **Propose a swap policy** — submit a `TaskRequest` proposal via DAODAO UI or CosmJS
2. **Vote** — 7-of-13 quorum to pass most proposals, 9-of-13 for contract upgrades
3. **Watch it execute** — WAVS picks up the task, runs the WASI component, posts attestation
4. **Audit the receipt** — query `task_ledger` for the attestation hash, verify on-chain
5. **BreakChannel** if something goes wrong — prune a bad branch, re-assign the seat

No multisig ceremony per action. No trust-the-founder. Policy in, verified execution out.

---

## Repo Structure

```
contracts/
  agent-company/     # DAO core + task queue (34 tests)
  agent-registry/    # Agent registration + permissions (13 tests)
  task-ledger/       # Task lifecycle + attestation storage (12 tests)
  escrow/            # Fund custody for agent tasks (14 tests)
  junoswap-factory/  # DEX factory, deploys pair contracts (6 tests)
  junoswap-pair/     # XYK AMM, the agentic swap engine (7 tests)
  junoclaw-common/   # Shared types + helpers

wavs/
  components/        # WASI verification component (494KB compiled)
  bridge/            # CosmJS deploy + governance scripts
  operator/          # WAVS operator config

tools/
  bud-seal/          # Encrypted secrets handoff (X25519 + ChaCha20-Poly1305)

docs/
  GOV_PROP_COPYPASTE.md    # Full Proposal #373 text
  DIMI_HANDOFF_PLAN.md     # 10-step genesis handoff guide
  LEGAL_CAVEATS.md         # Risk disclosures (Apache 2.0)
```

---

## Next Steps / Roadmap

| Phase | Status |
|-------|--------|
| Testnet full stack live | ✅ Done |
| TEE attestation proven (Azure SGX) | ✅ Done |
| Governance Proposal #373 submitted | ✅ Voting until March 24, 2026 |
| Mainnet deployment | Pending proposal pass |
| Genesis bud distribution (13 seats) | Pending mainnet deploy |
| Validator sidecar attestation nodes | Roadmap |
| NOIS/drand randomness for sortition | Roadmap |
| Neutron DeFi protocol forks | Roadmap |

---

## Scaling

### Where each layer sits today

| Layer | Component | Current capacity |
|-------|-----------|-----------------|
| Governance | agent-company (13 buds, recursive tree) | Scales with Juno block throughput. Human consensus is the bottleneck, not tech |
| DEX | Junoswap v2 pairs (XYK AMM) | ~5 swaps/sec/pair (~300K gas each, ~10M gas/block). Multiple pairs run in parallel |
| Verification | WAVS operator (494KB WASI component) | Single operator: ~100+ verifications/sec. Scales linearly with more operators |
| Compute | Akash deployment | $8.76/month handles testnet. ~$50-100/month for 1000s of swaps/day |

### At what point does it break?

| Volume | What happens | Fix |
|--------|-------------|-----|
| < 100 swaps/day | No issues — current setup handles it | — |
| 1K swaps/day | Single WAVS operator under moderate load | Add 2-3 validator sidecar operators |
| 10K swaps/day | Attestation TX volume grows on Juno | Batch attestations (1 TX per N verifications) |
| 100K swaps/day | Juno block space contention | Juno throughput upgrades or L2 |
| 1M+ swaps/day | Beyond current Juno capacity | Different problem entirely |

### The honest answer

For a revived Juno DEX, **the bottleneck is liquidity, not compute**. If JunoClaw gets 100–1,000 swaps per day — which would already be a massive success for a chain with near-zero DeFi activity today — the architecture handles it trivially on one $9/month Akash deployment.

The system scales comfortably into the tens of thousands before you need to add operators. By that point, you'd have the community and resources to do it.

---

*Apache 2.0. Built in the open. Questions: open an issue on GitHub or reach out on the Juno validator channels.*
