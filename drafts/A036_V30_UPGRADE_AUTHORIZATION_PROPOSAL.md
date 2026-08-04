# A036 — Authorize Submission of Juno v30 Chain Upgrade Proposal to Mainnet Governance

> The Juno Agents DAO signals support for the v30 chain upgrade (BN254 precompile) and authorizes builders to submit the `MsgSoftwareUpgrade` proposal to juno-1 governance on the DAO's behalf. PR #1202 was merged on 2026-07-16, approved by dimiandre. Our code review (May 12) found 1 critical + 2 important bugs, all fixed before merge. The upgrade handler is 20 lines across 2 files. No state migrations. Mandate from signaling proposal #374 (~80% Yes).

---

## Copy-paste box 1: Title

```
A036 — Authorize Submission of Juno v30 Chain Upgrade Proposal to Mainnet Governance
```

## Copy-paste box 2: Description

```
Juno v30 is ready. PR #1202 (CosmosContracts/juno) was merged on 2026-07-16, approved by dimiandre. The upgrade adds BN254 elliptic-curve precompiles to CosmWasm — the same cryptography used by Groth16 zero-knowledge proofs. This enables cheap on-chain verification of agent attestations, ZK proofs, and TEE hardware proofs.

What v30 does:
- Bumps wasmvm to include BN254 host functions
- Registers the "bn254" capability so contracts can use it
- Pure-Wasm Groth16 verification gas drops from ~371k to ~200k SDK gas
- No state migrations, no param changes, no new modules

What this proposal does:
1. Signals DAO support for the v30 chain upgrade.
2. Authorizes builders to submit MsgSoftwareUpgrade to juno-1 governance with a 5,000 JUNO deposit.
3. Authorizes builders to publish pre-built binaries (linux-amd64, linux-arm64, darwin-amd64, darwin-arm64) with GPG-signed checksums.
4. Directs builders to coordinate with dimiandre for co-authorship of the upgrade commit.
5. Requires a rollback plan to be published before the proposal vote ends.
6. Directs builders to post the proposal text to #juno-validators on Telegram before submitting on-chain.

Background:
- Our code review of PR #1202 (May 12, 2026) found 1 critical bug (pruneVotingPower deletes last snapshot for sparse delegators) and 2 important bugs (LST quorum asymmetry, O(n) prune scan every block). All were fixed in commit 5d04a6f before merge.
- Signaling proposal #374 passed with ~80% Yes, mandating the BN254 precompile.
- The upgrade handler is 20 lines across 2 files (upgrade.go + constants.go), pattern-matched on dimiandre's v28→v29 work.
- The MCP skill-registry is now live on juno-1 mainnet (contract juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s). AI agents can discover, query, and interact with Juno dApps on-chain. v30 unlocks the full JunoClaw contract suite — zk-verifier, agent-registry, moultbook — on mainnet.

In scope:
- Submitting the MsgSoftwareUpgrade proposal to juno-1 governance
- Publishing pre-built binaries with signed checksums
- Coordinating with dimiandre for co-authorship
- Publishing a rollback plan
- Validator outreach via Telegram

Out of scope:
- The upgrade itself (fires automatically at the planned height if the proposal passes)
- Any changes beyond BN254 capability registration (per Marius's constraint: no "while we're here" cleanups)
- Funding for the 5,000 JUNO deposit (builders self-fund or source from mother wallet)

Voting:
- YES = authorize builders to submit the v30 upgrade proposal to juno-1 governance.
- NO = do not submit; defer the upgrade.
- ABSTAIN = defer to builders.

No DAO funds spent. The 5,000 JUNO deposit for the juno-1 governance proposal is separate from the DAO treasury and will be sourced from the builder/mother wallet. If the governance proposal passes, the deposit is returned. If it is vetoed, the deposit may be burned.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A036 — Authorize Submission of Juno v30 Chain Upgrade Proposal to Mainnet Governance",
  "description": "Juno v30 is ready. PR #1202 merged 2026-07-16 (approved by dimiandre). The upgrade adds BN254 precompiles to CosmWasm — enabling cheap on-chain Groth16 ZK proof verification (~371k → ~200k gas). No state migrations, no new modules. Handler is 20 lines. Mandate from signaling proposal #374 (~80% Yes). Our code review (May 12) found 1 critical + 2 important bugs, all fixed before merge. This proposal: (1) signals DAO support for v30, (2) authorizes builders to submit MsgSoftwareUpgrade to juno-1 governance with 5,000 JUNO deposit, (3) authorizes publishing pre-built binaries with GPG-signed checksums, (4) directs coordination with dimiandre for co-authorship, (5) requires rollback plan before vote ends, (6) directs validator outreach via Telegram. In scope: submitting the gov proposal, publishing binaries, coordinating with dimiandre, rollback plan, validator outreach. Out of scope: the upgrade itself (auto-fires at planned height), any changes beyond BN254, deposit funding (builders self-fund). Voting: YES = authorize submission; NO = defer; ABSTAIN = defer to builders. No DAO funds spent. 5,000 JUNO deposit sourced from builder/mother wallet, returned if proposal passes.",
  "funds": []
}
```

---

## Status: DRAFT — ready for submission now (parallel with A035)

## V30 Binary Build — VERIFIED
- Binary: `/usr/local/bin/junod-v30.0.0`
- Version: `v30.0.0`
- Commit: `c0b3a8d258d52d16e5bc39a75168a99aab9d098e` ✓
- Cosmos SDK: `v0.53.7`
- Go: `go1.25.2 linux/amd64`
- SHA-256: `c0056408923508a4085d4fc313652996690c690a536358addb38b314e673183d`
- Node: juno-1 mainnet, synced at block ~40,040,511

## Dependencies
- v30 binary is built and SHA-256 verified ✓
- Pre-built binaries must be published on GitHub Releases with GPG-signed checksums before submitting MsgSoftwareUpgrade.
- Rollback plan (docs/V30_ROLLBACK_PLAN.md) must be drafted and published before the governance vote ends.
- A035 (Akash TEE outreach) runs in parallel — not a blocker.

## Post-A036 steps (after DAO authorization)
1. Build & publish v30 binaries on GitHub Releases
2. Draft V30_ROLLBACK_PLAN.md
3. Coordinate with dimiandre for co-authorship
4. Post proposal text to #juno-validators on Telegram
5. Submit MsgSoftwareUpgrade to juno-1 governance (5,000 JUNO deposit)
6. Vote with DAO voting power
7. Article: "AI Agent Submits Juno v30 Chain Upgrade Proposal on Mainnet"
