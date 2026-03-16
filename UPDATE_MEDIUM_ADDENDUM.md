# Medium Article — Addendum (March 16, 2026)

> **How to use**: Append this as an update section at the bottom of the existing Medium article, or publish as a short standalone follow-up post linking to the original.

---

## Update: WAVS Attestation Is Live on Testnet

The original article said WAVS integration was next. It's done.

A DAO proposal now triggers a verifiable compute pipeline end-to-end — from vote to execution to hardware-attested proof, all on-chain.

Here's what was deployed on uni-7 this week:

**agent-company v2** (code 59) — the same governance contract from the original announcement, upgraded with three new capabilities:

- **Trigger events** — when the DAO executes certain proposals, the contract emits typed events (`wasm-outcome_create`, `wasm-wavs_push`, `wasm-sortition_request`) that the WAVS operator network can index
- **Attestation submission** — a bridge daemon relays the TEE-attested result back to the contract, storing it immutably alongside the proposal that requested it
- **On-chain verification** — anyone can query any attestation by proposal ID. The data hash, attestation hash, submitter, and block height are all public

The original contract (code 57) remains live and functional — governance, voting, payment distribution, sortition all work exactly as described. The v2 instance adds the verification layer on top.

**New contract address:**
`juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6`

**First attestation on-chain:**
- Proposal 2: *"Will JunoClaw WAVS attestation integration pass E2E test?"*
- Attestation TX: `162DA466622425B1522D8E32066A899C57A2B75819124213FC2221BAA2A46795`
- Result: Verified. Queryable. Immutable.

The pipeline is: **DAO votes → contract emits event → WAVS operator processes in TEE → bridge relays proof → contract stores attestation → anyone queries it.**

No intermediary holds the keys. No single party can falsify the result. The hardware attests. The chain records.

70 tests passing across all four contracts. The bridge daemon, CLI tools, and WASI component are all open source.

Thank you to Jake Hartnell — co-founder of Juno and WAVS/Layer.xyz — for the early encouragement and for confirming that this architecture is exactly how WAVS is meant to be used. The conversation that started with "very cool" led to this milestone being shipped weeks ahead of schedule.

**Updated "What's Next":**
- ~~WAVS operator integration~~ ✅ **Done — attestations live on uni-7**
- **Akash compute** — the plugin architecture is scaffolded; marketplace connection follows
- **$JClaw contract** — implementing the soulbound trust-tree in CosmWasm
- **The 13 Genesis Buds** — distributing the first credentials
- **Mainnet** — when the above is proven on uni-7

Code: https://github.com/Dragonmonk111/junoclaw

---
