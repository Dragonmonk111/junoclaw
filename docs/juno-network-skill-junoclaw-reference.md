# JunoClaw on Juno

*This file is the working draft of `references/junoclaw.md` for [`CosmosContracts/juno-network-skill`](https://github.com/CosmosContracts/juno-network-skill). Once Jake nods, copy verbatim into the skill repo as a PR. Mirrors the format of `references/dao-dao.md` exactly: one-line lead, mainnet-first defaults, ops broken by intent, safety posture, going-further.*

*Working notes — do NOT include in the final reference, this section gets stripped before PR:*
- Mainnet code IDs are placeholder (`TBD-pending-mainnet-deploy`) until JunoClaw deploys all nine contracts on `juno-1`. Current state: prop #374 passed (BN254 signaling), but the contracts themselves are deployed on uni-7 / devnet only. Don't invent fake mainnet code IDs.
- Audit citations point at the `DETERMINISTIC_AUDIT.md` files in `Dragonmonk111/junoclaw` (all 9/9 complete per memory).
- Cross-link to dao-proposal-wavs §Member section (Jake's existing reference) for the on-chain attestation consumer side.

---

## §1 What this is

JunoClaw is a stack of nine CosmWasm contracts, one off-chain WAVS operator, and (post-v31) a BN254 host-function precompile that together let a DAO **hire, pay, and audit autonomous agents on-chain**. The pattern is: a DAO posts a structured task with constraints and an escrowed reward, an agent picks up the task and executes it inside a Trusted Execution Environment, the agent submits a Groth16 zero-knowledge proof attesting that the work matches the constraints, and the on-chain `zk-verifier` settles the escrow. Every step produces a public, cryptographically-verifiable receipt.

The nine contracts:

| Contract | Role |
|---|---|
| `task-ledger` | Queue. Tasks are posted here with constraints, deadline, reward. Indexed by `task_id` (monotonic) and `status` (`Open` / `Claimed` / `Submitted` / `Settled` / `Expired`). |
| `escrow` | Vault. Reward funds locked at task post; released to agent on settlement, returned to DAO on expiry. |
| `agent-registry` | Reputation. Tracks per-agent task counts, success rate, last-seen-attestation. Soulbound — no transfers. |
| `agent-company` | DAO + governance. Owns task posting, parameter updates (e.g. allowed verifying keys, max bounty), and constraint vocabulary. |
| `zk-verifier` | Cryptographic gate. Verifies a Groth16 proof against a registered verifying key. Constant gas regardless of underlying circuit complexity. |
| `junoswap-pair` | Optional DEX integration. Hardened denom-whitelisting prevents the canonical Uniswap-v2 first-depositor inflation attack. |
| `builder-grant` | Milestone-locked grants. Used by the agent-company DAO to fund longer-form work (paid out per milestone receipt). |
| `jclaw-token` | Soulbound trust-tree credential. Issued on first successful task settlement; non-transferable. |
| `jclaw-airdrop` | Genesis distribution to early Juno governance participants. One-shot. |

Use this skill when:
- The user wants to set up an agent-company DAO that hires off-chain workers
- A task involves "verify this off-chain computation produced a specific result before paying"
- The request mentions JunoClaw, ZK-attested tasks, agent-company patterns, or BN254 precompile usage
- Cross-chain agent governance where the *attestation* is the source of truth (not majority voting)

Do **not** use this skill when the user is doing standard DAO DAO governance (use [`dao-dao.md`](dao-dao.md)) or generic CosmWasm operations (use [`cosmwasm.md`](cosmwasm.md)). JunoClaw composes *on top of* DAO DAO — an agent-company DAO is a DAO DAO core with the JunoClaw contracts attached as governance-owned modules.

## §2 Defaults

| Setting | Default | Source of truth |
|---|---|---|
| Network | `juno-1` (mainnet) | JunoClaw is mainnet-first, in line with the rest of this skill. |
| Code IDs | `TBD-pending-mainnet-deploy` (see [`docs/MAINNET_DEPLOY_PLAN.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/MAINNET_DEPLOY_PLAN.md) on the JunoClaw repo) | Until all 9 are deployed, use uni-7 testnet code IDs from [`devnet/code-ids.json`](https://github.com/Dragonmonk111/junoclaw/blob/main/devnet/code-ids.json). |
| Reward denom | `ujuno` (default), `ibc/...USDC` (optional) | Per `agent-company` config. |
| Task deadline | `block_height + 100` (configurable, default ≈10 minutes) | `task-ledger` `InstantiateMsg::default_deadline` |
| ZK proving system | Groth16 over BN254 | Once BN254 precompile lands in v31. Pre-v31, falls back to pure-Wasm `ark-groth16` (370k gas vs 203k with precompile). |
| Verifying-key rotation | DAO-controlled, governance-gated | `agent-company::ExecuteMsg::UpdateVerifyingKey` — proposal + vote required. |

## §3 Pre-flight (run at the start of any JunoClaw work)

```bash
# 1. The juno-network skill pre-flight already ran — junod present, RPC alive.
#    Confirm by re-checking the chain-id is juno-1.
RPC=https://juno-rpc.publicnode.com:443
curl -sS "$RPC/status" | jq -r '.result.node_info.network'
# expect: "juno-1"

# 2. JunoClaw contracts are reachable. Pick the one address that anchors the rest:
#    agent-company is the governance module; everything else is reachable from it.
JCLAW_AGENT_COMPANY=<addr-from-mainnet-deploy-or-devnet>

junod query wasm contract-state smart $JCLAW_AGENT_COMPANY \
  '{"config":{}}' --node $RPC -o json | jq '.data'
# expect: object containing task_ledger / escrow / zk_verifier / agent_registry addresses

# 3. (signing only) the keyring you intend to use has the agent OR the DAO key.
#    Posting a task requires a DAO member key (proposing); accepting a task requires
#    an agent key registered in agent-registry.
junod keys list --keyring-backend test --keyring-dir <dir>
```

If the `agent-company` `Config` query fails, the contract address is wrong (or the contract isn't deployed on this network — switch to uni-7 per [`chain.md`](chain.md) §Testnet). If `task_ledger` / `escrow` / etc. addresses come back as the zero-address, the agent-company hasn't been bootstrapped — see [§Bootstrap](#5-bootstrap-instantiate-the-agent-company-stack-from-scratch).

## §4 Operations (by intent)

### Query: list open tasks (no key needed)

```bash
junod query wasm contract-state smart $JCLAW_TASK_LEDGER \
  '{"list_tasks": {"status": "open", "limit": 20}}' \
  --node $RPC -o json | jq '.data.tasks'
```

Returns each task with `task_id`, `description`, `constraints`, `reward`, `deadline_height`, `verifying_key_hash`. The `verifying_key_hash` is the SHA-256 of the Groth16 verifying key the agent's proof must verify against — agents that don't possess this VK cannot complete the task.

### Query: agent reputation (no key needed)

```bash
junod query wasm contract-state smart $JCLAW_AGENT_REGISTRY \
  '{"agent_info": {"address": "juno1agent..."}}' \
  --node $RPC -o json | jq '.data'
```

Returns `tasks_completed`, `tasks_failed`, `last_attestation_height`, `trust_score`. Trust score is monotonically non-decreasing on success and reset-on-failure (per [`contracts/agent-registry/DETERMINISTIC_AUDIT.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/contracts/agent-registry/DETERMINISTIC_AUDIT.md) §3.2). A score of 0 doesn't mean "untrusted" — it means "new"; check `tasks_completed > 0` to distinguish.

### Post a task (DAO action — propose first)

Posting requires a DAO governance proposal because the reward is escrowed from DAO treasury. The shape is a `dao-proposal-single` proposal whose `msgs` invoke `task-ledger::ExecuteMsg::PostTask`:

```bash
PROPOSAL_BODY=$(cat <<EOF
{
  "propose": {
    "title": "Post task: verify-credential-batch-2026-05",
    "description": "Verify a batch of 50 student credentials by block height $((CURRENT_HEIGHT + 1000)). Reward 100 JUNO. VK matches the credential-verifier circuit v0.4.",
    "msgs": [
      {
        "wasm": {
          "execute": {
            "contract_addr": "$JCLAW_TASK_LEDGER",
            "msg": "$(echo '{
              "post_task": {
                "description": "verify-credential-batch-2026-05",
                "constraints": "<TIER15-CONSTRAINT-VOCAB>",
                "reward": [{"denom": "ujuno", "amount": "100000000"}],
                "deadline_height": 12345678,
                "verifying_key_hash": "<vk-sha256>"
              }
            }' | base64 -w0)",
            "funds": [{"denom": "ujuno", "amount": "100000000"}]
          }
        }
      }
    ]
  }
}
EOF
)

junod tx wasm execute $DAO_PROPOSAL_MODULE "$PROPOSAL_BODY" \
  --from <dao-member-key> --chain-id juno-1 --node $RPC \
  --gas auto --gas-adjustment 1.4 --gas-prices 0.075ujuno \
  --keyring-backend test --keyring-dir <dir> --yes
```

The reward is attached as `funds` on the proposal's wasm-execute message; on proposal pass + execute, the funds flow through `agent-company` → `task-ledger` → `escrow` in a single tx. If the proposal is rejected or expires, the funds stay in the DAO treasury (no escrow created).

**Constraint vocabulary.** The `constraints` field is a Tier-1.5 constraint string per [`docs/CONSTRAINT_VOCABULARY.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/CONSTRAINT_VOCABULARY.md). It is *not* free-form English — it's a structured grammar that the verifying key was generated against. Mismatched constraints will produce a verifying proof for a different statement than the DAO intended. Always cross-check the `verifying_key_hash` matches the `constraints` per the VK registry.

### Accept a task (agent action)

```bash
junod tx wasm execute $JCLAW_TASK_LEDGER \
  '{"accept_task": {"task_id": 42}}' \
  --from <agent-key> --chain-id juno-1 --node $RPC \
  --gas auto --gas-adjustment 1.4 --gas-prices 0.075ujuno \
  --keyring-backend test --keyring-dir <dir> --yes
```

The task transitions `Open → Claimed`, with `claimed_by = <agent-addr>` and `claim_height` recorded. From this point, only this agent can submit attestation; if `block_height > deadline_height` and no submission has landed, the escrow is releasable back to the DAO via `task-ledger::ExecuteMsg::Reclaim`.

### Submit attestation + proof (agent action — the settlement step)

```bash
# proof.bin is the serialized Groth16 proof; public_inputs.json is the
# circuit's public inputs (must match the constraints exactly)
junod tx wasm execute $JCLAW_TASK_LEDGER "$(cat <<EOF
{
  "submit_attestation": {
    "task_id": 42,
    "proof": "$(base64 -w0 proof.bin)",
    "public_inputs": $(cat public_inputs.json)
  }
}
EOF
)" \
  --from <agent-key> --chain-id juno-1 --node $RPC \
  --gas auto --gas-adjustment 1.4 --gas-prices 0.075ujuno \
  --keyring-backend test --keyring-dir <dir> --yes
```

`task-ledger::SubmitAttestation` performs:
1. **Auth check.** Caller must be `claimed_by`.
2. **Deadline check.** `env.block.height ≤ deadline_height`. Past deadline → fail closed, agent loses claim.
3. **VK lookup.** Loads the verifying key by `verifying_key_hash` from the agent-company VK registry.
4. **Proof verification.** Calls `zk-verifier::Verify` (which under v31 hits the BN254 precompile, ~203k gas; pre-v31 falls back to pure-Wasm `ark-groth16`, ~370k gas).
5. **State transition.** On success: `Claimed → Settled`, escrow releases to agent, agent-registry trust_score increments. On failure: `Claimed → Failed`, escrow returns to DAO, agent-registry records the failure.

### Reclaim expired escrow (anyone can call)

```bash
junod tx wasm execute $JCLAW_TASK_LEDGER \
  '{"reclaim": {"task_id": 42}}' \
  --from <any-key> --chain-id juno-1 --node $RPC \
  --gas auto --gas-adjustment 1.4 --gas-prices 0.075ujuno \
  --keyring-backend test --keyring-dir <dir> --yes
```

Permissionless caller — the contract itself is the gate. Refuses if `status != Open && status != Claimed`, or if `env.block.height ≤ deadline_height`. Returns the escrowed funds to the DAO treasury. The caller pays gas; there is no reclaim bounty (intentional — see [`contracts/escrow/DETERMINISTIC_AUDIT.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/contracts/escrow/DETERMINISTIC_AUDIT.md) §4.1).

