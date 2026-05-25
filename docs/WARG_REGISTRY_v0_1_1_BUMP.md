# `warg-registry` v0.1.1 — operational hygiene bump

*Click-paste runbook. Cluster C of the CAB plan. Cascade prepared the source changes; this file is the user-action-only sequence.*

## Pre-flight (already done)

- ✅ OCI labels added on the final stage of [`wavs/warg-registry/Dockerfile`](../wavs/warg-registry/Dockerfile) — moved out of the builder stage so they survive into the published image.
- ✅ [`LICENSE`](../LICENSE) (Apache-2.0) already at repo root.
- ✅ [`SECURITY.md`](../SECURITY.md) already at repo root.
- ✅ Recall-layer entry [`memory/warg-registry-package.md`](../memory/warg-registry-package.md).
- ✅ Per-package [`wavs/warg-registry/README.md`](../wavs/warg-registry/README.md) (added 2026-05-14 PM) — GHCR pulls this preferentially over the repo-root README; gives the package page a focused, link-and-status-rich landing card without pulling the giant repo-root content.

---

## Step 0 — 2FA (HARD DEADLINE 2026-06-11)

Do this before anything else. ~10 minutes. Account is restricted from account actions if not enabled by the deadline.

1. Open https://github.com/settings/security
2. Click **"Enable two-factor authentication"**.
3. Pick **"Set up using an app"** (not SMS — TOTP is the safer default).
4. Scan the QR code with any TOTP authenticator (Aegis on Android, Raivo / 2FAS on iOS, 1Password / Bitwarden if you use a password manager).
5. **Save the recovery codes to a password manager.** Print and store one copy offline. If you lose the TOTP device and the recovery codes, the account is unrecoverable.
6. Confirm by entering the 6-digit code from the authenticator.

After this, every `git push` over HTTPS needs a Personal Access Token (PAT) instead of password. If you use SSH for `git push`, no change.

---

## Step 1 — Change the GitHub display name (optional polish)

For external readers, the package publisher field currently shows `kingbpop7` rather than something professional. ~30 seconds:

1. Open https://github.com/settings/profile
2. Change **"Name"** from `kingbpop7` to `VairagyaNodes` (or `Tajbinder Bains`).
3. Save.

This does not change the username (`Dragonmonk111`) or any URLs / package ownership / permissions. It only changes the display name shown next to the avatar on the package page and on commit hovers.

---

## Step 2 — Rebuild and push v0.1.1

Open a Docker-capable terminal at `c:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw\wavs\warg-registry`.

```powershell
# Authenticate to GHCR if not already
docker login ghcr.io -u dragonmonk111
# (paste a PAT with write:packages scope as the password)

# Build with the new labels — note the .1 bump
docker build -t ghcr.io/dragonmonk111/warg-registry:0.1.1 .

# Push
docker push ghcr.io/dragonmonk111/warg-registry:0.1.1

# (Optional) re-tag latest if you point Akash deployments at :latest
docker tag ghcr.io/dragonmonk111/warg-registry:0.1.1 ghcr.io/dragonmonk111/warg-registry:latest
docker push ghcr.io/dragonmonk111/warg-registry:latest
```

**Do not delete `:0.1.0`.** Anyone pinning to it stays on a working image; `:0.1.1` is a strict superset (only adds labels and metadata, no behaviour change).

If GHCR write fails with a 403 / "denied", check the PAT has the **`write:packages`** scope (Settings → Developer settings → Personal access tokens).

---

## Step 3 — Connect package to repo

Now that the OCI label declares `image.source=https://github.com/Dragonmonk111/junoclaw`, GitHub auto-links it. To force the link to refresh:

1. Open https://github.com/users/Dragonmonk111/packages/container/package/warg-registry
2. The "Connect Repository" panel should now offer a one-click connect — click it, pick **`Dragonmonk111/junoclaw`**, confirm.
3. The README of the linked repo will then appear on the package page.

If the panel doesn't auto-detect after the v0.1.1 push, force it manually via the green button on the same page.

---

## Step 4 — SBOM scan (optional, ~5 minutes)

Cheap proof-of-due-care under the EU CRA framing. Install [`syft`](https://github.com/anchore/syft) once (`scoop install syft` or `winget install anchore.syft`), then:

```powershell
# Generate an SPDX-JSON SBOM for the new image
syft scan ghcr.io/dragonmonk111/warg-registry:0.1.1 -o spdx-json=warg-registry-0.1.1.spdx.json

# (Optional) commit it to the repo as audit evidence
mv warg-registry-0.1.1.spdx.json wavs/warg-registry/SBOM.spdx.json
```

The SBOM lists every transitive dep + their licenses. If anything has a non-permissive license, surface it.

---

## Step 5 — Cosign signature (optional, ~10 minutes)

Free credibility for any procurement-side scoring. Install [`cosign`](https://github.com/sigstore/cosign):

```powershell
# Generate a keypair (one-time; password-protect the private key)
cosign generate-key-pair

# Sign the new image (uses the cosign.key from previous step)
cosign sign --key cosign.key ghcr.io/dragonmonk111/warg-registry:0.1.1

# Verify (anyone can run this)
cosign verify --key cosign.pub ghcr.io/dragonmonk111/warg-registry:0.1.1
```

Commit `cosign.pub` to the repo (it's the public key, safe to share). **Never commit `cosign.key`** — that's the private signing key.

If you want keyless signing (Sigstore-style, no key management), skip the keypair and use `cosign sign --yes ghcr.io/dragonmonk111/warg-registry:0.1.1` — it'll OIDC-prompt against your GitHub identity.

---

## Verification

After Steps 0-3 (the only required ones), the package page at https://github.com/users/Dragonmonk111/packages/container/package/warg-registry should show:

- Linked to repo `Dragonmonk111/junoclaw`.
- License: **Apache-2.0** (auto-discovered from the `org.opencontainers.image.licenses` label).
- Description: from the label.
- README: pulled from `wavs/warg-registry/README.md` if one exists, else from the repo root.

If any of those don't show, the build / push didn't pick up the new labels. Re-pull and inspect:

```powershell
docker pull ghcr.io/dragonmonk111/warg-registry:0.1.1
docker inspect ghcr.io/dragonmonk111/warg-registry:0.1.1 | findstr opencontainers
```

You should see all the `org.opencontainers.image.*` labels listed.

---

## After this lands

Update [`memory/warg-registry-package.md`](../memory/warg-registry-package.md) §"Action items" — strike through items 1, 2, 3, 4 once done. Leave 5 (LICENSE / SECURITY) annotated as "already done" and 6 (README badges) optional.

Add a one-line entry to the next `memory/lessons-YYYY-MM-DD.md` noting the v0.1.1 bump landed.

Update [`memory/SESSION_PROTOCOL.md`](../memory/SESSION_PROTOCOL.md) §3 T9 to "DONE — see `memory/lessons-...md`" once Steps 0-3 are complete.

---

*Apache-2.0. Created 2026-05-14 as part of CAB-Cluster-C.*
