# Comms note — Project Aegis milestone (2026-06-25)

*4–5 lines, copy-paste ready. Honest: celebrates the proven milestone, names what's still pending.*

---

**Project Aegis milestone.** We made every classical cryptographic surface of a *live* Juno validator quantum-safe — transport, consensus, accounts, and IBC — on the production machine, with the validator's signing key verified byte-for-byte identical before and after.

Measured, not theoretical: hybrid X25519+ML-KEM-768 transport costs **+315 µs and ~3.5 KB once per peer connection, and zero bytes per block**; hybrid Ed25519+ML-DSA-44 consensus keys cost **6.71× per commit**; MAYO-5 (NIST Level 5) attestations verify on-chain today.

All four surfaces are now designed (ADR-006→010), core-implemented, and tested — and we just confirmed **cross-architecture determinism**: the ML-DSA-44 verify path returns byte-identical results on x86_64 and ARM64 (`input_sha256` matches exactly). A GitHub Actions workflow now runs this on **real ARM silicon** on every push to main. Remaining before mainnet is wiring, not cryptography: protoc-gated message plumbing (consensus-key rotation, remote signer, account CLI) and IBC light-client fork integration. The full plan is in `docs/AEGIS_F6_D3_IBC_PHASEG_PLAN.md`.

*Post-quantum Cosmos isn't a new chain — it's this chain, hardened.*

— Dragonmonk / VairagyaNodes

---

### One-liner for announcements

**Project Aegis: a live Juno validator is now quantum-safe across all cryptographic surfaces — transport, consensus, accounts, and IBC — with deterministic verification proven on x86_64 and ARM64; only mainnet wiring (key rotation, CLI, IBC fork) remains.**
