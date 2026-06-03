# Jake FYI — Upstream Issues Published

**Sent:** 2026-06-03
**Channel:** Telegram DM
**Tone:** FYI only, zero ask

---

## Message text (copy-paste verbatim)

```
Hey Jake — quick FYI: opened upstream issues on CosmWasm/cosmwasm and wasmvm
for the BN254 precompile work (post-#374).

Links:
• cosmwasm: https://github.com/CosmWasm/cosmwasm/issues/2685
• wasmvm: https://github.com/CosmWasm/wasmvm/issues/735

No ask, just keeping you in the loop. Ethan and Simon cc'd on both.
```

---

## Context

These are the Phase 1 upstream issues from [`POST_VOTE_EXECUTION_PLAN.md`](./POST_VOTE_EXECUTION_PLAN.md). Both opened on 2026-06-03:

- **Issue 1** (`CosmWasm/cosmwasm#2685`): Proposes BN254 host functions (`bn254_add`, `bn254_scalar_mul`, `bn254_pairing_equality`) behind a `cosmwasm_2_3` feature flag. Cites measured 1.823× gas reduction, 3 parallel patch series (v2.2.2 / v2.2.7 / v3.0.6), and 6 confirmation questions for maintainers.
- **Issue 2** (`CosmWasm/wasmvm#735`): Asks whether Go-side wrappers are intentional design or missing feature. Includes 2 side-findings (`__rust_probestack` linker error on v2.2.x, BLS12-381 has no Go wrappers in any version).

Per the plan's pacing: no Twitter/Discord broadcast until at least one substantive maintainer reply. Silent issues + public broadcast = reads as stalled project.
