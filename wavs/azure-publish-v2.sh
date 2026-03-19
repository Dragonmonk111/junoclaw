#!/bin/bash
source ~/.cargo/env

echo "=== Step 1: Create namespace 'junoclaw' ==="
warg publish start junoclaw:verifier 2>&1
# If that doesn't work, try direct namespace init
warg namespace init junoclaw 2>&1 || echo "(namespace init not available, trying publish flow)"

echo "=== Step 2: Check available publish commands ==="
warg publish --help 2>&1 | head -20

echo "=== Step 3: Try full publish flow ==="
# The publish flow: start -> init -> release -> submit
warg publish start junoclaw:verifier 2>&1
warg publish init junoclaw:verifier 2>&1  
warg publish release --name junoclaw:verifier --version 0.1.0 ~/junoclaw_wavs_component.wasm 2>&1
warg publish submit 2>&1

echo "=== Step 4: Verify ==="
warg info junoclaw:verifier 2>&1

echo "=== Done ==="
