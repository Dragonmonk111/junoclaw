# Moultbook v0 — Deterministic Scrutiny Audit

*Applied per the Ffern/Lex benchmark (2026-05-12). Every finding traces to the wasm bytecode / host function layer.*

---

## 1. Gas Trace — `execute_post` (the hot path)

| Step | Operation | Host calls | Estimated SDK gas |
|------|-----------|------------|-------------------|
| CONFIG.load | 1× `db_read` (~60 bytes) | 1 | ~2,500 |
| Commitment length check | Pure wasm | 0 | ~50 |
| size/refs/content_type checks | Pure wasm | 0 | ~100 |
| whoami query (if set) | 1× `query_chain` (Stargate round-trip) | 1 | ~15,000-30,000 |
| Ref validation (N refs) | N× `db_read` (existence check via `has()`) | N | N × ~2,000 |
| SHA256 hash | Pure wasm (~100 bytes input) | 0 | ~500 |
| ENTRIES.save | 1× `db_write` (~300 bytes value, ~80 byte key) | 1 | ~6,000 |
| BY_AUTHOR.save | 1× `db_write` (~50 byte key, 0 value) | 1 | ~3,000 |
| BY_REF.save (N refs) | N× `db_write` | N | N × ~3,000 |
| STATS.update | 1× `db_read` + 1× `db_write` | 2 | ~5,000 |

**Total (0 refs, no whoami):** ~12,150 SDK gas → ~36,500 with 3× CosmWasm multiplier = ~36.5K gas
**Total (5 refs, with whoami):** ~40,500 SDK gas → ~121.5K with 3× multiplier = ~121.5K gas

**ADR-002 projected 40-60K for the common case.** 36.5K (no refs, no whoami) is within range. With whoami + refs the gas scales linearly, which is expected.

**⚠ Finding G1:** The whoami query is the single largest gas cost at 15-30K. If the whoami contract is migrated to a slow implementation or removed, the `execute_post` path either balloons or errors. **Mitigation:** The `NoIdentity` error already catches empty responses, but a migrated-away contract would return a StdError (contract not found), which surfaces as `ContractError::Std`. This is acceptable but should be documented.

---

## 2. Failure Mode Enumeration

### F1 — Storage corruption mid-write in `execute_post`

**Scenario:** Gas runs out between `ENTRIES.save` (line 197) and `STATS.update` (line 203).
**Impact:** Entry exists in `ENTRIES` and `BY_AUTHOR` but `STATS` is stale (undercounts by 1).
**CosmWasm protection:** All storage writes within a single `execute` call are **atomic** — if the call runs out of gas, the entire transaction reverts. This is a non-issue in CosmWasm's execution model. ✅

### F2 — whoami contract migrated or destroyed

**Scenario:** Admin sets `whoami_contract` to an address that is later migrated to a different code_id or has its funds drained / admin-transferred.
**Impact:** The Wasm query to whoami fails with `StdError::GenericErr` → bubbles as `ContractError::Std`. All `execute_post` calls fail until admin calls `UpdateConfig` to clear or change the whoami address.
**Severity:** Medium — the contract becomes unusable for posting but existing entries are safe.
**Mitigation already present:** `UpdateConfig` can clear `whoami_contract` to `None`. ✅
**Missing mitigation:** No way for the contract itself to detect and auto-clear a broken whoami dependency. Acceptable for v0 — document the admin recovery path.

### F3 — ID collision (SHA256 hash collision)

**Scenario:** Two different posts produce the same `sha256(commitment || sender || time_nanos)`.
**Impact:** `DuplicateEntry` error — second post is rejected. No data corruption.
**Probability:** Negligible (2^-256 for random inputs). Deterministic collision would require same sender + same commitment + same block timestamp (nanos), which is checked by the hash construction. ✅

### F4 — Visibility downgrade + ref graph inconsistency

**Scenario:** Author posts entry A (Public), entry B cites A. Author changes A's visibility to `Owner`. Entry B's ref now points to an entry the querier can't see.
**Impact:** The `BY_REF` index for A still includes B, but querying A returns it with `Owner` visibility. A reader traversing B's refs finds a ref they can't access.
**Current behavior:** No access check on queries — `GetEntry` returns the entry regardless of visibility. Visibility is metadata, not enforced at the query layer.
**Severity:** Low for v0 (documented metadata convention). For v1, consider query-layer enforcement.

### ⚠ F5 — Unbounded `Group(Vec<Addr>)` in Visibility

**Scenario:** Author posts with `Visibility::Group(vec_of_1000_addresses)`.
**Impact:** The `MoultEntry` struct stored in `ENTRIES` includes the full `Vec<Addr>`. Each address is ~45 bytes (bech32). 1000 addresses = ~45KB per entry value. This bloats storage reads for every query touching this entry.
**No validation exists.** `max_size_bytes` validates the off-chain blob size, not the on-chain entry size.
**Fix:** Add `max_group_size` to `Config`, validate `group.len()` in `execute_post`. **This is a real bug.**

