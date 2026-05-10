# Mesh Security & tiablob — audit-aware constraints for JunoClaw

*Status: scoping note. Captures the constraint Jake mentioned in the May 2026 Twitter Spaces, before any Mesh- or tiablob-adjacent work is committed.*

*Owner: VairagyaNodes. Audience: any contributor about to scope work that touches Mesh Security, Celestia, or tiablob.*

---

## TL;DR

> **Mesh Security is not yet audited. Estimated audit cost ~$200k.** Until that audit lands, JunoClaw will not ship any feature whose **safety property** depends on Mesh primitives. We *will* design optional adapters so that a future Mesh-on-Juno deployment is a one-config-switch addition, but the default code path stays Mesh-free.

The same posture applies, more weakly, to anything we contribute upstream that touches Celestia DA via tiablob.

---

## 1. What Jake said (paraphrased)

In the Spaces with the netadao creator and Cybernetics members:

- Mesh Security is the obvious next scaling primitive for Cosmos-aligned chains that want shared security without merging into one validator set.
- The Mesh codebase is **not yet audited**. The audit estimate is **~$200k** and has not been funded or scheduled at the time of speaking.
- Several teams are designing against Mesh anyway, on the assumption an audit lands within the year.

For our project that translates into a hard constraint, not a planning suggestion.

## 2. Why this matters for JunoClaw specifically

Two adjacencies bring us close to Mesh / tiablob:

1. **Validator-sidecar / scaling track.** `01_VALIDATOR_SIDECARS.md` already sketches a posture where JunoClaw nodes can run alongside Juno validators. The natural next step is "shared security" — provider chain, consumer chain, restaking. Mesh is the obvious vehicle. We must not commit to that path while Mesh is unaudited.

2. **Off-chain DA for the Moultbook layer.** `AI_DAO_FRAMING_AND_MOULTBOOK.md` §4c proposes putting the **bulk** of agent knowledge on a DA layer (most likely Celestia) with chain-stored commitments. tiablob is the bridge that makes Celestia DA queryable from Cosmos chains. **Tiablob itself is healthier than Mesh** (it has more upstream attention) but a Celestia PR from us still needs to be careful about what assumptions it bakes in.

## 3. Posture rules

These are project-wide rules, not suggestions. Any PR that violates one needs an explicit "Mesh waiver" approval line in the PR body.

### 3.1 Default code paths are Mesh-free

Every contract, every operator, every script must function correctly when Mesh modules are not enabled on the host chain. Mesh integration, if any, is a feature flag (`features = ["mesh"]`) that compiles out cleanly. The non-Mesh build is the audited build.

### 3.2 No safety property may depend on Mesh

A "safety property" here is anything we promise in a Medium article, a HackMD, a governance proposal, or a Ffern audit response: e.g. "agent slashing is enforceable", "task escrow is unrecoverable by the operator". None of those promises may rely on a Mesh-side check. They must hold against a stock `wasmd` chain.

### 3.3 Liveness properties may use Mesh under a flag

Liveness ("agents reach quorum within N blocks", "the bridge can recover after a 5-block outage") may use Mesh primitives **if** the alternative non-Mesh code path is also tested and shipped. This lets us write Mesh adapters today without staking the audit on them.

### 3.4 Documentation discipline

Any doc that mentions Mesh must include the audit-status footnote. Until the audit lands, the footnote text is:

> **Mesh Security audit status (as of May 2026): not yet audited; estimated cost ~$200k; not on the JunoClaw critical path.**

### 3.5 Upstream PR discipline (Celestia / tiablob)

If we contribute a tiablob or Celestia PR — most likely as a result of the Moultbook off-chain DA layer — the PR must:

- Not change Mesh-related code surface (out of scope).
- Add or update tests against a non-Mesh chain config first.
- Cite the JunoClaw use case in the PR description, so reviewers understand why we care.
- Ask, in the PR description, whether maintainers want the change gated behind a feature flag.

## 4. Concrete current scope

### 4a. What we are scoping (audit-aware)

- **Moultbook v0 off-chain DA backend.** Pluggable; Celestia is one option, plain S3+Merkle is another. No Mesh code path. (`AI_DAO_FRAMING_AND_MOULTBOOK.md` §4c.)
- **Validator sidecar liveness adapter.** A read-only hook that lets a JunoClaw operator subscribe to Mesh-style cross-chain events *if available*, falling back to native IBC events otherwise. Behind a feature flag.
- **Documentation.** This file plus footnote in any Mesh-mentioning doc.

### 4b. What we are not scoping

- Restaking-style shared security between JunoClaw and any other chain.
- Mesh-as-coordinator for agent reputation portability across Cosmos chains.
- Any audit-pending Mesh contract upstream PR.

These are **good ideas** to pick up the day after the Mesh audit publishes. Until then, they live in `OPEN_ENDS.md` only.

## 5. Trigger conditions to revisit this note

Update this file when **any** of the following happens:

- [ ] Mesh Security audit is funded *and* scheduled.
- [ ] Mesh Security audit publishes (with or without findings).
- [ ] An IBC-Eureka-equivalent or other shared-security primitive lands on Cosmos with a clean audit.
- [ ] Jake (or another core Juno contributor) signals a different posture in writing.
- [ ] We commit to a Celestia / tiablob PR — at that point, scope §3.5 lifts off the page and into the PR template.

## 6. Authoritative footnote (paste into other docs)

> *Mesh Security audit status (as of May 2026): not yet audited; estimated cost ~$200k; **not** on the JunoClaw critical path. JunoClaw default code paths are Mesh-free; any Mesh adapter is feature-flagged. See `docs/MESH_TIABLOB_CONSTRAINTS.md`.*

---

*Drafted 10 May 2026.*
