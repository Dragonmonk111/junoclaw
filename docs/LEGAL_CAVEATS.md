# JunoClaw — Legal Caveats & Risk Disclosure

**Last updated**: March 19, 2026
**Applies to**: All JunoClaw software, contracts, documentation, and related infrastructure.

---

## 1. Experimental Software

JunoClaw is experimental, pre-production software. It is provided "AS IS" and "AS AVAILABLE" without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose, and non-infringement.

**Use at your own risk.**

---

## 2. No Audit

The smart contracts (agent-company, agent-registry, escrow, task-ledger, junoswap-factory, junoswap-pair) have **86 unit tests** but have **not undergone a formal third-party security audit**. A professional audit is planned post-mainnet deployment, funded by DAO treasury or community support.

Unit tests verify logic correctness but do not substitute for adversarial review by professional auditors.

---

## 3. Testnet Only (Pre-Proposal)

All contracts and infrastructure described are deployed on **Juno testnet (uni-7)** only. Mainnet deployment follows if Governance Proposal #373 passes. Testnet tokens have no monetary value.

---

## 4. Solo Developer Disclaimer

JunoClaw is built by one person (VairagyaNodes — Juno staker since December 30, 2021). The 13-bud soulbound governance model is designed to distribute responsibility, but until the DAO is fully operational, this is a single point of failure.

---

## 5. No Financial Advice

Nothing in JunoClaw's code, documentation, governance proposals, articles, or social media constitutes financial, legal, or investment advice. The $JClaw token (when implemented) is a soulbound governance credential, not a tradeable asset.

---

## 6. Smart Contract Risk

Smart contracts on any blockchain carry inherent risks:
- **Logic bugs** may exist despite testing
- **Upgrade risk**: The CodeUpgrade governance pathway (67% supermajority) can modify contract code. This is a feature, not a bug — but it means the DAO has root-level power over deployed contracts
- **Dependency risk**: Contracts depend on cosmwasm-std, cw-storage-plus, and other ecosystem libraries. Vulnerabilities in upstream dependencies could affect JunoClaw

---

## 7. Infrastructure Risks

- **Memory-based registry**: The warg component registry uses in-memory storage. If the Akash container restarts, the component is re-published automatically from the baked-in WASM binary (494KB, ~10s startup). The content hash (`sha256:b40d3fca...`) is deterministic — no data loss, but brief downtime is possible.
- **Single operator**: Currently one WAVS operator instance on Akash. The architecture supports multiple operators. The validator sidecar proposal would distribute attestation across Juno's validator set.
- **TEE attestation**: Proven on Azure DCsv3 (Intel SGX). Production attestation will come from validator sidecars or Akash TEE containers — neither exists in production yet.

---

## 8. No Liquidity Guarantee

Junoswap v2 pairs exist on testnet but have no real liquidity. Liquidity provision is a post-deployment community effort. No market-making guarantees are made.

---

## 9. Governance Risks

- **Quorum gaming**: The DAO requires 33.4% quorum. Small member sets (13 genesis buds) mean individual votes carry significant weight.
- **Concentration**: If trust-tree budding is not distributed carefully, governance power can concentrate.
- **BreakChannel**: The DAO can prune entire trust branches via BreakChannel governance action. This is by design but carries social risk.

---

## 10. AI-Generated Code Disclosure

Portions of the off-chain codebase (daemon, runtime, CLI, plugins) were written with AI assistance. The dependency tree has been trimmed (dirs, uuid, chrono, tower removed — March 2026 cleanup), but AI-generated code may contain patterns that differ from hand-written Rust. All code is open source and subject to community review.

---

## 11. License

JunoClaw is licensed under **Apache License 2.0**. See [LICENSE](../LICENSE) for full text.

The Apache 2.0 license includes:
- No warranty (Section 7)
- No liability (Section 8)
- Patent grant (Section 3)
- Contribution terms (Section 5)

---

## 12. Limitation of Liability

In no event shall the authors, contributors, or the JunoClaw DAO be liable for any direct, indirect, incidental, special, exemplary, or consequential damages arising from the use of this software, including but not limited to loss of funds, data, or goodwill.

---

## 13. Regulatory Notice

JunoClaw operates on decentralized blockchain infrastructure. Users are responsible for compliance with all applicable laws and regulations in their jurisdiction. The authors make no representation that JunoClaw is legal or appropriate for use in any particular jurisdiction.

---

*This document consolidates risk disclosures from the governance proposal, HackMD page, and Medium article into a single authoritative reference.*
