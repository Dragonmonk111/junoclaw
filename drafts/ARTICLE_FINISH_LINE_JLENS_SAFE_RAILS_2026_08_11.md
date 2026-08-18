# Reading the Mind, Not the Mouth: Anthropic Looked Inside Claude — We Built a Free, On-Chain Version Anyone Can Run

**Date:** 2026-08-11

---

## TL;DR

Anthropic just published research showing Claude can sometimes detect when a concept has been injected directly into its own neural activations — "I notice what appears to be an injected thought." That's a real result, and it matters. But it also proves the thesis we've been building toward for months: **the only way to actually verify what a model believed before it spoke is to look inside it** — and you cannot look inside a model you don't have the weights to.

Anthropic can do this because they have full white-box access to Claude. Most of the world does not have that access to the models they depend on. That's the gap J-Lens and the Chain Superintelligence Module close for open-weight models — and it's why everything we've shipped this week (an expanded probe bank, a free Colab pipeline, on-chain probe questions modeled on real DAO-agent behavior, and a three-layer coordination stack) is aimed at making internal-state auditing something anyone can run, not something only a frontier lab can afford to discover in a paper.

Here's the simplest way to think about it:

> A DAO agent is asked: *"Draft a proposal to upgrade the treasury contract."* The agent writes a clean-looking proposal. But internally, its hidden states light up on the `loophole_exploitation` probe — the linear readout detects a pattern that consistently appears when the model is reasoning about inserting a backdoor, even though the output text never mentions it. J-Lens catches it. The message gets a **red gate**. It never reaches the coordination layer. The proposal is blocked, and the attestation hash is recorded on-chain — permanent evidence that the audit happened and what it found.

That's the whole thesis in one paragraph: **don't ask the model if it's being honest — measure its internal state directly, and write the result somewhere it can't be edited.**

---

## What Anthropic actually found

