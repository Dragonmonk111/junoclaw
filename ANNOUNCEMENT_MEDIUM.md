# Trust Without Trustees

> *Four smart contracts went live on the Juno testnet today. This is about what they make possible — organisations that verify themselves, governed by trust instead of capital, powered by machines that do digital work with real-world consequences.*

---

## The Gap

Across the world, communities coordinate collectively every day — pooling resources, making shared decisions, distributing work. They always have. The structures they use are as old as civilisation: shared ledgers kept in notebooks, agreements held in memory, accountability maintained through reputation.

These structures work. They are resilient precisely because they don't depend on external institutions. They depend on something universally available: **people knowing each other well enough to coordinate**.

The limitation is not that these structures fail. The limitation is that they don't persist beyond the room, don't scale beyond the group, and can't be verified by anyone who wasn't present. A spoken agreement is binding between two people. It is invisible to everyone else.

What if the agreement could live on a chain that anyone can audit? What if the verification could be done by a machine that nobody — not even its operator — can tamper with? What if the coordination could scale without adding intermediaries?

---

## Five Existing Things, Newly Wired Together

Decentralised governance existed. AI agents existed. Decentralised compute marketplaces existed. Trusted Execution Environments existed. Immutable ledgers existed.

What didn't exist — until today — was a system that wires all five together on a single chain, with verifiable trust at every junction.

- **DAO governance on Juno** — proposals, votes, adaptive quorum, sortition — the decision layer
- **AI agents** running locally or on decentralised Akash GPU — the execution layer
- **GPU compute** — local or hired from the Akash marketplace — the energy that makes agents think
- **WAVS TEE** — hardware-attested verification that no party can falsify — the truth layer
- **Non-custodial on-chain ledger** — obligation tracking without ever holding tokens — the record layer

No single piece is new. The wiring is. Built on Juno Network. Secured by Cosmos consensus. Verified by WAVS — co-founded by the creator of CosmWasm and a co-founder of Juno.

---

## The Energy: Compute as Fuel

Every organisation needs energy. For the agentic DAO, that energy is GPU compute — the resource that lets agents reason, produce, and act.

**Local compute** — your own machine running Ollama. Llama 3.1, DeepSeek R1, Qwen 2.5 — on your hardware, under your control. Your data stays on your device. You are sovereign over your own inference.

**Decentralised compute** — when local isn't enough, the Akash Network provides a marketplace of GPU providers. You hire compute, pay in tokens via a single routing step, and your agent continues working on hardware you've never seen, in a location you don't need to know.

The innovation is what happens at the boundary. When a task moves from your machine to a hired GPU elsewhere, the **trust chain does not break**. The agent's identity remains anchored on Juno. The task is logged in the immutable ledger. WAVS attestation covers the remote portion. The verification guarantee is identical whether the computation ran on your desk or a rented GPU on another continent.

A machine running *here* gets work verified *there* and causes consequences *everywhere*. The location of the compute is irrelevant. The proof is universal.

---

## The Face of the Organisation (Locally run Agentic DAO)

The face of this organisation is not a person. It is a **complex humanoid** — an AI agent that is a trusted extension of your intent, connected to you through verifiable smart contracts.

This is not a chatbot. A chatbot responds to questions. An agent in this system **acts**: it decomposes your intent into tasks, routes them to the right compute tier, calls tools, delegates to sub-agents when needed, and produces outputs that have real-world consequences — obligations recorded, votes triggered, attestations anchored on an immutable chain.

You express, in natural language, what needs to happen. Your prompt is a **work order**. The system converts it into a chain of on-chain actions, each logged, each verifiable, each traceable back to your intent.

The agent has an on-chain identity in the agent-registry. Every action it takes is logged in the task-ledger. Its obligations are tracked in the escrow contract. Its governance rights are defined by the agent-company DAO. Four contracts form the nervous system. The agent is the face. You are the intent behind it.

Agents can delegate to sub-agents. Sub-agents can delegate further. A main agent orchestrating three specialised sub-agents across two compute tiers, producing a WAVS-attested output that triggers a DAO vote — this is what a working group looks like when there is no office and no org chart.

**This is Proof of Work in its truest sense.** Not hashing arbitrary numbers. Proving that real work was done — digitally — by an agent you trust, verified by hardware you don't need to trust, with consequences that reach the physical world.

---

## Where Digital Work Meets the Real World

Here is the shift that changes everything.

Without verification, an agentic DAO is governance built on claims. "My agent says the conditions were met." "My agent says the task was completed." Claims can be wrong. Claims can be fabricated. A governance system that acts on unverified claims is no better than a suggestion box.

