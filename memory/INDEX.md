# `memory/` — recall map

*Distilled, topic-keyed knowledge files. Each entry below is a tight summary that doubles as the canonical citation target for cross-references in code comments, ADRs, and chat. The full-context counterparts live in `docs/` and the per-contract `DETERMINISTIC_AUDIT.md` files; this folder is the **recall layer** above them.*

*Convention adopted 2026-05-13 to mirror Jake Hartnell's structured-memory pattern (cf. `memory/v30-upgrade-plan.md` referenced from `dao-voting-juno-staked` source). Aligning with this pattern lets cross-repo code comments stay short and citation-friendly.*

---

## Format rules

1. **One topic per file.** If a topic genuinely has two distinct sub-topics, split them.
2. **Filename is the citation key.** `kebab-case`, no version suffix unless the topic is version-specific (e.g., `v30-upgrade-pr-1202.md`).
3. **First section: 3-line summary.** Anyone scanning the file should be able to leave after 3 lines with the right answer for "what is this about?".
4. **Second section: key facts as a table.** Numbers, file paths, commit SHAs, addresses. Anything you'd want without scrolling.
5. **Third section: full context.** Detail-rich exposition.
6. **Last section: cross-references.** Links to `docs/` long-form, related `memory/` files, external sources.

---

## Index

### Strategic / cross-cutting

| Key | Topic | One-line |
|---|---|---|
| [`lessons-2026-05-13.md`](./lessons-2026-05-13.md) | Daily-cluster lessons (2026-05-13) | Memory systems, LavaMoat, monorepo, JunoCommsDept revival, PRs #928-#929 amplification |
| [`deterministic-audit-benchmark.md`](./deterministic-audit-benchmark.md) | Deterministic scrutiny method | The 4-axis benchmark applied to JunoClaw contracts; per-contract finding index |

### BN254 / wasmvm / cosmwasm

| Key | Topic | One-line |
|---|---|---|
| [`bn254-precompile.md`](./bn254-precompile.md) | BN254 host functions for cosmwasm | What, why, where the patches live, gas measurement, status |
| [`track-b-forward-port.md`](./track-b-forward-port.md) | v2.2.7 → v3.0.x patch forward-port (Track B) | Day-1 baseline result, drift map, timeline |

### Juno / dao-contracts upstream

| Key | Topic | One-line |
|---|---|---|
| [`v30-upgrade-pr-1202.md`](./v30-upgrade-pr-1202.md) | CosmosContracts/juno PR #1202 (Juno v30) | x/voting-snapshot, x/cw-hooks, our review |
| [`dao-contracts-prs-928-929.md`](./dao-contracts-prs-928-929.md) | DA0-DA0/dao-contracts PRs #928 + #929 | Gauges + dao-voting-juno-staked, our review of #929 |

---

## Migration status

This is **phase 1** (2026-05-13) of the docs/ → memory/ migration. The selection criteria for phase 1:

- **High-recall** = referenced from code comments, ADRs, or other memory entries; needed during fresh-context loads.
- **Durable** = the topic has a stable identity that won't be renamed; doesn't decay over a sprint.
- **Tight** = the underlying material can fit in <300 lines of summary while retaining citable facts.

Phase 1 covers the ~7 most-cited topics. Future phases will migrate the architectural ADRs (`docs/ADR-*.md`) and the most-cited Medium-article-facing notes if and when those get cited from code.

The full-fat `docs/` directory remains the long-form source of truth — these `memory/` files are the **fast-recall layer**, not a replacement.

---

*Apache-2.0. Index updated whenever a new memory file lands.*
