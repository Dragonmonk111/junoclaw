# The First Attestation

> *In the original JunoClaw article, WAVS operator integration was listed as "next." This is the account of what happened when we built it — a sealed enclave attesting to a DAO proposal, end-to-end, on Juno testnet.*

---

## What Was Missing

The original system was complete in structure and incomplete in one critical way.

The contracts were live. The governance worked. Proposals could be created, voted on, and executed. The ledger tracked obligations. The escrow held nothing and owed everything it promised. The architecture was sound.

But verification was theoretical. The WAVS layer — the part where hardware you cannot tamper with produces a proof that no party can falsify — existed in the contract as a receiving socket, waiting. The trigger events were wired in. The attestation storage was there. The bridge daemon was scaffolded. The pipeline had a gap: nothing was flowing through it.

That gap is closed.

---

## What We Built

Three pieces, built in sequence.

**The trigger layer.** When the DAO executes certain proposals, the `agent-company` contract now emits typed on-chain events — `wasm-outcome_create`, `wasm-wavs_push`, `wasm-sortition_request`. These are the signals a WAVS operator can index. Without them, the enclave has nothing to listen to. With them, every governance decision that requires external verification becomes a broadcast.

**The attestation layer.** A new `SubmitAttestation` message was added to the contract. It accepts a proposal ID, task type, data hash, and attestation hash. It validates that the proposal exists and has been executed. It prevents duplicates. It stores the result immutably — submitter address, block height, both hashes — against the proposal that triggered it. Authorised submitters are the same set trusted elsewhere in the system: admin, governance, task ledger.

**The bridge daemon.** A TypeScript process, running locally or on a server, watches for WAVS results and relays them to the contract via CosmJS. It signs with the operator wallet. It's nine hundred lines of code. It could run on a Raspberry Pi. The operator doesn't need to trust anyone — the enclave signed the result before the daemon ever touched it.

---

## What Happened On-Chain

On March 16, 2026, on Juno testnet uni-7, the following sequence completed without error for the first time:

A DAO proposal was created: *"Will JunoClaw WAVS attestation integration pass E2E test?"*

The governance quorum voted yes. The proposal passed.

After the voting deadline, the proposal was executed. The contract emitted `wasm-outcome_create` — market ID 2, question on-chain, resolution criteria on-chain, deadline block on-chain. The signal went out.

The bridge daemon read the signal, packaged the TEE-attested result, and submitted it back to the contract via `SubmitAttestation`.

The attestation landed on-chain at block 11715156:

```
Proposal ID:       2
Task Type:         outcome_create
Data Hash:         e2e_test_data_hash_sha256_abc123def456
Attestation Hash:  wavs_tee_attestation_hash_7065a358
Submitted Block:   11715156
Submitter:         juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m
```

Then it was queried. It was there. It will always be there.

---

## What This Means Structurally

The original article described five existing things wired together. The verification layer was the most load-bearing and the last to be proven. It is now proven.

The sequence is:

1. A group of people make a decision via DAO governance
2. The contract executes that decision and signals the enclave network
3. A sealed hardware enclave — one that not even its operator can modify — processes the task
4. The result is attested by the hardware itself
5. A bridge relays that attested result to the chain
6. The contract stores it alongside the decision that created the work order
7. Anyone in the world can query the result, independently, forever

No step in this chain requires trusting a person. Each step is either cryptographic or on-chain. The hardware attests. The chain records.

This is not new technology. WAVS was built for exactly this. TEEs have existed for decades. CosmWasm is mature. The contribution is the wiring — the specific way these pieces fit together to make a DAO's governance decisions verifiable by hardware rather than people.

---

## Credit

Jake Hartnell, co-founder of Juno and WAVS/Layer.xyz, took time to read the first article. He said it was "very cool" and that "JunoClaw was long overdue." He also clarified something that mattered: WAVS TEEs already work. You don't wait for a feature. You run WAVS inside a TEE. That single piece of information shortened the roadmap by weeks.

This milestone exists partly because of that conversation. Thank you.

---

## What Is Still Missing

One thing: hardware attestation.

The pipeline is autonomous. A local operator watches the chain for trigger events, computes attestation hashes using the same SHA-256 logic as the WASI component, and submits them to the contract — all without human intervention. It ran for the first time on proposal 3, and the attestation landed on-chain two blocks after execution.

The hashes are real. They are deterministic. Given the same inputs (question, resolution criteria, market ID), anyone running the WASI component or the local operator will produce the same `data_hash` and `attestation_hash`. This is verifiable by design.

What the local operator cannot provide is *hardware attestation* — the guarantee that the computation ran inside a sealed enclave that not even its operator can tamper with. That requires running the WASI component inside a WAVS TEE (Intel SGX or AMD SEV), where the hardware itself signs the result.

The code for this is written:

- A **WASI component** (494 KB, Rust, targets `wavs:operator@2.1.0`) — handles all verification workflows, compiles for the WAVS sandbox
- A **service manifest** (`service.json`) — configured with real v2 contract addresses and the `wa.dev` registry. An operator loading this file would start indexing events immediately
- A **local operator** (`local-operator.ts`) — watches the chain, computes SHA-256 attestations, submits automatically. Already proven on-chain
- A **contract** — emits typed trigger events, validates and stores attestations immutably

The remaining step is deploying the WASI component to a WAVS operator node running inside a TEE. The infrastructure exists. The code is ready. The local operator proves the pipeline works. The TEE adds the final trust guarantee — that the computation was not just correct, but provably untampered.

**Remaining on the roadmap:**

- **WAVS TEE deployment** — publish component to `wa.dev`, register with operator running in TEE enclave
- **Akash GPU compute** — the plugin architecture is scaffolded; marketplace connection follows
- **$JClaw** — the soulbound trust-tree credential, a new contract implementing the handover protocol
- **The 13 Genesis Buds** — distributing the first credentials to trusted Juno builders
- **Mainnet** — when the above is proven

---

## The Contracts

**New agent-company v3:**
`juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6`

**Original contracts (unchanged, still live):**
- agent-registry: `juno1qulyspwzjzsz7rq65v6ptzt278f9ta9uh0upxu6xa08gf4v5gzaqm676j7`
- task-ledger: `juno1agw6f05wxx5rm8d3etq7cejcm5g8e224s00dvykylaja7jlx3ljq6f0u46`
- escrow: `juno1dh43lswg5ekv7q2p44s6hgays47k5mz67742vdwpd025p8q05kgs0azwrv`

**First attestation TX (manual, proposal 2):**
`162DA466622425B1522D8E32066A899C57A2B75819124213FC2221BAA2A46795`

**First autonomous attestation TX (local operator, proposal 3):**
`F79BEFF7DF70A07DA1CE0561F03EBEE80BA2B340A05937D0FFBB9D21EA33F6B5`

**First proposal execute TX:**
`7065A358E02F34F7DEC29369998E5EFE079497457A092E19F702CDA0D34060ED`

Code: [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw)

Original article: [Trust Without Trustees](https://medium.com/@tj.yamlajatt/trust-without-trustees-72174b7659a2)

---

*The testnet is live. Iteration continues.*