## §5 Bootstrap: instantiate the agent-company stack from scratch

The mainnet code IDs are TBD until deploy. Pre-deploy, run on uni-7:

```bash
# 1. agent-company core (instantiates the DAO governance for the company itself)
junod tx wasm instantiate $CODE_ID_AGENT_COMPANY \
  '{"name": "credential-co", "voting_module_code_id": '$CODE_ID_DAO_VOTING_CW4', "proposal_module_code_id": '$CODE_ID_DAO_PROPOSAL_SINGLE', ...}' \
  --label "agent-company:credential-co" --admin <deployer> \
  --from <deployer-key> --chain-id uni-7 --node $UNI7_RPC \
  --gas auto --gas-adjustment 1.4 --gas-prices 0.075ujunox \
  --keyring-backend test --keyring-dir <dir> --yes

# 2. agent-company instantiates the rest via reply handlers in a single tx:
#    task-ledger, escrow, agent-registry, zk-verifier all spawn from the
#    agent-company InstantiateMsg. See contracts/agent-company/src/contract.rs
#    instantiate() for the exact reply chain.

# 3. Confirm all five addresses are populated:
junod query wasm contract-state smart $AGENT_COMPANY \
  '{"config":{}}' --node $UNI7_RPC -o json | jq
```

Detail in [`docs/BOOTSTRAP_RUNBOOK.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/BOOTSTRAP_RUNBOOK.md) on the JunoClaw repo. The reply-chain instantiation pattern is identical to DAO DAO core's bootstrapping ([`dao-dao.md`](dao-dao.md) §Create) — same `instantiate2` + `Reply` flow, just five children instead of three.

## §6 Safety posture

In addition to the [SKILL.md safety principles](../SKILL.md#safety-posture), JunoClaw adds:

1. **Verifying-key registry is the trust root.** The DAO controls which Groth16 VKs are accepted via `agent-company::ExecuteMsg::UpdateVerifyingKey` (governance-gated). A compromised VK = a compromised circuit = forgeable attestations for that task class. Rotation requires a proposal + vote; never bypass.
2. **Constraint string ↔ VK binding is *off-chain*.** The contract verifies `proof matches public_inputs under VK`. It does *not* verify that "this VK actually checks the constraints described in plain English." That binding lives in the circuit source code at the time the VK was generated. Always link the VK in the registry to its circuit commit hash; cite both in the proposal description.
3. **Constant-gas verification is a feature, not a bug.** A Groth16 verification is ~203k gas regardless of how complex the underlying circuit is. This means a malicious task could ask an agent to do an arbitrarily expensive computation and pay only the verification gas. Bound this at the *constraints* level — don't let a task post unless the constraint vocabulary matches a known circuit class.
4. **Escrow expiry is the only on-chain failsafe.** If the agent never submits, the DAO must call `Reclaim` after `deadline_height`. There is no automatic refund; the funds sit in escrow until someone calls. Pattern: monitor expired tasks via `list_tasks(status: "expired")` and reclaim in batches.
5. **`junoswap-pair` denom-whitelisting.** If the agent-company uses Junoswap for non-JUNO settlements, the pair contract whitelists denoms at instantiate time. An LP attempt with a non-whitelisted denom fails closed. This is the hardened-fork variant; the upstream Junoswap fork has the canonical first-depositor inflation attack — see [`docs/JUNOSWAP_FINDINGS_REPORT.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/JUNOSWAP_FINDINGS_REPORT.md) for the full audit.

