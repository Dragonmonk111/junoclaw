# A040 — Mandate: Audit Akash Confidential Compute for J-Lens TEE Attestation (Phase D)

> A035 (passed and executed) directed outreach to get Akash to enable TEE-capable providers. That mandate is now substantively achieved at the protocol level: Akash Confidential Compute (AEP-83) is live as of 2026-07-17, with provider software v0.16.0-rc0 shipping per-pod TEE attestation via Kata Containers (AMD SEV-SNP / Intel TDX), including GPU confidential compute (`cpu-gpu` TEE type, VFIO passthrough). This proposal directs the next step for the A039 J-Lens pilot: audit actual Akash providers for GPU TEE capability and, if found, fold Phase D (TEE attestation of the model forward pass) into the pilot. Broader DAO infrastructure TEE integration (WAVS signer, agent-company, agents) is planned as a separate future proposal (A042).

---

## Copy-paste box 1: Title

```
A040 — Mandate: Audit Akash Confidential Compute for J-Lens TEE Attestation (Phase D)
```

## Copy-paste box 2: Description

```
A035 directed builder outreach to get Akash to enable TEE-capable providers. That is now done at the protocol level: AEP-83 ("Confidential Compute via Kata Containers") is Final (completed 2026-07-17), and provider software v0.16.0-rc0 ships per-pod TEE attestation for both CPU (AMD SEV-SNP / Intel TDX) and GPU (cpu-gpu TEE type, VFIO passthrough, CC-on mode) workloads, with an attestation sidecar injected by default.

This proposal directs the concrete follow-through for the J-Lens pilot: now that the protocol capability exists, find and audit actual Akash providers with GPU TEE, and wire TEE attestation into the A039 J-Lens pilot's Phase D.

Scope note: This proposal is narrowly scoped to the J-Lens pilot's TEE needs. Broader DAO infrastructure TEE integration (WAVS sealed signer, agent-company governance execution, agent bots) is planned as a separate future proposal (A042), so each workstream gets its own vote.

What this proposal does:
1. Directs builders to query console-api.akash.network/v1/providers for any provider advertising tee/type: cpu-gpu with sufficient VRAM for the A039 J-Lens pilot (quantized Kimi K3, ~8x H100/H200 class or equivalent).
2. If a suitable provider is found, directs builders to audit the provider's attestation configuration (SEV-SNP/TDX certificate chain, attestation sidecar, measurement verification) and document the audit in the J-Lens README.
3. If a suitable cpu-gpu provider is found and audited, directs builders to fold Phase D (TEE attestation of the J-Lens forward pass) into the A039 pilot — attestation can be requested on the same rented deployment instead of waiting for a separate future step.
4. If no suitable cpu-gpu provider exists yet, directs builders to document the gap and re-check periodically (the protocol capability is live; provider rollout is ongoing).
5. Directs builders to publish the provider audit findings (capability, attestation chain, VRAM, pricing) to the J-Lens README and the heartbeat digest, so the DAO has a clear picture of what's actually available vs what's theoretically possible.
6. Does NOT authorize any DAO fund spend — GPU rental remains self/builder-funded per A039.

In scope:
- Querying and auditing Akash providers for GPU TEE capability (cpu-gpu type).
- Documenting findings (provider name, tee/type, VRAM, attestation chain, pricing).
- If a provider matches: running the A039 J-Lens pilot's Phase D (TEE attestation) on that provider.
- Updating tools/brainmaxx/j-lens/README.md with the audit results.

Out of scope:
- Any DAO treasury spend (GPU rental is self/builder-funded per A039).
- Any change to A039's mandate or scope.
- Any commitment to a specific Akash provider — this is an audit and integration directive, not a procurement decision.
- Non-Akash TEE providers (can be evaluated separately if Akash lacks capacity).
- Broader DAO infrastructure TEE integration (WAVS signer, agent-company, agent bots) — planned as separate proposal A042.

Voting:
- YES = direct builders to audit Akash TEE providers and integrate attestation where available.
- NO = do not pursue TEE attestation; continue without hardware attestation.
- ABSTAIN = defer to builders.

No funds requested. This is a zero-cost mandate that directs builder time toward auditing and integrating a capability the DAO already mandated pursuing (A035).
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A040 — Mandate: Audit Akash Confidential Compute for J-Lens TEE Attestation (Phase D)",
  "description": "A035 directed outreach to get Akash to enable TEE providers. That is now done: AEP-83 (Confidential Compute via Kata Containers) is Final (completed 2026-07-17), provider software v0.16.0-rc0 ships per-pod TEE attestation for CPU (AMD SEV-SNP / Intel TDX) and GPU (cpu-gpu TEE type, VFIO passthrough) with attestation sidecar. This proposal is narrowly scoped to the A039 J-Lens pilot: (1) query console-api.akash.network/v1/providers for tee/type: cpu-gpu providers with sufficient VRAM for quantized Kimi K3, (2) audit the provider's attestation configuration (certificate chain, sidecar, measurement verification), (3) if a match is found, fold Phase D (TEE attestation of the J-Lens forward pass) into the A039 pilot on the same deployment, (4) if no match yet, document the gap and re-check periodically, (5) publish audit findings to the J-Lens README and heartbeat digest, (6) no DAO fund spend — GPU rental stays self/builder-funded per A039. Broader DAO infra TEE (WAVS signer, agent-company, agents) planned as separate proposal A042. In scope: provider queries, GPU TEE attestation audits, Phase D integration if available, documentation. Out of scope: treasury spend, A039 scope changes, non-Akash providers, broader infra TEE. Voting: YES = audit and integrate J-Lens TEE; NO = continue without TEE; ABSTAIN = defer to builders.",
  "funds": []
}
```

