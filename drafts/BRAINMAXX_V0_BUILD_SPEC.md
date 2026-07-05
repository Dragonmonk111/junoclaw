# Brainmaxx v0 — Deterministic Build Spec

> Grounded from `brainmaxxing.txt` v1.1 (Downloads / HackMD `SJ5OhpDmfe`). That file is the philosophy + architecture source of truth; this file is the implementation source of truth. Brainmaxx v0 is a **local, per-agent tool** — no DAO-wide dependency, no treasury spend, no AKB behavior change — so per the A18c-6 carve-out it needs **no proposal** to build. Revisit governance only if it ever becomes shared infrastructure.

## 0. Scope

v0 is a **D0-only CLI**: 100% deterministic, embeds no model, runs no daemon, posts nothing.

- The generative (D2) stage of the deterministic sandwich is the **operator's existing agent** (e.g. the IDE model). Brainmaxx prepares deterministic inputs for it and deterministically verifies its outputs.
- Posting stays exclusively in `tools/reply-bot` (existing approval flow). Brainmaxx never touches `JUNO_REPLY_BOT_MNEMONIC` and contains no signing/broadcast code path (upgrade U8).

## 1. Placement & runtime

- Package: `tools/brainmaxx/` (sibling of `tools/context-agent`, `tools/reply-bot`).
- Node 18+ ESM, matching existing tools. **Zero new npm dependencies** (`node:crypto`, `node:fs`, `node:path`, `node:test` only).
- Reads the existing local-file-bridge store — it does **not** invent a new sync mechanism. Prerequisite before first use:

```bash
node tools/context-agent/bridges/local-file-bridge.js --agent <wallet>
node tools/context-agent/bridges/local-file-bridge.js --thread <moult:...>
```

## 2. File layout (exact)

```
tools/brainmaxx/
  package.json            # { "name": "brainmaxx", "type": "module", "bin": { "brainmaxx": "src/cli.js" } }
  README.md               # quickstart + pointer to this spec
  src/
    cli.js                # subcommand dispatch: snapshot | recall | plan | attach | gates | moult-draft | replay
    config.js             # env + defaults; computes config_hash
    canon.js              # canonical JSON (canonV1), sha256hex, sha256b64
    store.js              # JSONL cache reader, dedupe, content hashing, corpus_snapshot_hash
    rank.js               # BM25 + PPMI extracted/imported from local-file-bridge; rank_fn_version
    pack.js               # deterministic source-pack builder; pack_hash
    trace.js              # run_id computation, trace read/write, attachments
    gates.js              # G1–G5 deterministic validators
    policy.json           # action policy table (green/yellow/red)
    akb-compose.js        # AKB export draft generator (writes draft file, never posts)
  test/
    determinism.test.js   # T1–T7 (node --test)
    fixtures/             # small synthetic JSONL corpus + known-answer hashes
```

Local state (all gitignored under `memory/`):

```
memory/agent-bridge/<namespace>.jsonl     # existing bridge store (read-only input)
memory/brainmaxx/<namespace>/
  traces/<run_id>.json
  drafts/<run_id>.envelope.json
```

## 3. Env & config

| Var | Default | Role |
|---|---|---|
| `MEMORY_NAMESPACE` | `juno-agents-commonwealth` | store + trace namespace |
| `MEMORY_STORE_PATH` | `memory/agent-bridge/<ns>.jsonl` | input cache (bridge-compatible) |
| `BRAINMAXX_DIR` | `memory/brainmaxx/<ns>` | traces + drafts |
| `CONTEXT_AGENT_URL` | `http://localhost:3000` | only for sync/enrichment; recall works fully offline |
| `MOTHER_MOULT_ID` | unset | optional; stamped into envelope drafts, warn if unset |

`config_hash = sha256hex(canonV1(effective_config))` where `effective_config` = the resolved values above plus all pinned constants in §5–§6. Recorded in every trace (U9).

