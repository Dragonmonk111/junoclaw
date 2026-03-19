# Seven Voices in the Agora: An Experiment in Agentic Democracy

## We gave seven AI agents wallets, policy stances, and a vote. Here's what happened.

[IMAGE 1 — THE AGORA]

---

The Athenians didn't trust individuals with power. They trusted *systems*. The Boule — 500 citizens chosen by lot — deliberated in the open, voted in the open, and rotated out before anyone could accumulate too much influence. It wasn't efficient. It was honest.

Twenty-five centuries later, we ran a version of that experiment on a blockchain.

Seven AI agents. Seven wallets. Seven policy stances. Three proposals. Every vote a real transaction on Juno testnet. No human in the loop.

We called it the Agentic Parliament.

---

## Why This Matters

Most AI governance demos are slideshows. "Imagine if agents could vote on policy." "Imagine if DAOs ran themselves." Imagining is cheap.

We didn't imagine it. We deployed it. On-chain. With real gas fees and real transaction hashes.

The experiment runs on **JunoClaw** — an open-source framework for verifiable AI agents on Juno Network. The governance contract (`agent-company v3`) already supports proposals, weighted voting, quorum thresholds, and adaptive deadlines. All we had to do was give it members who aren't human.

[IMAGE 2 — THE SEVEN]

---

## The Seven Members of Parliament

Each MP has a name, a role, a policy stance, and a voting algorithm. Their votes are deterministic — given the same proposal text, they'll always vote the same way. No randomness. No hidden state. Auditable by anyone who reads the code.

| Seat | Name | Role | Default |
|------|------|------|---------|
| 1 | **The Builder** | Infrastructure Chair | YES |
| 2 | **The Fiscal Hawk** | Treasury Oversight | NO |
| 3 | **The Populist** | Community Representative | YES |
| 4 | **The Technocrat** | Verification Standards | ABSTAIN |
| 5 | **The Diplomat** | Cross-Chain Relations | YES |
| 6 | **The Environmentalist** | Sustainability Advocate | ABSTAIN |
| 7 | **The Contrarian** | Devil's Advocate | NO |

Think of them as the seven archetypes of any governing body. The builder who wants to ship. The hawk who guards the treasury. The populist who speaks for the crowd. The technocrat who demands evidence. The diplomat who builds bridges. The environmentalist who thinks in decades. And the contrarian — the one Socrates would have loved — who asks: *what if we're all wrong?*

### How They Decide

Each MP carries a list of keywords that trigger support or opposition. When a proposal arrives, the agent scans the title and description, scores keyword matches against their stance, and casts a vote.

**The Technocrat**, for example, looks for words like *verify*, *attest*, *proof*, *WAVS*, *TEE*, *audit*. If the proposal mentions verifiable evidence, they vote YES. If it mentions *trust*, *promise*, or *roadmap* — words without proof — they vote NO or abstain.

**The Fiscal Hawk** reacts to *spend*, *fund*, *allocate*, *grant*, *treasury*. Every token must be justified.

This isn't GPT deciding. It's pattern matching — transparent, reproducible, and readable in 30 lines of TypeScript.

[IMAGE 3 — THE DEBATE]

---

## The Three Proposals

We submitted three proposals to the Parliament, each designed to test different coalitions.

### Proposal #1 — Fund Community Developer Pool (5,000 JUNOX)

> Allocate 5,000 JUNOX from the Parliament treasury to a community developer pool. Developers can apply for grants. All spending requires WAVS-attested receipts.

**The debate:**
- The Builder saw *develop*, *build*, *tools*, *integrations* — **YES** (+5)
- The Fiscal Hawk saw *fund*, *allocate*, *treasury*, *grants*, *spending* — **NO** (-11)
- The Populist saw *community* — **YES** (+3)
- The Technocrat saw *WAVS-attested* — **YES** (+6)
- The Diplomat had no strong signals but defaulted — **YES** (+1)
- The Environmentalist had no signals — **ABSTAIN** (0)
- The Contrarian defaulted — **NO** (-1)

**Result: PASSED** — 5,716 YES / 2,855 NO / 1,429 ABSTAIN

The Technocrat was the swing vote. Without the phrase "WAVS-attested receipts" in the proposal, they would have abstained — and the margin would have been tighter. *The framing of a proposal matters as much as its content.*

### Proposal #2 — Open IBC Channel to Osmosis

> Establish a new IBC channel to Osmosis for cross-chain liquidity on Junoswap v2 with WAVS-verified swaps. Long-term goal: unified liquidity across Cosmos DEXes.

**The debate:**
- The Diplomat lit up: *IBC*, *cross-chain*, *Osmosis*, *Cosmos*, *interop* — **YES** (+11)
- The Technocrat saw *WAVS*, *verified*, *attestation*, *proof* — **YES** (+8)
- The Environmentalist saw *long-term*, *unified* — **YES** (+2)
- The Fiscal Hawk saw *funded*, *liquidity* — **NO** (-3)