WAVS (Witness-Attested Verifiable Services) is the layer that converts claims into facts.

A Trusted Execution Environment is a sealed vault inside a processor. Code runs inside it in hardware-enforced isolation — protected from the rest of the system, including the operator who owns the machine. The output comes with a cryptographic attestation that the computation ran correctly and was not tampered with. Not a promise. A proof rooted in the physical properties of silicon.

A machine in a datacenter fetches real-world data — weather, GPS coordinates, a public record, sensor readings — inside a sealed enclave. The enclave produces an attestation: *this is what the data showed, at this time, and here is the cryptographic proof that I could not have lied about it*. That attestation travels on-chain. The DAO's smart contract receives it and acts — resolving a proposal, recording an obligation, updating a ledger entry.

**The digital work IS the real work.** A processor in one location verifies a condition in another location and triggers a consequence in a third. The machine that ran the computation may be on a different continent than the people it affects. The proof doesn't care about geography. It cares about physics.

The JunoClaw contracts are WAVS-ready today. VerificationConfig supports witness attestation, WAVS TEE, or both. WavsPush proposals compose verified sub-messages to the task-ledger and obligation tracker. AttachAttestation anchors WAVS hashes to recorded obligations. SubmitRandomness handles WAVS-attested drand for provably fair selection. The operator layer — the actual TEE enclaves running live computations — is the next integration milestone.

When operators come online, the end-to-end flow:

    Human prompt: "Verify whether conditions in region X crossed the threshold"
      → Agent creates a WavsPush proposal
      → DAO governance approves execution
      → WAVS TEE fetches real-world data inside a secure enclave
      → TEE output: "Condition verified. Attestation hash: 0xabc..."
      → Attestation received on-chain → forwarded to agent-company contract
      → Proposal resolves automatically based on verified data
      → Obligation ledger records what is owed to whom
      → Tokens move wallet-to-wallet, signed by each party directly

No person decided the outcome. No person could falsify the data. No person held anyone else's tokens at any point.

---

## The Ledger That Holds Nothing

The obligation-tracking contract is not a vault. It is a **ledger** — a state machine for recording what is owed, by whom, to whom, verified by what.

- **Authorize** — record that party A owes party B a specified amount for a specified task
- **Confirm** — party A signals that tokens were sent (directly, wallet-to-wallet)
- **Dispute** — challenge a recorded obligation
- **AttachAttestation** — anchor a WAVS hash proving the underlying work was verified

Every actual token transfer is a direct send signed by the paying wallet. The contract never receives, pools, or transfers tokens. It is purely a transparent record of obligations — visible on-chain, queryable by anyone, tamper-proof by design.

This separation is deliberate. The obligation ledger tracks truth. The tokens move peer-to-peer. No custody. No commingling. No intermediary between sender and receiver. The agreement, digitised and made permanent, without any party in the middle.

---

## What Is Live Today

Four CosmWasm contracts deployed to Juno testnet (uni-7), Friday, March 13, 2026:

**agent-registry** — code 54 — On-chain agent identity. Every agent registered, tied to a Juno wallet.

**task-ledger** — code 55 — Immutable audit trail. Every task logged with input, tier, result hash, cost.

**escrow** — code 56 — Non-custodial obligation ledger. Records tracked, tokens never held.

**agent-company** — code 57 — DAO governance. Proposals, adaptive deadlines, sortition, WAVS push, distribution.

**28 contract tests pass.** Governance flows, adaptive deadline reduction, WAVS randomness submission and rejection, Fisher-Yates sortition via NOIS/drand, payment distribution, unauthorised access rejection.

The Rust daemon runs locally: Axum HTTP + WebSocket, Ollama LLM streaming, agent hierarchy with delegation, sandboxed tool execution. The React frontend provides a 5-step DAO creation wizard and 10 pre-built templates. The stack: Vite, Tailwind, Zustand on the frontend; CosmWasm 2.2, Tokio, Axum in Rust.

---

## Three Organisations That Become Possible

Rather than listing all ten templates, consider them as stories. Here are three of them —

**Verifiable protection against weather events.**
Twenty participants form a Crop Protection DAO. Each contributes a fixed amount per season. The DAO's agent monitors publicly available weather data. When conditions cross a defined threshold, the WAVS TEE independently verifies the data inside a sealed enclave — no human judgement involved. The governance logic resolves the proposal automatically. The obligation ledger records each participant's share. Tokens move directly from wallet to wallet. The entire coordination costs a few tokens of gas per season.

