FROM rust:1.78-slim

WORKDIR /build

# Copy Cargo files first for dependency caching
COPY Cargo.toml .
COPY src/ ./src/

# Build the prover daemon
RUN cargo build --release

# Copy binary to runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=0 /build/target/release/junoclaw-prover /usr/local/bin/junoclaw-prover

# Copy default config
COPY prover-config.example.toml /etc/prover/config.toml

WORKDIR /app
ENTRYPOINT ["junoclaw-prover"]
