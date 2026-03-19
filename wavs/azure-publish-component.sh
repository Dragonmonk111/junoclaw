#!/bin/bash
source ~/.cargo/env

echo "=== Publishing junoclaw:verifier v0.1.0 ==="
echo "WASM file: $(ls -la ~/junoclaw_wavs_component.wasm)"

# Initialize the package namespace
echo "--- Step 1: Init namespace ---"
warg publish init junoclaw:verifier 2>&1

echo "--- Step 2: Publish release ---"
warg publish release --name junoclaw:verifier --version 0.1.0 ~/junoclaw_wavs_component.wasm 2>&1

echo "--- Step 3: Verify ---"
warg info junoclaw:verifier 2>&1

echo "=== Done ==="
