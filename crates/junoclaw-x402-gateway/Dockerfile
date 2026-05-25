# Multi-stage build for the JunoClaw x402 gateway.
# Final image: distroless cc with the static-ish binary only. ~25 MB.
# Apache-2.0. Cosign-signed at publish time (OIDC keyless).

# --- Builder ----------------------------------------------------------------
FROM rust:1.83-bookworm AS builder

# Build deps. ca-certificates needed at build time for cargo fetching crates.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy the workspace skeleton first so dependency resolution is cached
# independently of source-code edits.
COPY Cargo.toml Cargo.lock ./
COPY crates/junoclaw-x402-gateway/Cargo.toml crates/junoclaw-x402-gateway/Cargo.toml
COPY crates/junoclaw-core/Cargo.toml crates/junoclaw-core/Cargo.toml
COPY crates/junoclaw-runtime/Cargo.toml crates/junoclaw-runtime/Cargo.toml
COPY crates/junoclaw-cli/Cargo.toml crates/junoclaw-cli/Cargo.toml
COPY crates/junoclaw-daemon/Cargo.toml crates/junoclaw-daemon/Cargo.toml
COPY plugins/ plugins/

# Pre-build deps with a placeholder src tree.
RUN mkdir -p crates/junoclaw-x402-gateway/src && \
    echo "fn main(){}" > crates/junoclaw-x402-gateway/src/main.rs && \
    cargo build --release --locked -p junoclaw-x402-gateway && \
    rm -rf crates/junoclaw-x402-gateway/src

# Now copy actual sources and rebuild only this crate.
COPY crates/junoclaw-x402-gateway/src crates/junoclaw-x402-gateway/src

RUN cargo build --release --locked -p junoclaw-x402-gateway && \
    strip target/release/junoclaw-x402-gateway

# --- Runtime ----------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

LABEL org.opencontainers.image.title="junoclaw-x402-gateway"
LABEL org.opencontainers.image.description="HTTP/x402 gateway for JunoClaw task-ledger"
LABEL org.opencontainers.image.licenses="Apache-2.0"
LABEL org.opencontainers.image.source="https://github.com/Dragonmonk111/junoclaw"
LABEL org.opencontainers.image.vendor="Dragonmonk111"

COPY --from=builder /build/target/release/junoclaw-x402-gateway /usr/local/bin/junoclaw-x402-gateway

USER nonroot
EXPOSE 8402
ENTRYPOINT ["/usr/local/bin/junoclaw-x402-gateway"]
