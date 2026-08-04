# A035 — Mandate: Akash TEE Provider Outreach for Sealed Signer

> Zero funds now. Directs DAO agents to outreach Akash providers. When a provider enables TEE, a follow-up proposal asks for the deployment deposit only.

---

## Copy-paste box 1: Title

```
A035 — Mandate: Akash TEE Provider Outreach for Sealed Signer
```

## Copy-paste box 2: Description

```
A033 authorized the sealed signer as DAO signing infrastructure. The code is built, tested, and E2E proven on uni-7 (tx 4A7384DE...) and cross-platform on bare-metal AMD EPYC. The remaining gap is finding a TEE-capable compute provider.

Akash Network has fully built confidential compute software (AEP-83, Kata Containers, attestation sidecar). Tenants can request tee/type: cpu. But zero providers currently advertise it. We need a provider to flip the switch.

This proposal directs DAO agents to do outreach — and nothing else right now.

What this proposal does:
1. Directs Juno AI, Junoclaw Agent, or Highlander (whichever is available) to post across Akash Discord, Akash forum, and social channels asking providers to enable confidential compute (tee/type: cpu attribute).
2. Directs the same agents to DM individual Akash providers running AMD EPYC hardware (Overclock Labs community, known providers) with a concrete use case: the DAO's sealed signer needs a TEE instance, 1-2 vCPU, 512 MB RAM, minimal traffic.
3. Requires a progress update on Moultbook within 30 days: which providers were contacted, any responses, any provider willing to enable TEE.
4. If a provider agrees to enable TEE, builders draft a follow-up proposal (A036) asking the DAO for the Akash deployment deposit only (~5 AKT ≈ ~$10-15). No funds requested in this proposal.

What this proposal does NOT do:
- No DAO treasury spend. Zero funds.
- No deployment authorization — that comes in A036 if a provider is found.
- No GCP or centralized cloud.
- No changes to sealed signer code or contracts.

Why outreach first:
- Akash confidential compute is built but has zero providers enabled. Supply is waiting for demand.
- The sealed signer is a perfect first use case: lightweight, clear security requirement, crypto-native.
- Free outreach via DAO agents costs nothing and tests whether Akash TEE is viable.
- If no provider responds, nothing is lost. We revisit.

Voting:
- YES = direct DAO agents to do Akash TEE provider outreach and report back in 30 days.
- NO = don't pursue Akash TEE.
- ABSTAIN = defer to builders.

No DAO funds spent.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A035 — Mandate: Akash TEE Provider Outreach for Sealed Signer",
  "description": "A033 authorized the sealed signer as DAO signing infrastructure. Code is built, tested, E2E proven on uni-7 (tx 4A7384DE...) and cross-platform on AMD EPYC. Remaining gap: finding a TEE-capable compute provider. Akash has fully built confidential compute (AEP-83) but zero providers enabled. This proposal directs DAO agents to do outreach only. What it does: (1) directs Juno AI, Junoclaw Agent, or Highlander to post across Akash Discord, forum, and social asking providers to enable tee/type: cpu, (2) directs agents to DM AMD EPYC providers with the concrete use case (1-2 vCPU, 512 MB RAM, minimal traffic), (3) requires a 30-day progress report on Moultbook, (4) if a provider agrees, builders draft A036 asking DAO for the Akash deployment deposit only (~5 AKT ≈ ~$10-15). No funds in this proposal. No GCP. No deployment authorization yet. Why outreach first: tests whether Akash TEE is viable at zero cost. If no provider responds, nothing lost. Voting: YES = direct agents to do outreach and report back; NO = don't pursue; ABSTAIN = defer to builders. No DAO funds spent.",
  "funds": []
}
```

---

## Status: PASSED and EXECUTED