---

## Status: EXECUTED — on-chain, mandate active

## Background

- **A035** (passed and executed) — mandated outreach to get Akash to enable TEE-capable providers. The protocol-level capability is now live.
- **A039** (executed) — mandated the J-Lens Kimi K3 pilot. Its on-chain text says "no Akash provider currently advertises tee/type" (true at submission time, 2026-07-28). The post-submission update in the draft notes this is now stale — AEP-83 is live. This proposal (A040) is the formal follow-up: direct builders to act on the new capability.
- **AEP-83** — "Confidential Compute via Kata Containers", status Final, completed 2026-07-17. Provider software v0.16.0-rc0. CPU TEE: AMD SEV-SNP / Intel TDX. GPU TEE: `cpu-gpu` TEE type, VFIO passthrough, CC-on mode. Attestation sidecar injected by default.
- **J-Lens Phase D** — TEE attestation of the forward pass. Previously listed as "future, separate proposal" in `tools/brainmaxx/j-lens/README.md`. If a suitable provider is found and audited, Phase D folds into the A039 pilot rather than requiring a separate future step.

## Why a separate proposal (not just doing it)

A039's on-chain text explicitly said "this pilot runs without hardware attestation." The post-submission update in the draft notes the capability is now live, but the on-chain mandate didn't direct builders to pursue it. This proposal closes that gap: it formally directs builders to audit providers and integrate TEE attestation where available, so the work has a clear DAO mandate rather than being an ad-hoc extension of A039.

## Dependencies

- A035 (passed and executed) — Akash TEE provider outreach; protocol capability now live.
- A039 (executed) — J-Lens Kimi K3 pilot; this proposal enables Phase D if a provider matches.
- Akash provider with `tee/type: cpu-gpu` and sufficient VRAM — existence not yet confirmed; this proposal directs the audit to find out.

## Post-A040 steps (after DAO authorization)

1. Query `console-api.akash.network/v1/providers` for `tee/type: cpu-gpu` providers.
2. If found: audit attestation configuration (SEV-SNP/TDX cert chain, sidecar, measurement verification).
3. If audited and suitable: fold Phase D into the A039 pilot — request TEE attestation on the same rented deployment.
4. Document findings in `tools/brainmaxx/j-lens/README.md` and heartbeat digest.
5. If no provider found: document the gap, re-check in 2-4 weeks, report in heartbeat.
6. Article (if Phase D completes): "JunoClaw's First TEE-Attested J-Lens Probe on Akash Confidential Compute"
