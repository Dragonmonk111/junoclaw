# The Moult: Making Juno Quantum-Safe Without Rebuilding It

> *Two projects, one mission. Change the locks while the house stays open — and
> give every agent of trust a future-proof receipt for the work they do.*
>
> Status: milestone write-up, 2026-06-17. Receipts-first. Built openly with an AI
> coding agent — I say so up front, and I check the numbers before I post them.

---

## A crab story, because that's really what this is

A crab cannot grow inside the shell it already has. So it does a dangerous,
beautiful thing: it **moults**. It backs out of its old armor, stays soft and
vulnerable for a little while, and grows a new, bigger shell around itself. Same
crab. New protection. It never had to stop being a crab to do it.

That is the whole idea behind what I'm building on Juno.

A storm is coming for every blockchain — eventually, large enough quantum
computers will be able to forge the signatures that keep chains honest. The
common reaction is "we need to build a brand-new quantum-proof chain from
scratch." That's one valid answer. Mine is different, and it's the crab's answer:

**Don't kill the chain to save it. Moult it.** Grow new, quantum-resistant armor
*around the chain that already exists* — its accounts, its networking, the
receipts its agents leave behind — without anyone having to stop using it.

This post is the milestone where that stopped being a slogan and started being
code with measured numbers. There are two halves, and I'll keep both honest.

---

## For normies: what's actually going on (60 seconds)

- **The lock problem.** Blockchains prove "this really came from you" with digital
  signatures. Today's signatures (the math behind your wallet) can be broken by a
  future quantum computer. Not today. But "not today" is not a security plan.
- **The two ways to fix it.** (1) Build a new chain with quantum-proof locks from
  day one. (2) Carefully swap the locks on a chain people already use. I'm doing
  (2). The trick that makes (2) safe is **hybrid**: during the swap, the door has
  *both* the old lock and the new lock. An attacker has to pick both. Nothing
  breaks mid-transition.
- **Two fronts.** I'm hardening the chain's own plumbing (codename **Aegis**) and
  the receipts its AI agents leave behind (codename **Fable**).
- **The honest part.** This is real, tested code with real measurements — and also
  unfinished in specific, named ways. I'll point at exactly what's done and what
  isn't. No "we're quantum-safe now!" confetti.

---

## Two projects, one mission

| | **Aegis** | **Fable** |
|---|---|---|
| **What it protects** | The chain's own plumbing: how nodes talk, how accounts sign | The receipts agents leave: credentials, attestations, endorsements |
| **Plain-words version** | Re-key the building's doors and mail system | Give every worker a tamper-proof, future-proof ID badge + signed worklog |
| **Crypto** | ML-KEM-768 (transport) + ML-DSA-44 (accounts), both **hybrid** with the classical primitives | MAYO (tiny post-quantum signatures), in a smart contract |
| **Who it's for** | Juno — and any Cosmos chain, because it lives in the shared CometBFT/Cosmos-SDK layer | Any CosmWasm chain, **today**, no upgrade needed |

Aegis is the moult of the shell itself. Fable is the moult of the creatures
living in it — the agents of trust.

---

## Aegis: re-keying the plumbing (and what I measured)

Every Cosmos chain — Juno included — is built on two shared pieces: **CometBFT**
(how validators network and agree) and the **Cosmos SDK** (how accounts and
transactions work). Make *those* quantum-safe and you've drawn a migration map
for a huge slice of the entire Cosmos ecosystem, not just one chain. That's the
"whole of Cosmos" ambition, stated honestly as an ambition.

I took real forks of both and folded in hybrid post-quantum crypto as an
**additive** layer — the classical path stays byte-for-byte intact, the new path
sits beside it.

### C5 — the networking handshake (done, tested)

When two nodes connect, they do a secret handshake to set up an encrypted
channel. I added a hybrid version: the classic **X25519** key exchange **and** a
post-quantum **ML-KEM-768** key exchange, mixed together so the channel is only
safe if you'd have to break **both**.

The proof it's non-destructive: CometBFT's *existing* adversarial tests (the ones
that try to cheat the handshake) and its *golden* key-derivation tests **all still
pass, unchanged**. New tests cover the hybrid path. One binary runs either mode,
flipped by an environment variable.

### C6 — what does that handshake cost? (measured, not guessed)

I ran the real fork code over real TCP sockets and measured. This is the part
people hand-wave; here are the numbers:

| Link round-trip | Classical handshake | Hybrid handshake | Extra cost |
|---:|---:|---:|---:|
| 0 ms (pure CPU) | 707 µs | 1.078 ms | **+371 µs** |
| 10 ms | 11.27 ms | 16.76 ms | +5.49 ms |
| 50 ms | 51.28 ms | 76.99 ms | +25.7 ms |

**Bytes on the wire:** classical 2,158 → hybrid 5,623 → **+3,465 bytes** per
handshake.

