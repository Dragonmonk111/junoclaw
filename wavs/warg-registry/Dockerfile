# JunoClaw Warg Component Registry — Akash-to-Akash
# Serves junoclaw:verifier WASM component to the WAVS operator
#
# Build:
#   docker build -t ghcr.io/dragonmonk111/warg-registry:0.1.1 .
# Push:
#   docker push ghcr.io/dragonmonk111/warg-registry:0.1.1
#
# The WASM binary is baked into the image at build time.
# To update the component, rebuild with the new .wasm file.

FROM rust:slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install warg-server --version 0.10.0
RUN cargo install warg-cli --version 0.10.0

FROM debian:bookworm-slim

# ── OCI standard labels (on final stage so they survive into the published image) ──
# SBOM tooling (syft, trivy, anchore) and procurement scanners self-discover
# the image's origin, license, and SPDX identity without external lookup.
LABEL org.opencontainers.image.title="warg-registry"
LABEL org.opencontainers.image.description="JunoClaw warg component registry — serves junoclaw:verifier WASM component to a WAVS operator. Akash-deployable, in-memory storage, deterministic content hash."
LABEL org.opencontainers.image.source="https://github.com/Dragonmonk111/junoclaw"
LABEL org.opencontainers.image.url="https://github.com/Dragonmonk111/junoclaw/tree/main/wavs/warg-registry"
LABEL org.opencontainers.image.documentation="https://github.com/Dragonmonk111/junoclaw/blob/main/docs/LEGAL_CAVEATS.md"
LABEL org.opencontainers.image.licenses="Apache-2.0"
LABEL org.opencontainers.image.vendor="VairagyaNodes"
LABEL org.opencontainers.image.authors="VairagyaNodes <ops@vairagyanodes.com>"
LABEL org.opencontainers.image.version="0.1.1"

RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/cargo/bin/warg-server /usr/local/bin/warg-server
COPY --from=builder /usr/local/cargo/bin/warg /usr/local/bin/warg

# Bake the WASM component into the image
COPY junoclaw_wavs_component.wasm /opt/component.wasm
COPY entrypoint.sh /opt/entrypoint.sh
RUN chmod +x /opt/entrypoint.sh

EXPOSE 8090

ENTRYPOINT ["/opt/entrypoint.sh"]