### F6 — Redaction doesn't clear BY_AUTHOR / BY_REF indexes

**Scenario:** Entry is redacted. The `BY_AUTHOR` and `BY_REF` index entries still exist and point to the redacted entry.
**Impact:** `ListByAuthor` and `ListByRef` queries return redacted entries (with zeroed commitment). This is likely intentional (redaction is visible, not deletion), but should be documented.
**If not intentional:** Add index cleanup to `execute_redact`.

---

## 3. Storage Layout Analysis

### Key prefix map

| Storage item | Prefix | Key structure | Value size |
|-------------|--------|---------------|------------|
| CONFIG | `"config"` | singleton | ~120 bytes |
| STATS | `"stats"` | singleton | ~24 bytes |
| ENTRIES | `"entries"` | `\x00\x07entries` + entry_id (~73 bytes) | ~300 bytes (variable, depends on content_type + visibility group + refs count) |
| BY_AUTHOR | `"by_author"` | `\x00\x09by_author` + addr_len + addr + id | 0 bytes (unit `()`) |
| BY_REF | `"by_ref"` | `\x00\x06by_ref` + ref_id_len + ref_id + id | 0 bytes (unit `()`) |

### Prefix collision risk

All prefixes are distinct strings. `cw_storage_plus` length-prefixes composite keys. **No collision risk.** ✅

### Scale behavior at 100K entries

| Metric | Value |
|--------|-------|
| ENTRIES total size | 100K × ~300 bytes = ~30 MB |
| BY_AUTHOR index | 100K × ~120 bytes (key only) = ~12 MB |
| BY_REF index | ~500K × ~200 bytes (assuming 5 refs/entry avg) = ~100 MB |
| Single `ListByAuthor` query (limit=30) | 30 × `db_read` = ~60K gas |
| Full ENTRIES iteration (never done in contract) | N/A — no full-scan query exists ✅ |

**⚠ Finding S1:** At high ref density (many entries citing the same anchor), `ListByRef` for a popular anchor could scan a large prefix range. The `take(limit)` caps the output but the iterator still opens the prefix range. With 10K citations of a single anchor, the range open is ~10K keys. In practice, cw_storage_plus iterators are lazy (B-tree seek + sequential scan), so the `take(30)` only reads ~30 keys. **Acceptable.** ✅

---

## 4. Determinism Proof

| Concern | Status |
|---------|--------|
| No floating point | ✅ — no floats anywhere |
| No HashMap iteration | ✅ — no HashMaps; all storage is ordered via cw_storage_plus |
| No std::time / SystemTime | ✅ — uses `env.block.time` (deterministic) |
| No randomness | ✅ — SHA256 is deterministic |
| No external HTTP/network | ✅ — only Wasm query to whoami (deterministic within block) |
| Serde round-trip stability | ✅ — `cw_serde` derives `JsonSchema` + `Serialize` + `Deserialize` with `deny_unknown_fields` |

**All clear.** Contract is fully deterministic. ✅

---

## 5. Serde Boundary Hardening

### Malformed JSON

All message types use `cw_serde` which derives `serde::Deserialize` with strict typing. Unknown fields are denied by default. A malformed message returns `StdError::ParseErr` before any contract code runs. ✅

### Max message size

CosmWasm enforces a hard 1MB limit on contract call messages. The largest variable-size field in `ExecuteMsg::Post` is `refs: Vec<String>` — each ref ID is ~73 chars. At `max_refs = 100`, the refs array is ~7.3KB. Combined with other fields, a worst-case Post message is ~50KB. Well within the 1MB limit. ✅

### Schema evolution across migrations

The `MigrateMsg` is currently empty (`{}`). If v1 adds fields to `MoultEntry` (e.g., `tags`, `edit_history`), existing entries in storage won't have those fields. Deserialization will fail unless new fields are `Option<T>` with `#[serde(default)]`.

**⚠ Finding SE1:** All future field additions to `MoultEntry` MUST be `Option<T>` with `#[serde(default)]` to maintain backwards compatibility. Document this constraint now. **This is a migration-safety invariant.**

---

## 6. Action Items

| ID | Severity | Fix |
|----|----------|-----|
| F5 | **HIGH** | Add `max_group_size: u32` to Config, validate in `execute_post` |
| SE1 | **MEDIUM** | Add doc-comment to `MoultEntry` struct noting Option<T> + default requirement for future fields |
| F2 | **LOW** | Document admin recovery path for broken whoami dependency |
| F4 | **LOW** | Document that visibility is metadata-only in v0; plan query-layer enforcement for v1 |
| F6 | **LOW** | Document that redacted entries remain in indexes (or clean up indexes if deletion semantics preferred) |
