#!/usr/bin/env bash
# Orchestrated moultbook deployment: zk-verifier FIRST, then moultbook-v0
# wired to it.
#
# This closes the gap where deploy-moultbook.sh needed a zk_verifier address
# but had no way to obtain one. Here we:
#   1. Run deploy-zk-verifier.sh   (builds + deploys pure + precompile variants,
#                                   writes devnet/deploy.env with PRECOMPILE_ADDR)
#   2. Source deploy.env and export ZK_VERIFIER=PRECOMPILE_ADDR
#   3. Run deploy-moultbook.sh     (instantiates moultbook with zk_verifier wired,
#                                   runs the Post -> GetEntry -> Redact smoke test)
#
# The anonymous-publish path (PublishAnon) additionally needs a membership
# verifying key stored on the zk-verifier plus a matching Groth16 proof
# fixture. That fixture does not exist yet, so this script deliberately wires
# the zk_verifier address but leaves MEMBERSHIP_VK_HASH unset — PublishAnon
# then reports MembershipVkNotConfigured while the standard Post/Redact path
# (which the smoke test exercises) works end to end. See the "Follow-up"
# note printed at the end.
#
# Environment overrides (passed through to the child scripts):
#   BUILD=1          (default 1)  — set 0 to reuse pre-built wasm artefacts
#   SMOKE=1          (default 1)  — set 0 to skip the moultbook smoke test
#   CHAIN_ID         (default junoclaw-bn254-1)
#   CONTAINER        (default junoclaw-bn254-devnet)
#   NODE             (default http://localhost:26657)
#   KEYRING_BACKEND  (default test)
#   GAS_PRICES       (default inherited per child script)
#   MEMBERSHIP_VK_HASH  (optional) — if set, moultbook is instantiated with the
#                                    anonymous-publish path fully configured
#   USE_PURE_ZK=0    (default 0)  — set 1 to wire the pure-Wasm zk-verifier
#                                   instead of the precompile variant

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_DIR="$(cd "${HERE}/.." && pwd)"

CONTAINER="${CONTAINER:-junoclaw-bn254-devnet}"
USE_PURE_ZK="${USE_PURE_ZK:-0}"

# ── 0. Sanity: devnet container running. ─────────────────────────────────

if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER}\$"; then
  echo "error: devnet container '${CONTAINER}' is not running" >&2
  echo "       start it with: bash ${DEVNET_DIR}/scripts/run-devnet.sh" >&2
  exit 1
fi

# ── 1. Deploy the zk-verifier (build + store + instantiate both variants). ─

echo "════════════════════════════════════════════════════════════════════"
echo " STEP 1/2 — zk-verifier"
echo "════════════════════════════════════════════════════════════════════"
bash "${HERE}/deploy-zk-verifier.sh"

if [ ! -f "${DEVNET_DIR}/deploy.env" ]; then
  echo "error: deploy-zk-verifier.sh did not produce ${DEVNET_DIR}/deploy.env" >&2
  exit 2
fi

# shellcheck disable=SC1090
source "${DEVNET_DIR}/deploy.env"

if [ "${USE_PURE_ZK}" = "1" ]; then
  ZK_ADDR="${PURE_ADDR:-}"
  ZK_KIND="pure-Wasm"
else
  ZK_ADDR="${PRECOMPILE_ADDR:-}"
  ZK_KIND="precompile"
fi

if [ -z "${ZK_ADDR}" ]; then
  echo "error: could not read a zk-verifier address (${ZK_KIND}) from deploy.env" >&2
  cat "${DEVNET_DIR}/deploy.env" >&2
  exit 3
fi

echo
echo "[deploy-moultbook-full] zk-verifier (${ZK_KIND}) addr: ${ZK_ADDR}"

# ── 2. Deploy moultbook, wired to the zk-verifier. ───────────────────────

echo
echo "════════════════════════════════════════════════════════════════════"
echo " STEP 2/2 — moultbook-v0 (zk_verifier=${ZK_ADDR})"
echo "════════════════════════════════════════════════════════════════════"

ZK_VERIFIER="${ZK_ADDR}" \
MEMBERSHIP_VK_HASH="${MEMBERSHIP_VK_HASH:-}" \
  bash "${HERE}/deploy-moultbook.sh"

# ── 3. Summary. ──────────────────────────────────────────────────────────

echo
echo "════════════════════════════════════════════════════════════════════"
echo " DONE — orchestrated deployment complete"
echo "════════════════════════════════════════════════════════════════════"
echo "  zk-verifier (${ZK_KIND}): ${ZK_ADDR}"
echo "  moultbook env:            ${DEVNET_DIR}/moultbook.env"
echo
if [ -z "${MEMBERSHIP_VK_HASH:-}" ]; then
  echo "Follow-up (PublishAnon end-to-end):"
  echo "  1. Generate a membership-circuit verifying key (VK) + a Groth16 proof"
  echo "     fixture (mirror contracts/moultbook-v0/src/tests.rs gen_proof)."
  echo "  2. StoreVk on the zk-verifier at ${ZK_ADDR}."
  echo "  3. Re-run with MEMBERSHIP_VK_HASH=sha256:<vk-hash> to enable PublishAnon,"
  echo "     then exercise PublishAnon with the matching proof."
fi
