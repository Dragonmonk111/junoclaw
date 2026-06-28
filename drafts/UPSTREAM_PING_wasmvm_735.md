# Upstream ping draft — CosmWasm/wasmvm issue #735

Topic: should BN254 host functions expose Go-side wrappers, or stay VM-internal (BLS12-381 precedent)?
Where to post: https://github.com/CosmWasm/wasmvm/issues/735
Tone: polite, concrete, decision-seeking. Bi-weekly cadence.

---

Hi team — quick follow-up on this one, which is the companion to CosmWasm/cosmwasm#2685. 👋

Our open question is purely about the **integration pattern**, and your answer determines
whether our `do_bn254_verify` work can be upstreamed as a clean patch:

- **Option A — VM-internal only** (the BLS12-381 precedent): the BN254 ops live entirely inside
  the VM and are reachable only via the cosmwasm-std host-function imports. No new Go-side public API.
- **Option B — Go-side wrappers too**: expose Go wrappers (analogous to other `api/` functions) so
  chains can call BN254 ops directly from Go as well.

We currently run Option A in our fork (it's the smaller surface and matches BLS12-381), and we're
happy to align with that if it's your preferred direction. If you'd rather have Go-side wrappers,
we can prepare that instead — we just want to match the pattern you'll accept before sending a PR.

We've drafted a PR description at our end (`docs/WASMVM_BN254_PR_DESCRIPTION.md`) and can adapt it
to whichever direction you choose. Could you confirm A vs B? Thanks!