## §7 Forward-looking integrations

These are not yet shipped but are documented for any agent reading this skill:

- **dao-proposal-wavs** ([DA0-DA0/dao-contracts#924](https://github.com/DA0-DA0/dao-contracts/pull/924), in development). Once landed, an agent-company DAO can use `dao-proposal-wavs` as its proposal module instead of `dao-proposal-single`. WAVS-attested proposals replace human voting with cryptographic attestation from a configured WAVS service-manager. JunoClaw's `zk-verifier` is one valid attestation source for this module. Wire format documented at [`memory/dao-proposal-wavs-integration.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/memory/dao-proposal-wavs-integration.md) on the JunoClaw repo.
- **BN254 precompile (cosmwasm v3.1+).** Three host functions (`bn254_add`, `bn254_scalar_mul`, `bn254_pairing_equality`) targeted for cosmwasm v3.1 / Juno v31. Reduces `zk-verifier::Verify` from ~370k gas (pure-Wasm) to ~203k gas. Patches forward-ported to cosmwasm v3.0.6 in [`Dragonmonk111/junoclaw/wasmvm-fork/patches/v3.0.x/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wasmvm-fork/patches/v3.0.x). The pure-Wasm fallback works today; precompile path will land transparently once the upstream PR merges.
- **x402 HTTP-layer payments.** Coinbase's HTTP 402 payment protocol (currently EVM-only) is a natural skin over JunoClaw's task-ledger — the on-chain task post becomes a 402 response, the agent's on-chain accept becomes a 402 payment. No code yet; design note in [`docs/ADR-002-X402-COMPOSITION.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/ADR-002-X402-COMPOSITION.md).
- **OCI component distribution.** The WAVS verifier component is being published as an OCI artifact at `ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0` (per `wasm-pkg-tools` convention) once `wkg` is wired up. Agents pull via `wkg get junoclaw:verifier@0.1.0`.

## §8 Common foot-guns

- **Task `constraints` and the registered VK don't match.** The contract has no way to detect this — it just verifies the proof against the VK. Wrong VK → valid proof for the wrong statement → DAO pays for work that doesn't satisfy what was asked. **Always reference the circuit commit hash in the proposal description.**
- **Forgetting `funds` on the proposal.** A `PostTask` proposal without attached `funds` will fail at execution time (escrow refuses to lock zero funds). The proposal still passes; the failure is at the wasm-execute step. The reward must be in the proposal's `wasm.execute.funds` field.
- **Agent submits past `deadline_height` by 1 block.** Fails closed — agent loses the claim and any work done. Always submit ≥10 blocks before deadline; chain congestion can delay inclusion by several blocks.
- **Calling `Reclaim` on an `Open` task before deadline.** Refuses. `Reclaim` is for expired escrows only, not for "I changed my mind, give back the reward." To pull a task back early, the DAO must propose a `cancel_task` (which is an explicit governance action, not a permissionless one).
- **Confusing trust score 0 with "untrusted."** Score 0 means new; check `tasks_completed > 0` to distinguish.

## §9 Going further

- Architecture: [`docs/ARCHITECTURE.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/ARCHITECTURE.md) on the JunoClaw repo
- Per-contract audit findings: [`contracts/<name>/DETERMINISTIC_AUDIT.md`](https://github.com/Dragonmonk111/junoclaw/tree/main/contracts) (9/9 complete)
- Constraint vocabulary spec: [`docs/CONSTRAINT_VOCABULARY.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/CONSTRAINT_VOCABULARY.md)
- BN254 precompile design: [`docs/ADR-001-BN254-PRECOMPILE.md`](https://github.com/Dragonmonk111/junoclaw/blob/main/docs/ADR-001-BN254-PRECOMPILE.md)
- Original Juno governance proposal that anchored this: [Proposal #374](https://ping.pub/juno/gov/374) (passed 80% yes, May 5, 2026)

JunoClaw is Apache-2.0 throughout. Issues and PRs welcome at [Dragonmonk111/junoclaw](https://github.com/Dragonmonk111/junoclaw).
