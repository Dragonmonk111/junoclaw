#!/usr/bin/env bash
# Runs once on container start. Creates a single-validator genesis with
# two pre-funded accounts, then execs `junod start` so tx signals reach
# the process.

set -euo pipefail

CHAIN_ID=${CHAIN_ID:-junoclaw-bn254-1}
MONIKER=${MONIKER:-bn254-validator}
KEYRING=${KEYRING_BACKEND:-test}
HOME_DIR=${HOME_DIR:-/root/.juno}

# Deterministic mnemonics — safe because the chain is ephemeral and the
# keyring is the `test` backend (plaintext, in-container only).
ADMIN_MNEMONIC="afford uphold crystal depart pluck myth fancy demand vague legend swamp decline couple pond motion speak bless swallow warrior above grid emerge spider donkey"
BENCH_MNEMONIC="brief clog liberty decline camp rain unlock jaguar narrow hawk trend inner fossil reform cinnamon minute frozen stomach tornado glory afraid toward hotel angry"
VALIDATOR_MNEMONIC="cement robust hollow hammer gossip heart clown fly kiwi absent nerve cash equal voyage ill rare tank cabbage bulb arctic squirrel banana empty quote"

if [ ! -f "${HOME_DIR}/config/genesis.json" ]; then
  echo "[init-genesis] Fresh node — generating genesis…"

  junod init "${MONIKER}" --chain-id "${CHAIN_ID}" --home "${HOME_DIR}" --overwrite

  # Keys.
  echo "${ADMIN_MNEMONIC}"     | junod keys add admin     --keyring-backend "${KEYRING}" --home "${HOME_DIR}" --recover
  echo "${BENCH_MNEMONIC}"     | junod keys add verifier  --keyring-backend "${KEYRING}" --home "${HOME_DIR}" --recover
  echo "${VALIDATOR_MNEMONIC}" | junod keys add validator --keyring-backend "${KEYRING}" --home "${HOME_DIR}" --recover

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
  python3 - <<PY
import json, pathlib
p = pathlib.Path("${HOME_DIR}/config/genesis.json")
g = json.loads(p.read_text())
g["consensus_params"]["block"]["max_gas"] = "80000000"
p.write_text(json.dumps(g, indent=2))
PY

  # Permissive CORS + broadcast.
  sed -i 's/cors_allowed_origins = \[\]/cors_allowed_origins = ["*"]/' "${HOME_DIR}/config/config.toml"
  sed -i 's/^laddr = "tcp:\/\/127.0.0.1:26657"/laddr = "tcp:\/\/0.0.0.0:26657"/' "${HOME_DIR}/config/config.toml"
  sed -i 's/^enable = false/enable = true/' "${HOME_DIR}/config/app.toml"
  sed -i 's/^address = "tcp:\/\/localhost:1317"/address = "tcp:\/\/0.0.0.0:1317"/' "${HOME_DIR}/config/app.toml"

  echo "[init-genesis] Done. admin=${ADMIN_ADDR} verifier=${BENCH_ADDR}"
else
  echo "[init-genesis] Existing node — skipping genesis creation."
fi

exec junod "$@" --home "${HOME_DIR}"
