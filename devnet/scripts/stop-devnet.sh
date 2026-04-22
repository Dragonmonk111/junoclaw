#!/usr/bin/env bash
# Tears down the devnet and removes its volume.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"

docker compose -f "${DEVNET_DIR}/docker-compose.yml" down -v

echo "[stop-devnet] Container stopped, volume wiped."