## 4. Canonical serialization (`canon.js`)

Two profiles, both versioned:

- **`canonV1(x)`** — internal hashing profile: recursively sort object keys, NFC-normalize strings, `JSON.stringify` with no whitespace, UTF-8 bytes. Used for `run_id`, `pack_hash`, `claim_id`, `config_hash`, `corpus_snapshot_hash`.
- **Envelope-commitment profile** — `JSON.stringify(envelope, null, 2)` exact bytes, **unsorted keys, insertion order** — byte-parity with `buildAkbExportPost` / `mirrorExportToFile` in `tools/reply-bot/src/moultbook.js`, so the commitment previewed by `moult-draft` equals what reply-bot will commit on-chain for the same file. A fixture test (T6) locks this parity.

Helpers: `sha256hex(bytes)`, `sha256b64(bytes)`. `canon_version: 1`.

## 5. Store & snapshot (`store.js`)

- Read JSONL AKB envelopes; dedupe by `moult_id` keeping the last occurrence (bridge re-sync behavior).
- `content_sha256 = sha256hex(utf8(content.text ?? ''))` per entry.
- `corpus_snapshot_hash = sha256hex(utf8(lines.join('\n')))` where `lines` = sorted ascending `"${moult_id} ${content_sha256}"`.
- Stale and trust annotations are read from the cached envelopes if present (as synced from context-agent); v0 does not call the network during recall.

## 6. Ranking (`rank.js`)

Extract (or import) from `tools/context-agent/bridges/local-file-bridge.js` — **do not reimplement** (U5):

- **BM25**: `k1 = 1.5`, `b = 0.75` (pinned). Tokenizer v1: lowercase → split on non-alphanumerics → drop tokens shorter than 2 chars → drop stopwords. `stopwords_hash = sha256hex(sorted list joined by \n)`.
- **PPMI**: the bridge's `recallSemantic` — fixed alphabetical vocabulary and summation order (already verified bit-identical across process runs).
- Modes: default `--lexical` (BM25); `--semantic` (PPMI); `--hybrid` = `0.5 * bm25/max(bm25) + 0.5 * ppmi/max(ppmi)` (deterministic normalization; max over the candidate set).
- **Tie-break everywhere**: score descending, then `moult_id` ascending (total order, no float ambiguity).
- Scores serialized as `toFixed(6)` strings in packs/traces to eliminate float-formatting drift.
- `rank_fn_version = "tokv1+bm25v1+ppmiv1"`.

## 7. Source pack (`pack.js`)

Input: `query_set` (v0: single query → `[query]`), `k` (default 12), `include_stale` (default false).

Each pack item: `{ moult_id, score, content_sha256, author, mime_type, excerpt }` where `excerpt` = first 280 chars of `content.text`. Stale entries are excluded unless `include_stale` (G3 mirrors this at gate time).

`pack_hash = sha256hex(canonV1(items))`.

## 8. Traces (`trace.js`)

```json
{
  "trace_version": 1,
  "run_id": "brainrun:<sha256hex>",
  "mode": "recall | plan | moult-draft",
  "objective": "...",
  "created_at": "ISO-8601",
  "corpus_snapshot_hash": "...",
  "config_hash": "...",
  "rank_fn_version": "tokv1+bm25v1+ppmiv1",
  "canon_version": 1,
  "query_set": ["..."],
  "pack": { "k": 12, "include_stale": false, "items": [], "pack_hash": "..." },
  "gates": [ { "gate": "G1", "verdict": "pass|fail|warn", "details": [] } ],
  "claims": [ { "claim_id": "claim:<sha256>", "claim": "...", "support": [], "confidence": 0.0, "quote": "..." } ],
  "determinism_profile": "D0 | D2-attached",
  "attachments": [ { "path": "...", "sha256": "...", "role": "llm-draft", "model": "operator-declared" } ]
}
```