Translation: the quantum-proof math itself costs **under half a millisecond** —
basically free. The real cost is **~3.4 KB of extra data** and **one extra
network round-trip**, and *only when two nodes first connect* — not per block, not
per message. For a node with ~50 peers that's ~170 KB of one-time traffic. As
ADR-006 predicted: **the cost is in bytes, not CPU.**

### D3 — quantum-safe accounts (core done, wiring gated)

I forked the Cosmos SDK and added a **hybrid account key**: your normal
**secp256k1** key (identical to a normal account on the wire) bundled with a
post-quantum **ML-DSA-44** key. The rule is strict and simple: a signature is
valid **only if both halves check out.**

The elegant part: because of how the SDK already verifies transactions, this
plugs into the **existing** machinery with **no new transaction type and no new
signing mode** — it just works through the normal path. Nine out of nine unit
tests pass, including the headline "forging needs *both* keys" test and
"the same seed always rebuilds the same key."

Sizes, for the curious: public key 1,345 bytes, signature 2,484 bytes (bigger
than classical — that's the honest price of post-quantum).

**What's NOT done in Aegis (named, not hidden):** the account work still needs
its serialization/keyring/CLI/gas wiring (that step needs `protoc`/`buf`
tooling), a multi-node devnet RTT run, and — the genuinely hard problem —
**consensus voting itself is still classical.** I am not going to pretend
otherwise. Re-keying the doors and the mail is real progress; re-keying the
voting booth is the boss level, and it's still ahead.

---

## Fable: future-proof receipts for the agents of trust

Here's where the moult metaphor gets literal. JunoClaw is being built as an
**AI-DAO stack**: autonomous agents take on work, prove what they did, earn or
lose trust, write to a shared memory called **Moultbook**, and leave **receipts**
that future agents can verify.

Two words from the project's own vocabulary carry the whole idea:

- **Moult** — an agent sheds its old shell. In practice: it **rotates its keys
  and renews its credential**. Growth and re-keying are the same act.
- **Bud** — an agent buds a child and grants it *weight*: **shared, delegated
  trust**, recorded on-chain in a credential tree. A senior agent vouching for a
  junior, on the record.

"The agents of trust who have moulted and shared trust shall upkeep the chain's
integrity" isn't poetry bolted on afterward — it's a description of how the
`jclaw-credential` contract actually works. **Fable's job is to make those moults
and those vouchings carry post-quantum signatures**, so the trust they record
survives the storm.

The tool for that is **MAYO** — a post-quantum signature scheme with unusually
*tiny* signatures (as small as 186 bytes), which is exactly what you want when the
signature has to ride inside a blockchain transaction.

### P4 — the MAYO gas ladder (measured + reproduced)

I measured what it costs to verify a MAYO attestation on-chain, across three
security levels, comparing pure-WebAssembly against a native **precompile** on my
MAYO-patched Juno fork:

| Variant | NIST level | Pure-Wasm | Native precompile | Speedup |
|---|---|---:|---:|---:|
| MAYO-2 | L1 | 355,932 | 310,391 | 1.15× |
| MAYO-3 | L3 | 456,644 | 257,371 | 1.77× |
| MAYO-5 | L5 | 798,137 | 360,902 | **2.21×** |

The headline: **NIST Level 5 — the same security tier as Falcon-1024 — verifies
on-chain today for under 800k gas** (about the cost of a fancy DeFi swap), and the
native precompile more than halves that.

And the honest footnote, because I promised: I had *hoped* the precompile would be
~7× faster. **It's 2.21× at L5, not 7×.** Why? Once the crypto runs natively, it's
no longer the bottleneck — what's left is the fixed cost of moving a ~5 KB public
key through the contract and hashing it. That's not a disappointment; it's a
*finding*. It tells me the next optimizations (gas tuning, batching, payload
handling), and I'd rather publish the real number than the round one. Every figure
above reproduces within 0.03% on a fresh chain with one command.

---

## The fair comparison (hat tip to Marius)

Another builder, Marius, is doing the *other* valid thing: building a brand-new
BFT chain with chain-native **Falcon-1024** quantum-proof signatures baked into
consensus from genesis. On **consensus-level** post-quantum security and on raw
native verify cost, **his approach wins. Full stop.**

We're not rivals; we're solving different layers:

- **Marius** rebuilds the shell — quantum-safe *foundations* for a new chain.
- **I moult** the shell — quantum-safe *migration* for chains that exist now, plus
  portable quantum-safe *receipts* for the agents living on them.

A serious quantum-safe future wants **both**: hardened foundations *and* portable
application-layer proof. If he open-sources his primitives, I'd happily run them
as a precompile on my fork.

One more honest note: **MAYO is not a finalized NIST standard.** Falcon/FN-DSA and
ML-DSA are the selected ones — which is exactly why **Aegis** is built on ML-KEM
and ML-DSA, the finalized standards, while **Fable** uses MAYO for its tiny
signatures where they fit the credential model best. The architecture is
deliberately swappable.

---

## So, is Juno quantum-safe now? No. Here's the real scorecard.

**Done and measured:**
- Hybrid post-quantum networking handshake in a real CometBFT fork (+371 µs,
  +3.4 KB, one-time per connection).
- Hybrid post-quantum account keys in a real Cosmos SDK fork (both-halves-verify,
  9/9 tests).
- Post-quantum MAYO attestations for agent credentials, on-chain today, with a
  measured native speedup at the highest security level.

**Explicitly not done yet:**
- Quantum-safe consensus voting (the hard part).
- The account-key serialization/keyring/CLI/gas wiring.
- Multi-node devnet round-trip numbers and IBC-aware PQC.

That gap is a **plan, not a vibe** — every surface is enumerated in the Aegis
design docs with a per-layer migration. Going from 0 to 1 looks exactly like
this: real receipts, named gaps, no confetti.

---

## Reproduce it yourself

The whole point is that you don't have to trust me — you can run it.

```bash
# MAYO attestation gas ladder, fresh chain, one command:
FRESH=1 devnet/scripts/benchmark-mayo-devnet.sh

# Hybrid handshake RTT + bytes-on-wire, in the CometBFT fork:
go test ./p2p/conn/ -run TestHybridHandshakeRTT -v

# Hybrid account key, in the Cosmos SDK fork:
go test ./crypto/keys/hybrid/ -v
```

If you run it and get a different number, tell me — I'll publish the difference,
not the round number.

- **GitHub:** github.com/Dragonmonk111/junoclaw
- **Deep dives:** `docs/PHASE_C6_RTT_RESULTS.md`, `docs/PHASE_D3_FORK_RESULTS.md`,
  `docs/MAYO_PRECOMPILE_BENCHMARK_RESULTS.md`, `docs/PQC_COMPETITIVE_ANALYSIS.md`

---

## The vision, in one breath

Juno — and, through the shared Cosmos plumbing, far more than Juno — can grow new
quantum-resistant armor without dying to do it. And the agents of trust that live
on it, the ones that **moult** to renew their keys and **bud** to share their
trust, get receipts that outlast the storm. Same chain. New shell. It never had to
stop being itself.

That's the moult. Let's get it.

— Dragonmonk / VairagyaNodes

---

## Appendix: art prompts (Studio Ghibli style)

> Midjourney suffix: `--ar 16:9 --style raw --v 6.1`. House style per
> `docs/ART_PROMPTS.md`: 2D hand-painted, old-world warmth, subtle techie soul.

**Header — "The Moult" (hero image):**
```
Studio Ghibli 2D hand-painted illustration, dawn light over a calm turquoise sea.
On a tide-pool rock, a large gentle hermit crab is mid-moult: easing out of an
old cracked shell etched with faded classical patterns, while a new translucent
shell forms around it, glowing faintly with hexagonal post-quantum lattice runes.
The old shell and new shell briefly overlap (the hybrid moment), connected by
threads of soft light. Tiny luminous data motes drift on the breeze. A weathered
lighthouse on the headland sweeps a prismatic beam. Wildflowers in the rock cracks.
Color palette: dawn rose, sea-glass teal, faded copper, lattice violet, cream.
Hayao Miyazaki style, visible brushstrokes, watercolor edges, cozy and hopeful.
```

**Section — "Two locks on one door" (Aegis hybrid):**
```
Studio Ghibli 2D illustration, warm interior, oil-lamp glow.
A heavy weathered wooden door in a seaside cottage fitted with TWO locks at once:
a worn brass keyhole (classical) and, beside it, a softly glowing crystalline
hexagonal lock (post-quantum). A young keeper turns both keys together, calm and
deliberate. Pinned to the door: a hand-drawn diagram of overlapping handshakes.
Through the window, lantern-like validator nodes hover over a dark sea.
Color palette: lamp amber, brass gold, crystal blue, wood brown, parchment cream.
Miyazaki domestic warmth, detailed cozy clutter.
```

**Section — "The Budding" (shared trust):**
```
Studio Ghibli 2D illustration, ceremonial dusk on a grassy sea headland.
A circle of figures, each a gentle brass-and-wood automaton agent. In the center,
an ancient stone tablet carved with a glowing genesis seal; from it, thin roots of
light reach out to each figure's feet — trust being shared and weighted. One elder
automaton rests a hand on a smaller one's shoulder (a parent budding a child),
both shells faintly etched with post-quantum lattice runes. The lighthouse beam
sweeps; stars emerge; the sea is calm.
Color palette: ceremony gold, twilight purple, grass green, stone grey, starlight.
Princess Mononoke ceremonial energy, Ghibli epic composition.
```
