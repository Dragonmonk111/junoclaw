# PR Body — `references/junoclaw.md`

*Paste this into the PR description box on GitHub after running the command sequence below.*

---

## Title

`docs(references): add references/junoclaw.md — JunoClaw agent-company skill reference`

## Body

Adds a new reference file documenting the JunoClaw agent-company pattern for any Juno agent that wants to set up verifiable off-chain compute backed by escrowed bounties.

### What this adds

A single new file at `references/junoclaw.md` (no changes to `SKILL.md` itself). Structure mirrors `references/dao-dao.md` exactly — one-line summary, mainnet-first defaults, ops broken by intent, safety posture, going-further. ~360 lines.

The file covers:

- **§1 What this is** — the nine-contract stack (task-ledger, escrow, agent-registry, agent-company, zk-verifier, junoswap-pair, builder-grant, jclaw-token, jclaw-airdrop) and when to route the agent here vs `dao-dao.md` / `cosmwasm.md`
- **§2 Defaults** — `juno-1`, code IDs marked TBD pending mainnet deploy, Groth16/BN254 proving system, governance-gated VK rotation
- **§3 Pre-flight** — bash blocks to confirm `agent-company` is reachable and the child contracts (task-ledger / escrow / etc.) are bootstrapped
- **§4 Operations by intent** — list tasks, query agent reputation, post a task (DAO proposal), accept a task, submit attestation + proof, reclaim expired escrow
- **§5 Bootstrap** — instantiate-the-stack runbook (reply-chain pattern matching DAO DAO core)
- **§6 Safety posture** — five JunoClaw-specific principles on top of the SKILL.md base (VK as trust root, constraint↔VK off-chain binding, constant-gas verification implications, escrow expiry semantics, junoswap-pair denom-whitelist)
- **§7 Forward-looking** — `dao-proposal-wavs` consumer, BN254 precompile, x402 HTTP gateway, OCI component distribution
- **§8 Common foot-guns** — constraint/VK mismatch, missing `funds`, deadline-by-1-block, premature `Reclaim`, score=0 ≠ untrusted
- **§9 Going further** — links into the JunoClaw repo for architecture / audit / circuit details

### Why this fits the skill

The existing `references/dao-dao.md` §Member already mentions `dao-proposal-wavs` as forward-looking. JunoClaw is the concrete consumer of that module — a nine-contract pattern that uses WAVS-attested proofs to settle escrowed agent tasks. Any agent reading the skill with intent to operate as a verifiable off-chain worker should be routed here; without this reference, the skill ends at "WAVS-attested proposals exist" without showing the producer side.

### Mainnet readiness note

The file ships with `TBD-pending-mainnet-deploy` placeholders where mainnet code IDs would go. JunoClaw's nine contracts are deployed on uni-7 / devnet today; mainnet deploy is queued behind Juno v30 → v31 (BN254 precompile lands in v31). Once mainnet code IDs land, a follow-up PR populates the placeholders.

### Verification

- Format mirrors `references/dao-dao.md` exactly (same heading hierarchy, same bash-block style, same defaults table shape)
- All chain-side assertions reference the upstream contract source at [`Dragonmonk111/junoclaw`](https://github.com/Dragonmonk111/junoclaw) with file paths
- No claims about contract behaviour that aren't grounded in the per-contract `DETERMINISTIC_AUDIT.md` files
- This contribution follows the **MIT License** of this repository (standard contribution model). The JunoClaw contract source code described herein is Apache-2.0 at https://github.com/Dragonmonk111/junoclaw — compatible with MIT for documentation purposes

### Out of scope for this PR

- BN254 precompile patches (separate upstream issue in `CosmWasm/cosmwasm`, queued behind Juno v30 testnet)
- `dao-proposal-wavs` PR #924 comment (separate)
- x402 gateway runtime code (lives in JunoClaw repo, not the skill — this reference just points at it)
- `SKILL.md` decision-tree update (optional; if you'd like the decision tree to mention JunoClaw I'll send a follow-up)

Happy to revise to fit the skill's tone better — flag anything that drifts.

---

## Tone notes (for my own reference, NOT for the PR body)

- News-not-ask framing
- Defer to Jake's shape on optional `SKILL.md` decision-tree update
- Acknowledge mainnet code IDs are TBD up-front — no pretending we're already deployed
- Apache-2.0 vs MIT — explicit compatibility note since the skill repo is MIT
- Close with "happy to revise" — soft hand-off
