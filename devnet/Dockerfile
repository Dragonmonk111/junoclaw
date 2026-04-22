# syntax=docker/dockerfile:1.6
#
# Build: junod linked against a BN254-patched libwasmvm.
#
# Three stages:
#   1. rust-builder → compiles the patched cosmwasm-vm + libwasmvm
#   2. go-builder   → builds junod against that libwasmvm
#   3. runtime      → slim debian with junod + entrypoint script

ARG COSMWASM_TAG=v2.2.0
ARG WASMVM_TAG=v2.2.0
ARG JUNO_TAG=v29.0.0
ARG RUST_TAG=1.78-bookworm
ARG GO_TAG=1.22-bookworm

# ══════════════════════════════════════════════════════════════════════════
# Stage 1 — Rust builder: patched cosmwasm + libwasmvm.a
# ══════════════════════════════════════════════════════════════════════════
FROM rust:${RUST_TAG} AS rust-builder

ARG COSMWASM_TAG
ARG WASMVM_TAG

RUN apt-get update && apt-get install -y --no-install-recommends \
    git clang llvm-dev make pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /src

# ── Our wasmvm-fork crate + patches come in as a build context. ──────────
# The run-devnet.sh script tars up the ../wasmvm-fork directory from the
# junoclaw repo and pipes it in.
COPY wasmvm-fork/ /src/wasmvm-fork/

# ── Clone + patch CosmWasm/cosmwasm ─────────────────────────────────────
RUN git clone --branch ${COSMWASM_TAG} --depth 1 \
        https://github.com/CosmWasm/cosmwasm /src/cosmwasm && \
    cp -r /src/wasmvm-fork/cosmwasm-crypto-bn254 /src/cosmwasm/packages/crypto-bn254 && \
    cd /src/cosmwasm && \
    git apply /src/wasmvm-fork/patches/cosmwasm-vm.imports.rs.patch && \
    git apply /src/wasmvm-fork/patches/cosmwasm-std.imports.rs.patch && \
    git apply /src/wasmvm-fork/patches/cosmwasm-std.traits.rs.patch && \
    cargo build --release -p cosmwasm-vm

# ── Clone + patch CosmWasm/wasmvm ───────────────────────────────────────
RUN git clone --branch ${WASMVM_TAG} --depth 1 \
        https://github.com/CosmWasm/wasmvm /src/wasmvm && \
    cd /src/wasmvm && \
    git apply /src/wasmvm-fork/patches/wasmvm.api.rs.patch && \
    git apply /src/wasmvm-fork/patches/wasmvm.lib.go.patch && \
    cd libwasmvm && \
    # Rewrite the Cargo.toml patch path to point at our vendored crate.
    sed -i 's|../../cosmwasm-bn254/packages/crypto-bn254|/src/cosmwasm/packages/crypto-bn254|g' \
        Cargo.toml && \
    cargo build --release --lib

# Both the static and shared libraries end up in:
#   /src/wasmvm/libwasmvm/target/release/libwasmvm.a
#   /src/wasmvm/libwasmvm/target/release/libwasmvm.so

# ══════════════════════════════════════════════════════════════════════════
# Stage 2 — Go builder: junod linking against our libwasmvm
# ══════════════════════════════════════════════════════════════════════════
FROM golang:${GO_TAG} AS go-builder

ARG JUNO_TAG

RUN apt-get update && apt-get install -y --no-install-recommends \
    git make && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /src

RUN git clone --branch ${JUNO_TAG} --depth 1 \
        https://github.com/CosmosContracts/juno /src/juno

# Pull in our patched libwasmvm + wasmvm Go source.
COPY --from=rust-builder /src/wasmvm /src/wasmvm

# Force juno's go.mod to resolve CosmWasm/wasmvm against our local copy.
RUN cd /src/juno && \
    go mod edit -replace=github.com/CosmWasm/wasmvm/v2=/src/wasmvm && \
    go mod tidy && \
    LEDGER_ENABLED=false LINK_STATICALLY=true make build
# Output: /src/juno/bin/junod

# ══════════════════════════════════════════════════════════════════════════
# Stage 3 — Runtime
# ══════════════════════════════════════════════════════════════════════════
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates jq curl && \
    rm -rf /var/lib/apt/lists/*

COPY --from=go-builder /src/juno/bin/junod /usr/local/bin/junod
COPY --from=rust-builder /src/wasmvm/libwasmvm/target/release/libwasmvm.so \
     /usr/local/lib/libwasmvm.so
RUN ldconfig

COPY devnet/scripts/init-genesis.sh /opt/juno/init-genesis.sh
COPY devnet/genesis-template.json /opt/juno/genesis-template.json
RUN chmod +x /opt/juno/init-genesis.sh

EXPOSE 26656 26657 1317 9090
ENV HOME=/root
VOLUME ["/root/.juno"]

ENTRYPOINT ["/opt/juno/init-genesis.sh"]
CMD ["start"]
