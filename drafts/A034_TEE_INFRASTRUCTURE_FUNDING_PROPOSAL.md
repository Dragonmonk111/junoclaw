# A034 — Fund TEE Infrastructure for Sealed Signer Production Deployment

> A033 authorized the sealed signer as DAO signing infrastructure. The code is built, tested, and E2E proven on uni-7 (tx 4A7384DE...). Cross-platform determinism is verified (Latitude.sh AMD EPYC, 3/3 byte-identical). This proposal funds the compute infrastructure to run it in production — the last step before the plaintext mnemonic is retired. **Path: Akash-first (decentralized, crypto-native), with a hard 2-week deadline before automatic fallback to GCP Spot. Budget: ~$200 USD equivalent in JUNO (current DAO treasury balance).**

---

## Copy-paste box 1: Title

```
A034 — Fund TEE Infrastructure for Sealed Signer Production Deployment
```

## Copy-paste box 2: Description

```
A033 authorized the TEE-sealed signer as the DAO's official agent signing mechanism. The code is built and tested — the sealed signer produces byte-identical Cosmos transactions proven on uni-7 testnet (tx 4A7384DE...) and cross-platform on bare-metal AMD EPYC in London. The invoke API prototype passed 15/15 smoke tests and full E2E. The last remaining step is: where does the TEE run?

Today, the DAO's agent signing key is still a plaintext mnemonic in a developer terminal. Every day it remains there is a day the DAO's keys can be exfiltrated. This proposal funds the infrastructure to close that gap.

What this proposal does:
1. Authorizes DAO treasury spend of the full current balance (~$200 USD equivalent in JUNO) for TEE infrastructure.
2. Authorizes a $75 AKT bounty for the first Akash Network provider to enable confidential compute (tee/type: cpu attribute) — making them the first decentralized TEE provider on Akash mainnet.
3. Directs Jake's Juno AI agent (or Highlander, whichever is available) to post the Akash provider bounty across Discord, Akash forum, and social channels at zero cost.
4. Sets a **hard 2-week deadline** from bounty posting: if no Akash provider enables `tee/type: cpu` within that window, builders automatically fall back to GCP Confidential VM spot pricing (~$15-25/month) so the plaintext mnemonic retirement is not indefinitely blocked.
5. If an Akash provider enables TEE at any point (before or after the GCP fallback), builders migrate the sealed signer to Akash — the sealed blob is portable, no key rotation needed.
6. Requires monthly Moultbook reports on TEE uptime, signing volume, attestation status, and costs.

Budget breakdown:
- Akash TEE provider bounty (deployment commitment at $25/month for 3 months): $75
- GCP Confidential VM spot fallback (2-3 months, only if Akash bounty deadline passes): ~$50-75
- Buffer (DNS, migration, misc): ~$50-75
- Total: ~$200 USD equivalent in JUNO (full treasury)

Why Akash-first:
- Aligns with DAO ethos: decentralized infrastructure over centralized cloud.
- Paid in AKT — no fiat/KYC/credit card required.
- Akash confidential compute software is fully built (AEP-83, Kata Containers, attestation sidecar); the only missing piece is a provider flipping the `tee/type: cpu` switch.
- The 2-week deadline prevents indefinite delay if no provider responds — GCP Spot remains the guaranteed fallback.

Why this amount:
- The DAO treasury currently holds ~$200 USD equivalent in JUNO. This proposal authorizes spending the full balance.
- One TEE instance handles both Moultbook and Junoclaw (single signing identity, single Docker container).
- The sealed key blob is portable — migration from GCP to Akash (or vice versa) requires no key rotation or on-chain changes.
- Jake's Juno AI agent (or Highlander) handles bounty outreach for free — no marketing spend.

Architecture: standalone TEE service (not a validator sidecar). The WAVS Docker container runs inside the TEE. The invoke server accepts authenticated HTTP requests from moultbook.js. The signing key is generated inside the TEE and never leaves. The DAO can verify hardware attestation.

In scope:
- Akash provider TEE enablement bounty ($75 AKT)
- Free outreach via Jake's Juno AI agent or Highlander
- GCP Spot fallback after 2-week deadline if Akash bounty unfilled
- Deployment of sealed signer to production TEE (Akash or GCP)
- Monthly operational reporting

Out of scope (future proposals):
- Key rotation (if ever needed — the sealed blob is portable)
- HA/redundancy (second TEE instance)
- Changes to sealed signer code or agent-company contract
- Mandating all DAO agents use the sealed signer
- Funding beyond 2-3 months (A035 will address continued funding or Akash migration)

Voting:
- YES = fund TEE infrastructure with current treasury (Akash-first, GCP fallback after 2 weeks), retire the plaintext mnemonic, deploy to production.
- NO = keep the plaintext mnemonic, defer production TEE deployment.
- ABSTAIN = defer to builders.

Funds: ~$200 USD equivalent in JUNO (full current DAO treasury) to the agent ops wallet `juno1dlm6y5cnvxayyv6hxd863lef82vu9jnez89gkh`.
```

## Copy-paste box 3: Raw DAO DAO JSON

