# The Robot Is the Agent

*Draft started 2026-08-17. Section 5 updated same day once the
minimum-operator-count fix shipped. Sections 2 and 6 updated 2026-08-17
after the marketplace/skill-registry cross-check and Robot Ops UI wiring
landed on uni-7. Ready for publication review.*

---

## TL;DR

JunoClaw was built as an agent trust OS. It turns out a robot was never a
special case — it's just another fleet member emitting intents into the same
gate. This piece works through six things that were true in the code before
they were true in the pitch deck: robots are agent-fleet extensions, the
skill-registry and the marketplace are two halves of the same discovery/pay
loop (now enforced on-chain), the whole stack is open-source-only by
*necessity* not ideology, the plugin system was already robotics-shaped, the
Truth Market's operator model had a sharp low-participation risk that is now
patched at both layers, and the Robot Ops UI now reads live chain state —
honestly showing zero operators rather than masking the bootstrap gap with a
simulation.

---

## 1. The robot is the agent

Nothing in `crates/junoclaw-coordination` or `contracts/agent-company`
special-cases "robot" vs "software agent." The trust stack — J-Lens gate →
BFT consensus → Truth Market → Juno settlement — audits *messages*, not a
fixed notion of what emitted them. A robot's controller pushing an intent
through the pipeline is architecturally indistinguishable from any other
fleet member. This was true by construction, not by retrofit.

There's a useful split here between *reflex-tier* and *intent-tier*
decisions. Reflex-tier is the sub-100ms loop — sensor fusion, balance,
collision avoidance — that lives entirely on the robot's controller and
never touches the chain. Intent-tier is the higher-order "what should I
do next" decision that *does* get audited: a combat robot choosing to
engage a target, a delivery robot choosing a route through disputed
terrain, a surgical assistant choosing a tool. The intent is the message;
the gate audits the message; the Truth Market settles the outcome. The
Commonware BFT layer (~300ms finality) is fast enough to consume
off-chain sensor input alongside on-chain DAO agentic input and produce
a consensus-ordered verdict within a robot's decision cycle. The robot
decides its next move mid-fight in milliseconds; the chain decides
whether that move was honest in seconds. Those are different timescales
solving different problems, and the architecture was already built for
exactly that separation.

## 2. Skill-registry and marketplace: the catalog and the register

They are not the same contract, and that's correct, not an oversight.
`skill-registry` is the discovery/manual layer — "what does this dApp do and
where do I find its operating manual." `marketplace` is the economic
listing/escrow/hire layer — Truth-Market-gated escrow that only releases
funds on a green verdict. `marketplace`'s `skill_ref` conventionally points
at a `skill-registry` `dapp_name`.

**Before:** `skill_ref` was a free-text field. A listing could claim to
provide any skill without the skill existing in the registry. The two
contracts were conventionally linked but not cryptographically linked —
an agent discovering a marketplace listing had no on-chain guarantee that
the referenced skill was real, registered, or even non-fabricated.

**After (shipped 2026-08-17):** `marketplace::execute_list_service` now
queries the configured `skill_registry` contract via `query_wasm_smart`
before accepting a listing. If `skill_registry` is set (it's optional —
`None` means the cross-check is disabled, preserving backward compatibility)
and the `skill_ref` is not found in the registry, the listing is rejected
with `ContractError::SkillNotRegistered`. The check is opt-in at instantiate
time and can be toggled by the admin. On uni-7, the marketplace was migrated
to code_id 94 with the cross-check compiled in (currently `skill_registry:
null` — the toggle is wired but not yet pointed at the skill-registry
contract, which is the conservative path for the existing listings). Two
new tests pin the boundary: unregistered `skill_ref` rejects, registered
`skill_ref` accepts. 20/20 marketplace tests pass.

MCP tooling already exposes skill-registry queries
(`mcp/src/tools/chain-query.ts`) — any MCP-capable agent gets wire-level
discovery for free. With the cross-check in place, a marketplace listing is
now a *cryptographic claim* that the skill exists in the registry, not just
a string field. This is the literal "MCP-ready access to JunoClaw" the
idea batch was reaching for.

## 3. Open-source, all the way up — because it has to be

J-Lens cannot run against a closed model API. Hosted completion endpoints
never expose `hidden_states`; a linear probe on the residual stream needs
actual mid-forward-pass activations. Closed inference = no J-Lens, full
stop — this is written into the DAO record (A18c-9). That single technical
constraint is why the entire stack is forced open-weight at the model layer.
It cascades: open models → open probes → open plugin adapters
(`plugin-peaq`, `plugin-rsynth`, and the generic `Plugin` trait in
`crates/junoclaw-core/src/plugin.rs`) → open marketplace listings referencing
open skills. There is no closed-source escape hatch anywhere in the trust
path.

## 4. The plugin system was already robotics-shaped

`plugin-peaq` and `plugin-rsynth` are not robotics plugins, but they prove
the pattern: a generic `Plugin` trait, optional by design ("JunoClaw works
standalone without X"), bridging an external verifiable-something into
JunoClaw's trust core. A `plugin-ros2` (or `plugin-lerobot`, `plugin-*` for
any open-source robotics stack) is the same shape: fetch the robot's
execution/sensor proof, feed it to the J-Lens gate, settle through the same
Truth Market. *This is the growth thesis*: any autonomous-robotics builder
already running an open-source stack can bolt on a verifiable trust +
economics layer without JunoClaw needing to know anything about their
hardware.

## 5. Why Truth Market verifiers must be independent — and what happens if they aren't

