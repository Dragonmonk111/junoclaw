# syntax=docker/dockerfile:1.6
#
# Build: junod linked against a BN254-patched libwasmvm.
#
# Three stages:
#   1. rust-builder → compiles the patched cosmwasm-vm + libwasmvm
#   2. go-builder   → builds junod against that libwasmvm
#   3. runtime      → slim debian with junod + entrypoint script

ARG COSMWASM_TAG=v2.2.0
ARG WASMVM_TAG=v2.2.4
ARG JUNO_TAG=v29.0.0
ARG RUST_TAG=1.78-bookworm
ARG GO_TAG=1.24-bookworm

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
    # cosmwasm-crypto-bn254/Cargo.toml declares `[workspace]` so the crate
    # is standalone when developed inside the junoclaw repo. Once it lives
    # *inside* the cosmwasm workspace we have to drop that declaration so
    # cargo does not see two competing workspace roots.
    sed -i '/^# Standalone workspace/,/^\[workspace\]/d' \
        /src/cosmwasm/packages/crypto-bn254/Cargo.toml && \
    cd /src/cosmwasm && \
    find /src/wasmvm-fork/patches -name "*.patch" -exec sed -i 's/\r$//' {} + && \
    git apply /src/wasmvm-fork/patches/cosmwasm-vm.imports.rs.patch && \
    git apply /src/wasmvm-fork/patches/cosmwasm-std.imports.rs.patch && \
    git apply /src/wasmvm-fork/patches/cosmwasm-std.traits.rs.patch && \
    cargo build --release -p cosmwasm-vm

# ── Clone CosmWasm/wasmvm (no patching needed) ──────────────────────────
# Rationale: BN254 host functions are wired entirely on the cosmwasm-vm
# side (do_bn254_* + env_imports.insert in instance.rs). libwasmvm just
# re-exports cosmwasm-vm via `cosmwasm-vm = { git = ... rev = v2.2.0 }`,
# so it transitively picks up our patched cosmwasm-vm with the BN254
# additions. We override the git ref to point at our patched local clone
# of cosmwasm so the resolved cosmwasm-vm is the patched one, not pristine.
RUN git clone --branch ${WASMVM_TAG} --depth 1 \
        https://github.com/CosmWasm/wasmvm /src/wasmvm && \
    cd /src/wasmvm/libwasmvm && \
    # Redirect cosmwasm-std and cosmwasm-vm git refs to our patched local
    # cosmwasm checkout so the BN254 additions are picked up at link time.
    # libwasmvm in v2.2.x pins cosmwasm via `git = ..., rev = vX.Y.Z`. The
    # rev value drifts across patch releases (v2.2.0..v2.2.4), so we accept
    # any v2.2.x and rewrite the dep to point at our local patched cosmwasm.
    sed -i -E 's|cosmwasm-std = \{ git = "https://github\.com/CosmWasm/cosmwasm\.git", rev = "v2\.2\.[0-9]+"|cosmwasm-std = { path = "/src/cosmwasm/packages/std"|' Cargo.toml && \
    sed -i -E 's|cosmwasm-vm = \{ git = "https://github\.com/CosmWasm/cosmwasm\.git", rev = "v2\.2\.[0-9]+"|cosmwasm-vm = { path = "/src/cosmwasm/packages/vm"|' Cargo.toml && \
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
    # Dynamic linking against libwasmvm.so. The runtime stage below copies
    # the .so to /usr/local/lib and runs ldconfig, so the resulting junod
    # picks it up. Static linking trips ld on glibc's getpwnam_r/getgrnam_r
    # NSS shims which have no static counterparts in glibc.
    LEDGER_ENABLED=false make build
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
# wasmvm's Go binding `dlopen`s the arch-specific name `libwasmvm.x86_64.so`.
# Symlink to the architecture-neutral .so we just installed so dynamic
# loading succeeds regardless of which name juno is built to look up.
RUN ln -s /usr/local/lib/libwasmvm.so /usr/local/lib/libwasmvm.x86_64.so && \
    ldconfig

COPY devnet/scripts/init-genesis.sh /opt/juno/init-genesis.sh
COPY devnet/genesis-template.json /opt/juno/genesis-template.json
RUN sed -i 's/\r$//' /opt/juno/init-genesis.sh && chmod +x /opt/juno/init-genesis.sh

EXPOSE 26656 26657 1317 9090
ENV HOME=/root
VOLUME ["/root/.juno"]

ENTRYPOINT ["/opt/juno/init-genesis.sh"]
CMD ["start"]
