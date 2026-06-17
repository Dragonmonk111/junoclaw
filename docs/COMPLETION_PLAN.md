# Completion Plan — Remaining Work (updated 2026-06-17)

> Consolidated plan for all remaining workstreams across Project Aegis (transport + accounts),
> Project Fable (MAYO-5 PQC), and upstream tracking. Created from memory `e5b41d64`.
>
> **2026-06-17 progress:** Aegis **D1 (ADR-007) DONE**, **D2 (aegis-accounts harness) DONE & verified
> 20/20 green**. Project **Fable P3 found ALREADY DONE** (contract dispatches MAYO-1/2/3/5 + ML-DSA-44/65/87
> with tests). Remaining Aegis/Fable items are all devnet-/fork-gated (C5/C6, D3, P4).

---

## 1. Executive Summary

| Workstream | Items | Priority | Effort Estimate | Blocker |
|-----------|-------|----------|-----------------|---------|
| **Aegis Phase C (Transport)** | C5: fold into CometBFT fork; C6: devnet RTT | High | 2–3 days | Fork clone + devnet |
| **Aegis Phase D (Accounts)** | ~~D1 ADR-007~~ ✅; ~~D2 harness~~ ✅; D3 SDK fork | Medium | 2–3 days left | D3: SDK fork + devnet |
| **Project Fable** | ~~P3 multi-variant~~ ✅; P4 benchmark; P5 comms | Medium | 2–3 days left | Devnet stability |
| **Infrastructure** | Devnet WSL2 clock jump fix | High | ½ day | WSL2 behavior |
| **Upstream** | cosmwasm#2685 follow-up; v30 proposal tracking | Low | Ongoing | External maintainers |

**Critical path:** Devnet stability → Phase C6 RTT + Fable P4 benchmarks. The remaining non-gated
work (D1, D2, P3) is **complete**; everything left needs either an external fork clone (C5, D3) or a
stable devnet (C6, P4).

---

## 2. Aegis Phase C — Transport (Remaining: C5, C6)

### C5: Fold `secretconn/` into CometBFT fork

**Priority:** High  
**Effort:** 1–2 days  
**Prerequisites:** `aegis-transport/secretconn/` package (done — 5 tests green, `PORTING.md` drafted)  
**Gating:** None

#### Steps

