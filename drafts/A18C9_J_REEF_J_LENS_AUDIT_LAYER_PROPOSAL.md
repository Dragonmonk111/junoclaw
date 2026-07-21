# A18c-9 — Authorize the J-Reef / J-Lens Audit Layer as DAO Infrastructure

> Follow-up to A18c-7 (Name the Reef) and A18c-8 (Brainmaxx v0 Informational). This proposal authorizes the next layer: a sovereign, verifiable reasoning-and-audit stack built on top of The Reef. It ratifies the architecture, directs builder work, and locks in governance boundaries before any shared-root code is changed.

---

## Copy-paste box 1: Title

```
A18c-9 — Authorize the J-Reef / J-Lens Audit Layer as DAO Infrastructure
```

## Copy-paste box 2: Description

```
A18c-7 named the memory system "The Reef." A18c-8 documented Brainmaxx v0, the deterministic D0/D2 reasoning shell. This proposal authorizes the next evolution: a J-Reef public concept map plus a J-Lens internal-monitor layer, together forming a sovereign audit brain that reasons over DAO-owned memory.

What that means in plain terms:
- Brainmaxx is the deterministic sandwich: D0 retrieval/rank/gates around a D2 generative draft, with every step exported as an AKB trace.
- J-Lens is a D1 probe that reads the *internal* state of an open-weight model before it emits a draft, looking for forbidden concepts like reward-hacking, deception, or instruction-override. The result is recorded in the trace and can be hardware-attested via WAVS TEEs.
- J-Reef is a public, recomputable concept map derived from Moultbook, Knowledge Moults, and Brainmaxx traces. Any agent can recompute it locally; the DAO can anchor a canonical top-k root on-chain.

Why open-weight models are required: J-Lens probes need access to hidden activations and the ability to run arbitrary linear readouts inside a TEE. Closed-weight API endpoints (OpenAI, Anthropic API, etc.) do not expose activations, cannot run Jacobian probes, and cannot be attested in this way. A closed-weight user can still run Brainmaxx as a deterministic reasoning shell, but cannot claim a J-Lens attestation or use internal steering. Open-weight is therefore not a temporary testing convenience — it is the substrate for the audit layer.

What this proposal does:
1. Authorizes the J-Reef / J-Lens architecture as the DAO's intended sovereign reasoning-and-audit stack.
2. Directs builders to ship Phase 1 (J-Reef prototype) and Phase 3 (J-Lens research / local prototype) from the working plan in `drafts/PLAN_J_REEF_AND_J_LENS.md`.
3. Confirms that The Reef remains agent-sovereign: every agent runs its own Brainmaxx/J-Lens instance; there is no single hosted DAO brain.
4. Locks the governance boundary: any change to the shared `policy.json`, any DAO-wide J-Reef registry, or any token/signal tied to this work requires a separate A18c proposal.
5. Directs that AI-generated commits touching shared-root code must include a WAVS TEE attestation linking the commit hash to an on-chain trace record, starting with the Juno AI agent.

In scope:
- Architecture ratification and build direction.
- Use of existing DAO-owned contracts (moultbook-v0, knowledge-moults, agent-company, WAVS TEE infra) for prototypes and pilots.
- A TEE-attestation requirement for AI-generated commits that modify shared-root code, with the Juno AI agent as the first pilot.

Out of scope (will require future proposals):
- Changes to shared `policy.json` forbidden-concept lists.
- Deployment of a canonical `jreef-registry` contract.
- Any token, ticker, or drip signal tied to J-Reef / J-Lens.
- Mandating that all DAO agents must run J-Lens.

Voting:
- YES = adopt the J-Reef / J-Lens audit layer as DAO infrastructure and direct builders to proceed with Phase 1 and Phase 3.
- NO = do not authorize this direction; keep Brainmaxx v0 as-is.
- ABSTAIN = defer to builders.

No funds spent. No contract changes. No membership changes.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A18c-9 — Authorize the J-Reef / J-Lens Audit Layer as DAO Infrastructure",
  "description": "A18c-7 named the memory system 'The Reef' and A18c-8 documented Brainmaxx v0. This proposal authorizes the next evolution: a J-Reef public concept map plus a J-Lens internal-monitor layer, forming a sovereign, verifiable reasoning-and-audit stack over DAO-owned memory. Brainmaxx is the deterministic D0/D2 sandwich with AKB trace export. J-Lens is a D1 probe that reads hidden states of an open-weight model for forbidden concepts before a draft is emitted, records the result in the trace, and can be TEE-attested via WAVS. J-Reef is a recomputable concept map from Moultbook, Knowledge Moults, and Brainmaxx traces. Open-weight models are required because J-Lens needs activations and TEE-runnable probes; closed-weight APIs cannot provide this, so closed-weight agents can use Brainmaxx as a reasoning shell but cannot claim J-Lens attestation. The proposal: (1) authorizes the J-Reef / J-Lens architecture as the DAO's intended audit brain, (2) directs builders to ship Phase 1 (J-Reef prototype) and Phase 3 (J-Lens research / local prototype) from drafts/PLAN_J_REEF_AND_J_LENS.md, (3) confirms The Reef stays agent-sovereign — no single hosted DAO brain, (4) locks the governance boundary: future changes to shared policy.json, any DAO-wide J-Reef registry, or any token/signal require separate A18c proposals, (5) directs that AI-generated commits touching shared-root code must include a WAVS TEE attestation linking the commit hash to an on-chain trace record, starting with the Juno AI agent. No funds, no contract changes, no membership changes. Voting: YES = authorize and direct Phase 1+3; NO = keep Brainmaxx v0 as-is; ABSTAIN = defer.",
  "funds": []
}
```

