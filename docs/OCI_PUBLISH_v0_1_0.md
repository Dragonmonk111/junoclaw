# OCI Publish — `junoclaw:verifier` v0.1.0

*Step-by-step user runbook for the OCI publishing pivot decided in [`memory/oci-component-publishing.md`](../memory/oci-component-publishing.md). Replaces the warg-server as the canonical component distribution path. Drafted 2026-05-17. ~30 minutes user-side work, fully click-paste.*

## Pre-flight

- [ ] GitHub Personal Access Token with `write:packages` scope already exists (used for the warg-registry container push). Re-use the same token.
- [ ] Rust toolchain ≥1.75 installed (already present — `rustc --version`).
- [ ] The `junoclaw_verifier.wasm` artifact is built and present at `wavs/component/target/wasm32-wasip2/release/junoclaw_verifier.wasm`. If not, run `cargo component build --release -p junoclaw-verifier` from the repo root.
- [ ] `cosign` is installed (per [`docs/GITHUB_2FA_STEPWISE_GUIDE.md`](./GITHUB_2FA_STEPWISE_GUIDE.md) — `winget install sigstore.cosign`).

## Step 1 — Install `wkg` (~3 min)

```powershell
cargo install wkg
```

Verify:

```powershell
wkg --version
# expect: wkg 0.x.x
```

If `cargo install` is too slow, use `cargo binstall wkg` (downloads pre-built binary).

## Step 2 — Authenticate to GHCR (~2 min)

```powershell
$env:CR_PAT = "<your-ghcr-pat-here>"
$env:CR_PAT | docker login ghcr.io -u dragonmonk111 --password-stdin
```

`wkg` re-uses Docker's GHCR credentials, so a single `docker login` is enough.

## Step 3 — Configure `wkg` for GHCR (~2 min)

```powershell
wkg config --edit
```

Add or merge in:

```toml
[default_registry]
type = "oci"

[registry."ghcr.io"]
type = "oci"

[registry."ghcr.io".oci]
auth = { username = "dragonmonk111", password = "$env:CR_PAT" }

[namespace_registries]
junoclaw = "ghcr.io"
```

This tells `wkg` that any `junoclaw:*` reference resolves to `ghcr.io/dragonmonk111/junoclaw/*` (per the `wasm-pkg-tools` namespace-prefix convention).

## Step 4 — Publish the component (~2 min)

```powershell
wkg oci push ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0 `
    wavs/component/target/wasm32-wasip2/release/junoclaw_verifier.wasm
```

Capture the digest the command returns — it will look like `sha256:abc123...`. This is the OCI manifest digest, NOT the wasm content hash. Both are useful: the manifest digest is for Cosign signing; the wasm content hash should match what the warg-registry container reports (`sha256:b40d3fca...`).

## Step 5 — Sign with Cosign (OIDC keyless) (~5 min)

```powershell
cosign sign ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0
```

This opens a browser. Authenticate via GitHub OAuth → Fulcio issues a 10-minute certificate → signature lands in Rekor (the Sigstore transparency log) → ephemeral key is discarded.

Verify yourself:

```powershell
cosign verify `
  --certificate-identity=dragonmonk111@users.noreply.github.com `
  --certificate-oidc-issuer=https://github.com/login/oauth `
  ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0
```

Expected output: a JSON object with `critical.identity.docker-reference` matching our package and `optional.Issuer` confirming GitHub OIDC.

## Step 6 — Publish a discovery `registry.json` (optional but recommended, ~5 min)

This lets WAVS operators auto-resolve `junoclaw:verifier@0.1.0` without manual `wkg config`. Skip if not running a static site already.

If you have a GitHub Pages site at `dragonmonk111.github.io`:

```bash
mkdir -p docs/.well-known/wasm-pkg
cat > docs/.well-known/wasm-pkg/registry.json <<'EOF'
{
  "preferredProtocol": "oci",
  "oci": {
    "registry": "ghcr.io",
    "namespacePrefix": "dragonmonk111/"
  }
}
EOF
git add docs/.well-known/wasm-pkg/registry.json
git commit -m "feat: publish wasm-pkg discovery metadata"
git push
```

After ~1 minute (GitHub Pages build), the file is live at `https://dragonmonk111.github.io/.well-known/wasm-pkg/registry.json`. Any WAVS operator running `wkg get junoclaw:verifier@0.1.0` against namespace `dragonmonk111` will auto-resolve via this discovery file.

## Step 7 — Operator-side pull verification (~3 min)

From a clean machine (or `--config /tmp/wkg-test.toml` to bypass local config):

```bash
wkg get junoclaw:verifier@0.1.0 --output /tmp/verifier.wasm
sha256sum /tmp/verifier.wasm
# expect: matches the wasm content hash reported by warg-registry container
```

This proves the OCI path works end-to-end without any registry config — operators just install `wkg` and pull.

## Step 8 — Update repo metadata (~3 min)

After publishing, the GHCR package page at `https://github.com/users/dragonmonk111/packages/container/package/junoclaw%2Fverifier` needs the same metadata fix as the warg-registry container:

- Link package to source repo: Settings → Package settings → "Connect repository" → `Dragonmonk111/junoclaw`
- Set visibility: Public
- Add description: "JunoClaw zero-knowledge verifier — WAVS-compatible WebAssembly component for BN254 Groth16 proof verification"

## Step 9 — Update `memory/oci-component-publishing.md` and the warg README

Replace the "to be published" placeholder in [`memory/oci-component-publishing.md`](../memory/oci-component-publishing.md) §"Implementation plan" with the actual digest + signing date. Update [`wavs/warg-registry/README.md`](../wavs/warg-registry/README.md) to add a "see also OCI artifact at..." pointer.

## Verification checklist

- [ ] `wkg oci pull ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0` succeeds from a fresh terminal
- [ ] `cosign verify` returns success with GitHub OIDC issuer
- [ ] GHCR package page shows "Published by Dragonmonk111" + repository link
- [ ] WASM content hash matches the warg-registry container's reported hash (`sha256:b40d3fca...`)
- [ ] `memory/oci-component-publishing.md` updated with digest + date

Once all checked, the OCI pivot is operationally complete. The warg-registry container stays running as the warg-protocol fallback.

---

*Apache-2.0. Created 2026-05-17.*
