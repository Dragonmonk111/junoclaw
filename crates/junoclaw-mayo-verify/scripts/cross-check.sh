#!/usr/bin/env bash
# Run the MAYO-2 cross-check test against the reference C implementation.
# Requires: CMake, a C toolchain, and Rust.
# Usage: ./scripts/cross-check.sh

set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== Running junoclaw-mayo-verify cross-check against sriracha-mayo (C impl) ==="
echo "Note: This downloads and builds sriracha-mayo which requires CMake."
echo ""

cargo test --features test-c test_mayo2_cross_check_sriracha -- --nocapture

echo ""
echo "=== Cross-check passed ==="