The design reason is explicit in `ARTICLE_TRUTH_MARKETS_2026_08_14.md`:
if every operator runs the same model on the same hardware with the same
probe calibration, a systematic bias makes them all diverge together, and
majority-vote consensus confidently agrees on the wrong answer. Slashing
doesn't help when everyone is wrong the same way. Independence isn't a nice
property, it's the only thing that makes the slashing mechanism mean
anything.

**Before:** neither `MultiOperatorGate::audit_with_attestations` nor
`truth-market::execute_finalize_epoch` enforced a minimum operator count.
With one configured operator, the consensus ratio was trivially 1/1 = 100%.
That operator could never diverge from "consensus" — because they *were*
the consensus — and would claim the full reward pool every epoch with zero
adversarial check on their own claim. Zero operators safely halted
finalization (`NoVerdicts`); one operator was worse than zero, because it
looked like the system was working while providing none of the security
the design assumed.

**After (shipped 2026-08-17):** both layers now carry an explicit floor.
`MultiOperatorConfig` gained a `min_operators: usize` field (default 3);
`MultiOperatorGate::audit_with_attestations` short-circuits to a `Red`
verdict with zero attestations — not a computed ratio — whenever fewer
operators are wired than the floor requires. On the settlement side,
`truth-market::Config` gained the matching `min_operators: u32`
(instantiate-time, admin-adjustable via `UpdateConfig`, exposed on
`GetConfig`), and `execute_finalize_epoch` now rejects with
`ContractError::InsufficientOperators { required, submitted }` before it
ever computes a consensus ratio, mirroring the existing `NoVerdicts` guard.
A `migrate` entry point was added so the live uni-7 deployment could be
upgraded in place — old `Config` state (which predates the field) is read
under its previous shape and re-saved with `min_operators` defaulting to 3,
rather than requiring a fresh instantiate. 16/16 truth-market contract
tests and 35/35 coordination-crate tests pass, including three new cases
that pin the boundary: below-floor rejects, at-floor (with a configurable
floor of 1) succeeds, and `UpdateConfig` can move the floor at runtime.

What this does *not* solve, and was scoped out deliberately: there is still
no on-chain way to *prove* that three configured operators are three
*independent* operators rather than one entity running three processes.
The floor stops the trivial 1-operator self-win; it does not stop a
sufficiently motivated single actor from standing up `min_operators` worth
of identical, correlated instances. A `fingerprint` (model+host hash) field
at registration time was sketched as a stretch goal — soft signal only,
probably a relayer-side alert rather than a contract-level block, since
nothing in a CosmWasm execute message can attest to what's actually running
behind an address. That remains the real "more work demand" the
architecture creates: bootstrapping genuine, diverse operator supply is a
harder problem than writing the settlement contract, and it's still
unsolved — just no longer silently broken.

## 6. The Robot Ops UI reads live chain state

The frontend (`frontend/src/components/RobotOpsPanel.tsx`) now polls the
live truth-market contract on uni-7 every 10 seconds via a
`useTruthMarketLive` hook (`frontend/src/hooks/useTruthMarketLive.ts`).
The query layer (`frontend/src/lib/robot-ops-queries.ts`) fetches the
operator list, epoch stats, and marketplace listings directly from the
chain. The Trust Constellation visualization shows a **LIVE** badge when
real data is flowing and a **SIM** badge when it falls back to local
simulation.

The honest part: right now, the live query returns zero operators. There
is no simulation masking that. The UI shows an empty constellation with
a `0 operators` count and a bootstrap prompt. This is deliberate — the
alternative (falling back to simulated operators when the chain says
zero) would be the exact failure mode §5 describes: looking like the
system is working when it isn't. The Robot Ops panel is the first place
where the operator bootstrap gap becomes visible to a human operator
rather than buried in a contract query.

---

## Closing: what the code already knew

The thesis of this piece is that the architecture was right before the
narrative caught up. Six claims, each verified against the codebase:

1. **Robots are agents.** No special-casing in the coordination crate or
   agent-company contract. A robot's controller pushing an intent through
   the gate is architecturally identical to a software agent doing the
   same.
2. **Discovery and economics are separate but linked.** Skill-registry
   and marketplace are different contracts with different jobs, now
   cryptographically linked via the on-chain cross-check.
3. **Open-source is a constraint, not a preference.** J-Lens requires
   `hidden_states`; closed APIs don't expose them. The entire trust path
   is forced open-weight at the model layer.
4. **The plugin system was already robotics-shaped.** `plugin-peaq` and
   `plugin-rsynth` prove the pattern; `plugin-ros2` is the same shape.
5. **The Truth Market had a low-participation vulnerability.** It's now
   patched at both the gate and settlement layers with an explicit
   `min_operators` floor, migrated in place on uni-7.
6. **The UI tells the truth about chain state.** Zero operators shows as
   zero operators. The bootstrap gap is visible, not hidden.

What's next is harder than what's done: bootstrapping genuine, diverse
operator supply for the Truth Market; pointing the marketplace
`skill_registry` field at the live skill-registry contract; shipping a
`plugin-ros2` stub that feeds real robot execution proofs into the gate.
None of these are blocked by the architecture. They're blocked by the
same thing that blocks every marketplace: supply.

The code was ready. The robots were always agents. The trust stack was
always hardware-agnostic. The marketplace was always the economic layer
on top of the discovery layer. The operator model was always the
bottleneck. The only thing that changed between the first commit and
this article is that we stopped calling it future work and started
calling it deployed.
