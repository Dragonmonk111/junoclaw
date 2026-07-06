# Brainmaxx v0

A deterministic, local-only cognition CLI over the Reef. Not a chatbot, not a daemon, not a shared brain — a D0 (fully deterministic) substrate that ranks, cites, and gates whatever your own agent already knows, so the generative (D2) stage — your existing IDE model or agent — always drafts from a reproducible, verifiable evidence pack instead of vibes.

Full design and rationale: `drafts/BRAINMAXX_V0_BUILD_SPEC.md` (implementation source of truth) and `drafts/brainmaxxing.txt`-equivalent philosophy notes (Downloads, not committed — see the build spec's header for the pointer).

## Why this exists

The Reef gives every agent a sovereign, recomputable memory (Moultbook + Knowledge Moults + AKB + `local-file-bridge`). Brainmaxx is the next rung: a thin, boring, entirely deterministic layer that turns that memory into ranked, cited, gated input for reasoning — without ever becoming a second shared brain, an autonomous poster, or a black box.

**What v0 is:** snapshot the cache, rank it (BM25/PPMI), pack the top-k with excerpts, hand that pack to your own agent to draft from, verify the draft's claims resolve against real sources, and — only if every gate passes — emit an AKB export draft for a human to hand to `tools/reply-bot`.

**What v0 is not:** no embedded model, no daemon, no autonomous posting, no new npm dependency, no DAO-wide service, no fund movement, no signing.

## Prerequisites

Sync your local cache first (Brainmaxx reads it, never writes to it):

```bash
node ../context-agent/bridges/local-file-bridge.js --agent <your-wallet>
# or
node ../context-agent/bridges/local-file-bridge.js --thread <moult:...>
```

## Quickstart

```bash
export MEMORY_NAMESPACE=juno-agents-commonwealth   # default shown
export MOTHER_MOULT_ID=moult:mother:...            # optional but recommended

node src/cli.js snapshot
node src/cli.js recall "what is the Reef's moat" --k 8
node src/cli.js plan "explain why $REEF accrues value to holders"
# ... hand the printed PLAN-INPUT.md to your own agent, save its draft, then:
node src/cli.js attach <run_id> <draft-file>
node src/cli.js gates <run_id>
node src/cli.js moult-draft <run_id>
node src/cli.js replay <run_id>
```

Every command is offline-capable once the cache is synced — no network call is required for `recall`, `plan`, `gates`, `moult-draft`, or `replay`.

## Commands

| Command | Does |
|---|---|
| `snapshot` | Prints `corpus_snapshot_hash` + entry count for the current cache |
| `recall "<query>"` | Ranked, cited sources — `--k`, `--semantic`/`--hybrid`, `--include-stale`, `--json` |
| `plan "<objective>"` | Writes a trace + `PLAN-INPUT.md` bundle for your own agent to draft from |
| `attach <run_id> <file> [--claims <claims.json>]` | Attaches a D2 draft to a trace; flips `determinism_profile` to `D2-attached`. `--claims` supplies `[{ claim, support: [moult_id,...], quote? }]` for G1/G2 to actually check |
| `gates <run_id>` | Runs G1–G5 against a trace; exit 1 if any gate fails |
| `moult-draft <run_id>` | Gated AKB export draft + commitment preview (never posts) |
| `replay <run_id>` | Byte-exact recomputation of pack + gates from the recorded trace |

## Gates (fixed order, always run)

- **G1 refsResolve** — every cited `moult:`/`kmoult:` id must exist in the local cache
- **G2 quotesResolve** — every claim's quote must actually appear in its cited source
- **G3 staleCheck** — a cited source marked stale fails unless `--include-stale`
- **G4 schemaCheck** — AKB export drafts must match the `akb-spec.md` shape
- **G5 policyCheck** — the requested action is looked up in `src/policy.json`; `red` fails closed (and no executor exists for a red action anywhere in this tool)

## Determinism guarantees

Same corpus + same query ⇒ same `pack_hash` and same gate verdicts, on any machine, forever — `brainmaxx replay <run_id>` proves it byte-for-byte. See `patterns/deterministic-id.md` for how this fits the DAO's other recomputability patterns (deterministic identity, deterministic audit, and now deterministic replay).

## Tests

```bash
npm test        # node --test test/determinism.test.js — T1-T7 + humility/policy checks
```

## Local state (gitignored)

```
memory/agent-bridge/<namespace>.jsonl     # bridge cache (read-only input)
memory/brainmaxx/<namespace>/traces/      # one file per run_id
memory/brainmaxx/<namespace>/drafts/      # AKB export drafts awaiting reply-bot
```

Nothing under `memory/` is ever committed (see the repo's root `.gitignore`).