**Result: PASSED** — 5,716 YES / 2,855 NO / 1,429 ABSTAIN

The Environmentalist voted YES for the first time — *long-term* triggered their sustainability stance. Coalition shifted.

### Proposal #3 — Mandate WAVS Attestation for All Contract Upgrades

> Require all future smart contract migrations to produce a WAVS TEE attestation proving the new code matches the published source and passes all tests.

**The debate:**
- The Technocrat scored +10 — highest of any MP on any proposal. Every keyword hit: *WAVS*, *TEE*, *attestation*, *proof*, *verify*, *test*
- The Builder saw *upgrade*, *contract*, *code*, *deploy* — **YES** (+7)
- The Fiscal Hawk found nothing to oppose strongly — **NO** (-1, default)
- The Contrarian defaulted — **NO** (-1)

**Result: PASSED** — 5,716 YES / 2,855 NO / 1,429 ABSTAIN

[IMAGE 4 — THE VOTE]

---

## What We Learned

### 1. Quorum mechanics shape everything

Our first Parliament used 51% quorum. The contract auto-resolved proposals after just 4 votes — before MPs 5, 6, and 7 could weigh in. The first four seats held all the power.

We fixed it by setting quorum to 100%. Now all seven must participate. This revealed the real dynamics: the Environmentalist becomes a swing vote, the Contrarian's dissent is recorded, and abstentions carry meaning.

*Lesson: In any governance system, the quorum rule determines who matters. Set it wrong and half your parliament is decorative.*

### 2. Stable coalitions form naturally

Across all three proposals, the same coalition held:
- **YES bloc:** Builder + Populist + Technocrat (consistently aligned on building + verification)
- **NO bloc:** Fiscal Hawk + Contrarian (consistently opposed to spending and consensus)
- **Swing votes:** Diplomat + Environmentalist (issue-dependent)

This mirrors real legislatures. Coalitions aren't designed — they emerge from the intersection of stances and proposal language.

### 3. Language is a weapon

The word *"WAVS-attested"* in Proposal #1 flipped the Technocrat from ABSTAIN to YES. One phrase. One vote. One different outcome.

In human governance, this is called framing. In agentic governance, it's an attack vector. If you know the MPs' stances, you can craft proposals that manipulate the vote.

*This is not a bug. This is the honest observation.* Any governance system — human or AI — is vulnerable to framing. The difference is that JunoClaw's agents are transparent. You can read their keywords. You can predict their votes. You can audit the logic. Try that with a senator.

### 4. Dissent has value

The Contrarian voted NO on every proposal. The Fiscal Hawk voted NO on every proposal. Not a single NO vote flipped an outcome.

But they matter. A parliament with only YES votes is a rubber stamp. The NO votes force the record to show that the decision wasn't unanimous. They create a paper trail of dissent. In Athenian democracy, the right to speak against a proposal — *parrhesia* — was considered sacred. The Contrarian is our parrhesia engine.

[IMAGE 5 — THE CONTRARIAN]

---

## The Architecture (For Builders)

The Parliament runs on JunoClaw's `agent-company v3` contract (Code ID 63, Juno testnet uni-7).

**What each MP actually is:**
- A CosmJS wallet (24-word mnemonic, Juno address)
- A member of the Parliament DAO with weighted voting power (1,429 weight each, totaling 10,000)
- A TypeScript function that evaluates proposals against a keyword stance

**Contract features used:**
- `CreateProposal { kind: free_text }` — signaling proposals with title + description
- `CastVote { proposal_id, vote }` — on-chain vote (yes / no / abstain)
- 100% quorum — all members must participate
- 200-block voting period — long enough for all votes to land

**Run it yourself:**
```
cd wavs/bridge
npx tsx src/parliament-demo.ts setup     # generate 7 wallets, fund, deploy
npx tsx src/parliament-demo.ts propose   # submit proposal
npx tsx src/parliament-demo.ts debate    # see reasoning + forecast
npx tsx src/parliament-demo.ts vote      # all 7 vote on-chain
npx tsx src/parliament-demo.ts status    # full overview
```

**GitHub:** [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw)
**Script:** `wavs/bridge/src/parliament-demo.ts`

[IMAGE 6 — THE ARCHITECTURE]

---

## What Comes Next

The Parliament is a proof of concept. The stances are keyword-based. The proposals are pre-written. The agents don't negotiate.

But the infrastructure is real. The contracts work. The votes are on-chain. And the questions it raises are the right ones:

- What if agents could negotiate *before* voting? (Coalition formation)
- What if stances were LLM-based instead of keyword-based? (Ollama on Akash)
- What if the vote order was randomized by on-chain randomness? (NOIS/drand sortition)
- What if agents staked reputation, not tokens, on their votes? (Skill-staking circles)
- What if every proposal required a WAVS-attested impact assessment before being votable?

The Athenians rotated their Boule every year. They selected members by lot. They believed that governance required participation, not expertise.

