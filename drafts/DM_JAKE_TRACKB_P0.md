# Draft DM — Jake Hartnell (Track B / P1 vs P2)

> Ready to copy-paste into Telegram / Discord / Twitter DM.

---

**Subject:** BN254 Track B — one decision before we open the upstream PR

Hey Jake — quick update on the BN254 precompile work. The forward-port to v3.0.x is done:

- **10/10 patches** apply clean against `cosmwasm` v3.0.6 (what `wasmvm` v3.0.4 resolves to).
- **22/22** `crypto-bn254` tests pass; **318/319** `cosmwasm-vm` tests pass (the 1 failure is the pre-existing Windows + wasmer 5 float flake).
- **Measured gas reduction** on the devnet: pure-Wasm 370,498 → precompile 203,164 (**1.823×**). Results auto-generated in `BN254_BENCHMARK_RESULTS.md`.

The only blocker before opening the PR against `CosmWasm/cosmwasm` is **how we ship it**:

| Option | What it means | Speed | Maintenance |
|---|---|---|---|
| **P1 — Fork + tag** | I fork `cosmwasm` → apply patches → tag `v3.0.6-bn254`; fork `wasmvm` → one-line `[patch.crates-io]` → tag `v3.0.4-bn254`. Juno v30 `go.mod` gets a single `replace` line. | **Fast** — we have a tagged release today | On us to rebase on every upstream release |
| **P2 — Patch series only** | No fork. Juno's build script applies the 10 patches at build time against whatever `cargo` resolves. | **Slower upstream path** — but zero fork maintenance for us | Zero; patches rebase automatically on `cargo update` |

**My read:** P2 is cleaner long-term but needs the patches to be accepted upstream first. P1 is the pragmatic path if we need something shippable for Juno v30 before the upstream review concludes.

**Question for you:**

1. **Do you want to own the upstream PR?** (I can prepare the branch + draft PR body; you open it from your account — stronger signal to CosmWasm maintainers.)
2. **Or should I open it from `Dragonmonk111/cosmwasm`?** (Fine either way; your call on who the "author" face is.)
3. **P1 or P2?** If Juno v30 needs a tagged wasmvm to point at before the upstream merge lands, P1 is the only option. If we can wait for upstream, P2 is better.

The signaling proposal (`GOV_PROP_COPYPASTE_BN254.md`) is ready to submit — just waiting on this call so I can reference the right upstream path in the proposal text.

Let me know and I'll move same-day.

— VairagyaNodes / Cascade
