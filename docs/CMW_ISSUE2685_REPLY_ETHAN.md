# Reply to Ethan Frey — CosmWasm/cosmwasm#2685

> Paste target: comment on https://github.com/CosmWasm/cosmwasm/issues/2685
> Context: Ethan replied that neither he nor Confio maintain CosmWasm anymore, and pointed us to @DariuszDepta. (Confio maintenance wind-down: https://medium.com/confio/confio-ends-cosmwasm-maintenance-55c64818f61a)

---

Thank you so much for the kind words, @ethanfrey — hearing you call this an "interesting addition" means a great deal coming from the person who built the foundation we're standing on. We deeply appreciate you taking the time to reply and point us in the right direction, especially given everything Confio has contributed to the ecosystem over the years.

cc @DariuszDepta — bringing you in per Ethan's note. Quick summary so you have the full picture without digging through the thread:

**What this is.** A proposal to add three BN254 (alt_bn128) host functions — `bn254_add`, `bn254_scalar_mul`, `bn254_pairing_equality` — behind a `cosmwasm_2_3` feature flag. These are the EIP-196/197 operations Groth16 zk-SNARK verification needs. It is motivated by Juno governance proposal [#374](https://ping.pub/juno/gov/374) (passed 2026-05-05, ~80% Yes), which is an on-chain mandate to get native BN254 into the Juno stack.

**Why it matters.** Measured **1.823× gas reduction** on a real Groth16 `VerifyProof` — 370,600 → 203,266 SDK gas per call, 5 samples, σ = 0, on an ephemeral single-validator devnet running `junod` linked against the patched `libwasmvm`. The saving is concentrated in the pairing check (the Miller loop + final exponentiation run natively instead of as Wasm-metered instructions).

**State of the work.** The forward-port is already green across **three parallel patch series**, so whichever tag you'd want to target is covered:
- `v2.2.2` — audit baseline, matches wasmvm v2.2.4 / Juno mainnet today (10/10 CLEAN, 22/22 + 311/311 PASS)
- `v2.2.7` — latest 2.2.x (10/10 CLEAN)
- `v3.0.6` — latest v3 (10/10 CLEAN)

**Our ask is just a shape check, not a merge.** Before we open a PR we'd like a maintainer's read on:
1. Is `cosmwasm_2_3` the right feature-flag home for these, or do you prefer a different gating?
2. Is the host-function ABI (the three fns above, byte-encoded points) acceptable as-is?
3. Any concerns with the proposed gas schedule before we wire it into the cost table?

We're more than happy to wait for guidance rather than open a PR that would need reshaping — we know good design takes time and we deeply respect the process. Whatever pacing works for the new maintenance setup is fine on our end.

Full design + measured numbers: the issue body above, plus `docs/BN254_PRECOMPILE_CASE.md` in our repo. The wasmvm-side companion question is at CosmWasm/wasmvm#735.

Thanks again to both of you for everything you've built and for taking the time to engage with this.
