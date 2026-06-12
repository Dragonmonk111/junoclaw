# Draft comment — CosmWasm/cosmwasm#2685 (to @DariuszDepta)

> Paste target: new comment on https://github.com/CosmWasm/cosmwasm/issues/2685
> Context: thread now assigned to @DariuszDepta, labeled `g:fea` (Feature).
> Purpose: confirm the upstream shape is a patch-series PR branch (not a fork),
> and narrow the open questions to the ABI/gas/capability decisions only.
> Tone: defer to maintainer pacing; we wait for "send it" before opening the PR.

---

Thanks for picking this up, @DariuszDepta — no rush at all on our side, and happy to work at whatever pace suits the new maintenance setup.

One thing I want to make easy for you: **how we'd deliver this upstream.**

The whole change is authored as a **numbered patch series** (9 patches against `cosmwasm`, one concern each) that we maintain as the source-of-truth and re-baseline against new tags with a scripted harness. It currently verifies **10/10 clean against `v3.0.6`** (`cosmwasm-crypto-bn254` 22/22, `cosmwasm-vm` 318/319 — the one failure is the pre-existing `contract_with_floats_passes_check` float flake on Windows + wasmer 5, reproduces on vanilla unpatched v3.0.6).

So when you're ready, **what we'd open is an ordinary feature-branch PR** — one commit per patch, reviewable as a normal diff. There's no fork or vendoring you'd need to take on; a tagged convenience fork exists purely for the Juno v30 chain timeline and is generated from the same patches, so the PR branch and that tag are byte-identical. Upstream only ever sees the patch series.

**The patches break down as:**
- `crypto`: new `packages/crypto-bn254/` crate (arkworks `ark-bn254 0.5`, same backend the Juno zk-verifier already uses, so native and Wasm paths are bit-identical — we have a 1,000-proof differential test, 1000/1000 agree).
- `vm`: registers `bn254_add` / `bn254_scalar_mul` / `bn254_pairing_equality` as host-fn imports with gas hooks; capability `cosmwasm_2_3`.
- `std`: guest-side `extern "C"` decls + `Api::bn254_*` trait methods + mock impl.

**The only decisions that are genuinely yours, before we open anything:**
1. **Feature/capability home** — `cosmwasm_2_3`, or a different gate (e.g. a dedicated `bn254` capability)?
2. **ABI** — three fns, EIP-196/197 byte layout (`bn254_add` 128→64 B, `bn254_scalar_mul` 96→64 B, `bn254_pairing_equality` 192·N→1 bit). Keep as-is, or reshape?
3. **Gas schedule** — EIP-1108 constants × 100 (matching the existing BLS12-381 multiplier). Methodology sound?
4. **Empty-pairing semantics** — we return `Ok(true)` per EIP-197. Confirm?
5. **Subgroup checks** — `is_on_curve` then `is_in_correct_subgroup_assuming_on_curve` on every G2 decode. Matches your soundness expectations?
6. **Scope** — intentionally just the three fns; no `hash_to_curve`, no signature verify. Acceptable minimal scope, or would you want more/less in the first PR?

No need to answer all of these inline — even a "yes to the shape, open it against `main`" is enough and we'll handle the rest in review. We'd genuinely rather wait for your read than open a PR that needs reshaping.

The wasmvm-side companion question (CosmWasm/wasmvm#735) resolved itself, fwiw: BLS12-381 has no Go-side wrappers in v3.0.4, so there's no parallel surface to mirror — BN254 is cosmwasm-only. That issue can close once you confirm the absence is intentional.

Full design + measured numbers are in the issue body above and `docs/BN254_PRECOMPILE_CASE.md` in the repo. Thanks again for taking this on.
