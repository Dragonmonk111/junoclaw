# Pre-Demo Checklist — JunoClaw g1 /attest surface

Status as of 2026-11-10. Everything below must be TRUE before this is
shown to anyone outside. Ordered roughly by dependency.

## Must-fix before ANY demo

- [ ] **Production signer story.** Demo ran on `attest-gw-test`
      (keychain wallet `juno1qehe…x75`, throwaway mnemonic). For real
      use: dedicated hot key whose ONLY privilege is anchoring — not
      jlens-ops (court admin + verifier bonds + ops balance). Decide:
      fresh merkle-verifier instance with gateway-key admin, or
      `transfer_admin` on `juno12k0sr…jz2xpq`.
- [ ] **Gateway behind TLS + reverse proxy.** Caddy/nginx on the OVH
      seat; gateway binds loopback only. Never expose :9090 publicly —
      the gateway is the only thing that should speak to it.
- [ ] **REST gateway deployed publicly.** `rest.g1.<domain>` behind the
      same proxy — this is what Anode/UTSA/third parties point at.
      Read-only subset only; `POST /txs` stays OFF (documented).
- [ ] **Fresh demo contract instances.** The demo used test instances
      (merkle `juno1t8zy…f5m4`, court `juno104r6t…k34xwt` with a
      deliberately small 50k bond). Decide whether demo shows those
      (fine — it's g1) or the production contract.
- [ ] **Demo API key issued + documented.** `ATTEST_API_KEYS` entry for
      the demo customer; key value given out-of-band, never in docs.

## Should-fix (credibility risks if skipped)

- [ ] **Faucet funded to a useful level.** Currently ~50k ujclaw (one
      drip). Fund to ~5–10 JCLAW via `Fund{}` from jlens-ops, or lower
      `drip_amount`. Contract: `juno1gfre33…mwuc5l`, admin jlens-ops.
- [ ] **Bonded-tier bond pool + finalize sweep.** Each bonded anchor
      locks the operator bond until finalize. Need a `finalize_attestation`
      sweep job or the gateway wallet drains on any real volume.
      Production court min bond = 1M ujclaw — price the tier.
- [ ] **`anchor_failed` observability.** Failed anchors currently just
      log + park. Before demo: a dead-letter view (GET /attest shows
      anchor_failed already) + an alert path.
- [ ] **Spool retention note in docs.** "Testnet data is public and
      permanent on-chain; don't attest secrets." One line, visible.
- [ ] **README for the demo flow.** POST → receipt → on-chain
      `verify_proof` → block cert at that height. The 4-step story a
      partner can run themselves.

## Nice-to-have / explicitly deferred

- [ ] `POST /cosmos/tx/v1beta1/txs` broadcast on the REST shim (needs
      abuse controls — rate limits aren't enough for free tx ingress).
- [ ] x402 payment envelopes on /attest (crate exists; wire later).
- [ ] Bonded-tier dispute lifecycle endpoints (`get_dispute` passthrough
      on receipts — currently receipt shows court_id, not dispute state).
- [ ] 4th verifier seat → k<n sortition test (all 3 seats = committee
      of 3 today; sortition untested below quorum pool).
- [ ] Per-customer `robot_id` namespaces (single `attest:default` now).
- [ ] ZK receipt tier (weeks-scale spike, planned separately).

## Demo script (the thing to actually show)

1. `POST /attest` with demo key → `202` + `attestation_id`
   (do a `tier: bonded` one too — shows the court path).
2. `GET /attest/{id}/receipt` ~60s later → anchored, leaf_hash, proof,
   court info for the bonded one.
3. On-chain `verify_proof` via the REST gateway or a small script —
   the proof we generated verifies under the contract's own algorithm.
4. Point at the block cert at `anchored_at_height` — hybrid BLS+MAYO2
   certificate over executed state, ~200ms finality.
5. Optional: `POST` to the faucet contract → fresh wallet gets drip.
6. Optional: show `Attestation already exists` recovery — the gateway
   reconciles g1's execute-then-fail quirk instead of double-anchoring.

## Known landmines (don't get caught on a call)

- g1 failed txs CONSUME sequence — retries must re-query, never offset.
- The mcp wallet auto-fee simulate double-consumes sequence; the Rust
  `chain-grpc` static-gas path is the reliable write path.
- g1 `GetTx` lags indexing — a just-broadcast tx can 404 for ~seconds.
- Timed contract ops (challenge/verdict windows) advance on BLOCK time,
  which only moves while blocks are produced.
- jlens-court `--update-config` flags are snake_case; kebab silently
  no-ops.
- "Only chain at 200ms" / "only single-cert finality" are FALSE claims
  (Monad/Sei/Sui/Tendermint-class BFT all do this). The honest claim is
  PQ-hybrid certs over executed state, portable and offline-verifiable.
