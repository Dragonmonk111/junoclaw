#!/bin/bash
set -e

echo "=== Installing Node.js ==="
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh"
nvm install --lts
npm install -g pnpm @go-task/cli
echo "Node: $(node --version)"
echo "pnpm: $(pnpm --version)"

echo "=== Installing Rust cargo tools ==="
source "$HOME/.cargo/env"
cargo install cargo-binstall
cargo binstall cargo-component wasm-tools warg-cli wkg --locked --no-confirm --force

echo "=== Configuring wa.dev registry ==="
wkg config --default-registry wa.dev

echo "=== Cloning repos ==="
cd ~
[ -d wavs-foundry-template ] || git clone https://github.com/Lay3rLabs/wavs-foundry-template.git
[ -d junoclaw ] || git clone https://github.com/Dragonmonk111/junoclaw.git

echo "=== Verifying SGX ==="
ls -la /dev/sgx* 2>/dev/null || echo "No SGX devices found"

echo "=== DONE ==="
echo "All dependencies installed."
