#!/usr/bin/env bash
# Transfers junoclaw-devnet.tar from a Windows host (where it was saved
# via `docker save`) onto the VirtualBox Ubuntu VM, loads it, and starts
# the devnet on /mnt/extdrive with localhost-only port bindings.
#
# This bypasses the patch-corruption / docker-build path that has been
# repeatedly failing on the VM.
#
# Run on the VirtualBox VM (NOT on Windows).
#
# Three transfer modes are supported:
#
#   1. shared-folder  — VirtualBox shared folder mounts (default if /media/sf_*
#                       exists). No network needed.
#   2. http           — Windows host serves the tar over HTTP (python3 -m http.server).
#                       Set TRANSFER_MODE=http and WIN_HOST=<windows-ip>.
#   3. local          — tar is already on /mnt/extdrive (you scp'd it earlier).
#                       Set TRANSFER_MODE=local TAR_PATH=/path/to/tar.
#
# Examples:
#
#   # If you've shared D: as 'D_DRIVE' in VirtualBox shared folders:
#   ./transfer-image-from-windows.sh
#
#   # If you've started `python -m http.server 8000` on Windows and the
#   # VM can reach the host at 192.168.1.100:
#   TRANSFER_MODE=http WIN_HOST=192.168.1.100:8000 ./transfer-image-from-windows.sh
#
#   # If you've already copied the tar to /mnt/extdrive:
#   TRANSFER_MODE=local TAR_PATH=/mnt/extdrive/junoclaw-devnet.tar \
#       ./transfer-image-from-windows.sh

set -euo pipefail

# ── Config ─────────────────────────────────────────────────────────────────

DEVNET_DIR="${DEVNET_DIR:-/mnt/extdrive/junoclaw-devnet}"
DATA_DIR="${DATA_DIR:-${DEVNET_DIR}/juno-data}"
CACHE_TAR="${DEVNET_DIR}/junoclaw-devnet.tar"
IMAGE_TAG="${IMAGE_TAG:-junoclaw/junod-bn254:devnet}"
CONTAINER_NAME="${CONTAINER_NAME:-junoclaw-devnet-ext}"

# Port mappings — localhost only, well clear of any host validator.
RPC_PORT="${RPC_PORT:-36657}"
P2P_PORT="${P2P_PORT:-36656}"
REST_PORT="${REST_PORT:-11317}"

TRANSFER_MODE="${TRANSFER_MODE:-auto}"

# ── Helpers ────────────────────────────────────────────────────────────────

say() { printf '\n[devnet-transfer] %s\n' "$*"; }
die() { printf '\n[devnet-transfer] error: %s\n' "$*" >&2; exit 1; }

require_cmd() { command -v "$1" >/dev/null 2>&1 || die "missing dependency: $1"; }

# ── Pre-flight ────────────────────────────────────────────────────────────

require_cmd docker
require_cmd curl

# Disk-space sanity: a docker load of a 2GB tar typically inflates to ~3GB
# on disk after layer extraction. Demand at least 5GB free at /mnt/extdrive.
free_kb=$(df -kP "$(dirname "${DEVNET_DIR}")" | awk 'NR==2 {print $4}')
if [ "${free_kb}" -lt 5242880 ]; then
  die "less than 5GB free at $(dirname "${DEVNET_DIR}") (have ${free_kb} KB)"
fi

mkdir -p "${DEVNET_DIR}" "${DATA_DIR}"

# Already loaded? Skip the transfer entirely.
if docker image inspect "${IMAGE_TAG}" >/dev/null 2>&1; then
  say "image ${IMAGE_TAG} already present — skipping transfer"
