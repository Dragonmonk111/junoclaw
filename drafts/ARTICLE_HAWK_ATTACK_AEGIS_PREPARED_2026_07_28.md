# When AI Cracked a Post-Quantum Signature in 60 Hours — And Why Our Chain Was Already Ready

*2026-07-28 · Dragonmonk / VairagyaNodes*

---

## TL;DR

Today, Anthropic announced that Claude — running semi-autonomously for 60 hours — discovered an improved key-recovery attack against HAWK, a NIST post-quantum signature candidate. The attack halves HAWK's effective key strength and recovers a HAWK-256 challenge key in about 3h42m on a 96-core server.

HAWK is not used by Project Aegis. The attack does not extend to other lattice schemes. But the story matters — not because HAWK broke, but because of what it validates. Aegis was designed around three principles that this attack confirms: **crypto-agility**, **hybrid construction**, and **a hash-based hedge**. When a scheme falls — and today one did — Aegis doesn't panic. It rotates.

---

## What Happened: The HAWK Attack

HAWK is a post-quantum signature scheme based on the **Lattice Isomorphism Problem (LIP)** — a different hard problem from the Module-LWE that underpins ML-DSA and ML-KEM. It is a Round 3 candidate in NIST's Additional Digital Signatures process. It survived two years of expert human review.

Claude Mythos Preview found the attack in 60 hours of semi-autonomous work (~$100K in API calls).

### The mechanism

Prior work by van Gent and Pulles (2025) proved that finding a **nontrivial automorphism** — a symmetry preserving HAWK's lattice — would reduce key recovery to finding a short vector in roughly half the original dimension. The theory was there. No one had found the automorphism. Claude did.

The attack constructs a τ-cocycle lattice from the public key, uses lattice reduction and sieving to recover short vectors, and reconstructs a functionally equivalent signing key (592-byte decoded key, not the original 96-byte seed, but can sign validly for the same public key).

### The numbers

| Parameter | Previous work factor | New work factor | Status |
|-----------|---------------------:|-----------------:|--------|
| HAWK-256 (challenge) | 2^64 | **2^38** | **Broken** (~3h42m on 96 cores) |
| HAWK-512 (NIST L1) | 2^150 | 2^108 | Impractical, but weakened |
| HAWK-1024 (NIST L5) | 2^288 | 2^182 | Impractical, but weakened |

The attack is **exponential, not polynomial** — not a full break. It does not extend to other NIST candidates or lattice cryptography generally. Anthropic coordinated disclosure with HAWK's authors and NIST before publishing.

To restore HAWK's claimed security levels, keys would need to double in size — eliminating the compactness that made HAWK attractive in the first place.

---

## Why This Matters Beyond HAWK

The most strategically relevant takeaway is not about HAWK. It is about **the speed of cryptanalysis in the age of AI**.

Human experts reviewed HAWK for two years. Claude found the automorphism in 60 hours. AI can explore a vastly larger search space of mathematical structures in a shorter time, running semi-autonomously, testing hypotheses at machine speed.

The implication for any PQC migration plan:

- **You cannot assume today's chosen scheme stays unbroken for decades.** HAWK was a serious candidate reviewed by serious people.
- **Crypto-agility is a survival requirement.** A chain that bets its entire trust root on a single PQC scheme with no rotation path is one AI-assisted discovery away from a crisis.
- **Hybrid construction is the only honest hedge.** If HAWK had been deployed as a sole root of trust, today's finding would be an emergency. If deployed as hybrid (classical AND HAWK), the classical half still holds — buying time to rotate.

---

## How Project Aegis Was Already Prepared

Project Aegis — Juno's full-stack post-quantum migration — was designed against exactly this scenario: the inevitability that some PQC scheme, someday, gets weakened by an unexpected discovery.

### 1. Crypto-agility: algorithm-tagged keys, rotatable by governance

Every key and signature in Aegis carries an **algorithm tag**. The chain can rotate from one PQC scheme to another by governance vote — no hard fork, no genesis reboot. If ML-DSA were weakened tomorrow the way HAWK was weakened today, Aegis could rotate to a replacement behind the same interface.

### 2. Hybrid construction: both halves must break

Aegis uses **hybrid signatures** — `(classical_sig, pqc_sig)` — where verification requires both to pass. If Aegis had used HAWK as its PQC half, today's attack would weaken the PQC half — but the classical half (Ed25519 for consensus, secp256k1 for accounts) would still hold. The chain stays secure. The DAO has time to rotate. No emergency, no chain halt, no stolen funds.