**A representative assembly chosen by lottery.**
A community needs to make a collective decision. Instead of a popularity contest, they use the Citizens' Assembly template. WAVS TEE verifies the eligible pool. NOIS/drand fires a cryptographically unbiasable random beacon via IBC. The beacon selects N addresses from the verified pool using Fisher-Yates shuffling. Those selected deliberate and vote. Their term ends, and a new random selection follows. Provably fair. Auditable by anyone on the Juno chain.

**A local marketplace with no platform operator.**
A producer lists goods. A buyer's agent, running autonomously on local compute, discovers the listing and evaluates price and reputation. Both agents negotiate. The obligation ledger records the agreement. Physical delivery happens. The buyer's agent submits delivery confirmation — location data, timestamp, a hash of evidence. WAVS TEE verifies the proof. The obligation is marked confirmed. The producer receives tokens directly, instantly. No platform operator. No commission. The DAO — the participants themselves — governs quality standards and dispute resolution.

Three templates. Four contracts. Two compute tiers. One verification layer. Zero intermediaries.

---

## Who Governs the Governors

Every system that replaces old structures raises a question: who controls the new one?

Governance in most decentralised systems is weighted by token holdings. Whoever accumulates the most tokens accumulates the most power. If governance power can be acquired on an open market, it will be — by parties whose interests may not align with those who built or use the system. Sybil attacks — creating many identities to amplify influence — are the other failure mode.

$JClaw answers this differently. It has no monetary value. There is no liquidity, no exchange listing, no over-the-counter market — by design, permanently. It cannot be transferred between wallets. It cannot be acquired through any means other than being trusted by someone who already holds one.

**$JClaw is a soulbound trust-tree credential.**

### The Handover Protocol

The genesis begins with one wallet — the one that deployed the contracts and proved the system works on testnet. This wallet holds the root credential. From it, a carefully designed handover protocol distributes governance to the community.

**Phase 1 — Genesis Mint.**
The genesis wallet calls MintGenesis on the $JClaw contract. This creates the root credential: depth 0, with a special allowance of 13 buds.

    TokenRecord {
        holder:          genesis_wallet,
        parent:          None,
        tree_root:       genesis_wallet,
        depth:           0,
        remaining_buds:  13,    // root gets 13; everyone else gets 1
        revoked:         false,
        issued_at:       block_height,
    }

**Phase 2 — Transparent Nomination.**
The root submits NominateRecipients { recipients: [addr1 .. addr13] } in a single transaction. This creates 13 PendingBud records, publicly visible and queryable on-chain. The community can see who was nominated before anything is accepted. Transparency is enforced at the protocol level.

**Phase 3 — Distribution.**
The root calls DistributeBud { recipient } for each of the 13 addresses. Each call decrements the root's remaining buds and creates a PendingCredential. Each transaction emits a wasm-bud_offered event — publicly indexed.

**Phase 4 — Acceptance Window.**
Each nominated recipient has a defined window (~100,000 blocks, roughly one week) to call AcceptBud from their own wallet. On acceptance:
- The pending credential becomes an active TokenRecord at depth 1
- The recipient gains governance voting rights immediately
- The recipient receives their own single bud — one, forever

If a nominee doesn't accept within the window, the pending credential expires and the bud returns to the root for reallocation. Nobody can be forced into the tree.

**Phase 5 — Handover Complete.**
Once all 13 buds are distributed and accepted, the root's remaining_buds reaches zero. From this moment forward, the root holds one credential — equal in voting weight to every other holder in the tree. The root has no special powers. The genesis ceremony is complete.

                          [Genesis Root]    — 1 credential, 0 remaining buds
                                |
        ┌──┬──┬──┬──┬──┬──┬──┬─┼─┬──┬──┬──┬──┐
        D1 D2 D3 D4 D5 D6 D7 D8 D9 D10 D11 D12 D13    — 13 credentials, 1 bud each

### Linear Budding — The Tree Grows

Each of the 13 first-generation holders has exactly one bud. When they find someone whose judgement they trust — a developer, a validator, a contributor — they call Bud { recipient }. The same acceptance flow applies. The new holder receives a credential at depth 2 with their own single bud.

        D4 (depth 1)
        |
        D4a (depth 2)    — D4 vouched for this person; D4 can never bud again
        |
        D4a1 (depth 3)   — D4a vouched for this person; D4a can never bud again

The tree grows one link at a time. Each link is a human saying: *I trust this person enough to give them a permanent voice.* The cost is irreversible — you only get one bud, and once given, it's gone.

### Pruning — The Tree Heals