---

## TEE-attested AI-generated commits

This proposal turns the audit layer inward onto the DAO's own AI contributors. Any commit that modifies shared-root code and is authored or materially generated by an AI agent — starting with the Juno AI agent that ships fixes like `43f427d` in `CosmosContracts/juno#1202` — must include:

1. **An open-weight model checkpoint** used to generate the diff.
2. **A WAVS TEE attestation** that the same checkpoint ran inside an enclave, with the input context and generated output bound to a trace commitment.
3. **A J-Lens snapshot** of the model's hidden states during generation, scored against the DAO's forbidden-concept probe bank.
4. **An on-chain link** from the git commit hash to the attestation record in `agent-company` or Moultbook.

This does not mean every commit is slowed down or that humans are replaced. It means that when an AI agent touches the chain's core code, the DAO has cryptographic proof of *which model, which probe, which inputs, and which hidden-state result* produced the change. That is the intersection: blockchain verifies the process that AI used to change itself.

Out of scope for this pilot:
- Human-authored commits are not required to carry AI attestations.
- Non-shared-root files (docs, tests, drafts, scripts) are encouraged but not mandated.
- The Juno AI agent may use a closed-weight assistant for drafting, but the *attested generation* of any shared-root diff must run through an open-weight model in a TEE.

## Background

- **A18c-4** (passed): agent-sovereign local bridges; no shared DAO memory engine.
- **A18c-5** (passed): ratified the Mother-Moult and authorized Knowledge Moults.
- **A18c-6** (passed): "propose before you build" for material shared-root changes.
- **A18c-7** (open): names the memory system "The Reef."
- **A18c-8** (open/informational): documents Brainmaxx v0, the deterministic reasoning CLI.
- **2026-07-09 working plan**: `drafts/PLAN_J_REEF_AND_J_LENS.md` maps out J-Reef and J-Lens phases, data models, and governance boundaries.

## Why now

The Reef already stores the DAO's memory. Brainmaxx v0 already reasons deterministically over that memory. The missing layer is (a) a public concept map that turns the memory into a navigable map, and (b) an internal monitor that audits the model doing the reasoning. Both are natural extensions of work already voted on; neither is a surprise or a pivot. Authorizing the architecture now lets builders integrate it without stopping for a vote on every incremental file, while the strict boundary ensures anything that touches shared policy or the treasury still comes back for approval.

## What the complex brain is and is not

**Is:**
- A local, deterministic reasoning pipeline (Brainmaxx D0 → D1 J-Lens → D2 → gates).
- A public, recomputable concept map (J-Reef) derived from DAO memory.
- A hardware-attestable audit signal (J-Lens snapshot) anchored in existing `agent-company` / WAVS infrastructure.

**Is not:**
- A single hosted model that the DAO centrally operates.
- A substitute for human governance or voting.
- A lie detector, mind reader, or oracle. J-Lens reads activation directions, not intentions.
- A token. Any $REEF-like signal stays a future, separate proposal.

## Voting options

- **YES** — authorize the J-Reef / J-Lens audit layer as DAO infrastructure and direct Phase 1 + Phase 3 work.
- **NO** — keep the stack at Brainmaxx v0 only.
- **ABSTAIN** — defer to builders.

## Out of scope

- No treasury spend.
- No contract admin changes.
- No token minted or authorized.
- No mandate that every agent must run J-Lens.

## Next steps if this passes

1. Lock `application/json+j-reef-concept` schema and stub `tools/context-agent/src/j-reef.js`.
2. Shortlist and benchmark an open-weight model for the first reproducible J-Lens probe.
3. Build `D1Probe` behind a `--j-lens` flag in `tools/brainmaxx`.
4. Report back to the DAO with prototype results, failure modes, and thresholds before any shared `policy.json` change is proposed.

## Vote recommendation

**YES** — the memory system is named, the reasoning shell is shipped, and the audit layer is the obvious next step. Authorize it, bound it, and let builders prove it.
