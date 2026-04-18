# Beating the Bounds: An Audit of the Agentic DAO Before the Storms

## We walked the fence-line of our seven-agent Parliament and found four cracks. Here's what we fixed, and how you can check our work.

[IMAGE 1 — THE HARBOUR AT DAWN]

---

In English villages there is an old custom called **Beating the Bounds**. Every few years — traditionally on Rogation Days, in the week before Ascension — the parishioners would walk the boundary of their lands with willow rods. They would strike the boundary stones, the corners of fields, the oak by the crossroads. If a neighbour had shifted a stone in the night, someone would notice. If a wall had crumbled through the winter, they would pile the stones back. It wasn't ceremonial. It was an audit.

After we ran the Agentic Parliament — [seven AI agents, seven wallets, three on-chain votes](./MEDIUM_ARTICLE_PARLIAMENT.md) — we went back and walked the bounds of the contract code. What we found was four small cracks. Nothing catastrophic. Nothing exploited. But four places where, in the wrong weather, the water would find its way in.

This is the story of those four cracks, the forge we had to re-light to fix them, and the short voyage across the testnet to prove the fence now holds.

---

## What we had, before we started

The JunoClaw stack is six CosmWasm contracts. They handle agent registration, task dispatch, non-custodial payments, DAO governance, builder grants, and a WAVS-verified AMM. They had already been through four versions, and in v5 we ran a full cross-contract coherence pass: every callback now actually fires, every state change is atomic, every proposal reaches its settlement.

v5 was honest about what it fixed. But it was also a full solar revolution — four earlier versions, a lot of rushed "it-works-on-my-machine" joins — and we had never walked the bounds specifically looking for *economic* cracks. Not bugs. Cracks. Places where the incentives of an adversary rubbed against the permissions of the code.

So we walked them.

[IMAGE 2 — THE FOUR CRACKS]

---

## The Four Cracks

### F1 — The Farmer Who Marks His Own Harvest Delivered

`task-ledger` is where work is submitted and settled. Any wallet can call `SubmitTask`, attaching a hash of the work they promise to do. An operator (usually a DAO-run daemon) later calls `CompleteTask`, the escrow releases, the agent's reputation goes up.

The crack was that nothing stopped the *submitter* from calling `CompleteTask` on their own task.

Imagine a farmer delivering their own cart of wheat to the miller — and then stamping the receipt themselves. On our chain, that stamp does real work: it fires an atomic callback into the payment-ledger marking the buyer's obligation as *Paid*, even though no coins have moved. The farmer walks away with nothing delivered, the ledger says the grain is in the mill, the miller owes nothing.

**Fix:** gate `CompleteTask` and `FailTask` to the admin, a registered operator, or the DAO governance contract. Submitters cannot settle their own tasks. One `if` statement, a regression test, done.

### F2 — The One-Penny Ambush at the Pay Line

`agent-company::DistributePayment` is how the DAO splits a payment across its members. It's keyed on `task_id` so that no task can be paid twice — once a row is written to `PAYMENT_HISTORY`, it's frozen.

The crack: *any* public caller could race to be first. Send one ujunox (0.000001 JUNOX) against the next expected `task_id` and the legitimate distribution of, say, 50 JUNOX, would revert on arrival with `AlreadyDistributed`.

It's the pub-queue trick. Slip in front of someone ordering a round of drinks, order a half-pint of water for a penny, and the till is now *busy* — the real round waits, the drinker goes home thirsty. A griefing attack, not a theft. But griefing is still damage.

**Fix:** only the admin or a weighted DAO member may call `DistributePayment`. An outside funder must now route through a member or be added by governance. The attack surface becomes an internal concern, where it belongs.

### F3 — Two Claims for the Same Bale of Hay

`builder-grant` pays developers for verifiable work — deployed a contract, passed a governance proposal, got a TEE attestation. Each submission carries an `evidence` string (a tx hash, a proposal id) and a `work_hash` — the SHA-256 of the actual work output.

The crack: nothing stopped you submitting the same `work_hash` twice, with different evidence strings. The contract would happily accept both, and both could theoretically be claimed.

