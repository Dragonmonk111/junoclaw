#!/usr/bin/env bash
# Fix devnet config for single-node fast block production
HOME_DIR=/root/.juno

# Change timeout_commit to 0s for instant block production in single-node mode
sed -i 's/timeout_commit = "2s"/timeout_commit = "0s"/' "${HOME_DIR}/config/config.toml"

# Fix client.toml to use IPv4 localhost instead of localhost (which may resolve to IPv6)
sed -i 's|tcp://localhost:26657|tcp://127.0.0.1:26657|' "${HOME_DIR}/config/client.toml"

echo "[fix-config] timeout_commit=0s, client node=tcp://127.0.0.1:26657"