1. **Fork CometBFT** (target: `cometbft/cometbft` v0.38.x or Juno's pinned version).
   - Clone upstream, create branch `aegis-phase-c-hybrid-secret-conn`.
2. **Apply the patch plan from `PORTING.md`**:
   - Copy `aegis-transport/secretconn/secretconn.go` → `p2p/conn/secret_connection.go` (overwrite, preserving CometBFT package name).
   - Copy `aegis-transport/secretconn/secretconn_test.go` → `p2p/conn/secret_connection_test.go`.
   - Resolve import paths: replace `github.com/junoclaw/aegis-transport/secretconn` with `github.com/cometbft/cometbft/p2p/conn`.
   - Wire `ProtocolVersion` negotiation into CometBFT's `node_info` handshake (if not already present).
   - Ensure graceful fallback: if remote does not support hybrid, fall back to classical X25519 (fail-closed downgrade detection stays in tests; production fork needs graceful degrade for peer compatibility during transition).
3. **Build check:**
   ```bash
   make build
   make test_p2p_conn
   ```
4. **Verify tests:** all `secretconn` tests pass against real `net.Conn` (not just `net.Pipe`).

#### Success Criteria
- `make build` passes with zero new lint/vet warnings.
- `go test ./p2p/conn/...` passes (including new hybrid tests).
- A node with the patch can still connect to an unpatched node (classical fallback).
- A node with the patch connects to another patched node via the hybrid path (confirmed via version byte or debug log).

#### Failure Modes
- CometBFT's `secret_connection.go` has drifted from our template: update `PORTING.md` and re-sync.
- Build fails on `crypto/mlkem` (Go 1.24 required): verify CometBFT's Go version constraint.
- Classical fallback not working: peer negotiation logic needs adjustment.

---

### C6: Devnet real-link RTT measurement

**Priority:** High  
**Effort:** 1 day  
**Prerequisites:** C5 complete (CometBFT fork builds); devnet stable (see Infrastructure §6)  
**Gating:** Devnet stability

#### Steps

1. **Build the patched CometBFT node binary** for the devnet image.
2. **Deploy two patched nodes** on the devnet (or one patched + one classical for comparison).
3. **Measure:**
   - Handshake RTT (hybrid vs classical) over real network (not loopback).
   - Bandwidth per handshake (should match C3: ~2422 B vs ~72 B).
   - Connection throughput post-handshake (should be identical — same AEAD).
4. **Record results** in `aegis-transport/docs/PHASE_C6_RTT_RESULTS.md`.

#### Success Criteria
- RTT measurement documented with ≥3 samples, mean + stddev.
- Bandwidth matches C3 prediction within 10%.
- No connection failures between patched nodes.

#### Failure Modes
- Devnet stalls (WSL2 clock jump): see §6 Infrastructure fix.
- Nodes cannot peer: firewall, P2P port, or handshake negotiation bug.

---

## 3. Aegis Phase D — Opt-in PQC Accounts (Remaining: all)

### D1: ADR-007 spec — ✅ DONE (2026-06-17)

**Status:** Committed at `docs/ADR-007-PQC-HYBRID-ACCOUNTS.md`. Covers the hybrid
`secp256k1 + ML-DSA-44` key type, both-must-verify rule, 20-byte domain-separated
address (`address.Hash("pqc/hybrid-secp256k1-mldsa44", secp||mldsa)[:20]`), HD
(BIP-44 classical + HKDF-derived ML-DSA seed), SignMode/AnteHandler/gas, wire
encoding, alternatives, and consequences. Matches ADR-006 style.

**Priority:** Medium  
**Effort:** 1 day  
**Prerequisites:** ADR-006 (done), §5.1 ML-DSA-44/65 decision input (done, measured)  
**Gating:** None

#### Content Outline (from PROJECT_AEGIS_JUNO_FULL_PQC.md §4.5)

- Hybrid account type: `secp256k1` classical half + `ML-DSA-44` PQC half.
- Address derivation: `SHA-256(varint(type) || secp256k1-pubkey-33 || mldsa44-pubkey-1312)[:20]` (or similar deterministic scheme).
- `SignMode` for hybrid: sign with both halves, verify both.
- HD derivation: classical path from BIP-44/ Cosmos standard; PQC key derived from same seed via deterministic KDF (HKDF-SHA256 over mnemonic entropy).
- AnteHandler decorator: verify hybrid signature before state transition.
- Keyring support: store both halves, CLI support for hybrid key generation.
- Opt-in: classical accounts untouched; hybrid is a new `Account` subtype.

#### Success Criteria
- ADR-007.md committed to `docs/`.
- Peer review: at least one self-consistency check (does the address format avoid collisions with classical? Yes — prefix byte).

---

### D2: `aegis-accounts/` Go harness — ✅ DONE & VERIFIED (2026-06-17)

**Status:** `aegis-accounts/` Go module created and **20/20 tests green, `go vet`
clean** (WSL go1.24). Implements ADR-007 end to end: keygen (random + from-seeds),
hybrid sign/verify, 20-byte bech32 address, HD-from-mnemonic. Files: `hybrid.go`,
`bip32.go`, `hd.go`, `bech32.go` (in-tree BIP-173), 3 test files, `README.md`.
**ML-DSA-in-Go decision RESOLVED:** used `github.com/cloudflare/circl`
(`sign/mldsa/mldsa44`) — pure-Go, no CGO bridge needed (the §8 risk is closed).
Other deps: decred `secp256k1/v4`, `cosmos/go-bip39`, stdlib `crypto/hkdf`.
Headline test `TestHybridBothHalvesRequired` proves forgery needs BOTH primitives;
`TestBIP32Vector1` checks the classical HD against the canonical BIP-32 xprv via an
in-test base58check decoder (authoritative, non-circular).

**Priority:** Medium  
**Effort:** 2–3 days  
**Prerequisites:** D1 (ADR-007 spec frozen)  
**Gating:** D1

#### Steps

1. Create `aegis-accounts/` Go module (Go 1.24, stdlib-only where possible).
2. Implement:
   - `HybridKey` struct holding `secp256k1.PrivateKey` + `mldsa44.PrivateKey`.
   - `Generate(seed []byte)` — deterministic from seed (BIP-39 mnemonic → entropy → HKDF → both halves).
   - `Sign(msg []byte) → HybridSignature` — produces `[secp256k1-sig || mldsa44-sig]`.
   - `Verify(msg, sig, pubkey) → bool` — verifies both halves independently.
   - `Address() → string` — bech32 with Juno prefix, using ADR-007 derivation.
   - `HDFromMnemonic(mnemonic, path string) → HybridKey` — BIP-44 compatible.
3. Tests:
   - Keygen determinism (same seed → same key).
   - Sign/verify round-trip.
   - Wrong key rejection.
   - Tampered signature rejection.
   - Address collision resistance (two different keys → different addresses).
   - HD path derivation consistency.
4. `go vet ./...` clean.

#### Success Criteria
- All tests pass.
- `go vet` clean.
- No external crypto deps beyond Go stdlib (`crypto/ecdsa`, `crypto/mlkem` is transport; for accounts use `crypto/ecdsa` + vendored ML-DSA or pure-Go impl — check what Phase B uses).

**Note:** Phase B's `cosmwasm-crypto-mldsa` is Rust/wasm. For Go accounts harness, we need a Go ML-DSA-44 impl. Options: (a) use `fips204` Rust via CGO (heavy), (b) use pure-Go implementation if one exists, (c) implement from spec (large effort). **Decision needed:** For the harness, we can call out to the Rust impl via a small CGO bridge, or we can use the Go `crypto` experimental packages if ML-DSA lands in Go 1.25. **Recommendation:** check if Go 1.24/1.25 has `crypto/mldsa`; if not, use a thin Go wrapper around the existing Rust `fips204` crate via CGO for the harness only. The SDK fork integration will need its own impl strategy.

---

### D3: Cosmos SDK fork integration

**Priority:** Low  
**Effort:** 2–3 days  
**Prerequisites:** D2 (harness tested and stable)  
**Gating:** D2

#### Steps

1. Fork Cosmos SDK (target: Juno's pinned SDK version).
2. Add new `cryptotypes.PubKey` implementation for hybrid key.
3. Add `SignMode_SIGN_MODE_HYBRID` to `tx.SigningMode`.
4. Add AnteHandler decorator `HybridSigDecorator` (after `SigVerificationDecorator`).
5. Keyring support: extend `keyring` to store hybrid keys (two key records or one composite record).
6. CLI: `junod keys add --hybrid`, `junod tx sign --sign-mode hybrid`.
7. Tests: unit tests for each new component; integration test for a hybrid-key account sending a tx.

#### Success Criteria
- `make test` in SDK fork passes with new tests.
- A devnet node with the patched SDK can create a hybrid account and send a tx.

---

## 4. Project Fable — MAYO-5 (Remaining: P3, P4, P5)

### P3: Multi-variant contract support — ✅ ALREADY DONE (discovered 2026-06-17)

**Status:** A deep re-audit found this was **already implemented** (the earlier
"pending" was stale). `jclaw-credential` dispatches **MAYO-1/2/3/5**
(`contract.rs` ~296–306) and **ML-DSA-44/65/87** (~383–388) via clean `match`
arms, both with pure-Wasm and precompile paths. `tests.rs` (~33 tests) includes
`mayo3_verify_valid_signature`, `mayo5_verify_valid_signature`,
`mayo_verify_wrong_variant_rejected`, and `mldsa_static_vectors_verify` (44/65/87).
The verifier crate `junoclaw-mayo-verify` has all four C cross-check tests
(`test_mayo1/2/3/5_cross_check_sriracha`, behind `test-c`).

**Design divergence (intentional, cleaner):** storage holds a **variant-agnostic**
`SHA-256(compact_pk)` and the variant is supplied at **verify time** (the
`variant` field on `VerifyMayoAttestation`, default `Mayo2`), rather than the
planned stored `variant_tag`. PK lengths differ per variant, so no tag is needed.

**Deferred (not done, by decision):** the `PqcVerifier` trait extraction — the
existing `match` dispatch already cleanly supports MAYO + ML-DSA, so the trait
would be over-engineering on a working, tested contract. Revisit only if a third
signature family (e.g. Falcon/SLH-DSA) is added in-contract.

**Priority:** Medium  
**Effort:** 1–2 days  
**Prerequisites:** Phase 2 streaming done (2026-06-12)  
**Gating:** None

---

### P4: Benchmark ladder + MAYO precompile

**Priority:** Medium  
**Effort:** 2 days  
**Prerequisites:** P3; devnet stable (§6)  
**Gating:** Devnet stability

#### Steps

1. Build `jclaw-credential` with MAYO-2/3/5 variants.
2. Deploy to devnet (stable) and uni-7 testnet.
3. Benchmark matrix: `{Mayo2, Mayo3, Mayo5} × {Bud, Verify} × 3 samples`.
4. Expected numbers (extrapolated from MAYO-2's 356k):
   - MAYO-3 verify: ~700k–900k gas.
   - MAYO-5 verify: ~1.2–1.6M gas.
5. Fold MAYO precompile (`docs/MAYO_PRECOMPILE_PLAN.md`, 7× gas target) into devnet image.
6. Benchmark wasm vs precompile for MAYO-5 in one pass.
7. Record in `deploy/mayo-benchmark-ladder-results.json`.

#### Success Criteria
- Published benchmark table with all variants.
- MAYO-5 verified live on-chain (devnet or testnet).
- Precompile path shows measurable reduction vs pure-wasm for MAYO-5.

---

### P5: Docs + comms

**Priority:** Low  
**Effort:** ½ day  
**Prerequisites:** P4 (numbers in hand)  
**Gating:** P4

#### Steps

1. Update `docs/MAYO.md`, `docs/PQC_COMPETITIVE_ANALYSIS.md` with Level 5 numbers.
2. Draft article: "NIST Level 5 post-quantum attestations on a live Cosmos chain — no fork required."
3. Telegram reply to Jake/Marius thread with benchmark table.

#### Success Criteria
- Article published or PR'd to repo.
- Telegram reply sent.

---

## 5. Sequencing and Dependencies

```
Infrastructure (§6)
    │
    ▼ (unblocks)
C5 ──> C6 ──────────────────────> (Phase C done)
    │
    ▼ (parallel)
D1 ──> D2 ──> D3                (Phase D done)
    │
    ▼ (parallel)
P3 ──> P4 ──> P5                (Fable done)

Upstream tracking (§7): ongoing, non-blocking.
```

### Parallelizable now (no blockers)
- D1 (ADR-007 spec) — no code dependencies.
- P3 (multi-variant contract) — streaming is done.
- §6 (devnet stability fix) — infrastructure.
- §7 (upstream tracking) — passive.

### Sequential (must wait)
- C6 after C5 (needs fork build).
- D2 after D1 (needs spec frozen).
- D3 after D2 (needs harness stable).
- P4 after P3 (needs multi-variant contract).
- P5 after P4 (needs numbers).

---

## 6. Infrastructure — Devnet Stability

**Priority:** High  
**Effort:** ½ day (known fix)  
**Root cause:** WSL2 clock jumps on host sleep/resume halt CometBFT consensus.

#### Fix (known working)
1. `docker compose down -v` (tear down devnet).
2. `wsl.exe --shutdown` (reset WSL2 VM, clears clock drift).
3. Relaunch via `devnet/scripts/run-devnet.sh`.
4. **Verify:** blocks advance past prior stall height before any deploy/benchmark.

#### Preventive measures to investigate
- Windows power settings: disable sleep during long devnet runs.
- WSL2 `ntp` / time-sync configuration (though manual `date -s` is reverted by WSL active sync within ~2s).
- Container-level `ntpdate` inside devnet nodes (may not help if WSL2 clock itself jumps).
- Consider moving devnet to a native Linux VM or cloud instance for stability.

#### Success Criteria
- Devnet runs for ≥4 hours without stall.
- Blocks advance consistently; `deploy-zk-verifier.sh` + `benchmark.sh` run clean.

---

## 7. Upstream Tracking

### cosmwasm#2685 — BN254 host functions

| Field | Value |
|-------|-------|
| Assigned | DariuszDepta |
| Status | No maintainer reply since assignment |
| Action | Ping politely on the issue (every ~2 weeks). Offer to open the PR if shape is confirmed acceptable. |
| Blocking? | No — our fork works. This is the canonical upstream merge path. |

### wasmvm#735 — Go-side wrappers

| Field | Value |
|-------|-------|
| Status | Open, no maintainer reply |
| Action | Same ping cadence. If "intentional, leave it out" → close, cosmwasm-only path. If "contribution welcome" → open follow-up PR. |
| Blocking? | No. |

### CosmosContracts/juno#1202 — v30

| Field | Value |
|-------|-------|
| Status | DRAFT (142 commits). Review merged 2026-05-28. |
| Action | Track Jake for mainnet governance proposal submission date. No technical action needed from us. |
| Blocking? | No — v30 is Jake's timeline. Our Track B (BN254 wasmvm v3 forward-port) should wait until v30 mainnet upgrade lands. |

---

## 8. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Devnet keeps stalling despite fix | Medium | High | Move to cloud VM or native Linux for benchmarking |
| ~~Go stdlib lacks ML-DSA-44 (D2)~~ | — | — | **RESOLVED 2026-06-17:** used pure-Go `cloudflare/circl` ML-DSA-44; no CGO needed |
| ~~CometBFT fork API drift (C5)~~ | — | — | **RESOLVED 2026-06-17:** additive hybrid handshake folded into cometbft v0.38.x fork; go test ./p2p/conn/ green incl. existing evil+golden tests UNCHANGED |
| MAYO-5 gas > block limit (P4) | Medium | High | Precompile path; verify-once-store-hash pattern |
| cosmwasm maintainer unresponsive | Medium | Low | Our fork is functional; upstream merge is nice-to-have |
| v30 delays | Medium | Low | Independent of our work; Track B waits anyway |

---

## 9. Next Actions (Immediate)

1. **C6 hybrid-transport RTT:** build forked `cmd/cometbft`, run two local nodes with `AEGIS_HYBRID_TRANSPORT=1` vs classical, measure handshake RTT (self-contained — does NOT need the Juno devnet/`wsl --shutdown`).
2. **§6 Devnet stability:** apply the `wsl --shutdown` fix and verify stability (unblocks **P4** specifically).
3. **P4 Fable benchmark ladder:** MAYO-2/3/5 gas matrix once devnet is stable.
4. **D3 wiring (gated):** proto/codec + keyring/CLI + gas case (needs protoc/buf).
5. **§7 Upstream:** schedule a polite ping on cosmwasm#2685 (not urgent).

**Done since plan creation:** D1 (ADR-007) ✅, D2 (aegis-accounts, 20/20 green) ✅, P3 (found already done) ✅, **C5 (CometBFT fork build+tests) ✅**, **D3 core (SDK crypto/keys/hybrid, 9/9 green) ✅**.

---

## 10. Definition of "Done" for This Plan

- [~] Phase C: CometBFT fork builds ✅ + tests pass ✅ (C5); devnet/real-link RTT measured and documented (C6 — pending).
- [x] Phase D: ADR-007 spec committed ✅; aegis-accounts/ harness tests green ✅ (20/20); SDK fork core done ✅ (D3 — crypto/keys/hybrid 9/9 green; proto/keyring/CLI/gas wiring gated).
- [~] Fable: multi-variant contract tests pass ✅ (P3); benchmark ladder published (P4 — devnet-gated); precompile numbers in hand (P4).
- [ ] Infrastructure: devnet stable for ≥4-hour runs.
- [ ] Upstream: at least one follow-up ping sent on each open issue.

**Last updated:** 2026-06-17
