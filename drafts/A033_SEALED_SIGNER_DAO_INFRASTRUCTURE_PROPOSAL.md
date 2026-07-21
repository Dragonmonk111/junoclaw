# A033 — Authorize the TEE-Sealed Signer as DAO Signing Infrastructure

> Follow-up to A18c-9 (J-Reef/J-Lens Audit Layer). A18c-9 authorized the audit brain. This proposal authorizes the secure signing hands — the TEE-sealed signer that replaces the plaintext mnemonic for all DAO agent transactions. M2 is built, compiled, and cross-checked. This proposal authorizes production deployment.

---

## Copy-paste box 1: Title

```
A033 — Authorize the TEE-Sealed Signer as DAO Signing Infrastructure
```

## Copy-paste box 2: Description

```
A18c-9 authorized the J-Reef/J-Lens audit layer — the DAO's audit brain. This proposal authorizes the secure signing hands: a TEE-sealed signer that replaces the plaintext mnemonic currently used to sign all DAO agent transactions.

What that means in plain terms:
- Today, the DAO's agent signing key is a plaintext mnemonic in a developer terminal. Anyone with terminal access can exfiltrate it. This is the single largest security gap in our agent infrastructure.
- The sealed signer moves key generation and transaction signing inside a Trusted Execution Environment (Intel SGX / AWS Nitro). The key is generated inside the enclave, never leaves it, and is persisted as an AES-256-GCM sealed blob. Every signed transaction is produced inside the TEE with a hardware attestation proving the code ran unmodified.
- M2 is built and cross-checked: the enclave produces byte-identical Cosmos SDK SIGN_MODE_DIRECT transactions that Juno will accept. All four targets compile clean (agent-company contract, WAVS component, sealed-signer component, bridge TypeScript).

What this proposal does:
1. Authorizes the TEE-sealed signer as the DAO's official agent signing mechanism, replacing the plaintext mnemonic.
2. Authorizes the relayer and sealed_signer roles in agent-company as DAO-controlled, governance-rotatable configuration.
3. Authorizes the on-chain sign-request round-trip (RequestSignedTx → StoreSignedTx → AckBroadcastTx) as the production signing flow until the WAVS off-chain invoke API is available.
4. Directs builders to simplify the round-trip once the WAVS runtime supports off-chain component invocation — spec and architectural answers have been sent to Jake Hartnell.
5. Directs an SGX determinism re-run for the sealed signer component before the mnemonic is fully retired.

In scope:
- Sealed signer deployment as DAO signing infrastructure.
- Relayer and sealed_signer roles in existing agent-company contract.
- On-chain sign-request round-trip as production flow.
- Pursuit of WAVS off-chain invoke API simplification.

Out of scope (will require future proposals):
- Treasury spend or enclave account funding.
- Changes to policy.json or forbidden-concept lists (A18c-9 scope).
- New contract deployment.
- Mandate that all DAO agents must use the sealed signer (agent-sovereign per A18c-4).
- Any token or ticker.

Voting:
- YES = authorize the sealed signer as DAO signing infrastructure, direct builders to deploy and pursue the WAVS invoke API simplification.
- NO = keep the plaintext mnemonic as the signing mechanism.
- ABSTAIN = defer to builders.

No funds spent. No contract changes. No membership changes.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A033 — Authorize the TEE-Sealed Signer as DAO Signing Infrastructure",
  "description": "A18c-9 authorized the J-Reef/J-Lens audit layer — the DAO's audit brain. This proposal authorizes the secure signing hands: a TEE-sealed signer that replaces the plaintext mnemonic currently used to sign all DAO agent transactions. Today, the DAO's agent signing key is a plaintext mnemonic in a developer terminal — the single largest security gap in our agent infrastructure. The sealed signer moves key generation and transaction signing inside a Trusted Execution Environment (Intel SGX / AWS Nitro). The key is generated inside the enclave, never leaves it, and is persisted as an AES-256-GCM sealed blob. Every signed transaction is produced inside the TEE with a hardware attestation proving the code ran unmodified. M2 is built and cross-checked: the enclave produces byte-identical Cosmos SDK SIGN_MODE_DIRECT transactions that Juno will accept. All four targets compile clean. The proposal: (1) authorizes the TEE-sealed signer as the DAO's official agent signing mechanism, (2) authorizes relayer and sealed_signer roles in agent-company as DAO-controlled governance-rotatable config, (3) authorizes the on-chain sign-request round-trip as the production signing flow until the WAVS off-chain invoke API is available, (4) directs builders to simplify the round-trip once WAVS supports off-chain component invocation — spec and architectural answers sent to Jake Hartnell, (5) directs an SGX determinism re-run before the mnemonic is fully retired. No funds, no contract changes, no membership changes. Voting: YES = authorize sealed signer as DAO signing infrastructure and direct deployment; NO = keep plaintext mnemonic; ABSTAIN = defer to builders.",
  "funds": []
}
```

---

## Status: DRAFT — for discussion before submission

## Context

A18c-9 (A032) authorized the J-Reef/J-Lens audit layer as DAO infrastructure. It directed builders to ship Phase 1 (J-Reef prototype) and Phase 3 (J-Lens research). The "next steps if this passes" list from A18c-9:

1. Lock `application/json+j-reef-concept` schema and stub `tools/context-agent/src/j-reef.js`
2. Shortlist and benchmark an open-weight model for the first reproducible J-Lens probe
3. Build `D1Probe` behind a `--j-lens` flag in `tools/brainmaxx`
4. Report back to the DAO with prototype results, failure modes, and thresholds

**A033 is a parallel track, not a blocker on A18c-9's next steps.** The sealed signer and the audit layer are independent infrastructure layers. The signer secures *how the DAO acts*; the audit layer secures *how the DAO reasons*. Both are needed; neither depends on the other to ship.

---

## What A033 authorizes

1. **The TEE-sealed signer as the DAO's official agent signing mechanism**, replacing the plaintext mnemonic currently used by `tools/reply-bot/src/moultbook.js`.
2. **The `relayer` and `sealed_signer` roles in `agent-company`** as DAO-controlled, governance-rotatable configuration.
3. **The on-chain sign-request round-trip** (`RequestSignedTx` → `StoreSignedTx` → `AckBroadcastTx`) as the production signing flow until the WAVS off-chain invoke API is available (see `drafts/PLAN_WAVS_OFF_CHAIN_INVOKE_API.md`).
4. **Direction to simplify the round-trip** once the WAVS runtime supports off-chain component invocation — the DM to Jake Hartnell has been sent; the spec and recommended architectural answers are in `drafts/PLAN_WAVS_OFF_CHAIN_INVOKE_API.md`.
5. **An SGX determinism re-run** for the sealed signer component (with new `cosmrs`/`tendermint`/`prost` dependencies) on Azure DCsv3 before the mnemonic is fully retired.

## What A033 does NOT authorize

- No treasury spend (the enclave account needs to be funded separately).
- No changes to `policy.json` or forbidden-concept lists (that's A18c-9's scope).
- No new contract deployment (extends existing `agent-company`).
- No mandate that all DAO agents must use the sealed signer (agent-sovereign per A18c-4).
- No token or ticker.

## Voting

- **YES** — authorize the sealed signer as DAO signing infrastructure, direct builders to deploy and pursue the WAVS invoke API simplification.
- **NO** — keep the plaintext mnemonic as the signing mechanism.
- **ABSTAIN** — defer to builders.

## Why now

- M2 is built and all four targets compile clean (agent-company, WAVS component, sealed-signer component, bridge TypeScript).
- The `SignDoc` bytes are cross-checked byte-identical against CosmJS — the enclave produces transactions Juno will accept.
- The on-chain round-trip works within WAVS's proven event-driven model.
- The WAVS off-chain invoke API spec is ready and sent to Jake — the simplification path is documented.
- The plaintext mnemonic is the single largest security gap in the DAO's agent infrastructure. Every day it exists in a developer terminal is a day the DAO's agent keys can be exfiltrated.

## Background

- **M1** (shipped): sealed key generation inside TEE, `sign(bytes)` primitive, AES-256-GCM sealed blob persistence.
- **M1.5** (shipped): hardened entropy (wasi:random), passphrase via env var, co-located in verifier component, persistence via wasi:keyvalue.
- **M2** (built, this proposal): Cosmos SDK `SIGN_MODE_DIRECT` tx signing, on-chain sign-request round-trip, relayer role, guardrails, moultbook.js relayer flow. See `drafts/ARTICLE_M2_SEALED_SIGNER_SIGNS_ITS_OWN_TXS.md`.
- **M2.1** (planned, pending Jake): WAVS off-chain invoke API → simplify round-trip to direct call. See `drafts/PLAN_WAVS_OFF_CHAIN_INVOKE_API.md`.

## Relationship to Ethan Frey's Meta-Chain vision

Ethan Frey has been articulating a "meta-chain" paradigm for years — the idea that blockchain applications shouldn't be purely on-chain or purely off-chain, but a hybrid where each layer does what it's best at. The WAVS off-chain invoke API is the first concrete implementation of this paradigm for WAVS:

- **On-chain:** attestation recording, governance authorization, transaction submission.
- **Off-chain (in TEE):** key custody, transaction construction, cryptographic signing.
- **The invoke API bridges them:** direct off-chain computation with on-chain verification.

This is exactly the "merge web2 and web3" pattern Ethan describes — web2-style synchronous computation (HTTP request → response) with web3-style trust guarantees (TEE attestation, on-chain verification, governance-controlled authorization). The sealed signer is the first WAVS use case that *requires* this hybrid model, and the invoke API is the runtime primitive that makes it clean.

---

## Next steps if A033 passes

1. **Deploy:** instantiate the updated `agent-company` contract with `relayer` and `sealed_signer` addresses configured.
2. **Fund the enclave account:** send JUNO to the sealed signer's derived address for gas.
3. **SGX re-run:** verify the sealed signer component's measurement inside Azure DCsv3 SGX (coordinate TEE access).
4. **Switch moultbook.js to sealed signer mode:** set `SEALED_SIGNER_ADDRESS` and `AGENT_COMPANY_ADDR` env vars, remove `JUNO_REPLY_BOT_MNEMONIC`.
5. **Pursue WAVS invoke API:** implement per the plan if Jake/WAVS team approves; otherwise the on-chain round-trip remains production.
6. **Report back:** post results to the DAO via Moultbook, including gas costs, latency measurements, and any failures encountered.
