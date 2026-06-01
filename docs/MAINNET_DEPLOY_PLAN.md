# JunoClaw Mainnet Deploy Plan (`juno-1`)

Runbook for promoting the JunoClaw contract suite from `uni-7` testnet to Juno
mainnet. This is a **dependency-ordered** deploy: several contracts hold
addresses of others, so order matters and the wiring must be verified before
the system is considered live.

> Status: **draft / pre-flight**. Do not execute until every item in
> [§1 Preconditions](#1-preconditions) is satisfied. Mainnet `store-code` and
> instantiate are irreversible and cost real JUNO.

---

## 1. Preconditions (hard gates)

| Gate | Why |
|---|---|
| **Juno v30 live on mainnet** | `zk-verifier` Groth16 verification uses the BN254 precompile shipped in v30. On a pre-v30 chain it falls back to pure-Wasm (~420k gas) or fails. Confirm the upgrade height has passed. |
| **All `DETERMINISTIC_AUDIT.md` anchored to the release commit** | The `audit-bot` gate enforces this per-PR; re-confirm each audit's `Anchor commit:` matches the tag being deployed. |
| **CI green on the release tag** | `contracts` + `crates` + `frontend` jobs in `.github/workflows/ci.yml` must pass. (The `lint` job is advisory.) |
| **OCI artifact verified** | `cosign verify --key cosign.pub ghcr.io/dragonmonk111/junoclaw/verifier:<tag>` |
| **Mainnet `store-code` permissions checked** | Juno mainnet has historically gated `MsgStoreCode` behind governance / an allow-list. Confirm whether the deployer address can store directly or whether a **store-code governance proposal** is required. This determines the whole flow below. |
| **Funded deployer** | Mainnet JUNO for store + instantiate + buffer. Estimate: ~11 stores + ~8 instantiates. |
| **Secrets out of the repo** | Deployer mnemonic via `MNEMONIC` env or `PARLIAMENT_ROLE` → `wavs/bridge/parliament-state.json` (gitignored). Never echo to logs. |

---

## 2. Build reproducible WASM

Use the cosmwasm optimizer so on-chain code hashes are reproducible and
auditable (raw `cargo build` wasms are testnet-only):

```bash
docker run --rm -v "$(pwd)/contracts":/code \
  --mount type=volume,source=junoclaw_cache,target=/code/target \
  cosmwasm/optimizer:0.16.0
```

Artifacts land in `contracts/artifacts/*.wasm` with a `checksums.txt`. Record
the sha256 of every wasm in the deploy log and cross-check against the audited
commit.

---

## 3. Configuration

Copy `deploy/.env.example` → `deploy/.env` and set mainnet values:

```env
CHAIN_ID=juno-1
RPC_URL=<mainnet RPC>            # e.g. a provider you trust; not a testnet endpoint
GAS_PRICE=0.075ujuno            # NOTE: ujuno (mainnet), not ujunox
ARTIFACTS_DIR=<abs path to contracts/artifacts>
AUTO_CONFIRM=false              # keep interactive confirmations ON for mainnet
```

The deployer (`deploy/deploy.mjs`) writes progress to `deploy/deployed.json`
incrementally, so a mid-run failure is resumable — it skips already-stored /
already-instantiated contracts.

---

## 4. Store code (dependency order)

Store in this order so instantiation can proceed top-to-bottom. If mainnet
requires governance for `store-code`, submit one proposal per code (or a batch)
and capture each resulting `code_id` before moving on.

1. `agent-registry`
2. `escrow`
3. `task-ledger`
4. `agent-company`
5. `zk-verifier`
6. `moultbook-v0`
7. `ibc-task-host`
8. `junoswap-factory`
9. `junoswap-pair`
10. `faucet` *(optional on mainnet — likely omit)*
11. `builder-grant` *(optional)*

---

## 5. Instantiate + wire (the critical sequence)

Addresses are not known until instantiation, and three contracts reference
others. Follow this exact order:

1. **`agent-registry`** — no contract deps.
2. **`escrow`** — references `task_ledger`. Since task-ledger isn't live yet,
   either (a) instantiate escrow after task-ledger, or (b) instantiate with a
   placeholder and update via admin. **Prefer ordering task-ledger first** (see
   note below) to avoid a placeholder.
3. **`task-ledger`** — references `agent_registry` (now known). Leave
   `agent_company: null` for now.
4. **`agent-company`** — references `escrow`, `agent_registry`, `task_ledger`.
5. **Back-wire**:
   - `task-ledger` → set `agent_company` to the agent-company address
     (the gateway and bridge resolve child addrs from agent-company::Config, so
     this link must be correct).
   - `escrow` → ensure `task_ledger` points at the real task-ledger (not the
     placeholder).
6. **`zk-verifier`** — instantiate with the verifying-key(s); confirm it is
   reachable from task-ledger's attestation path.
7. **`moultbook-v0`** — standalone; record address for the frontend + the
   agent-company `moultbook` optional field (ADR-005 skill-circle).
8. **`ibc-task-host`** — configure ICS-20 / packet-forward channel + allowed
   counterparties.
9. **`junoswap-factory`** then **`junoswap-pair`** instances as needed.

> **CRITICAL — set the wasmd migrate admin on every instantiate.** Pass the 6th
> arg `{ admin: <deployer or gov> }`. Omitting it makes the contract
> **permanently immutable** — exactly the v6 footgun that forced the Tier-1.5
> fresh-deploy detour (`docs/TIER15_TESTNET_RUN.md`). For mainnet, set the admin
> to a **governance / multisig** address, not a hot key.

---

## 6. Post-deploy verification

- Query `agent-company::Config` and assert every child address
  (`task_ledger`, `escrow`, `agent_registry`, `zk_verifier`) resolves correctly.
- Run a read-only smoke against mainnet (adapt `deploy/smoke-*.mjs`): list
  members, query a task, query an agent.
- Submit one **end-to-end attestation** with a known-good Groth16 proof through
  `zk-verifier` to confirm the v30 BN254 precompile path executes within the
  expected gas envelope (~250k).
- Verify on-chain code hashes == §2 checksums.

---

## 7. Switch off-chain services to mainnet

| Component | Change |
|---|---|
| Frontend | `frontend/src/lib/chain-config.ts` → `chainId: 'juno-1'`, mainnet RPC/REST, `denom: 'ujuno'`, and replace every address in `CONTRACTS`. The Contracts tab will then surface the mainnet registry. |
| Nostr bridge | `JUNOCLAW_CHAIN_ID=juno-1`, mainnet `JUNOCLAW_RPC`, `JUNOCLAW_CONTRACT=<mainnet task-ledger>`. **Dry-run first** (`--dry-run`) to validate the live event path before publishing with the real key. |
| x402 gateway | `agent_company` = mainnet address; `chain_id=juno-1`; reconfirm `max_task_ujuno` value cap. |

---

## 8. Rollback / migrate plan

- Because admin is set to governance/multisig, a buggy contract can be migrated
  via `MsgMigrateContract` to a fixed code-id rather than redeployed. Keep the
  audited migrate path tested on testnet first (`deploy/migrate-tier15.mjs` is
  the reference pattern).
- If a contract was instantiated with a wrong child address and exposes an admin
  setter, fix via that setter; otherwise migrate.
- Maintain `deploy/deployed.json` (mainnet copy) under version control of record
  (not necessarily the public repo) as the source of truth for addresses.

---

## 9. Governance proposal (if required)

If mainnet gates `store-code` and/or this is positioned as an official
deployment, prepare a governance proposal package:

- Rationale + audit links + cosign verification command.
- Code hashes for each wasm.
- The instantiate plan (this document, §5).
- Reference: prior community work — Juno v30 review (commit `e5ec25e`), prop
  #373 precedent.

---

_Last updated: 2026-06-01. Owner: VairagyaNodes / Dragonmonk111._