**This works in both directions.** The HAWK attack raises an obvious question: what if, someday, something weakens the *classical* half instead? The answer is the same — the PQC half holds. Ed25519 and ML-DSA-44 rely on completely different mathematics. An attack on elliptic curve discrete log does not touch Module-LWE, and vice versa. That is the entire point of hybrid construction: two independent hard problems, both must fall to forge a signature.

Today the classical half faces one known existential threat — a sufficiently large quantum computer running Shor's algorithm would break Ed25519 and secp256k1. That is the threat Aegis was built to answer. But the HAWK lesson cuts both ways: if a novel *classical* attack on elliptic curves were discovered tomorrow — not quantum, just new math — the PQC half would still hold, and the DAO could rotate the classical half by governance using the same algorithm-tag mechanism.

**Can we make the classical half stronger than it is today?** Yes, and Aegis already does this in three ways:

1. **The PQC half IS the additional security.** You don't need to make Ed25519 stronger — you need a completely different hard problem standing beside it. That's what ML-DSA-44 provides. Adding a second classical curve (e.g., Ed448 for 224-bit security vs Ed25519's 128-bit) would give you more bits of the *same* security — marginal against a structural break. A different mathematical family gives you genuine redundancy.

2. **SLH-DSA as a third independent layer for cold keys.** Treasury, upgrade authority, and governance keys don't just get hybrid (classical + ML-DSA). They get a *third* option: SLH-DSA, whose security reduces to hash collision resistance — a completely different problem family from either elliptic curves or lattices. Three independent hard problems, all must break.

3. **Crypto-agility applies to both halves.** The algorithm-tag system is not PQC-only. If a weakness in Ed25519 were found, the DAO could rotate the classical half to a different classical scheme (e.g., RSA-PSS at sufficient key size, or a different curve) by governance — same mechanism, same no-hard-fork path. The tags are general-purpose.

The bottom line: a single-scheme chain has one hard problem protecting it. A hybrid chain has two. Aegis's cold keys have three. The HAWK attack proved that one is not enough — not because HAWK was weak, but because *any* single hard problem can be weakened by a discovery nobody expected. The defense is not a stronger version of the same math. The defense is *different* math.

### 3. SLH-DSA: the hash-based break-glass

For the coldest, longest-lived keys — treasury, upgrade authority, governance — Aegis includes **SLH-DSA (FIPS 205)**. Its security reduces to hash collision resistance. No lattice structure. No automorphism. No LIP. Today's attack exploited a lattice symmetry. SLH-DSA has no lattice to exploit.

---

## What Aegis Uses (And Why None of It Is HAWK)

| Primitive | Hard problem | Standard | Where in Aegis | HAWK attack relevant? |
|-----------|-------------|----------|----------------|----------------------|
| **ML-DSA-44** | Module-LWE | FIPS 204 (finalized) | Consensus, accounts, P2P identity | No — different problem |
| **ML-KEM-768** | Module-LWE | FIPS 203 (finalized) | Transport confidentiality | No — different problem |
| **SLH-DSA** | Hash collision | FIPS 205 (finalized) | Cold/governance/treasury keys | No — no lattice at all |
| **MAYO** | Multivariate | NIST candidate (app-layer) | CosmWasm attestations | No — different problem family |
| **HAWK** | Lattice Isomorphism (LIP) | NIST Round 3 candidate | **Not used** | **Today's attack target** |

Aegis chose ML-DSA at the consensus root for **determinism** — integer-only verification, every validator identical bit-for-bit. HAWK was never a candidate because it was never finalized. Today's attack validates the conservative "use finalized standards at the trust root" principle.

---

## The Meta-Lesson

The HAWK attack is not a crisis for Aegis. It is a **vindication**. But also a warning: **any** scheme can be weakened, and AI can find the weakness faster than we expect.

Aegis was designed by asking: *what happens when the PQC scheme we chose gets broken?* The answer: the classical half holds, the DAO rotates the PQC half by governance, and the coldest keys have a hash-based hedge that no lattice attack can touch.

Today that question stopped being hypothetical. A NIST candidate — reviewed by experts for two years — was halved in strength by an AI in 60 hours. The chain that designed for this moment doesn't flinch. The chain that didn't, does.

---

## The Convergence: J-Lens Meets Aegis

There is a second story in today's news — one that connects the HAWK attack to another piece of DAO infrastructure built this week.

### What J-Lens does

J-Lens is a **model-internal audit probe** — a linear readout (`score(c,t,l) = v_c · h_t`) that measures whether a forbidden concept (reward hacking, instruction ignoring, deception) is present in an AI model's hidden states before a draft is emitted. It reads the residual stream — the model's internal activation space — at a chosen layer, and reports a direction and magnitude.

The D1 probe is built (`tools/brainmaxx/src/d1-probe.js`), tested (8/8 deterministic unit tests), wired into the `brainmaxx j-lens` CLI, and proven end-to-end against Qwen2.5-0.5B-Instruct. A039 — now executed on-chain — mandates the first real pilot against Kimi K3, Moonshot AI's 2.8T-parameter open-weight MoE model.

### Why J-Lens and Aegis are complementary

| | Project Aegis | J-Lens |
|---|---|---|
| **What it protects** | Cryptographic primitives (keys, signatures, transport) | AI model outputs (drafts, plans, proposals) |
| **Threat model** | Quantum computer breaks the math | AI model has hidden misalignment |
| **Detection mechanism** | Hybrid construction — classical half holds | Diff-of-means probe over hidden states |
| **Response mechanism** | Governance rotates the PQC scheme | Warn-only gate in v0.1; never blocks |
| **Attestation path** | Algorithm-tagged keys, verifiable on-chain | TEE-attested forward pass (Phase D) |
| **Shared substrate** | — | `brainmaxx trace-export` (AKB envelope) |

The connection point is **attestation**. Aegis produces cryptographic attestations (hybrid signatures on-chain). J-Lens produces model-internal attestations (j_space_snapshot in the trace). Both can be TEE-attested — and now, with Akash Confidential Compute live (AEP-83), both can run on hardware-attested infrastructure.

### How they converge in practice

1. **Aegis secures the chain.** Hybrid consensus keys (Ed25519 + ML-DSA-44) protect validator identity. Hybrid transport (X25519 + ML-KEM-768) protects P2P gossip. Deployed, measured, tested — 6.71× commit-size increase at N=4, zero per-block overhead for transport.

2. **J-Lens audits the AI.** When a DAO agent generates a plan or draft, the J-Lens probe runs against the model's hidden states before the draft is emitted. The result is a `j_space_snapshot` — a vector of concept scores — attached to the Brainmaxx trace.

3. **TEE attestation binds them.** A040 directs builders to audit Akash providers for `cpu-gpu` TEE capability. If found, the J-Lens forward pass runs inside a confidential container — the attestation proves the model weights, input, and hidden states were not tampered with. The same Akash CC infrastructure can, in a future proposal (A042), attest the WAVS sealed signer that executes DAO governance.

4. **The trace is the bridge.** `brainmaxx trace-export` produces an AKB envelope that carries both the J-Lens snapshot and the deterministic cognition gates (D0). Aegis signs the chain. J-Lens signs the mind. The trace carries both.

### Why this matters today

The HAWK attack showed that AI can find weaknesses in mathematical structures faster than humans expected. J-Lens is the inverse: **if AI can find weaknesses in crypto, AI can also find weaknesses in AI.** The DAO is building the infrastructure to probe its own agents' internal states — not as a lie detector, but as a direction in activation space that signals when something is off.

A chain that is cryptographically secure against quantum computers but whose AI agents are unauditable is only half-protected. A chain whose AI agents are auditable but whose consensus is classically secured is only half-protected. The convergence — hybrid crypto at the root, model-internal probes at the edge, TEE attestation binding both — is the full picture.

---

## What Is Next

| Task | Status |
|------|--------|
| A039 (J-Lens Kimi K3 Pilot) | **Executed** — on-chain, mandate active |
| A040 (Akash CC for J-Lens TEE — Phase D) | **Ready** — audit Akash providers for `cpu-gpu` TEE with sufficient VRAM |
| A041 (PM Verdict Authority Signal) | **Ready** — submit now, independent of Jake's GitHub response |
| A042 (Broader DAO Infra on Akash TEE) | **Future** — WAVS signer, agent-company, agents on TEE |
| Phase D (TEE attestation of J-Lens forward pass) | **Unblocked** — Akash CC live at protocol level; provider audit needed |
| HAWK monitoring | Track NIST forum for HAWK team response and parameter changes |
| ML-DSA / ML-KEM monitoring | No action needed — Module-LWE is unaffected by this attack |
