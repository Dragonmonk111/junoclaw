# We Just Closed Three Aegis Gates in One Session

*2026-06-24 · Dragonmonk / VairagyaNodes*

This is a short operational note on what shipped today for Project Aegis — the migration path that puts post-quantum cryptography into an existing Cosmos chain instead of asking validators to move to a new one.

## 1. Reproducible `junod-aegis` binary

The Aegis Juno binary is the gate that wires the CometBFT and Cosmos SDK forks into a real Juno validator. We rebuilt it from the pushed forks:

- **SDK fork:** `Dragonmonk111/cosmos-sdk@aegis-phase-d3-hybrid` → `ea7c1af`
- **CometBFT fork:** `Dragonmonk111/cometbft@aegis-phase-cf-hybrid` → `7524f93`
- **Upstream:** `CosmosContracts/juno@v29.0.0`
- **Binary SHA-256:** `98e6813ac0dd8f004002a9b1896cea3f13362c4af985d40ffe2c9a73f2bcacdb`

GitHub shallow clones kept failing with `fetch-pack: unexpected disconnect`, so we synced the local `aegis-forks/` copies to the remote tips via HTTPS fetch and built from there. The resulting binary is 158 MB, lives at `/root/aegis-localnet-artifacts/junod-aegis`, and `version --long` reports the expected build graph. This is the exact binary that can run the hybrid-consensus localnet and the ML-KEM transport tests.

## 2. ML-DSA (FIPS 204) in wasmvm — Phase B closed

The ML-DSA patch series for cosmwasm v2.2.2 is now emitted and verified:

- `wasmvm-fork/patches/regen-mldsa.sh` regenerated patches 20–28.
- The full 00–28 series applies cleanly to a pristine `cosmwasm v2.2.2` checkout.
- The devnet image `junoclaw/junod-bn254:devnet` was rebuilt with the patches included.
- We built both `jclaw_credential_pure.wasm` (in-Wasm `fips204`) and `jclaw_credential_mldsa.wasm` (precompile via `env.ml_dsa_verify`) and benchmarked all three ML-DSA variants:

| Variant | Pure verify gas | Precompile verify gas | Saving |
|---------|----------------|-----------------------|--------|
| ML-DSA-44 | 269,604 | 260,381 | ~3.4% |
| ML-DSA-65 | 328,945 | 315,212 | ~4.2% |
| ML-DSA-87 | 408,298 | 387,124 | ~5.2% |

The precompile path is consistently cheaper and confirms the host-side wiring is working. Full JSON results are in `deploy/mldsa-devnet-benchmark-results.json`.

## 3. The 6.71× number is now in the article

The main Aegis consensus article, `drafts/ARTICLE_AEGIS_CONSENSUS_PQC_2026_06_23.md`, now includes the measured live-localnet result:

- Classical commit size at N=4: **2,267 bytes**
- Hybrid-44 commit size at N=4: **15,208 bytes**
- Increase: **6.71×**

The “What Is Next” table is updated to show the binary build, the bandwidth test, the ML-DSA patch regen, the devnet rebuild, and the gas benchmark as **Done**.

## What this means

The consensus-layer fork work is no longer just measured in a local clone — it is reproducible from the pushed forks, the wasmvm precompile path is verified on a live devnet, and the real bandwidth cost is documented. The remaining gates are transport-layer RTT measurement, IBC light-client migration design, and normal account-key migration planning — all downstream of the foundation that is now in place.

*Post-quantum Cosmos is not a new chain. It is this chain, hardened.*