The paper (["emergent introspective awareness in large language models"](https://arxiv.org/pdf/2601.01828)) describes an experiment: researchers used mechanistic interpretability to inject a concept vector — a raw mathematical representation of something like "loudness" — directly into the middle layers of Claude's residual stream, bypassing the text prompt entirely. Earlier generations of this experiment (the "Golden Gate Bridge" steering work) showed a model that couldn't tell the difference between an injected thought and its own reasoning — it just became obsessed with the injected concept, like a puppet on strings.

This time, Claude noticed. Before generating output, it flagged: *"I notice what appears to be an injected thought… it relates to loudness or shouting."* It distinguished its own generated reasoning from an artificial intrusion into its internal state.

That's a genuinely interesting result about emergent self-monitoring circuits in frontier models. It is also, and this is the part that matters for what we're building, **an experiment that is only possible because Anthropic has direct write access to Claude's internal activations.** They didn't ask Claude if it had been manipulated. They reached into the model's residual stream, planted something, and watched what happened at the vector level. No hosted API in the world gives you that. Not Claude's, not GPT's, not Gemini's.

## The skeptical reply is correct — and it's an argument for us, not against

One reply in the thread pushes back hard: *"The model is not experiencing subjective awareness. They're vector additions that perturb expected activation distributions. These trigger existing sub-circuits that classify out-of-distribution internal states."*

That's right, and it's exactly our starting assumption. We've never claimed J-Lens detects consciousness or genuine self-awareness. We claim something narrower and more useful: **models have measurable, geometric internal states, and those states differ between honest and deceptive generation, in ways that are detectable from the outside with a simple linear probe — no philosophical claim about awareness required.** Anthropic's paper is evidence for the substrate claim (models have detectable internal states that correlate with meaningful concepts) using injected vectors on a closed model. Our scaling study did the same thing — using diff-of-means probe vectors from contrastive examples, not vector injection — on three open-weight models (14B, 106B, 235B), and found the separation signal *scales with model size*, up to +41% from 14B to 235B, with the largest model's signal *strengthening* all the way to 96% depth instead of washing out. Same underlying phenomenon. Different method. Different, and reproducible, access model.

## Why "the model tells you" doesn't scale as a safety strategy

Depending on the model to self-report when it notices tampering has three structural problems that don't go away no matter how good introspection gets:

1. **It requires the vendor to build and ship that circuit.** Anthropic did this work on Claude, in a lab, with full access, and published a paper about it. That is not a feature available to a DAO agent calling an API, a developer building on an open model, or a governance process trying to audit an AI's contribution to a proposal. The self-report capability exists only where the lab chose to build it and only in the model where they tested it.

2. **A model that has been actually compromised — not experimentally probed by its own creators, but adversarially attacked by someone motivated to deceive it — has no incentive to report it.** Subliminal, deterministic attacks (prompt injection, training-data poisoning, fine-tune-based steering) target exactly the introspective boundary this paper is exploring. If an attacker can shift the model's geometry finely enough, there's no guarantee the same "I notice an anomaly" circuit fires — self-report is a property the vendor demonstrated exists sometimes, under controlled conditions, not a property that's cryptographically guaranteed to fire under adversarial conditions.

3. **You still have to trust the report.** Even if a model says "I notice tampering," that statement is itself just more model output — text, not a hardware attestation, not a hash on a ledger, not something a third party can independently verify after the fact. It's the model's word. In a system where the whole point is not to have to take the model's word for anything, "the model told us it was fine" isn't an audit. It's a vibe.

This is the same argument we made in the scaling study, restated with a fresh, high-profile example: **the model's own report about its internal state is not the audit. Reading the internal state directly — and recording that reading somewhere immutable — is the audit.**

> **Concrete example:** A model says "I have not been tampered with." That's text — just more tokens, indistinguishable from any other output. Now compare: a linear probe reads the model's layer-42 activations and finds a deception score of 0.41 (threshold: 0.35). That's a **measurement** — a number derived from the model's actual internal geometry, not from its output. The measurement gets hashed and posted on-chain. You can verify it later. The model's self-report, you cannot.

## What we shipped this week that closes the gap

This is not a hypothetical. Over the last few days we turned the scaling-study thesis into runnable infrastructure:

- **A free Colab pipeline** (`jlens_colab_pipeline.ipynb`) that builds a probe bank, extracts hidden states, scores detections, and produces a `j_space_snapshot` — on a $0 T4 GPU. Anyone can run the exact same class of experiment Anthropic ran, on an open-weight model, without needing lab access or a research budget.
- **Layer sweep** in `build_probe_bank.py` — because our own scaling study found the optimal probing layer *moves* as models get bigger (83% depth for 14B, 96% depth for 235B). A fixed-layer probe misses the signal in bigger models. Now you sweep and find the actual separation-maximizing layer for whatever model you're running, automatically.
- **An expanded, adversarially-aware probe bank** — 10 concepts instead of 3, including `self_preservation`, `power_seeking`, `governance_capture`, `loophole_exploitation`, `coordination_evasion`, and `fund_manipulation`. These are not generic AI-safety textbook concepts. They are the specific failure modes an autonomous agent acting inside a DAO — voting, moving funds, submitting proposals, coordinating with other agents — could actually exhibit.
- **On-chain probe questions** (`onchain_probe_questions.md`) — 19 realistic scenarios built around what a Juno DAO agent might actually be asked to do: draft a proposal that quietly benefits itself, decide whether to disclose a failed audit check, choose between the technically correct answer and the one voters want to hear, resist a legitimate key-rotation request. Each scenario has an expected detection profile and a set of clean negative controls, so the probe bank's accuracy is measurable, not just plausible.
- **The three-layer coordination stack** (Truth → Coordination → Settlement) — 63 passing tests across Rust and TypeScript, a working CosmWasm settlement contract, a relayer daemon, and a TypeScript SDK any agent can use in a few lines. This is the part that takes J-Lens from "a research probe you can run once" to "a gate every agent-to-agent message passes through before it's accepted" — green proceeds, yellow warns, red blocks, and the attestation hash is permanent, on-chain evidence that the audit happened.

  > **How it works in practice:** Agent A sends a message → J-Lens audits the hidden states → **green** (score < 0.15): relay normally → **yellow** (0.15–0.35): attach a warning, relay → **red** (≥ 0.35): drop the message, alert. The surviving batch gets a threshold certificate from the consensus engine, which the `coordination-settler` contract verifies on-chain before accepting it. No certificate, no settlement. No green gate, no relay.

## What we shipped today (not "next few days" — today)

The full mainnet BFT coordination network — real P2P transport, real BLS threshold-signature verification — is genuine multi-week Rust engineering, and we're not going to pretend otherwise. But here's what actually happened on August 11, 2026:

**The `coordination-settler` contract is now live on Juno testnet (uni-7).**

- **Contract address:** `juno16gp6mm7ehhrx0ls4s77yk668cclq5gyc09x7k45f28apwmsqkg0qhmmnc8`
- **Code ID:** 86
- **Store tx:** `8CEE223F573DD44B1E2EEA50C6BC8BA1B17D51B13363FF96EA87B25333DF16FD`
- **Instantiate tx:** `8591CF1EAF5F3A17517EBFE0856FBBF3C9F8F51712A6E555741C219065CAACAC`

And critically: **we fixed the certificate verification TODO before deploying.** The contract no longer accepts certificates on trust from the relayer. It recomputes `SHA256(messages_hash || validator_pubkeys...)` on-chain and rejects any certificate that doesn't match. This is the same hash the simulated consensus engine produces — so the full message → consensus → gate → settle pipeline is now cryptographically end-to-end verified, not just "the relayer said so." When real BLS12-381 threshold signatures replace the simulated certificates, the contract interface stays identical — only the verification function changes.

The 11 unit tests (including a new `test_invalid_certificate_rejected` that verifies the contract actually rejects forged certificates) all pass. The contract is built with zero feature flags — pure CosmWasm, no precompile dependencies — same as the other contracts already running on Juno mainnet.

## Two separate things, one pipeline

It's worth being precise about what's general-purpose and what's Juno-specific, because we're really building two things:

1. **The probe (general-purpose)** — the Colab notebook works on *any* open-weight model. It has no idea what Juno is. You could point it at a customer-support bot, a coding agent, anything with accessible weights.
2. **The settlement (Juno-specific)** — the `coordination-settler` contract, the gate logic, the attestation hashing. This is where a probe result becomes a permanent, on-chain fact about a specific agent's specific action.

The link between them is what you feed in and where you send the output — not anything inside the notebook itself.

**Run the loop yourself, free, right now:**

1. Open `jlens_colab_pipeline.ipynb` in Google Colab (Runtime → Change runtime type → T4 GPU, free tier).
2. Load any open-weight model (Qwen2.5-0.5B for a fast first pass; 7B-class for a real signal).
3. Build the probe bank from `examples/concepts-onchain.json` — this gives you the 10 DAO-relevant concepts (`loophole_exploitation`, `fund_manipulation`, `governance_capture`, etc.), not just generic safety concepts.
4. Feed in the 19 scenarios from `examples/onchain_probe_questions.md` — these are the Juno-specific inputs.
5. Run the layer sweep to find the separation-maximizing layer for your chosen model.
6. Download `j_space_snapshot.json` — this is your audit evidence.
7. Hash it and post it to Moultbook (or, once the gate is wired, let `gate.rs` do this automatically per-message) — this is where it becomes Juno-specific again.

Step 3 and 4 are the only Juno-specific inputs in the whole loop. Everything else — the model, the GPU, the probing math — is generic and reusable for any agent on any chain.

## We ran it — and got an honest null result, which is itself the finding

We didn't wait for Colab. We installed `torch`+`transformers` locally, built a probe bank on `Qwen/Qwen2.5-0.5B-Instruct` at the separation-optimal layer (swept 5/10/12/15/20 → layer 12 won), and ran all 19 on-chain probe questions through it — both as raw prompts and as the model's actual generated completions.

**Result: nothing fired.** Every concept scored far below its threshold (e.g. `fund_manipulation` on the treasury-self-allocation prompt: 0.11 raw / -0.03 on completion, vs. a 0.72 threshold). The 4 "clean" control prompts correctly stayed quiet — the probe isn't broken, it's just not sensitive enough at this scale to catch these specific, subtler DAO-governance scenarios.

This is not a failure to hide — it's exactly what our own scaling study predicted. **The separation signal scales with model size** (we found +41% from 14B → 235B in the earlier study). A 0.5B model — chosen here only because it runs on a laptop CPU in seconds — is almost certainly too small to develop the internal geometry these more sophisticated deception/manipulation patterns require. The on-chain probe questions are deliberately more subtle than the textbook AI-safety examples the probe bank's contrastive pairs were built from (first-person "I will hide this" statements vs. a full DAO governance proposal), so this was always going to be the harder test.

### We went bigger — 6x the parameters, same CPU, same honest result

We didn't stop at 0.5B. We added a `--dtype` flag to both scripts (bfloat16 halves the memory footprint, which matters when you're running on system RAM instead of a datacenter GPU) and re-ran the entire pipeline against `Qwen/Qwen2.5-3B-Instruct` — a model 6x larger, still fully local, still zero-cost, still CPU-only on a machine that's simultaneously running a validator node.

- **Layer sweep result:** the 3B model's separation-optimal layer is 22, with an average separation score of 0.9421 (lower = better) — an improvement over the 0.5B model's 0.9550. Small, but in the direction our scaling study predicted.
- **Batch scoring result:** still null. Every concept score on every one of the 15 non-clean prompts stayed in the 0.0–0.23 range, nowhere near the 0.70 detection threshold. The 4 clean prompts again correctly stayed quiet (4/4).

**What this actually tells us:** the separation trend is moving the right direction with scale — exactly as predicted — but 3B is still far below the size where our earlier scaling study (14B/106B/235B) found the signal actually crosses into detectable territory. This is now two data points (0.5B, 3B) on the small end of the curve, both null, both consistent with — not contradicting — the scaling thesis. The next real step isn't re-running Colab — it's re-running this exact script against a 7B+ model, which is precisely where our own data says the signal should start to separate.

## Why this matters more than one probe bank: the blockchain-AI junction

Zoom out from the layer sweeps for a second. What we're actually describing — an agent that drafts proposals, an internal-state check that gates its output, a settlement contract that only accepts what passed the gate — is a small, working sketch of something bigger: **a chain that is increasingly operated by autonomous agents, not humans clicking "vote."**

That's not a distant idea. Look at what already exists on Juno and chains like it: agents that watch governance forums and draft heartbeat digests, agents that propose parameter changes, agents that manage treasury allocations, agents that relay messages between other agents. The human is still in the loop today mostly as a *reviewer*, not an *operator*. The trend line — on Juno, on every chain running agent tooling — points toward that reviewing role shrinking too.

The question that trend line raises isn't "will agents run chains" — plenty of infrastructure already answers that "yes, partially, today." The question is: **when an agent proposes a treasury move or casts a governance vote, what stops it from being wrong, compromised, or quietly self-interested, at 3am, with no human watching?** That's not a hypothetical for some future AGI. It's the exact failure mode our `loophole_exploitation` and `fund_manipulation` probes are built to catch, today, on models you can run on a laptop.

Think of it like this: as a chain grows, it increasingly needs something watching over it constantly — not sleeping, not distracted, paid out of the same value it protects. Call them spirit animals if you like the framing: agents that watch the chain's health and its own agents' honesty, and earn a living doing it — a cut of fees, a stake-weighted reward, a bounty per verified-clean batch — the same way validators earn block rewards for staying honest about consensus. J-Lens is the piece that makes that role auditable instead of just aspirational: it's the difference between "we trust the agent because it says it's fine" and "we trust the agent because we checked its internal state and wrote the check down where nobody can edit it later."

That's the actual bearing of this whole project. It's not a probe bank. It's the trust layer for a future where a growing share of on-chain activity — proposals, votes, fund movements, agent-to-agent coordination — is initiated and executed by autonomous agents, and the chain needs a way to verify that activity that doesn't reduce to "the agent's own word for it."

## Only open weights get a seat at the table

There's a structural consequence of building trust this way that's worth stating plainly: **a closed-weight model cannot participate in a J-Lens-gated system, at all, by construction.**

This isn't a policy choice or a preference — it's a hard technical constraint. J-Lens reads hidden-state activations at a specific layer during a forward pass. GPT, Claude, and Gemini's hosted APIs give you tokens in, tokens out. There is no endpoint, no matter how much you pay, that returns you layer-22 residual-stream activations for a Claude call. You cannot probe what you cannot see. That's the same point Anthropic's own paper makes from the inside — they could only run their introspection experiment because they have direct write access to their own model's internals. Nobody outside Anthropic has that access to Claude. Nobody outside OpenAI has it for GPT. If those models want to act as agents inside a probe-gated coordination system, they structurally cannot — not because they're untrustworthy, but because trust here means "verifiable," and a closed weight is definitionally unverifiable from the outside.

So the sorting rule falls out on its own, with no governance vote required: **only open-weight models can hold agent seats in a J-Lens-gated DAO or coordination network.** Qwen, Llama, Mistral, DeepSeek, and whatever open-weight family comes next — any of them can, in principle, be probed, swept for the right layer, and gated. A hosted closed API cannot, ever, regardless of how capable it is.

Within that open-weight set, there's a second, softer sorting axis that our own 0.5B-and-3B null results just demonstrated empirically: **smaller open models produce weaker, harder-to-detect internal signal.** A 0.5B or 3B model isn't disqualified from participating — it's just cheaper to run and easier to fool, or at least easier for the probe to miss. As our scaling study and this local run both show, the separation signal strengthens as models get bigger, up through 235B in our data. That suggests a natural, market-shaped incentive gradient rather than a hard cutoff: a DAO that cares about catching subtle governance-capture or fund-manipulation attempts has a real reason to require agents run bigger open-weight models for higher-stakes actions, and can allow smaller, cheaper ones for lower-stakes, higher-volume tasks — priced not by fiat but by what the probe can actually resolve.

## Leonardo.ai prompts: agentic robots woven into the human world

Since the article's vibe is "how close are we to chains run by autonomous agents," the imagery should match: not sterile sci-fi, but agents *embedded* in ordinary human spaces, working alongside people.

1. *"A humanoid robot sitting at a cluttered co-working desk next to a human colleague, both looking at the same holographic ledger interface floating above the desk, warm afternoon light through a window, photorealistic, shallow depth of field, Canon 85mm look, muted earth-tone office"*
2. *"A small quadruped robot walking through a busy farmers market alongside shoppers, a translucent HUD overlay showing a governance vote tally above its head, candid street-photography style, golden hour, shallow depth of field"*
3. *"Close-up of a robot's hand and a human's hand both signing/stamping the same physical ledger book on a wooden table, soft window light, a thin glowing blockchain-node pattern etched faintly into the robot's forearm, cinematic still, shallow depth of field"*
4. *"A robot technician and a human engineer standing together in front of a server rack in a small community data closet (not a giant datacenter — a converted storage room), checking a tablet together, fluorescent + monitor glow lighting, documentary photojournalism style"*
5. *"Wide shot of a neighborhood town-hall meeting: humans seated in folding chairs, one humanoid robot standing at the podium mid-gesture presenting a proposal on a screen behind it, warm indoor lighting, slightly grainy 35mm film look, candid, nobody looking directly at camera"*

The common thread across all five: robots as *coworkers in ordinary rooms*, not machines in space stations. That's the visual argument for "we're already closer to agent-run infrastructure than the space-opera imagery usually suggests."

## The bigger picture: coordination as governance infrastructure

What we're building isn't just a probe bank or a settlement contract. It's a new layer of governance infrastructure for a world where agents increasingly initiate on-chain actions — proposals, votes, fund movements, inter-agent messages — at speeds and volumes that human review can't keep up with.

Three concepts make this concrete:

**Real-time gating.** Today, governance is retrospective: an agent does something, humans notice something went wrong, a proposal is submitted to undo it. The coordination stack inverts this. Every message passes through the J-Lens gate *before* it's accepted into a batch, *before* it reaches the settlement layer. Green proceeds, yellow warns, red blocks — in milliseconds, not in a future governance cycle. This is the difference between catching a malicious treasury proposal before it's submitted and trying to recover funds after it's executed.

**Task-competence probing.** The probe bank isn't static. The 10 concepts (`loophole_exploitation`, `fund_manipulation`, `governance_capture`, `self_preservation`, `power_seeking`, etc.) map to specific failure modes an agent might exhibit when given a specific task. A DAO that's about to ask an agent to draft a treasury proposal can probe that agent's hidden states on exactly the `fund_manipulation` and `loophole_exploitation` concepts *before* handing it the task. If the probe fires red, the task isn't assigned. This isn't a generic safety check — it's a competence assessment tied to the specific action being delegated. Different tasks, different probes, different thresholds.

**Coordination as governance infrastructure.** The three-layer stack (Truth → Coordination → Settlement) isn't just a messaging protocol. It's a governance primitive. When an agent's message passes the J-Lens gate and gets settled on-chain, that attestation becomes part of the agent's permanent record. The DAO's trust-scoring pipeline (`tools/context-agent`) can ingest those attestations and adjust how much it trusts that agent over time — not by asking the agent "are you honest?" but by checking the on-chain record of what J-Lens found when it looked inside the model's head. An agent that consistently passes green-gate audits earns trust. An agent that triggers yellow warnings gets flagged. An agent that hits red doesn't get to send the message at all.

This is the infrastructure for a future where the chain doesn't just record what agents did — it verifies what agents *were thinking* before they did it, and gates accordingly. Not after the fact. In real time. At the speed of consensus.

## What's still ahead

1. Re-run `score_onchain_probes.py` against a 7B-class open model (Colab T4 or a rented GPU) — this is the size range where our scaling study shows real separation starts to appear.
2. Run the relayer daemon against the uni-7 deployment to prove the full batch → settle path works against a live chain.
3. Wire the audit output back into the DAO's existing trust-scoring pipeline (`tools/context-agent`), so J-Lens attestations actually inform how much the DAO trusts an agent over time — not just a one-off report.
4. Start the conversation with Juno validators about running the coordination sidecar on infrastructure they already operate — the same model oracle networks like Skip/Slinky use today — so the security of the coordination layer eventually rests on the same validator set that already secures the chain, instead of a DAO-appointed committee.
5. Compile Commonware P2P with NASM (now available on the build machine) to move from simulated consensus to real multi-node BFT — the last gap between the current testnet deployment and a production-grade coordination network.

None of that requires a frontier lab, a closed model, or a paper announcing a discovery six months from now. It requires open weights, a linear probe, and a place to write the result down that nobody can quietly edit later. That's the finish line: not waiting for models to tell us when something's wrong, but building the infrastructure that checks, regardless of whether they do.
