# Ffern note — short version

> **Purpose.** Lightweight replacement for `docs/FFERN_THANK_YOU_PM.md`,
> calibrated for a git-fluent OG dev who reads commit messages directly.
> Copy-paste into DM / email / GitHub issue. Drop the long PM if this
> one fits the relationship better.

---

Hi <Ffern contact>,

Quick note: the audit response landed on `main`.

- `v0.x.y-security-1` — five walls (C-1, C-2, C-3, C-4, H-3), tagged on `main`
- `v0.x.y-security-2` — first lever (`signing_paused` runtime kill-switch), tagged on `main`
- `v0.x.y-security-3` — admin RPC + `egress_paused` (loopback levers + hot-flip), tagged on `main`

Full prose in `CHANGELOG.md` and `SECURITY.md` (`Walls` and `Levers` sections). **Five GHSAs are now published on the repo** — minimal text only (titles, severity, CVSS, CWE, affected/patched versions, commit SHAs, pointer back to `CHANGELOG.md` at the security tag). The full exploit narrative is held back deliberately and will appear in a post-audit retrospective (Medium + mirrored to `docs/`). This is a disclosure-hygiene choice: shipping minimal GHSAs gets CVE numbers into the downstream defender ecosystem quickly while holding back attacker-useful narrative against unpatched operators. CVE numbers are being assigned by GitHub's CNA (async — usually minutes to hours; visible on the advisory pages once minted). The five advisories:

- **C-1** `plugin-shell` shell-injection bypass — <https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-fvq5-79h6-952c> (CVSS 8.4 high)
- **C-2** `plugin-shell` shell-metacharacter injection — <https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-gpvm-3chf-2649> (CVSS 8.4 high)
- **C-3** MCP write tools exposed BIP-39 mnemonic — <https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-j75q-8xvm-6c48> (CVSS 9.8 critical)
- **C-4** `upload_wasm` arbitrary filesystem paths — <https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-rw59-34hw-pmwp> (CVSS 8.5 high)
- **H-3** SSRF in WAVS `computeDataVerify` — <https://github.com/Dragonmonk111/junoclaw/security/advisories/GHSA-q545-mvjf-q9pg> (CVSS 8.2 high)

If the minimal text reads wrong on any finding — wrong class label, wrong CWE, wrong vulnerable/patched range, severity off — I can `PATCH` any field on a published advisory at any time. The retrospective will be drafted at our pace; you'll be on the thank-you list there with the option to redact.

**One question that actually matters**: does the response fit the box you had in mind? i.e., did I understand each finding correctly and close it at the right layer, or is anything off — wrong primitive, wrong scope, missing case, weaker than the bug deserves? No rush, whenever you have a moment. If everything fits, a one-line "looks fine" is enough; if anything's off, even a hint at which finding and which direction will let me iterate.

**On AI-authorship** (per recent regulation, in case you were going to ask): all code in this response was authored by VairagyaNodes in pair-programming with Cascade (the Windsurf coding agent) under direction at every step. The project documents this in `NOTICE` and `CONTRIBUTORS.md`. The disclosure is consistent with EU AI Act Art. 50 transparency obligations (where applicable) and the US Copyright Office's March 2023 / Jan 2025 guidance on AI-assisted work — substantial human direction is the bar for human authorship, and we cleared it (design choices, threat model, test-first discipline, every commit message, every architectural call). If you have a perspective on how the older-school code-provenance norms map onto this — or any concern about the legal status of AI-assisted patches in a security context (CVE attribution, audit liability, copyright assignability) — I'd genuinely welcome that conversation. It's a novel area.

**Practicalities**:

- Repo: <https://github.com/Dragonmonk111/junoclaw>
- BN254 governance proposal held until you've had a chance to look
- Revised cost envelope for a re-check or the BN254 audit: $30–45k, 3–5 weeks, via DAO treasury post-mainnet (bridge funding earlier negotiable if it helps)
- Public credit in the eventual Medium retrospective is offered; redact if preferred

**Wider ecosystem credit** (so the whole picture is on the table): none of this response timeline would have been workable without quiet help from the Juno side. **Rattadan** (Juno team) has been steadily kind on the validator-ops and node-running side — uni-7 orientation, testnet token coordination, the small Juno-community things that make a project feasible for a single operator. **Jake Hartnell** co-signed Prop #373 (the original BN254 signaling proposal) and has kept engaging on and off since, which is exactly the kind of slow, careful community presence this ecosystem needs. Both are on the thank-you list for the eventual Medium retrospective and a handful of others will join them there. This context is just so you know the audit response didn't happen in a vacuum — it happened against a backdrop of small, repeated kindnesses from people who didn't have to show up.

Thank you again. The audit changed the shape of this project for the better.

— **VairagyaNodes** (with **Cascade**, the pair-programming AI agent, under direction)