else
  # ── Transfer mode resolution ────────────────────────────────────────────

  if [ "${TRANSFER_MODE}" = "auto" ]; then
    if [ -n "${TAR_PATH:-}" ] && [ -f "${TAR_PATH:-}" ]; then
      TRANSFER_MODE="local"
    elif compgen -G "/media/sf_*" >/dev/null 2>&1; then
      TRANSFER_MODE="shared-folder"
    elif [ -n "${WIN_HOST:-}" ]; then
      TRANSFER_MODE="http"
    else
      die "auto-detect failed. Set TRANSFER_MODE to one of: shared-folder, http, local"
    fi
  fi

  say "transfer mode: ${TRANSFER_MODE}"

  case "${TRANSFER_MODE}" in
    local)
      [ -f "${TAR_PATH:-}" ] || die "TAR_PATH not set or file missing"
      cp -v "${TAR_PATH}" "${CACHE_TAR}"
      ;;
    shared-folder)
      # Find first mount under /media/sf_* that contains the tar.
      found=""
      for mount in /media/sf_*; do
        [ -d "${mount}" ] || continue
        if [ -f "${mount}/junoclaw-devnet.tar" ]; then
          found="${mount}/junoclaw-devnet.tar"
          break
        fi
      done
      [ -n "${found}" ] || die "no /media/sf_*/junoclaw-devnet.tar found. Ensure the user is in 'vboxsf' group ('sudo usermod -aG vboxsf \$USER' then relogin) and the share contains the tar"
      cp -v "${found}" "${CACHE_TAR}"
      ;;
    http)
      [ -n "${WIN_HOST:-}" ] || die "WIN_HOST not set (e.g. WIN_HOST=192.168.1.100:8000)"
      url="http://${WIN_HOST}/junoclaw-devnet.tar"
      say "downloading from ${url}…"
      curl --fail --silent --show-error -o "${CACHE_TAR}" "${url}" \
        || die "download failed. Confirm 'python3 -m http.server' is running on Windows in the directory containing junoclaw-devnet.tar"
      ;;
    *)
      die "unknown TRANSFER_MODE=${TRANSFER_MODE}"
      ;;
  esac

  # Sanity-check the tar before docker chews on it.
  tar_size=$(stat -c %s "${CACHE_TAR}")
  if [ "${tar_size}" -lt 100000000 ]; then
    die "tar ${CACHE_TAR} is suspiciously small (${tar_size} bytes); transfer truncated?"
  fi
  say "tar transferred: ${tar_size} bytes"

  say "loading image into local Docker daemon…"
  docker load -i "${CACHE_TAR}"

  docker image inspect "${IMAGE_TAG}" >/dev/null 2>&1 \
    || die "loaded tar but ${IMAGE_TAG} not present. The tar may have been built with a different tag."
fi

# ── Run container ─────────────────────────────────────────────────────────

# Stop & remove any prior incarnation so port bindings are clean.
if docker ps -a --format '{{.Names}}' | grep -qx "${CONTAINER_NAME}"; then
  say "removing existing container ${CONTAINER_NAME}"
  docker rm -f "${CONTAINER_NAME}" >/dev/null
fi

say "starting devnet container ${CONTAINER_NAME}"
docker run -d \
  --name "${CONTAINER_NAME}" \
  --restart unless-stopped \
  -v "${DATA_DIR}:/root/.juno" \
  -p "127.0.0.1:${RPC_PORT}:26657" \
  -p "127.0.0.1:${P2P_PORT}:26656" \
  -p "127.0.0.1:${REST_PORT}:1317" \
  "${IMAGE_TAG}" \
  >/dev/null

# ── Health-check loop ─────────────────────────────────────────────────────

say "waiting for RPC at http://localhost:${RPC_PORT}/status …"
for i in $(seq 1 40); do
  if curl -sf "http://localhost:${RPC_PORT}/status" >/dev/null 2>&1; then
    say "✓ RPC up after $((i * 2)) seconds"
    curl -s "http://localhost:${RPC_PORT}/status" | head -c 240
    printf '\n'
    break
  fi
  sleep 2
  if [ "$i" -eq 40 ]; then
    say "✗ RPC did not come up in 80 seconds"
    docker logs --tail 60 "${CONTAINER_NAME}" >&2
    die "see container logs above"
  fi
done

cat <<EOF

[devnet-transfer] devnet is up.
  container : ${CONTAINER_NAME}
  data dir  : ${DATA_DIR}
  RPC       : http://localhost:${RPC_PORT}
  P2P       : tcp://localhost:${P2P_PORT}
  REST      : http://localhost:${REST_PORT}

Next steps (from this VM):

  # If contracts are not yet present, copy the .wasm files in:
  docker cp /path/to/junoclaw/devnet/zk_verifier_pure.wasm       ${CONTAINER_NAME}:/tmp/
  docker cp /path/to/junoclaw/devnet/zk_verifier_precompile.wasm ${CONTAINER_NAME}:/tmp/

  # Then run the existing deploy + benchmark scripts:
  NODE=http://localhost:${RPC_PORT} BUILD=0 ./deploy-zk-verifier.sh
  NODE=http://localhost:${RPC_PORT} ./benchmark.sh

EOF