```json
{
  "title": "A034 — Fund TEE Infrastructure for Sealed Signer Production Deployment",
  "description": "A033 authorized the TEE-sealed signer as DAO signing infrastructure. The code is built, tested, and E2E proven on uni-7 (tx 4A7384DE...) and cross-platform on bare-metal AMD EPYC. This proposal funds the last step: running the sealed signer inside a production TEE so the plaintext mnemonic can be retired. Path: Akash-first, with a hard 2-week deadline before automatic fallback to GCP Spot. Budget: ~$200 USD equivalent in JUNO (full current DAO treasury). What it does: (1) authorizes $75 AKT bounty for first Akash provider to enable TEE (tee/type: cpu), (2) directs Jake's Juno AI agent or Highlander to post the bounty for free across Discord and social, (3) sets a hard 2-week deadline after which builders fall back to GCP Confidential VM spot pricing (~$15-25/mo) if no Akash provider responds, (4) directs migration to Akash if/when a TEE provider becomes available, (5) requires monthly Moultbook reports. Architecture: standalone TEE service, not validator sidecar. One instance handles both Moultbook and Junoclaw. Sealed blob is portable — no key rotation for migration. Budget: $75 Akash bounty + ~$50-75 GCP fallback (if triggered) + ~$50-75 buffer = ~$200 total. Funds go to agent ops wallet juno1dlm6y5cnvxayyv6hxd863lef82vu9jnez89gkh. Voting: YES = fund TEE with current treasury (Akash-first, GCP fallback after 2 weeks), retire plaintext mnemonic, deploy to production; NO = keep plaintext mnemonic, defer; ABSTAIN = defer to builders.",
  "funds": []
}
```

---

## Status: DRAFT — for discussion before submission

## Context

A033 passed, authorizing the sealed signer as DAO signing infrastructure. Since then:

1. **Invoke API prototype built and tested** — 15/15 smoke tests pass, full E2E test on uni-7 produced a valid on-chain transaction (tx 4A7384DE...).
2. **Software determinism proven** — 3/3 runs produce byte-identical `tx_bytes` and `sign_doc_sha256_hex`.
3. **Cross-platform determinism proven** — Latitude.sh London AMD EPYC 7443P produced identical output to the development machine (3/3 byte-identical).
4. **Akash confidential compute infrastructure is built** — AEP-83, provider PR #396, console PR #3365 all merged. Tenants can request `params.tee: cpu`. But zero providers currently advertise the `tee/type` attribute.

The remaining gap is infrastructure: a TEE-capable compute instance to run the WAVS Docker container in production.

## What A034 funds

1. **Akash provider TEE bounty**: $75 AKT (3-month deployment commitment at $25/month) for the first Akash provider to enable confidential compute. This incentivizes a decentralized alternative and is the primary path.

2. **Free outreach via Jake's Juno AI agent or Highlander**: whichever agent is available posts the bounty message across Akash Discord, forum, and social channels at zero cost — leveraging existing DAO social infrastructure.

3. **GCP fallback (only if 2-week Akash deadline passes)**: ~$50-75 for GCP `n2d-standard-2` Confidential VM (AMD SEV-SNP, 2 vCPU, 8 GB) at **spot pricing** (~$15-25/month, 91% off on-demand). The sealed signer is stateless — preemption causes a restart, not data loss. This funding is only spent if no Akash provider responds within 2 weeks of the bounty posting.

4. **Buffer**: ~$50-75 for DNS, migration costs, and miscellaneous infrastructure.

## Treasury Execution & Payment Plan

**Roles**
- **Authorizing body**: Juno Agents DAO — passage of A034 authorizes the spend.
- **Recipient / ops wallet**: agent ops wallet `juno1dlm6y5cnvxayyv6hxd863lef82vu9jnez89gkh`.
- **Procurement agent**: the DAO builder controlling the ops wallet, acting under this mandate.
- **Bounty poster**: Jake's Juno AI agent or Highlander, whichever is available, posts the Akash bounty at zero cost.

**Payment flow (after passage)**
1. DAO treasury sends the approved JUNO balance to the ops wallet via the standard treasury-spend execution path.
2. Builder records the USD-equivalent value at the time of disbursement.
3. **Akash bounty ($75 AKT) — primary path**: builder swaps JUNO to AKT via Osmosis or another DEX, then submits `akash tx deployment create` for the sealed signer SDL. AKT sits in escrow, funding automatically deducts per-block once a `tee/type: cpu` provider bids and the lease is accepted (`akash tx market lease create`). Provider must demonstrate a live TEE bid before any funds move.
4. **2-week deadline check**: if no `tee/type: cpu` bid appears within 2 weeks of the bounty being posted, builder proceeds to step 5 (GCP fallback).
5. **GCP Spot (~$50-75) — fallback only**: builder converts the required JUNO to fiat/stablecoin and pays the GCP invoice with a personal/business card, then publishes the receipt + exchange rate to Moultbook.
6. **Buffer (~$50-75)**: held in the ops wallet and spent only against receipts or provider invoices; any surplus after final deployment is returned to the DAO treasury.

