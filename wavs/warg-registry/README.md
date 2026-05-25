# `warg-registry` — JunoClaw Warg component registry

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)
[![Image: GHCR](https://img.shields.io/badge/image-ghcr.io%2Fdragonmonk111%2Fwarg--registry-darkblue)](https://github.com/users/Dragonmonk111/packages/container/package/warg-registry)
[![Source: junoclaw](https://img.shields.io/badge/source-Dragonmonk111%2Fjunoclaw-darkgreen)](https://github.com/Dragonmonk111/junoclaw/tree/main/wavs/warg-registry)

A self-contained [Warg](https://warg.io/) component registry that serves the `junoclaw:verifier` WASM component to a [WAVS](https://github.com/Lay3rLabs/wavs) operator. Designed for **Akash-to-Akash** deployment — zero centralised cloud dependency between the operator and the registry it pulls from.

## Image

```
ghcr.io/dragonmonk111/warg-registry:0.1.1
```

| Item | Value |
|---|---|
| Base image | `debian:bookworm-slim` |
| Size | ~95 MB compressed |
| Listening port | `8090/tcp` |
| Storage | In-memory; component re-published from baked-in WASM on container start |
| License | [Apache-2.0](../../LICENSE) |
| Source | [Dragonmonk111/junoclaw `wavs/warg-registry/`](https://github.com/Dragonmonk111/junoclaw/tree/main/wavs/warg-registry) |
| SBOM | Available via `syft scan ghcr.io/dragonmonk111/warg-registry:0.1.1 -o spdx-json` |

## What this is

WAVS operators need to fetch their assigned WASM component from a Warg registry on startup. This image **bundles the registry, its operator key, and the component into one container** so that running a JunoClaw WAVS operator on Akash doesn't require a separate registry deployment on a centralised cloud.

The component bundled in this image is `junoclaw:verifier@0.1.0` — the BN254 / Groth16 verifier that fronts the [`zk-verifier`](../../contracts/zk-verifier/) CosmWasm contract. See [`memory/bn254-precompile.md`](../../memory/bn254-precompile.md) for the precompile context.

## What this is NOT

- **Not a general-purpose Warg registry.** It hosts exactly one component, baked at build time. It is not intended as a shared registry for multiple operators or projects.
- **Not durable storage.** Storage is in-memory by design (see `LEGAL_CAVEATS.md` §7). Container restart re-publishes the component from the baked WASM (~10 s startup); the content hash (`sha256:b40d3fca...`) is deterministic, so consumers see the same artifact across restarts.
- **Not a registry server you should publish other components to.** The `entrypoint.sh` runs `warg publish` against itself for the bundled component on every startup. Pushing other components from outside is not supported through any documented interface.

## How to deploy on Akash

The `wavs/akash.sdl.yml` in the parent directory deploys both the operator and this registry, configured to reach each other via Akash service-discovery. From the repo root:

```bash
provider-services tx deployment create wavs/akash.sdl.yml --from <key> --gas auto -y
```

Once both services are up, the operator pulls `junoclaw:verifier@0.1.0` from this image's port `8090` and begins answering verification work-items posted by the on-chain `zk-verifier` contract.

## How to deploy locally (development)

```bash
docker pull ghcr.io/dragonmonk111/warg-registry:0.1.1
docker run --rm -p 8090:8090 ghcr.io/dragonmonk111/warg-registry:0.1.1
```

Confirm the registry is live and the component is published:

```bash
curl http://localhost:8090/
# warg-server self-introduces with the operator key fingerprint

# (with warg CLI installed)
warg --registry http://localhost:8090 info junoclaw:verifier
```

## Operational risk disclosure

Per [`docs/LEGAL_CAVEATS.md`](../../docs/LEGAL_CAVEATS.md) §7:

> The warg component registry uses in-memory storage. If the Akash container restarts, the component is re-published automatically from the baked-in WASM binary (494 KB, ~10 s startup). The content hash (`sha256:b40d3fca...`) is deterministic — no data loss, but brief downtime is possible.

For a production-grade deployment with durable storage, you would replace this container with a hosted Warg registry (or a Warg server backed by S3 / IPFS / Jackal Protocol). For Juno mainnet during the BN254-precompile rollout, the in-memory deployment is the documented design and the trade-off is accepted.

### Strategic note — OCI is the canonical path going forward (2026-05-16)

Per [`memory/oci-component-publishing.md`](../../memory/oci-component-publishing.md), the canonical production publishing path for `junoclaw:verifier` is now OCI (via `wkg` from `bytecodealliance/wasm-pkg-tools`), not warg. The component is published as a standard OCI artifact to `ghcr.io/dragonmonk111/junoclaw/verifier:<VERSION>` alongside the container images. The warg-server container in this directory remains as the **warg-protocol fallback** for operators who prefer warg, and as a working reference implementation, but is no longer the strategic distribution channel.

## Security

Vulnerability disclosure: see [`SECURITY.md`](../../SECURITY.md) at the repo root. Findings against the registry image (the warg server itself, the entrypoint script, the operator key handling) are in scope alongside the rest of JunoClaw.

The image is signed under the OCI standard labels (`org.opencontainers.image.licenses`, `image.source`, `image.version`, `image.title`, `image.description`, etc.). SBOM tooling (Syft, Trivy, Anchore) self-discovers origin and license without external lookup.

Cosign signature is **not yet** attached to v0.1.1 — see [`docs/WARG_REGISTRY_v0_1_1_BUMP.md`](../../docs/WARG_REGISTRY_v0_1_1_BUMP.md) §"Step 5" for the planned key generation + sign step. Will land in v0.1.2.

## Versions

| Tag | Released | Changes |
|---|---|---|
| `0.1.0` | 2026-03-18 | Initial publication. No OCI labels. |
| `0.1.1` | 2026-05-14 (planned push) | Adds OCI standard labels; no behaviour change; strict superset of `0.1.0`. **Do not pin to `0.1.0` for new deployments.** |
| `0.1.2` | TBD | Planned: Cosign signature + checked-in SBOM. |

Pinning to a specific tag is recommended for production. `:latest` follows the most recent push and may move without warning.

## Cross-references

- [`docs/WARG_REGISTRY_v0_1_1_BUMP.md`](../../docs/WARG_REGISTRY_v0_1_1_BUMP.md) — operator-action runbook for the v0.1.1 push.
- [`memory/warg-registry-package.md`](../../memory/warg-registry-package.md) — recall-layer entry on the package's hygiene state and the 2026-06-11 2FA deadline.
- [`docs/LEGAL_CAVEATS.md`](../../docs/LEGAL_CAVEATS.md) §7 — operational risk disclosure.
- [`SECURITY.md`](../../SECURITY.md) — vulnerability disclosure policy.
- [`memory/bn254-precompile.md`](../../memory/bn254-precompile.md) — the BN254 precompile context that motivates this registry.
- [`wavs/akash.sdl.yml`](../akash.sdl.yml) — Akash deployment SDL pairing the registry with the operator.

---

*Apache-2.0. JunoClaw is heavily AI-assisted; see the [repo root README](../../README.md) and [`SECURITY.md`](../../SECURITY.md) §"License posture for AI-assisted contributions" for the disclosure.*