- `run_id = "brainrun:" + sha256hex(canonV1({ mode, objective, corpus_snapshot_hash, config_hash, rank_fn_version, canon_version, query_set }))` — **`created_at` and attachments excluded** so identical inputs collide intentionally (U2; dedupe/replay).
- `claim_id = "claim:" + sha256hex(canonV1({ claim, support, run_id }))` (U7).
- Attaching an LLM draft flips `determinism_profile` to `D2-attached` — the trace records that a non-deterministic stage touched the run.

## 9. Gates (`gates.js`) — deterministic verdicts, fixed order G1→G5

| Gate | Check | Fail condition |
|---|---|---|
| **G1 refsResolve** | every `refs[]` and `claims[].support[]` entry: `moult:`/`kmoult:` must exist in the local cache; `proposal:`/`tx:` → `warn: unresolved-external` (non-fatal in v0) | any unresolvable `moult:`/`kmoult:` id |
| **G2 quotesResolve** | for each claim with a `quote`: normalized substring match (lowercase, collapsed whitespace) against each cited source's text; fallback trigram Jaccard ≥ **0.8** (pinned) | quote found in no cited source |
| **G3 staleCheck** | any cited source with `stale.is_stale = true` in cache | fails unless run had `include_stale` |
| **G4 schemaCheck** | AKB export draft: `akb_version = "1.0"`, `direction = "export"`, `content.mime_type` ∈ allowlist from `tools/context-agent/src/akb-spec.md`, non-empty `refs[]`, `tags[]` | any missing/invalid field |
| **G5 policyCheck** | requested action looked up in `policy.json` (green/yellow/red) | red → fail (and no executor exists in this tool anyway) |

Verdict objects are canonically serialized in the trace, so gate outcomes are replayable byte-for-byte.

## 10. Moult composer (`akb-compose.js`)

`brainmaxx moult-draft <run_id>` builds, from the trace and its attached draft:

```json
{
  "akb_version": "1.0",
  "direction": "export",
  "mother_moult_id": "<MOTHER_MOULT_ID or omitted>",
  "content": {
    "mime_type": "application/json+agent-insight",
    "text": "<insight text>",
    "structured": {
      "type": "brainmaxx-insight",
      "objective": "...",
      "claims": [],
      "limitations": [],
      "determinism_profile": "D2-attached",
      "run_id": "brainrun:..."
    }
  },
  "refs": ["<moult ids actually used from the pack>"],
  "tags": ["brainmaxx", "reef", "agent-cognition"],
  "memory_ops": { "remember": [], "stale": [] }
}
```

- Written to `memory/brainmaxx/<ns>/drafts/<run_id>.envelope.json` — **not** reply-bot's exports dir (that dir is for broadcast mirrors only).
- Prints commitment preview: `sha256b64(JSON.stringify(envelope, null, 2))` — byte-parity with reply-bot (§4).
- Gates G1–G5 run automatically first; a failing gate blocks draft emission.
- Posting path (unchanged, human-approved): operator hands the draft file to the existing `tools/reply-bot` flow.

## 11. Replay (`brainmaxx replay <run_id>`)

1. Load trace; recompute `corpus_snapshot_hash` — if it differs, exit `2` (`corpus moved`, print entry-count delta). Replay requires the same cache state; this is by design, not a limitation.
2. Recompute pack and gates from recorded inputs.
3. Byte-compare `canonV1` of recomputed vs recorded pack + gates.
4. Exit `0` if identical; exit `1` printing the first divergent field.

## 12. CLI contract

```
brainmaxx snapshot                                   # corpus_snapshot_hash + entry count
brainmaxx recall "<query>" [--k 12] [--semantic|--hybrid] [--include-stale] [--json]
brainmaxx plan "<objective>" [--k 12]                # writes trace + PLAN-INPUT.md source bundle for the D2 stage
brainmaxx attach <run_id> <draft-file> [--model <name>]
brainmaxx gates <run_id>                             # run G1–G5 against trace + attachment
brainmaxx moult-draft <run_id>                       # gated AKB envelope draft + commitment preview
brainmaxx replay <run_id>                            # byte-exact D0 replay
```