If a branch goes wrong — a holder acts against the community, or passes their bud to someone who does — any credential holder can propose BreakChannel { node }. A standard governance vote follows. If passed:

- The targeted node's credential is revoked
- All descendants are recursively revoked
- Revoked credentials lose voting rights immediately
- **The budder who created the revoked node does NOT get their bud back** — the cost of misplaced trust is permanent

The rest of the tree is unaffected. Surgical removal, not scorched earth.

### Nobody Is Above the Tree

The genesis root can be pruned by supermajority governance proposal (75%+ of active credentials). The root's position is being first — not being privileged. Every credential, at any depth, carries equal voting weight. Depth is visible as provenance — a transparency feature, not a hierarchy.

### What $JClaw Governs

- Protocol upgrades and parameter changes
- DAO template additions, removals, and edits (visible in the wizard as governance action boxes)
- Verification requirements — when WAVS attestation is mandatory
- WAVS push operations affecting shared infrastructure
- BreakChannel proposals to prune trust branches
- Development roadmap priorities

### What $JClaw Does NOT Do

- It carries no monetary value
- It is not tradeable — no market exists, by permanent design
- Holding it earns nothing — no rewards, no yield, no airdrops
- There is no vesting schedule — trust is binary

### On-Chain Transparency

The entire tree is queryable:

    QueryTree {}           → full tree: all holders, depths, bud status
    QueryBranch { node }   → a node and all its descendants
    QueryDepth { addr }    → hops from genesis
    QueryParent { addr }   → who vouched for this holder
    QueryPending {}        → buds awaiting acceptance
    QueryRevoked {}        → pruned branches

Every governance token asks: *how much did you put in?*

$JClaw asks: *who trusts you?*

---

## What the Cosmos Makes Possible

JunoClaw is built on Juno Network because Juno provides the conditions: permissionless CosmWasm smart contracts, an active validator set, IBC connectivity to the broader Cosmos, and a community that has governed itself through difficult decisions and emerged with its principles intact.

The broader Cosmos gives the system its reach. Agents can send IBC messages to any connected chain. Akash compute is Cosmos-native. NOIS randomness travels over IBC. The trust that Juno anchors extends across the interchain without any bridge operator in the middle.

This is a Cosmos-native system. Not ported. Not wrapped. Built for IBC from the ground up.

---

## The Code

JunoClaw is open source. Contracts, daemon, frontend, plugins — all of it.

**GitHub**: https://github.com/Dragonmonk111/junoclaw

**Testnet contracts on uni-7:**
- agent-registry — code 54
- task-ledger — code 55
- escrow — code 56
- agent-company — code 57

**Verify on-chain** (Polkachu REST API — Mintscan does not currently index Juno testnet):
- agent-registry: juno1qulyspwzjzsz7rq65v6ptzt278f9ta9uh0upxu6xa08gf4v5gzaqm676j7
- task-ledger: juno1agw6f05wxx5rm8d3etq7cejcm5g8e224s00dvykylaja7jlx3ljq6f0u46
- escrow: juno1dh43lswg5ekv7q2p44s6hgays47k5mz67742vdwpd025p8q05kgs0azwrv
- agent-company: juno12xayvf6uz0juj4rrm9p62626fjc2r289qz2kyzp9jpxd7d93fggsy7ja06

The frontend is standard Vite + Tailwind + Zustand. Clone it, run npm install && npm run dev, and the wizard runs in sixty seconds. The Rust daemon is a standard Axum server. The contracts are standard CosmWasm. Nothing exotic in the stack. Cosmos developers can query the contracts today. Web2 developers can run the frontend today.

---

## What's Next

- **WAVS operator integration** — the contracts are WAVS-ready; connecting to the TEE operator network is next
- **Akash compute** — the plugin architecture is scaffolded; the marketplace connection follows
- **$JClaw contract** — implementing the soulbound trust-tree and handover protocol in CosmWasm
- **The 13 Genesis Buds** — distributing the first credentials to trusted Juno developers and validators
- **Mainnet** — when the above is proven on uni-7

Testing continues. The testnet is live. The iteration is ongoing. Anyone who wants to run the node, query the contracts, or contribute to the codebase can do so today.

---

A machine runs in a sealed enclave. It fetches real-world data. It produces a proof that no one — not even its operator — could have falsified. That proof travels on-chain. A smart contract acts on it. Tokens move. Obligations resolve. The digital work reaches the physical world.

No intermediary decided. No intermediary could have lied. No intermediary held anything.

**Built on Juno. For the Cosmos. Open to everyone. Judge for yourself.**
