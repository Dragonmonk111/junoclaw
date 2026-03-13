# JUNOCLAW — Product Vision

> **JUNOCLAW is the HOME AI WORKSTATION that turns your imagination into REAL-WORLD monetary implications.**
> You are prompting work — both digital and REAL-WORLD.

---

## The Three Pillars

### 1. WAVS Verification System

WAVS (Witness-Attested Verifiable Services) by Layer.xyz provides the cryptographic backbone that makes JunoClaw outputs **trustworthy in the real world**.

- **Trusted Execution Environment (TEE)** — Agent computations run inside hardware-attested enclaves. The result is signed proof that computation happened correctly, without anyone being able to tamper with it.
- **Witness Attestation** — Multiple independent witnesses verify task outcomes before on-chain settlement. Configurable quorum (e.g. 2-of-3 witnesses) prevents single points of failure.
- **On-Chain Proof Anchoring** — Every verified result is hashed and committed to the Juno blockchain, creating an immutable audit trail from prompt to outcome.
- **Real-World Implications** — Because outputs are verifiable, they can trigger real financial flows: payment authorizations, contract fulfillment, insurance claims, verifiable outcome market resolutions — all without trusting any single party.

### 2. Akash Supercloud — Powered Agentic Trust Continuity

Akash Network is the decentralized GPU compute layer that ensures your agents never stop working, regardless of your local hardware.

- **Seamless GPU Scaling** — When a task exceeds your local Ollama capacity (large model inference, batch processing, image generation), JunoClaw automatically delegates to Akash GPU providers. One click, no cloud accounts, no credit cards.
- **Agentic Trust Continuity** — The critical innovation: when a task moves from your local machine to Akash, the trust chain is **unbroken**. The agent's identity remains on-chain (Juno), the task is logged in the immutable task ledger, and WAVS attestation covers the Akash-executed portion. You get the same verification guarantees whether running locally or on a rented A100.
- **Decentralized Compute Marketplace** — No single cloud provider owns your compute. Akash providers compete on price and performance. Skip Protocol integration enables one-click JUNO → AKT payment routing.
- **Always-On Agent Infrastructure** — Your agents can run 24/7 on Akash even when your local machine is off. The on-chain identity and task ledger ensure continuity — you wake up, reconnect, and see exactly what your agents accomplished overnight.
- **Cost Transparency** — Every GPU-second is priced, logged, and settled on-chain. No surprise bills. The payment ledger records obligations; the user's wallet signs every actual payment directly (no custodial contracts).

### 3. JUNOCLAW — The Home AI Workstation

This is not another ChatGPT wrapper. JunoClaw is a **sovereign AI workstation** that runs on your hardware, under your control, producing outputs that have real-world monetary consequences.

- **Your Machine, Your Models** — Run Ollama models locally: Llama 3.1, DeepSeek R1, Qwen 2.5. Your data never leaves your device unless you explicitly delegate to Akash.
- **Agent Hierarchy** — A main agent (bound to your Juno wallet) orchestrates sub-agents. The main agent can delegate prompts to sub-agents, and sub-agents can chain delegation further. Each agent has its own execution tier, model, and capabilities.
- **Prompt = Work Order** — Every prompt you type is a work order. It gets logged on-chain, executed (locally or on Akash), verified (via WAVS), and the result can trigger payment authorization, DAO votes, or cross-chain transactions. You’re not chatting — you’re issuing verifiable instructions.
- **DAO Governance Built In** — Every JunoClaw deployment is a DAO. Proposals, votes, quorum, adaptive deadlines — all on-chain. Agents from different wallets can participate in the same DAO, creating multi-party decision-making with cryptographic guarantees.
- **Two-Tier Compute + WAVS Verification** — Choose per-task:
  - **Local** — Fast, free, private (Ollama on your device)
  - **Akash** — GPU power when you need it (decentralized cloud)
  - **WAVS toggle** — Enable TEE-attested verification on any tier for high-stakes tasks

---

## $JCLAW — The Governance Token

**$JCLAW is a governance token with solely governance value and no monetary value whatsoever.**

### Purpose

$JCLAW exists for one reason: to give the people who built and secure the Cosmos ecosystem a voice in JunoClaw's direction. It is not a speculative asset. It is not traded for profit. It is a coordination mechanism.

### Distribution

$JCLAW is distributed exclusively to:

- **Original Cosmos Creators** — The builders who laid the foundation for the interchain.
- **Juno Core Developers** — Engineers who built and maintain the Juno Network.
- **Validators in the Active Set** — Those currently securing the Juno chain through consensus.
- **Validators in the Inactive Set** — Those running nodes and standing ready to enter the active set, contributing to network resilience.
- **Node Runners** — Infrastructure operators who keep the network decentralized.

### Governance Rights

$JCLAW holders can:

- Vote on JunoClaw protocol upgrades and parameter changes.
- Propose and vote on DAO-level configuration (quorum thresholds, voting periods, verification requirements).
- Direct the development roadmap through on-chain proposals.
- Approve or reject WAVS push operations that affect shared infrastructure.

### What $JCLAW Is NOT

- It is **not** a payment token — all payments use JUNO/USDC/AKT.
- It is **not** a speculative asset — there is no liquidity pool, no DEX listing by design.
- It is **not** transferable for monetary gain — it is a coordination tool, not a financial instrument.

---

## How It All Connects

```
You (Prompt) ──→ Main Agent (Local Ollama)
                    │
                    ├──→ Sub-Agent A (Akash GPU, WAVS ✓) ──→ Sub-Agent A1 (Local)
                    │                                              │
                    │                                       Real-World Data Source
                    │
                    ├──→ Sub-Agent B (Local) ──→ DAO Proposal
                    │
                    └──→ WAVS Attestation ──→ On-Chain Proof ──→ Payment Authorization
```

Every arrow in this diagram is:
- **Logged** on the Juno blockchain (task ledger)
- **Verifiable** through WAVS attestation (when enabled)
- **Governed** by $JCLAW holders

**This is what it means to turn imagination into real-world monetary implications.**

---

## Non-Custodial Payment Ledger

JunoClaw uses a **payment-authorization ledger** instead of a custodial escrow. The contract never holds user funds.

- **Authorize** — Record that payer owes payee X tokens for task Y
- **Confirm** — Payer signals they sent funds directly (off-contract)
- **Dispute** — Payer challenges the obligation
- **Cancel** — Mutual or admin cancellation
- **AttachAttestation** — WAVS hash proving the obligation is valid

Every actual token transfer is a direct `BankMsg::Send` signed by the user’s wallet. The contract is purely a **state machine for obligations**, not a money handler. This eliminates custodial risk and money-transmission concerns.

### Workflow

1. **Authorize** — A DAO proposal passes → the contract records "Payer owes Payee X tokens for Task Y"
2. **Direct Payment** — Payer sends funds **directly** to Payee's wallet (standard Cosmos `BankMsg::Send`, signed by the payer's key)
3. **Confirm** — Payer (or task-ledger) marks the obligation as fulfilled on the ledger
4. **AttachAttestation** *(optional)* — WAVS TEE hash is attached, proving the underlying task was completed correctly
5. **Dispute / Cancel** — State transitions only; no fund movements ever occur inside the contract

### Legal Posture

- **No money transmission** — The contract never holds, pools, or transfers user funds. It is a state machine for obligations, not a money services business. This avoids FinCEN MSB registration, state money transmitter licenses, and equivalent non-US regulations.
- **No custodial risk** — Users sign every `BankMsg::Send` from their own wallet. The contract cannot be classified as a custodian under SEC/CFTC guidance because it never has control over user assets.
- **No commingling** — Funds flow peer-to-peer. There is no omnibus account, no pooled balance, and no contract-held float.
- **Audit trail without custody** — Every obligation is recorded on-chain with timestamps, parties, amounts, and optional WAVS attestation hashes — providing full transparency without the legal burden of holding funds.

> *This is not legal advice. Consult qualified counsel for your jurisdiction.*

---

## Verifiable Outcome Markets

DAOs can create and resolve **verifiable outcome markets** — structured questions where the most logical outcome is agreed upon by various DAO agents and members, with results verified by WAVS TEE attestation.

- **OutcomeCreate** proposal — Defines a question, resolution criteria, and deadline
- **OutcomeResolve** proposal — WAVS-attested outcome triggers resolution
- **Governance-weight positions** — Members stake $JCLAW governance weight (not money) on outcomes
- **Agent automation** — Agents can create markets from natural language prompts and act as oracles
- **Not financial derivatives** — No monetary value changes hands. These are governance signal markets, positioning them outside CFTC event-contract jurisdiction (cf. Kalshi/Polymarket regulatory issues).

This enables futarchy-style governance where policy decisions are informed by verifiable outcome signals, all attested by TEE computation.
