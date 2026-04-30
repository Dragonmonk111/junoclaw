#!/usr/bin/env bash
# Runs once on container start. Creates a single-validator genesis with
# two pre-funded accounts, then execs `junod start` so tx signals reach
# the process.

set -euo pipefail

CHAIN_ID=${CHAIN_ID:-junoclaw-bn254-1}
MONIKER=${MONIKER:-bn254-validator}
KEYRING=${KEYRING_BACKEND:-test}
HOME_DIR=${HOME_DIR:-/root/.juno}

if [ ! -f "${HOME_DIR}/config/genesis.json" ]; then
  echo "[init-genesis] Fresh node — generating genesis…"

  junod init "${MONIKER}" --chain-id "${CHAIN_ID}" --home "${HOME_DIR}" --overwrite

  # Keys: auto-generated, headless (--no-backup suppresses the mnemonic
  # display/prompt; --keyring-backend test needs no passphrase).
  junod keys add admin     --keyring-backend "${KEYRING}" --home "${HOME_DIR}" --no-backup
  junod keys add verifier  --keyring-backend "${KEYRING}" --home "${HOME_DIR}" --no-backup
  junod keys add validator --keyring-backend "${KEYRING}" --home "${HOME_DIR}" --no-backup

  ADMIN_ADDR=$(junod keys show admin     -a --keyring-backend "${KEYRING}" --home "${HOME_DIR}")
  BENCH_ADDR=$(junod keys show verifier  -a --keyring-backend "${KEYRING}" --home "${HOME_DIR}")
  VAL_ADDR=$(junod   keys show validator -a --keyring-backend "${KEYRING}" --home "${HOME_DIR}")

  # Pre-fund.
  junod genesis add-genesis-account "${ADMIN_ADDR}" 1000000000ujuno --home "${HOME_DIR}"
  junod genesis add-genesis-account "${BENCH_ADDR}" 1000000000ujuno --home "${HOME_DIR}"
  junod genesis add-genesis-account "${VAL_ADDR}"   1000000000ujuno --home "${HOME_DIR}"

  # Create a self-delegated validator tx and finalize genesis.
  junod genesis gentx validator 100000000ujuno \
      --chain-id "${CHAIN_ID}" \
      --keyring-backend "${KEYRING}" \
      --home "${HOME_DIR}"
  junod genesis collect-gentxs --home "${HOME_DIR}"
  junod genesis validate-genesis --home "${HOME_DIR}"

  # Bump block gas limit so VerifyProof (pure-Wasm) fits comfortably.
  # sed is used instead of jq/python3 — neither is in debian:bookworm-slim.
  sed -i 's/"max_gas": "-1"/"max_gas": "80000000"/' "${HOME_DIR}/config/genesis.json"

  # Permissive CORS + broadcast.
  sed -i 's/cors_allowed_origins = \[\]/cors_allowed_origins = ["*"]/' "${HOME_DIR}/config/config.toml"
  sed -i 's/^laddr = "tcp:\/\/127.0.0.1:26657"/laddr = "tcp:\/\/0.0.0.0:26657"/' "${HOME_DIR}/config/config.toml"
  sed -i 's/^enable = false/enable = true/' "${HOME_DIR}/config/app.toml"
  sed -i 's/^address = "tcp:\/\/localhost:1317"/address = "tcp:\/\/0.0.0.0:1317"/' "${HOME_DIR}/config/app.toml"

  echo "[init-genesis] Done. admin=${ADMIN_ADDR} verifier=${BENCH_ADDR}"
else
  echo "[init-genesis] Existing node — skipping genesis creation."
fi

# --wasm.skip_wasmvm_version_check: wasmvm is linked via go mod replace so
# Go reports its version as "(devel)"; this bypasses the equality check.
exec junod "$@" --home "${HOME_DIR}" --wasm.skip_wasmvm_version_check
