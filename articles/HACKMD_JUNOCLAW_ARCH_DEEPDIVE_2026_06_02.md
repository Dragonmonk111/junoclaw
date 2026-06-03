# JunoClaw — Architecture Deep-Dive & Engineering Backlog (peer-dev edition)

###### tags: `junoclaw` `cosmwasm` `wavs` `zk` `cosmos`

> Internal / peer-dev companion to the public status note. Audience: people who
> will read the code. Everything here is grounded in a 2026-06-02 deterministic
> pass over the tree; file paths are real. Numbers: 12 off-chain crates + 12
> contracts, 204 contract tests + off-chain suites green, live on `uni-7`.

---

## 1. Workspace topology

Two Cargo workspaces, deliberately split:

- **`contracts/`** (`resolver = 2`, `cosmwasm-std 2.2`, `cw-storage-plus 2.0`):
  `junoclaw-common`, `agent-registry`, `task-ledger`, `escrow`,
  `agent-company`, `junoswap-factory`, `junoswap-pair`, `faucet`,
  `builder-grant`, `zk-verifier`, `moultbook-v0`, `ibc-task-host`.
  Arkworks `0.5` (`ark-bn254`, `ark-groth16`, …) pinned for `zk-verifier`.
- **root `/`** off-chain: `junoclaw-core`, `-runtime`, `-cli`, `-daemon`,
  `-x402-gateway`, `-github-agent`, `-nostr-bridge`, `-ibc-relay`, plus seven
  `plugins/*`. `contracts/`, `wavs/`, `circuits/`, `wasmvm-fork/`, `devnet/`,
  `tools/bud-seal` are **excluded** (each has its own workspace table — notably
  the BN254 precompile crates).

Release profile is hardened for determinism on both: `lto = true`,
`codegen-units = 1`, `panic = "abort"`, `overflow-checks = true`.

---

## 2. The on-chain / off-chain boundary (read this first)

This is the most important architectural fact and the source of the biggest
backlog item.

```
            writes (today)                         reads
  ┌─────────────────────────────┐      ┌──────────────────────────────┐
  │ deploy/*.mjs   (holds key)  │      │ junoclaw-runtime (daemon)     │
  │ x402-gateway   (keyless     │      │  - watches chain events       │
  │                pass-through)│      │  - parses tasks               │
  │ frontend       (Keplr)      │      │  - builds + signs Nostr evts  │
  └─────────────────────────────┘      │  - DRY-RUN for chain writes   │
                                        └──────────────────────────────┘
```

Grep result that pins it down: the only `broadcast_*` call in `crates/` is
`junoclaw-x402-gateway/src/cosmos.rs` (`raw.broadcast_commit`), and it
broadcasts **client-signed** bytes — the gateway holds no key. The daemon
runtime never signs. DAO deploy returns `"dry_run_ready"`; the moultbook
endorsement query returns `[]`. So the autonomous agent runtime is, today,
**read + dry-run only**.

---

## 3. Six-layer model

| Layer | Contracts / services | Notes |
|---|---|---|
| Identity | `agent-registry`, membership circuit | soulbound; Merkle root of members |
| Coordination | `agent-company`, `task-ledger`, `escrow` | DAO → queue → non-custodial pay |
| Verification | `zk-verifier`, WAVS operators | BN254 Groth16 on-chain (~250k gas w/ v30 precompile, ~420k pure-wasm) |
| Privacy | `moultbook-v0` | anon publish; ZK membership, untraceable author |
| Bridges | `ibc-task-host`, `nostr-bridge`, `x402-gateway` | IBC / push discovery / HTTP 402 |
| DeFi | `junoswap-factory` + `-pair`, `faucet`, `builder-grant` | denom-whitelist AMM, milestone grants |

