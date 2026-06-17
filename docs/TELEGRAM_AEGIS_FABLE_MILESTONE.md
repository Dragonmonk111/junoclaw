# Comms — Aegis + Fable milestone (2026-06-17)

> Short, plain-language, receipts-first. Pairs with the long write-up
> `MEDIUM_ARTICLE_AEGIS_FABLE_MILESTONE.md`. Gracious to Marius; no defensiveness;
> no "we're quantum-safe now" confetti. Built openly with an AI coding agent.

---

## A) Telegram message (copy-paste below)

---

Update — a real milestone, with measured numbers and named gaps.

The thesis hasn't changed: a quantum computer will eventually break the signatures
that keep chains honest, and you don't have to *kill* a chain to save it — you can
**moult** it. Grow new quantum-resistant armor around the chain that already
exists, without anyone having to stop using it. Two fronts moved this week.

**Front 1 — the chain's own plumbing (codename Aegis).**
I forked CometBFT and the Cosmos SDK — the shared layer under *every* Cosmos chain
— and folded in **hybrid** post-quantum crypto (classical + PQC together, so
nothing breaks mid-transition):

- **Networking handshake (done + measured):** added an X25519 + **ML-KEM-768**
  hybrid handshake. The channel is safe only if you'd have to break *both*. The
  cost, measured over real TCP: **+371 microseconds of CPU** and **+3.4 KB on the
  wire**, and *only* when two nodes first connect — not per block, not per message.
  Proof it's non-destructive: CometBFT's existing adversarial + golden tests pass
  **unchanged**.
- **Accounts (core done):** a hybrid **secp256k1 + ML-DSA-44** account key. A
  signature is valid only if **both** halves verify — and it plugs into the SDK's
  existing transaction machinery with **no new tx type and no new signing mode**.
  9/9 tests, including "forging needs both keys."

**Front 2 — the receipts agents leave behind (codename Fable).**
JunoClaw is an AI-DAO stack: agents take work, prove it, and write trust to shared
memory (Moultbook). They **moult** (rotate keys/credentials) and **bud** (grant a
child weight = shared trust), all on-chain. Fable makes those receipts
post-quantum with **MAYO** (tiny signatures). Measured, pure-Wasm → native
precompile, same fresh devnet:

- MAYO-2 / L1: 355,932 → 310,391 gas — 1.15×
- MAYO-3 / L3: 456,644 → 257,371 gas — 1.77×
- MAYO-5 / L5: 798,137 → **360,904 gas — 2.21×**

So NIST **Level 5** — the same tier as Falcon-1024 — verifies on-chain *today* for
under 800k gas. I'll be straight: I'd hoped the precompile would be ~7×. It's
2.21× at L5. Once the crypto is native, what's left is moving the ~5 KB key around,
not the math. That's a finding, not a faceplant — and it points at the next work.

**Is Juno quantum-safe now? No — and I won't pretend it is.**
Done: hybrid transport, hybrid accounts (core), post-quantum agent receipts.
Not done: quantum-safe **consensus voting** (the hard part), the account
keyring/CLI/gas wiring, and IBC-aware PQC. Those are written down as a concrete
per-layer plan, not a vibe.

**On @Marius:** he's building the *other* valid answer — a new BFT chain with
chain-native Falcon-1024 in consensus from genesis. On consensus-level PQC and raw
verify cost, his approach wins, full stop. We solve different layers: he rebuilds
the shell, I moult it (and add portable receipts for the agents living in it). A
serious future wants both. Also fair: MAYO isn't a finalized NIST standard —
that's exactly why Aegis is built on the finalized ones, **ML-KEM** and **ML-DSA**.

**Receipts.** Everything reproduces:
- `FRESH=1 devnet/scripts/benchmark-mayo-devnet.sh` (MAYO gas ladder)
- `go test ./p2p/conn/ -run TestHybridHandshakeRTT -v` (handshake cost)
- `go test ./crypto/keys/hybrid/ -v` (hybrid account key)

I build with an AI agent, I say so openly, and I check the numbers before I post
them. Going from 0 to 1 looks like this. The vision: same chain, new shell — and
the agents of trust who moult and share trust keep its integrity. Let's get it.

🔗 GitHub: github.com/Dragonmonk111/junoclaw
🔗 Write-up: docs/MEDIUM_ARTICLE_AEGIS_FABLE_MILESTONE.md

— Dragonmonk / VairagyaNodes

---

## B) X / Twitter thread

**1/**
A crab can't grow inside its old shell. So it moults — sheds the armor, grows a
bigger one, never stops being a crab.

That's my answer to quantum computers vs blockchains: don't kill the chain to save
it. Moult it. 🦀

Receipts below. 👇

**2/**
The problem: quantum computers will eventually forge the signatures that keep
chains honest. Not today — but "not today" isn't a plan.

Two fixes: build a new quantum-proof chain, or carefully swap the locks on one
people already use. I'm doing the second.

**3/**
The trick that makes swapping safe = HYBRID. During the transition the door has
BOTH the old lock and the new lock. An attacker has to pick both. Nothing breaks
mid-moult.

I forked CometBFT + the Cosmos SDK (the shared layer under all of Cosmos) to do it.

**4/**
Quantum-safe networking handshake: X25519 + ML-KEM-768, measured over real TCP.

Cost: +371 microseconds of CPU, +3.4 KB on the wire — and ONLY when two nodes
first connect. Not per block. Not per message.

The cost is in bytes, not CPU. Exactly as predicted.

**5/**
Quantum-safe accounts: a secp256k1 + ML-DSA-44 hybrid key. Valid only if BOTH
halves verify.

Best part: it plugs into the SDK's existing tx machinery — no new tx type, no new
signing mode. 9/9 tests, including "forging needs both keys."

**6/**
The other front: AI agents on JunoClaw moult (rotate keys) and bud (share trust),
on-chain. I made their receipts post-quantum with MAYO (tiny signatures).

NIST Level 5 — the Falcon-1024 tier — verifies on-chain TODAY for <800k gas.
Native precompile: 2.21× cheaper.

**7/**
Honesty, because that's the whole brand:
- I hoped the precompile would be ~7×. It's 2.21×. Once crypto is native, moving
  the key dominates. A finding, not a faceplant.
- Juno is NOT fully quantum-safe yet. Consensus voting is still classical. That's
  the boss level, still ahead.

**8/**
Fair hat tip to @Marius: he's building a new BFT chain with Falcon-1024 native in
consensus. On consensus PQC, he wins, full stop.

He rebuilds the shell. I moult it + add portable receipts. A serious future wants
both.

**9/**
Everything reproduces in one command. If you get a different number, tell me — I'll
publish the difference, not the round number.

Built openly with an AI coding agent. Same chain, new shell. 🦀

🔗 github.com/Dragonmonk111/junoclaw

---

## C) Recommended art

Use the **"The Moult" hero image** prompt from
`MEDIUM_ARTICLE_AEGIS_FABLE_MILESTONE.md` (appendix) for the thread's lead image
and the Medium header. For the Telegram pin, the **"The Budding"** prompt suits the
shared-trust theme. Midjourney suffix: `--ar 16:9 --style raw --v 6.1`.