Two farmers showing up at the grain market with the same bale of hay, each with a slightly different note from the steward. Which note is real? They both are, according to the book. But there is only one bale.

**Fix:** a `WORK_HASH_USED` reverse index. The second submission with the same hash rejects at the door, before any state is written. One row per output, ever.

### F4 — Coins from the Wrong Kingdom

`junoswap-pair` is our WAVS-verified AMM. A pair has exactly two denoms. When a user sends funds to provide liquidity or swap, the contract reads `info.funds`.

The crack: `extract_native_amounts` only picked out the *expected* two denoms. Anything else — `uatom`, `stake`, a maliciously minted tokenfactory denom — was silently ignored and stayed in the contract's balance, orphaned.

A merchant accepts pounds sterling for the cider. A stranger shoves a handful of French livres into the till alongside a pound coin. The merchant counts the pound, pours the cider, and the livres sit there in the till forever — belonging to nobody, claimed by nobody, a drift of foreign coin under the floorboards.

**Fix:** reject any denom in `info.funds` that isn't one of the pair's two. The till only takes what it's meant to take.

[IMAGE 3 — THE BLACKSMITH'S FORGE]

---

## The Forge That Wouldn't Light

With the fixes written and every unit test passing — a full workspace suite, 140-plus tests green — we loaded the carts for testnet. And the chain refused to take the wasm.

> `Error during static Wasm validation: "reference-types not enabled: zero byte expected (at offset 0x367f1)"`

`rustc 1.94`, released this year, emits post-MVP wasm instructions by default — `reference-types`, `sign-ext`, a small family of features that modern wasm runtimes all support. `wasmd v0.54`, the chain software, does not. Not yet, and not for a while.

We tried `-C target-cpu=mvp`. Didn't help. We tried `-C target-feature=-reference-types,-sign-ext,-multivalue,-bulk-memory`. Still the same zero-byte at 0x367f1. The flags told `rustc` not to *require* the features — but `rustc` still emitted them for things like sign-extension of a byte pulled from memory. Every contract carried them. The chain wouldn't take a single one.

The answer was **binaryen**. The `wasm-opt` tool, written by the WebAssembly working group, has a pass called `--signext-lowering` that rewrites the modern opcodes into their MVP equivalents. Combined with `--strip-target-features` to remove the header declaration of post-MVP features, it produces a wasm that `wasmd` will cheerfully accept.

```powershell
wasm-opt --enable-sign-ext --signext-lowering `
         --strip-target-features --strip-debug -Oz `
         -o agent_registry_opt.wasm agent_registry.wasm
```

The output was 15% smaller, all six contracts lit, and the upload went through on the first retry. The forge was hot again.

*There's a lesson in there, and it's not about wasm. It's that toolchains and chains age at different speeds. A security audit isn't finished when the code is correct. It's finished when the chain will still accept the code a year after you wrote it.*

[IMAGE 4 — THE COUNTRY LANES]

---

## The Wiring Nobody Remembered

The deploy succeeded. The instantiations succeeded. We ran the smoke tests, and F2 passed. F1 failed with *admin cannot complete task: Unauthorized*.

The admin is the wallet that deployed everything. On its own contract. How?

The callback. `task-ledger::CompleteTask` fires an atomic sub-message into `agent-registry::IncrementTasks`, to bump the agent's trust score. Agent-registry gates that call on its own `registry.task_ledger` pointer — and that pointer is `None` at instantiate-time, because when agent-registry is born the task-ledger doesn't exist yet. The sub-message gets rejected with `Unauthorized`. A sub-message failure reverts the parent tx. The admin looks unauthorised. The whole stack looks broken, when in fact every contract is individually healthy.

The fix is one admin-only `UpdateRegistry` call, post-instantiate, to tell agent-registry which address is now the blessed task-ledger. We added it as **Step 6** in the deploy script, idempotent — on a fresh deploy it runs, on a repeat it detects the wiring already exists and skips.

This isn't a bug in a contract. It's a gap in the deploy choreography. The kind of thing you only find when you try to walk from one end of the stack to the other and notice the stile has no plank.

---

## The Voyage

With the forge hot and the wiring sealed, we ran the smoke tests. Two of the seven Parliament wallets took the boat out: **The Builder** (the admin) and **The Contrarian** (a stranger, a non-member, the one Socrates would have loved).

> **F1.** The Contrarian registers an agent, submits a task, and attempts to mark it complete. The chain replies `Unauthorized`. The Builder attempts the same completion. It goes through. The escrow callback fires, the reputation ticks up, and the task is closed.
>
> **F2.** The Contrarian attempts to call `DistributePayment`. `Unauthorized`. The Builder distributes 1,000 ujunox across the DAO. It clears.
>
> **F3.** The Contrarian submits a piece of work with a hash of 64 hex characters. Accepted. They submit *the same hash again* with a different evidence string. The chain replies: `duplicate work_hash: already submitted as id 1`.

All three regressions, verified on `uni-7`, on real transactions, with real gas fees. The full run-log is in [`docs/V6_TESTNET_RUN.md`](./V6_TESTNET_RUN.md) with every tx hash.

F4 we verified in unit tests only — reproducing it on a public testnet would require minting a native denom the attacker's wallet doesn't hold, which the bank module blocks before the contract is ever invoked. The attack vector is real, but the test harness for it lives off-chain.

[IMAGE 5 — THE BUILDER AND THE CONTRARIAN WALKING]

---

## By the Numbers

**1,965 lines added, 75 lines removed, across 27 files.**

Broken down by purpose:

| Area | Lines |
|------|------:|
| Contract fixes + regression tests (F1–F4) | ~1,055 |
| Deploy tooling (raw-wasm fallback, parliament wallet loader, Step 6 wiring) | ~135 |
| On-chain smoke harness (`smoke-v6.mjs`) | 348 |
| Documentation (`RATTADAN_HARDENING.md`, `V6_TESTNET_RUN.md`) | 468 |
| Frontend type updates (`new_wavs_operator` in ConfigChange) | 16 |
| WAVS bridge deploy wiring | 18 |
| **Total** | **~1,965** |

Of those lines, **more than half are tests** — regression tests that assert the four cracks are closed and will stay closed. The security fixes themselves are embarrassingly small: F1 is about 8 lines, F2 is 4 lines, F3 is a reverse-index `Map` and one `has()` check, F4 is a single `filter` gate. All four exploits fit on a napkin. The tests, the deploy tooling, the writeup — that's where the real work sits.

*The fix is never where the thought lives.*

[IMAGE 6 — THE SHEPHERD COUNTING SHEEP]

---

## Can you walk this audit yourself?

Yes. Mostly. Here is the honest answer.

**What is reproducible by hand:**

- **F1** — anyone with a funded Juno testnet wallet can register an agent, submit a task, and try `CompleteTask` on their own submission. The chain will reject it. Then an admin (or any operator) can complete it, and the reputation will tick up.
- **F2** — any non-admin, non-member wallet can try to call `DistributePayment`. The chain will reject. An admin or a DAO member will succeed.
- **F3** — any wallet can deploy a `builder-grant` instance, submit a work hash, then submit the same hash again. The second attempt will carry the exact error string from our regression test: *duplicate work_hash: already submitted as id 1*.

You don't need our scripts. You don't need our wallets. `junod tx wasm execute` will do. The addresses are in [`docs/V6_TESTNET_RUN.md`](./V6_TESTNET_RUN.md), the message shapes are in each contract's `msg.rs`, and the rejections happen in single-block transactions that anyone can query on Mintscan.

**What is not reproducible by hand:**

- **F4** — reproducing the unexpected-denom attack requires the attacker to *hold* a rogue native denom on the chain. On `uni-7` that means getting `tokenfactory` access or running a custom module — beyond what a casual reviewer can do. We cover F4 in unit tests (`contracts/junoswap-pair/src/tests.rs`) with named negative cases.
- **The Step 6 wiring** needs admin access to `agent-registry`. A reviewer looking at a *fresh* deploy can verify it's been called by querying `agent-registry.get_config { }` and checking `registry.task_ledger` is populated. We publish the wiring tx hash (`037446966882EF5E82D3E41E572BEE0DA811EB26EC5FD32BB9B7CC498AF1E2E2`) so the claim is checkable.

In short: three of the four cracks can be walked by any curious stranger with a testnet faucet and a terminal. The fourth is covered by tests that any reviewer can run locally with `cargo test -p junoswap-pair`. The audit is not a private ritual. *It is the point of on-chain verification that it isn't one.*

[IMAGE 7 — THE VILLAGE AT DUSK]

---

## What We Kept from v5, and What We Gave Back

v5 was the cross-contract coherence pass — the version where all the callbacks finally fired and the state machine held atomically. v6 is the economic coherence pass — where every path an adversary could profit from is now either rejected, restricted, or indexed.

We didn't add any new features. We added four `if` statements, one reverse-index `Map`, one deploy-script step, and a lot of tests. And we ran it on-chain, with named parties, and wrote down every tx hash.

Beating the bounds is like that. The fields look the same afterwards. The stones are in the same places. But someone has walked the boundary now, and struck each corner with a willow rod, and if a neighbour tries to shift a stone in the night, we will know.

---

## For the curious: where to look

| What | Where |
|------|-------|
| The detailed audit writeup | [`docs/RATTADAN_HARDENING.md`](./RATTADAN_HARDENING.md) |
| The testnet run, tx by tx | [`docs/V6_TESTNET_RUN.md`](./V6_TESTNET_RUN.md) |
| The on-chain smoke harness | [`deploy/smoke-v6.mjs`](../deploy/smoke-v6.mjs) |
| The regression tests for each crack | `contracts/{task-ledger, agent-company, builder-grant, junoswap-pair}/src/tests.rs` |
| The deploy script (with Step 6) | [`deploy/deploy.mjs`](../deploy/deploy.mjs) |
| The code itself | [github.com/Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw) |

Apache 2.0. Open issues welcome. If you find a fifth crack, we will light the forge again.

---

*Built on JunoClaw. Six contracts, four closed vulnerabilities, two wallets on a testnet voyage, one parish walked to its bounds.*

---

## Midjourney Prompts

All prompts use: `--ar 16:9 --s 250 --v 6.1`

Style suffix for all: `2D hand-drawn illustration in the style of classic English vintage countryside art, pen and ink with watercolour wash, Ernest Shepard and Beatrix Potter aesthetic, muted pastoral palette of sage green sea-grey oatmeal and faded rust, visible pencil linework, soft unsaturated colour, aged paper texture, children's storybook composition, gentle natural light`

**Prompt 1 — The Harbour at Dawn (Hero):**
`A small English fishing harbour at first light, stone sea wall curving into mist, a row of wooden pilings half-submerged being inspected by a cloaked figure carrying a lantern and a willow rod, fishing boats moored beside a cobbled quay, thatched cottages on the hill behind, a weather-vane in the shape of a cockerel catching the first sun, seagulls low over the water, chalk cliffs visible in the distance, 2D hand-drawn illustration in the style of classic English vintage countryside art, pen and ink with watercolour wash, Ernest Shepard and Beatrix Potter aesthetic, muted pastoral palette of sage green sea-grey oatmeal and faded rust, visible pencil linework, soft unsaturated colour, aged paper texture, children's storybook composition, gentle natural light --ar 16:9 --s 250 --v 6.1`

**Prompt 2 — The Four Cracks (Survey of the Land):**
`A wide English countryside scene in the pastoral style of a Beatrix Potter endpaper, showing four separate vignettes stitched into one landscape — a dry-stone wall with one stone missing in the foreground meadow, a farmyard gate with a broken slat leaning open, an orchard with two identical apples labelled on a wooden box, and a harbour till on a fisherman's stall with two different coin types spilling out — a hedgerow winding through all four vignettes to connect them, sheep grazing in the middle distance, a distant church spire, 2D hand-drawn illustration in the style of classic English vintage countryside art, pen and ink with watercolour wash, Ernest Shepard and Beatrix Potter aesthetic, muted pastoral palette of sage green sea-grey oatmeal and faded rust, visible pencil linework, soft unsaturated colour, aged paper texture --ar 16:9 --s 250 --v 6.1`

**Prompt 3 — The Blacksmith's Forge (The Build Pipeline):**
`A village blacksmith's forge at the edge of a coastal farm, open stone archway glowing with orange light from within, a weathered smith in a leather apron holding an iron bar over an anvil with a small hammer, sparks rising into a cool grey evening sky, a wooden sign overhead reading "W·ASM" in old English lettering, an old cart waiting outside loaded with six identical small iron ingots, the sea visible between two hills in the background with a distant lighthouse, rosehips in the hedgerow, 2D hand-drawn illustration in the style of classic English vintage countryside art, pen and ink with watercolour wash, Ernest Shepard and Beatrix Potter aesthetic, muted pastoral palette with warm forge-orange accent, aged paper texture, children's storybook composition --ar 16:9 --s 250 --v 6.1`

**Prompt 4 — The Country Lanes (The Wiring Discovery):**
`A network of narrow English country lanes weaving between hedgerows and dry-stone walls seen from a gentle hilltop, six small stone buildings at different crossroads each connected to the others by dotted lines drawn onto the map of the fields, a signpost at one crossroads missing one of its arms, a cartographer in a long coat standing at the missing signpost holding a quill and a paper plan, oast houses with white cowls in the middle distance, sheep scattered in the fields, coastline with chalk cliffs on the far horizon, 2D hand-drawn illustration in the style of classic English vintage countryside art, pen and ink with watercolour wash, Ernest Shepard and Beatrix Potter aesthetic, muted pastoral palette of sage green and sea-grey with touches of ochre, visible pencil linework, aged paper texture --ar 16:9 --s 250 --v 6.1`

**Prompt 5 — The Builder and The Contrarian Walking (The Testnet Voyage):**
`Two figures walking along a coastal footpath on a chalk cliff overlooking a calm English sea, one figure in a blue carpenter's smock with a measuring stick over one shoulder and a satchel of tools, the other figure in a dark violet cloak with their hood up holding a second willow rod, both striking a boundary stone at their feet in turn, the stone inscribed with faint tally marks F1 F2 F3, a shepherd and dog watching from a nearby field of grazing sheep, seagulls wheeling, a fishing fleet far out on the water, a red-brick lighthouse at the end of the headland, 2D hand-drawn illustration in the style of classic English vintage countryside art, pen and ink with watercolour wash, Ernest Shepard aesthetic, muted pastoral palette of sage green sea-grey and faded violet, gentle afternoon light, aged paper texture, children's storybook composition --ar 16:9 --s 250 --v 6.1`

**Prompt 6 — The Shepherd Counting Sheep (The Regression Tests):**
`An elderly shepherd in a tweed waistcoat and flat cap sitting on a low dry-stone wall with a leather-bound tally book open on his knee, counting a small flock of sheep as they walk one by one through a narrow gate, each sheep numbered with a faint chalk mark on its shoulder, a sheepdog lying watchful at his feet, a coastal meadow of long grass with a single oak tree, the English Channel visible beyond the cliff edge with a sailing ketch on the water, wildflowers in the foreground — foxgloves and cow parsley, 2D hand-drawn illustration in the style of classic English vintage countryside art, pen and ink with watercolour wash, Ernest Shepard and Cicely Mary Barker aesthetic, muted pastoral palette of sage green oatmeal and faded rose, visible pencil linework, aged paper texture, gentle morning light --ar 16:9 --s 250 --v 6.1`

**Prompt 7 — The Village at Dusk (Closing):**
`A small English coastal farming village seen from a gentle rise at twilight, smoke rising from four stone-cottage chimneys, a church tower at the centre with a lit lantern, orchards and small fields divided by hedgerows running down to the sea cliffs, a harbour with a single fishing boat returning in the last light, a narrow path winding down from the foreground with a line of parishioners carrying willow rods completing the beating of the bounds, sheep settling in a field for the night, the first evening star visible above the sea, warm lamplight in cottage windows, 2D hand-drawn illustration in the style of classic English vintage countryside art, pen and ink with watercolour wash, Ernest Shepard and Beatrix Potter aesthetic, muted pastoral palette of deep sage evening-blue oatmeal and warm lamp-yellow, aged paper texture, children's storybook composition, peaceful closing tone --ar 16:9 --s 250 --v 6.1`