Child-address resolution: `agent-company::Config` holds `task_ledger`,
`escrow`, `agent_registry`, `zk_verifier`. The x402 gateway and the bridge both
resolve children through it — so that wiring must be correct on deploy (see the
mainnet runbook's §5 back-wiring step).

---

## 4. Engineering backlog (by leverage)

### P1 — Wire guarded on-chain signing into the daemon
The "demo → autonomous" line. `junoclaw-runtime/src/lib.rs` has the seams:
`deploy_dao` → `"dry_run_ready"` (`// TODO: submit InstantiateMsg via CosmWasm
tx when chain signing is wired`) and the moultbook query stub. Proposal:

- Reuse the x402 `PaymentEnvelope` shape internally, or add a `Signer` behind a
  trait with a `signing_paused` guard (the kill-switch already exists).
- Key handling: load from env/KMS, never disk; respect `egress_paused`.
- Start with the lowest-risk write (moultbook publish / task claim), not DAO
  instantiate.

### P2 — Moultbook read path
`moultbook_operator.rs` uses a placeholder CID
(`pending:skill_endorsement:…`) and `lib.rs` returns an empty endorsement list
because RPC query isn't wired. Wiring the read (`QueryMsg::ListByTopic`) is a
prerequisite for P1 and unblocks the frontend endorsements UI.

### P3 — `junoswap-factory` Reply handler
`junoswap-factory/src/contract.rs` stores a **placeholder** pair address with a
`// In production, use Reply to capture the instantiated address`. This is a
correctness bug: the factory can't reliably resolve spawned pairs. Implement
`reply` with a `SubMsg::reply_on_success` and parse the instantiate event.

### P4 — `moultbook-v0` derivation-proof verification
`disclose` accepts `derivation_proof` at face value
(`// TODO: verify derivation_proof via zk-verifier sub-message`). Closes the v0
trust gap; depends on the membership circuit gaining a "disclosure mode."

### P5 — Compute plugins
`plugin-compute-local` (Phase 1), `plugin-compute-akash` (Phase 4: SDL gen +
Skip swap + deploy), `plugin-ibc`, `plugin-browser` all return "not yet
implemented." `compute-local` is the shortest path to a first fully on-chain,
end-to-end task completion and should land before Akash.

### P6 — WAVS resolution + ecosystem convergence
`wavs/src/lib.rs` hashes the criteria as a placeholder instead of
template-specific resolution. Strategic call: evaluate **`Lay3rLabs/cw-middleware`**
(service handlers to CosmWasm chains) as the attestation path rather than
maintaining a bespoke TEE forever — see §6.

### P7 — `CancelTask`
WS `CancelTask` is a stub in `junoclaw-runtime/src/lib.rs`. Small control-plane
gap; do it when touching the runtime for P1.

---

## 5. Upstream version matrix (2026-06-02)

| Project | Latest | Action |
|---|---|---|
| Juno | `v29.0.0` (`a63f2d3`) | No `v30` tag yet; our fix is in the PR. Chain correctly on v29. |
| CosmWasm | `v3.1.0-rc.0` / stable `v3.0.7` | We're on `cosmwasm-std 2.2`. Track a CW 3.x migration spike (post-v30). |
| wasmd | `v0.55.0` | Precompile baseline. |
| DAODAO | `v2.8.0-alpha.2` | Role-based Auth (unaudited); **optimizer 0.17.0** — bump deploy/runbook (was 0.16.0). |
| WAVS / Layer | active | Jake Hartnell CEO, Ethan Frey CTO, $6M (1kx). `cw-middleware`, `wavs-github-rewards`, `layer-sdk`, templates. |

---

## 6. The WAVS convergence thesis

JunoClaw has described itself as "the WAVS pattern in miniature." That pattern
is now a funded product run by the people who built CosmWasm and Juno. Concrete
implication for our verification layer:

- **Today**: `zk-verifier` (on-chain Groth16) + a bespoke off-chain attested
  compute story (`wavs/`, the parliament `bridge/`, `chain-watcher/`).
- **Target**: route attested results through `cw-middleware`'s service-handler
  contracts so JunoClaw services are first-class WAVS services. This swaps
  "trust our TEE" for "trust the audited WAVS operator set," and inherits the
  ecosystem's tooling (`wavs-foundry-template`, `cw-component-template`).
- **Cheap first step**: stand up a single read-only WAVS service that mirrors
  the Nostr-bridge event resolution, and compare its attestation to our
  deterministic pipeline test. Zero on-chain risk, validates the integration
  surface.

---

## 7. Build / test / CI quick ref

```bash
# contracts
cd contracts && cargo test --workspace --locked
# off-chain crates
cargo test --workspace --locked
# frontend
cd frontend && npm run build
# nostr bridge — validate live path with no secrets
cargo run -p junoclaw-nostr-bridge -- --dry-run
```

CI (`.github/workflows/ci.yml`) gates on the three test/build jobs; `fmt` +
`clippy` are an **advisory** job (`continue-on-error: true`) because the tree
isn't yet rustfmt/clippy-clean — flip it to blocking after a dedicated cleanup
PR. There's a known pre-existing doctest fix already landed in
`junoclaw-github-agent` (`open_pull_request` returns `(String, u64)`).

---

## 8. Open questions for the team

1. Signing key custody for P1 — env/KMS vs a co-signer service? What's the
   acceptable blast radius for an autonomous hot key on mainnet?
2. CW 2.2 → 3.x: do we migrate before or after the mainnet deploy?
3. WAVS convergence (P6): integrate `cw-middleware` now (greenfield) or after
   `compute-local` proves the end-to-end loop?
4. Mainnet `store-code` permissioning — does the deployer store directly or via
   governance? (Gates the whole runbook.)

---

*Companion docs: `docs/OVERVIEW_BRIEF_2026_05_29.md` (status), `docs/MAINNET_DEPLOY_PLAN.md` (runbook). Last updated 2026-06-02.*
