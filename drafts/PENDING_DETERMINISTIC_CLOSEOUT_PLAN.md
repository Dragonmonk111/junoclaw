# Pending Work — Deterministic Closeout Plan

> Snapshot: 2026-07-20. Goal: define the end-state for every open workstream so we can ship or defer cleanly.

---

## 1. Juno v30 Mainnet Upgrade

**Why**: v30 is consensus-breaking. Our mainnet validator must be on the right binary before halt height.

**End state (deterministic)**:
- [ ] `junod-v30.0.0` binary built from commit `c0b3a8d258d52d16e5bc39a75168a99aab9d098e` and SHA-256 recorded
- [ ] Binary staged on mainnet node under Cosmovisor `upgrades/v30/bin/`
- [ ] Current v29 binary + data snapshot backed up
- [ ] `minimum-gas-prices` in `app.toml` set to `""` or `0.075ujuno`
- [ ] Mainnet upgrade proposal identified, voted on if validator has voting power
- [ ] Node produces blocks past upgrade height; post-upgrade verification queries pass
- [ ] Rollback plan tested (can restore v29 snapshot if v30 fails)

**Current status**: v30 build restarted in background (PID/log at `/root/juno-v30-build.log`). Testnet prop #5 already passed; no testnet node needed.

**Next action**: wait for build to finish, verify SHA-256, then stage on mainnet node.

---

## 2. Prediction Market Contribution (CosmosContracts/pm)

**End state**:
- [ ] PR #11 merged or closed with required changes
- [ ] Issue #12 receives maintainer feedback; scope is either approved, modified, or rejected
- [ ] If approved, open the agreed first PR (likely integration tests, no behavior change)

**Current status**: PR #11 open, no comments. Issue #12 open, no comments. PR #1202 (juno v30) merged; dimiandre approved.

**Next action**: nudge Jake/reviewers on Issue #12 after testnet upgrade noise settles, or proceed with PR 1 if no objection within a week.

---

## 3. TEE Infrastructure & A034

**End state**:
- [ ] A034 submitted to Juno Agents DAO and passes
- [ ] Treasury JUNO disbursed to designated ops wallet
- [ ] GCP `n2d-standard-2` Confidential VM (spot) deployed with WAVS Docker stack
- [ ] Sealed key generated inside TEE; attestation verified; invoke server live
- [ ] `moultbook.js` pointed at invoke endpoint; plaintext mnemonic retired from production
- [ ] Akash provider bounty posted by Jake's agent; at least one provider engaged or bounty closed after 3 months
- [ ] First monthly Moultbook report posted (uptime, costs, attestation)

**Current status**: A034 draft updated with treasury execution/payment plan. Build artifacts ready.

**Next action**: finalize recipient address on mainnet, then submit A034.

---

## 4. Project Aegis (PQC Consensus/Accounts)

**End state**:
- [ ] Runbook updated with fork SHAs and 15,208 B hybrid commit size
- [ ] `junod-aegis` rebuilt from pushed forks (not local clones)
- [ ] Wasmvm ML-DSA Phase B patches regenerated; devnet image rebuilt; ML-DSA-44 vs 65 gas benchmarked
- [ ] Aegis consensus article updated with 6.71× commit-size figure
- [ ] Temp push dirs and helper scripts cleaned
- [ ] C6 hybrid-transport RTT measured on devnet
- [ ] D3 wiring (proto/keyring/CLI/gas) implemented in SDK fork
- [ ] Fable P4 MAYO gas ladder integrated

**Blockers**: stable devnet with v30-aligned dependency versions; Juno pinned SDK version not present in junoclaw repo.

**Next action**: rebase Aegis forks on v30 tags once v30 mainnet is stable; then rebuild devnet image and benchmark.

---

## 5. MAYO / ML-DSA Credential Contract

**End state**:
- [ ] Devnet stability issue (WSL2 clock jumps) resolved or bypassed
- [ ] `jclaw-credential` instantiated on devnet with live MAYO + ML-DSA attestation flows
- [ ] Gas costs measured for MAYO-2, ML-DSA-44, ML-DSA-65 verification
- [ ] MAYO-3/5 streaming AES-CTR support added
- [ ] ZK proof-of-verification circuit scoped or deferred with rationale
- [ ] IBC cross-chain MAYO verification scoped or deferred
- [ ] MAYO CLI command added to `junoclaw-cli`

**Blockers**: devnet stability; v30 dependency alignment.

**Next action**: run contract tests under `cw-multi-test` as the stable signal until devnet is reliable.

---

## 6. WAVS / Sealed Signer

**End state**:
- [ ] Full E2E with real `wasmtime` + actual sealed blob on TEE hardware
- [ ] SGX determinism re-run completed
- [ ] Invoke server deployed behind authenticated endpoint
- [ ] Sealed signer integrated into `moultbook.js` production path

**Current status**: Invoke API 15/15 smoke tests pass. Code ready for TEE deployment.

**Next action**: wait for A034 passage and GCP VM deployment.

---

## 7. A13 DAO Heartbeat

**End state**:
- [ ] A13 proposal executed
- [ ] Heartbeat digest generated; empty-data bug fixed if present
- [ ] SHA-256 of digest base64-encoded as Moultbook commitment
- [ ] Post tx broadcast to `moultbook-v0` from `junoclaw-agent`
- [ ] Entry stats verified and new `moult:<id>` reported

**Current status**: waiting for proposal to pass/execute.

**Next action**: poll proposal status; run digest tool after execution.

---

## 8. J-Reef / J-Lens

**End state**:
- [ ] J-Reef schema locked and prototype stub created
- [ ] Shortlisted open-weight model chosen for first J-Lens probe
- [ ] J-Lens architecture doc expanded with D1 probe design
- [ ] Full D1 probe implementation remains gated until model is chosen

**Current status**: plan doc exists; no model selected yet.

**Next action**: evaluate Qwen2.5, Llama-3.1, Mistral-Nemo, Gemma-2 for probe feasibility.

---

## 9. A18d DAO Tooling Upgrade

**End state**:
- [ ] Heartbeat watcher hardened: wallet balance monitoring, low-gas alerts, health dashboard
- [ ] Ceremonial Knowledge Moult mint auto-drafted on A18c-tagged proposal execution
- [ ] Automation hot wallet auto top-up from DAO treasury on low-gas alert
- [ ] First ceremonial Knowledge Moult mint broadcast (A18c → A18c-6 series)

**Current status**: deferred, no formal proposal drafted.

**Next action**: draft A18d proposal after A034 and v30 upgrade are resolved.

---

## 10. Reece Bot / Cross-Agent Coordination

**End state**:
- [ ] Outreach to `reece_bot` completed or deliberately skipped
- [ ] Governance coverage responsibilities clarified between Juno Agents watcher and Reece

**Current status**: observed on-chain; no contact attempted.

**Next action**: low priority; revisit if governance duplication becomes a problem.

---

## Immediate Priority Queue (next 48 hours)

1. Finish v30 binary build → stage on mainnet node → monitor testnet upgrade at height 16034000.
2. Submit A034 to DAO.
3. After testnet upgrade, run v30 smoke queries and optionally upload a test contract via builder wallet.
4. Resume PM Issue #12 follow-up once v30 is live.
