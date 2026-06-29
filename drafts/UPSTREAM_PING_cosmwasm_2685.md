# Upstream reply draft — CosmWasm/cosmwasm issue #2685

Topic: BN254 (alt_bn128) host functions for Groth16 verification
Where to post: https://github.com/CosmWasm/cosmwasm/issues/2685
Tone: polite, low-pressure, respect the maintainers' deferral.

> **Context (2026-06-29):** @DariuszDepta replied and moved this to the Backlog
> milestone. The team is mid-redesign of the CosmWasm libraries (API, capabilities,
> performance, gas) and **will not take external proposals of this size until ~end of
> Q3 / start of Q4**. They asked us to **maintain our branch until then**, and asked
> (P.S.) **how many chains besides ours would benefit**. This reply acknowledges the
> deferral and answers the P.S. — it does NOT push to open a PR.

---

Hi @DariuszDepta — thanks, that's completely fair, and it makes sense to land this once the
new API/capabilities/gas direction is settled rather than bolt it on beforehand. We're happy
to keep maintaining our branch (`cosmwasm-std-bn254-ext` + a patched `wasmvm`) in the meantime,
and we'll re-sync to whatever shape the redesign lands on. No rush on our side.

When you're ready (end of Q3 / Q4), we already have a working implementation + tests we can
reshape into a PR, so just ping us.

**Re: who else benefits —** BN254 / `alt_bn128` is the de-facto pairing curve for the existing
Groth16 tooling (circom / snarkjs), and it's an Ethereum precompile (since Byzantium), so any
Cosmos chain reusing that proving stack is a candidate. The concrete categories we see:

- **ZK light clients / bridges** verifying Groth16 proofs on-chain (the most common ask).
- **zk-rollups settling to a Cosmos chain**, where the settlement contract checks a pairing.
- **Private identity / credential / voting contracts** (this is our use case in JunoClaw —
  on-chain attestation verification).
- **General zkSNARK verifier contracts** that today either ship a pure-Wasm pairing (very
  expensive gas) or, like us, depend on a custom host-function fork.

Today those all either pay a large gas premium for in-Wasm pairings or carry a private fork —
which is exactly why a capability-gated upstream host function would help the wider ecosystem,
not just us. Whenever the redesign opens up, we'll bring concrete gas numbers to the table.

Thanks again for the heads-up on timing — happy to wait.

---

## Internal notes (do NOT post)

- Status: **deferred by maintainers to ~end Q3 / start Q4 2026** (Backlog milestone). Keep the fork.
- Next check: **late Q3 2026**, or sooner if a CosmWasm API-redesign discussion opens.
- When reshaping the PR later: standalone `bn254_add` / `bn254_scalar_mul` / `bn254_pairing`
  (mirroring the BLS12-381 host-fn pattern), capability-gated, with our measured gas schedule.
- Do NOT re-ping before Q3-end — they explicitly asked for space.