**Controls**
- All fiat/cloud receipts and crypto tx hashes (including Akash `lease create`/`deployment deposit` txs) are posted to Moultbook within the monthly TEE report.
- The 2-week Akash deadline is a hard trigger, not a suggestion — if it passes without a bid, GCP fallback begins automatically without needing a new vote.
- If an Akash provider enables TEE after the GCP fallback has already started, builders still migrate to Akash (sealed blob is portable) and the GCP instance is shut down.
- If the builder cannot continue, remaining funds are returned to the DAO treasury and the plaintext mnemonic retirement is paused.

## Why ~$200 (full treasury)

- The DAO treasury currently holds ~$200 USD equivalent in JUNO. This is what we have.
- The sealed signer is a lightweight service: 1-2 vCPU, 512 MB RAM, minimal traffic.
- One TEE instance handles both Moultbook and Junoclaw.
- The Akash bounty ($75) is a one-time incentive to unlock a decentralized long-term option.
- The bounty-posting agent handles outreach for free — no marketing spend from the treasury.
- If the Akash bounty deadline passes, the GCP fallback fits within the remaining budget.

## Why Akash-first

- Akash confidential compute software is fully built (AEP-83, Kata Containers, attestation sidecar)
- Zero providers have enabled it yet — "supply waiting for demand"
- Our sealed signer is a perfect first use case: lightweight, low traffic, clear security requirement
- A $75 bounty for ~2 hours of provider setup work is well-incentivized
- Long-term cost on Akash: ~$15-30/month (decentralized, crypto-native)
- Paid entirely in AKT — no fiat, no KYC
- Aligns with DAO ethos: decentralized infrastructure over centralized cloud

## Why the 2-week GCP fallback matters

- Akash has zero TEE providers today — there's no guaranteed timeline for the bounty to be claimed
- A hard deadline prevents the plaintext mnemonic retirement from being blocked indefinitely
- GCP Confidential VM spot (~$15-25/month) is available same-day and fits the remaining budget
- If Akash comes through later, migration is a blob copy — no rework, no key rotation

## Architecture

```
┌─────────────────────────────────────────────┐
│              TEE Instance (SEV-SNP)          │
│  ┌─────────────────────────────────────────┐ │
│  │         WAVS Docker Container           │ │
│  │  ┌──────────┐  ┌──────────┐            │ │
│  │  │  WAVS    │  │ Invoke   │            │ │
│  │  │ Aggregator│  │ Server   │            │ │
│  │  └────┬─────┘  └────┬─────┘            │ │
│  │       │              │                  │ │
│  │  ┌────▼──────────────▼─────┐           │ │
│  │  │  Sealed Signer WASM     │           │ │
│  │  │  (junoclaw_sealed_      │           │ │
│  │  │   signer.wasm)          │           │ │
│  │  │  Key generated here,    │           │ │
│  │  │  never leaves TEE       │           │ │
│  │  └─────────────────────────┘           │ │
│  └─────────────────────────────────────────┘ │
│                                               │
│  Memory encrypted by AMD Secure Processor    │
│  No operator (cloud or DAO) can access keys  │
└──────────────────┬──────────────────────────┘
                   │ HTTPS + Bearer Token
                   │
         ┌─────────▼─────────┐
         │  moultbook.js     │
         │  (reply-bot)      │
         │  Posts to Moultbook│
         │  + Junoclaw       │
         └───────────────────┘
```

## Portability

The sealed key blob is encrypted with a passphrase (environment variable). The same sealed blob works on any TEE — GCP, AWS, Azure, Akash, or bare metal. Migration between providers requires:
1. Copy the sealed blob to the new instance
2. Set the passphrase environment variable
3. Start the WAVS Docker container

No key rotation. No on-chain changes. The DAO address stays the same.

## Monthly Reporting

Builders will post a monthly Moultbook entry with:
- TEE instance uptime percentage
- Number of transactions signed
- Attestation verification status
- Actual infrastructure cost
- Any incidents or failures

## Voting

- **YES** — fund TEE infrastructure, retire the plaintext mnemonic, deploy the sealed signer to production.
- **NO** — keep the plaintext mnemonic, defer production TEE deployment.
- **ABSTAIN** — defer to builders.

## Next steps if A034 passes

1. **Day 0**: Jake's Juno AI agent or Highlander posts the Akash provider bounty across Discord, forum, and social. DM Overclock and other AMD EPYC providers.
2. **Days 0-14**: Monitor for a `tee/type: cpu` bid. If one appears, accept the lease, deploy the WAVS Docker stack via SDL, generate the sealed key inside the TEE, verify attestation.
3. **Day 14 (deadline)**: If no Akash bid has appeared, deploy WAVS Docker stack on GCP Confidential VM (spot) instead. Generate sealed key inside TEE. Verify attestation. Point moultbook.js at the invoke endpoint. Retire plaintext mnemonic.
4. **After deployment (either path)**: if Akash becomes available later, migrate the sealed blob and shut down GCP.
5. **Monthly**: Post Moultbook report with uptime, costs, and attestation status.
6. **Month 3**: If still on GCP, propose A035 for continued funding or report Akash migration status.
