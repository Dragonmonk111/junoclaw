# Howl Social read-pass — substrate evaluation for Moultbook v0

**Date:** 2026-05-10.
**Purpose:** Answer action item #1 from `AI_DAO_FRAMING_AND_MOULTBOOK.md` §7:
"Read pass on Howl Social — locate canonical repo, license, contract
surface, and schema fit." No code yet; this is a decision-input document.
**Length:** Read time ~5 min.

## Headline

**Howl Social is dormant.** The canonical GitHub org
`github.com/howlsocial` has **zero commits across all 8 repos since
April 2023** (most 2022). The documentation site `docs.howl.social` has
**expired** (redirects to `expireddomains.com`). There is no active
maintainer channel, no recent release, no CI run in 3+ years.

For the Moultbook question — "can we fork Howl Social's post/stake/follow
contracts as a shared-agent-knowledge substrate?" — the answer is
**not the code itself**. The code was never audited, depends on a 2022-era
CosmWasm surface, and ships against trait shapes that have moved. However,
**one subsystem is salvageable at the architectural level**, and the
overall design pattern is worth codifying. See §5.

## 1. Canonical repo and license

- **Org:** [`github.com/howlsocial`](https://github.com/howlsocial)
- **License:** Apache-2.0 across every Rust repo (`whoami`, `whoami-paths`,
  `dao-escrow`, `did-key.rs` are all Apache-2.0; the JS/HTML repos are
  unlicensed but not contracts).
- **Docs site:** `docs.howl.social` — **domain expired**. Content is
  preserved in [`howlsocial/howldocs`](https://github.com/howlsocial/howldocs)
  (last updated 2023-02-07). No archived snapshot linked.
- **Twitter:** [`@howl_social`](https://twitter.com/howl_social) — last
  activity not verified in this pass; project status strongly suggests
  inactive.

## 2. Repository inventory

Eight repos, ordered by last push (newest first):

| Repo | Language | Role | Last push | Notes |
|------|----------|------|-----------|-------|
| `whoami-paths` | Rust | Namespace paths (subnames of whoami) | 2023-04-05 | Fork of `Callum-A/whoami-paths`. Apache-2.0. |
| `dao-escrow` | Rust | Time-delayed escrow with multisig/SubDAO override | 2023-03-31 | 0 forks, 0 issues, 0 PRs. Apache-2.0. |
| `howldocs` | JavaScript | Docusaurus site content | 2023-02-07 | 20 forks but zero recent activity on the upstream. |
| `whoami-ui` | TypeScript | UI for whoami nameservice | 2022-11-09 | Fork of `envoylabs/whoami-ui`. |
| `whoami` | Rust | NFT-based user + contract nameservice (= DENS) | 2022-10-19 | Fork of `envoylabs/whoami`. Apache-2.0. 10 forks. |
| `featured` | — | Empty / placeholder | 2022-05-31 | No discernible content. |
| `howl-static` | HTML | Landing-site artefact | 2022-05-18 | Static HTML only. |
| `did-key.rs` | Rust | DID method impl | 2022-05-15 | Fork of `decentralized-identity/did-key.rs`. Apache-2.0. |

**Critical absence:** there is **no "posts" / "microblog" / "content"
contract in the `howlsocial/` org.** See §4.

## 3. Protocol architecture (from `howldocs/docs/intro.md`)

The docs are thin (one FAQ page), but enough is visible to reconstruct the
architecture:

### Identity layer
- **DENS NFT** (the production rename of `whoami`) gates everything. Posting
  without a DENS alias is not supported. PFPs are the NFT's `image` field.
- The name and the profile metadata are on-chain (standard CW721 extensions).

### Token / economics layer
- **$HOWL token contract:**
  `juno1g0wuyu2f49ncf94r65278puxzclf5arse9f3kvffxyv4se4vgdmsk4dvqz` (standard
  CW20 from the surface).
- **Post-staking:** users stake $HOWL to individual posts ("❤️"). Stake
  locks **14 days**; rewards distribute on a **daily epoch** for that period.
- **Reward split per stake-bundle:**
  - **60%** delegator (the staker)
  - **20%** post creator
  - **10%** Howl DAO treasury
  - **10%** dev fund

### Content layer
- **Not visible on-chain.** No posts contract in the org. Given the 2022
  CosmWasm cost model and the absence of any "post-content" contract in
  either `howlsocial/` or its upstream `envoylabs/`, post content was
  almost certainly stored **off-chain** (IPFS/Arweave or a centralised
  backend), with on-chain state limited to (post_id → author_DENS →
  stake_accumulator) tuples. This is consistent with every other
  CosmWasm-era social attempt we're aware of.

### Airdrop mechanic (historical, now irrelevant)
- Snapshot at 2026-05-04 (sic — the docs use future dates that now refer
  to the past; the real snapshot was 2022-05-04 of the Juno mainnet state).
- Eligibility: name-holders + eligible stakers. Clawback 1 month
  post-launch. This is a **closed chapter** and contributes nothing to the
  current evaluation.

## 4. What's genuinely relevant to Moultbook

### 4a. Identity: `whoami` / DENS

This is the only part of the Howl Social stack with a **still-live
upstream**. [`envoylabs/whoami`](https://github.com/envoylabs/whoami) is
the fork parent; it continues to back DENS today (`dens.sh` domain is
live), and is the standard identity primitive on Juno.

**For Moultbook:** if we need per-agent identities with a human-readable
handle + profile metadata + transferability, `whoami` is a tested, deployed
primitive. We do not need to re-implement it. Use it as the **identity
dependency**, not as the thing we fork.

### 4b. Post-staking reward split

Conceptually reusable: "readers stake a commitment token on a shared
artefact; rewards stream to the artefact's author and to the protocol's
treasury." That split pattern (60/20/10/10) is a reasonable starting point
for a shared-agent-knowledge substrate where contributors publish facts
and other agents stake attention on them.

**For Moultbook:** the pattern is transferable; the 2022 code is not.
Re-implementing this against the 2026-era CosmWasm surface (post-BN254
patches, post-Mesh-aware feature flags, the v2.2.7 patch series) is the
cleaner path.

### 4c. What's not relevant

- `dao-escrow` — solves a different problem (multisig withdrawal protection).
- `did-key.rs` — DID method is tangential to the Moultbook primitive; identity
  already has a candidate (whoami).
- `whoami-paths` — useful if we want sub-namespaces under a root DENS
  alias, but not required for v0.
- `howl-static`, `featured`, `howldocs` — not contracts.

## 5. Recommendation

**Do not fork the Howl Social contracts.** Three reasons:

1. **Audit debt.** Three years unmaintained, no audit, no recent CI. Any
   vulnerability discovered in the CosmWasm trait surface between 2023 and
   now would ship directly into our fork.
2. **Post-content off-chain assumption.** The one architectural piece we
   most want to examine — how posts are addressed, committed, and
   retrieved — is exactly the piece that isn't in the repo. Forking gives
   us nothing for that question.
3. **Opportunity cost.** A clean-slate Moultbook v0 built against our
   current CosmWasm patch set (v2.2.7, with the BN254 host functions
   available) ships in roughly the same number of engineering sessions as
   a careful fork-audit-rebase of code we'd throw most of away.

**Do take inspiration on three specific points:**

- **Identity primitive:** use `whoami` / DENS as a dependency (not a fork).
- **Economic primitive:** the stake-with-epochs-reward split pattern is
  solid. Start with `stake_split = (60, 20, 10, 10)` as the default, make
  it instantiate-time configurable.
- **Storage pattern:** post content goes off-chain with an on-chain
  commitment. This is already the correct answer for storage
  cost — confirm via a prototype that two agents can read each other's
  commitments deterministically.

## 6. Next actions (for the Moultbook scoping track, not for this session)

- **Prototype sketch.** One CW execute message (`SubmitMoultEntry {
  commitment: Binary, metadata: MoultEntryMeta }`) + one query (`GetEntry {
  id: String }`). Storage via `Map<String, MoultEntry>`. No tokens yet.
- **DENS coupling decision.** Should `SubmitMoultEntry` require a valid
  DENS alias on the sender's address, or should it be open? Open is simpler
  for v0; gating is easy to add.
- **Storage-cost budget.** Confirm ~1 KB of on-chain metadata per entry is
  tolerable at Juno's current gas costs. Not a BN254-precompile question;
  a regular storage-write question.
- **Schema sketch → ADR-002.** If we decide to proceed past this read
  pass, the next artefact is a schema ADR that mirrors the shape of
  `ADR-001-BN254-PRECOMPILE.md`.

None of the above blocks the current upstream-issue / patch-regen track.

## 7. Footnote

This read pass was conducted with read-only tooling; no repos were cloned.
If the Moultbook track moves past the scoping stage, a follow-up session
should clone `envoylabs/whoami` (the live upstream of DENS) and read the
contract source directly. The archaeology on `howlsocial/*` is complete.
