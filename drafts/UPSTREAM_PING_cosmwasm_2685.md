# Upstream ping draft — CosmWasm/cosmwasm issue #2685

Topic: BN254 (alt_bn128) host functions for Groth16 verification
Where to post: https://github.com/CosmWasm/cosmwasm/issues/2685
Tone: polite, low-pressure, offer to do the work. Bi-weekly cadence.

---

Hi @DariuszDepta (and CosmWasm team) — friendly follow-up on this proposal. 👋

We're still running BN254 Groth16 verification through a maintained fork
(`cosmwasm-std-bn254-ext` + a patched `wasmvm`) and would much rather converge on an
upstream host-function API so the fork can eventually retire.

To make a decision easy, here's the shape we've been running in production-style devnets:

- **Host functions:** `bn254_add`, `bn254_scalar_mul`, `bn254_pairing` (capability-gated),
  mirroring the existing BLS12-381 host-function pattern.
- **Feature flag:** gated behind a `cosmwasm_2_x` capability so contracts can detect support.
- **Gas:** measured on our devnet — happy to share the full schedule; pairing dominates, and
  our numbers are well under block limits.

Could you let us know your preference on:

1. **ABI shape** — standalone `bn254_*` host fns vs a single batched pairing-check entry point?
2. **Feature/capability flag** — which `cosmwasm_x_y` gate you'd want it behind?
3. **Gas** — are you open to us proposing a schedule from our measurements?

If the shape is acceptable we're glad to open the PR (we already have a working implementation
and tests). No rush — just flagging that we're ready to contribute whenever it's useful.

Thanks for all the maintenance work during the transition period!