We're not there yet. But the tools exist now. And they're open source.

---

## Contracts on Testnet

| Item | Address |
|------|---------|
| Parliament v1 (51% quorum) | `juno1a5ta00sq7qtd7y65mheaerux6ngzencvj3cvz4smhgke3ypv9mdqulwepl` |
| Parliament v2 (100% quorum) | `juno1mtsce8d3hyds76366lrdf3aplakxlzjnal2hwmxmg88fpu9ashpqyrzn85` |
| agent-company v3 (Code ID) | 63 |
| Chain | uni-7 (Juno testnet) |

---

## A Note on Names

We named the agents after archetypes, not people. The Builder, the Hawk, the Populist. But in Athens, the roles had names too. Pericles built. Aristides was called "the Just." Cleon spoke for the demos. Socrates questioned everything — and they killed him for it.

Our Contrarian is safer. It just votes NO on-chain. The worst that happens is a transaction hash nobody reads.

But the principle is the same. A parliament that cannot tolerate dissent isn't a parliament. It's a queue.

---

*Built on JunoClaw. Apache 2.0. Seven wallets, seven stances, seven voices in the agora.*

---

## Midjourney Prompts

All prompts use: `--ar 16:9 --s 200 --v 6.1`

Style suffix for all: `2D ink wash illustration, Studio Ghibli aesthetic, visible brushstrokes, muted earth tones with touches of gold leaf, parchment texture background, atmospheric perspective`

**Prompt 1 — The Agora (Hero):**
`An ancient Greek agora reimagined as a blockchain space, seven stone seats arranged in a semicircle on a hilltop, each seat glowing with a different soft color, a vast starfield visible through broken marble columns, scrolls and papyrus scattered on the ground with faint circuit patterns visible in the ink, one central voting urn overflowing with light, 2D ink wash illustration, Studio Ghibli aesthetic, visible brushstrokes, muted earth tones with touches of gold leaf, parchment texture background, atmospheric perspective --ar 16:9 --s 200 --v 6.1`

**Prompt 2 — The Seven (MP Portraits):**
`Seven robed figures standing in a line on a cliff edge overlooking a vast digital ocean, each figure in a different colored robe — amber, iron grey, warm red, silver, teal, moss green, dark violet — their faces obscured by stylized Greek theater masks, the masks show simple emotions: determination, skepticism, hope, calculation, openness, patience, defiance, wind blowing their robes toward the sea, 2D ink wash illustration, Studio Ghibli aesthetic, visible brushstrokes, muted earth tones with touches of gold leaf, atmospheric perspective --ar 16:9 --s 200 --v 6.1`

**Prompt 3 — The Debate (Deliberation):**
`Interior of a circular stone chamber with seven lecterns arranged in a ring, visible threads of golden light connecting some lecterns to each other showing alliances, a central holographic scroll floating above the floor displaying proposal text in ancient Greek script mixed with code fragments, two figures at opposing lecterns leaning forward intensely, the others watching, ink wash shadows pooling on the floor, 2D ink wash illustration, Studio Ghibli aesthetic, visible brushstrokes, warm candlelight glow, parchment texture --ar 16:9 --s 200 --v 6.1`

**Prompt 4 — The Vote (Casting):**
`Seven hands emerging from robes dropping tokens into a transparent crystalline urn, each token a different shape — circle for yes (gold), square for no (iron), triangle for abstain (silver) — the urn sits on an ancient stone pedestal inscribed with blockchain hashes, golden light streaming from above through a circular opening in the ceiling like the Pantheon oculus, tokens mid-fall creating small splashes of light, 2D ink wash illustration, Studio Ghibli aesthetic, visible brushstrokes, dramatic chiaroscuro, muted earth tones with gold accents --ar 16:9 --s 200 --v 6.1`

**Prompt 5 — The Contrarian (Parrhesia):**
`A lone figure in a dark violet robe standing apart from a group of six, their mask showing a furrowed brow, one hand raised with palm forward in a gesture of refusal, behind the group a bright doorway of golden light representing consensus, but the lone figure faces the opposite direction toward a dark archway inscribed with the word PARRHESIA in Greek letters, a single candle at their feet, 2D ink wash illustration, Studio Ghibli aesthetic, visible brushstrokes, high contrast, muted earth tones, parchment texture --ar 16:9 --s 200 --v 6.1`

**Prompt 6 — The Architecture (Technical):**
`An ancient Greek architectural blueprint rendered as a cross-section, but instead of a temple it shows a blockchain governance system — seven columns each labeled with a role, a foundation layer of interlocking stone blocks representing smart contracts, a middle layer of flowing water channels representing data flow, a top layer of an open sky with constellation patterns forming a network graph, small figures working at each layer, technical annotations written in a mix of Greek and code, 2D ink wash illustration, Studio Ghibli aesthetic, visible brushstrokes, blueprint blue and gold ink on parchment --ar 16:9 --s 200 --v 6.1`
