# JunoClaw Warg Component Registry — Akash-to-Akash
# Serves junoclaw:verifier WASM component to the WAVS operator
#
# Build:
#   docker build -t ghcr.io/dragonmonk111/warg-registry:0.1.0 .
# Push:
#   docker push ghcr.io/dragonmonk111/warg-registry:0.1.0
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