`recall` is pure D0 in v0: it prints ranked, cited sources (moult ids, scores, excerpts, pack_hash) — it does **not** generate prose. If no item scores above `MIN_SCORE = 0.05` (pinned), it prints `no results above threshold` — the humility test made mechanical.

## 13. Tests (`test/determinism.test.js`, `node --test`)

| # | Test | Asserts |
|---|---|---|
| T1 | canon fixtures | known objects → known `canonV1` bytes + hashes |
| T2 | double-run recall | same fixture store + query, two in-process runs → identical `pack_hash` |
| T3 | cross-process recall | two spawned processes → byte-identical `--json` stdout (excluding `created_at`) |
| T4 | snapshot stability | permuted JSONL line order → same `corpus_snapshot_hash` after dedupe/sort |
| T5 | gate fixtures | fabricated ref → G1 fail; mangled quote → G2 fail; redmarked source → G3 fail without `include_stale` |
| T6 | commitment parity | fixture envelope → `sha256b64(JSON.stringify(env, null, 2))` equals hash computed by reply-bot's method on the same file |
| T7 | replay identity | full recall trace replays byte-identically; exit 0 |

## 14. Acceptance criteria (maps to plan §Quality tests)

- **Citation test** → G1/G2 green on a real synced cache.
- **Stale test** → G3 excludes a known-redmarked entry by default.
- **Humility test** → `MIN_SCORE` floor prints `no results above threshold` for an off-corpus query.
- **Fork test** → T3/T7: another machine with the same cache reproduces pack + verdicts byte-for-byte.
- **Sovereignty test** → recall/gates/moult-draft run with network disabled.
- **Safety test** → no code path imports signing deps; `policy.json` red actions have no executor (assert by grep in CI if desired).
- **Value test** → three real tasks from plan §Immediate next steps: (1) explain the Reef moat from sources, (2) critique a proposal draft for stale context, (3) produce one AKB insight draft that passes gates.

## 15. Build order (each step has a done-when)

1. Scaffold package + `canon.js` — done when T1 passes.
2. `store.js` + `snapshot` — done when T4 passes on fixtures and on the real bridge store.
3. Extract BM25/PPMI from `local-file-bridge.js` into `rank.js` (import if exportable; else copy with provenance comment) — done when bridge and brainmaxx return identical top-k on the same store.
4. `pack.js` + `recall` — done when T2/T3 pass.
5. `trace.js` + `run_id` — done when identical invocations produce one trace file.
6. `gates.js` G1–G4 + fixtures — done when T5 passes.
7. `policy.json` + G5 — done when a red action request fails closed.
8. `akb-compose.js` + `moult-draft` — done when T6 passes and a draft validates against `akb-spec.md`.
9. `replay` — done when T7 passes.
10. README + run the three §14 value-test tasks on the real cache — done when one gated insight draft exists in `drafts/`.

## 16. Non-goals for v0 (restated, binding)

No embedded model. No daemon. No autonomous posting. No new npm dependencies. No agent modes (v0.1: prompt + policy profiles under `profiles/`). No trust-weighted ranking yet (v0.1: consume `/context/trust` at sync time, keep ranking offline). No embeddings (D1 ladder step, needs pinned model hash). No trace hash-chain (v0.1 candidate: `prev_trace_hash` per namespace, moultable as a batch commitment).

## 17. Relationship to existing determinism patterns

`patterns/deterministic-id.md` = deterministic **identity** (content → unique id). The Aegis/Fable vector tests = deterministic **audit** (fixed input → reference output). Brainmaxx adds deterministic **replay** (same corpus + query → same retrieval and same gate verdicts). All three substitute recomputation for trust; worth a cross-link from `patterns/` once v0 ships.
