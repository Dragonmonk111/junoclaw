# junoclaw-rest-gateway — Cosmos LCD compatibility shim

Read-only REST surface over JunoClaw g1's SDK gRPC (`:9090`). Exists so
standard tooling (explorers, cosmjs REST clients, curl) can query the
chain without speaking tonic/protobuf. **This is a bounded subset, not
full Cosmos REST compatibility.**

## Run

```
junoclaw-rest-gateway --bind 127.0.0.1:1317 \
    --grpc-endpoint 51.195.201.74:9090 --chain-id junoclaw-g1
```

Deploy behind a TLS reverse proxy for public use; keep `:9090`
firewalled — the gateway is the only intended public consumer.

## Implemented routes

| Route | Backs onto |
|---|---|
| `GET /cosmos/base/node/v1beta1/status` | `layer.lightclient LatestHeight` |
| `GET /cosmos/base/tendermint/v1beta1/blocks/latest` | LatestHeight + Block |
| `GET /cosmos/base/tendermint/v1beta1/blocks/{h}` | `layer.lightclient Block` |
| `GET /cosmos/auth/v1beta1/accounts/{addr}` | `auth.Query/Account` |
| `GET /cosmos/bank/v1beta1/balances/{addr}` | `bank.Query/Balance` (all denoms) |
| `GET /cosmos/bank/v1beta1/all_balances/{addr}` | `bank.Query/AllBalances` |
| `GET /cosmos/tx/v1beta1/txs/{hash}` | `tx.Service/GetTx` |
| `GET /cosmwasm/wasm/v1/contract/{addr}/smart/{b64}` | `wasm.Query/SmartContractState` |
| `GET /healthz` | local |

Response bodies follow the protobuf JSON mapping: `uint64` fields are
strings, bytes are base64, `Any` values carry `@type`.

## Known divergences from stock LCD

- **No Tendermint.** `/blocks/{h}` is translated from
  `layer.lightclient.Block` — it exposes height, timestamp, chain id,
  and the block/certificate bytes, not the full Tendermint block shape.
  The certificate is the JunoClaw hybrid BLS+MAYO2 cert, not a commit.
- **Not implemented:** governance, staking, distribution, slashing,
  mint, evidence, `txs?events=` search, `POST /txs` (broadcast),
  `POST /txs/simulate`, node config. Unmatched routes return 404, not
  a compat stub.
- **`GetTx` lags indexing.** A just-broadcast tx can 404 for a few
  seconds — poll, don't treat first miss as absence.
- **Pagination shape** matches (`next_key`, `total`) but only the
  routes above exist; there is nothing paginated beyond balances.

## Why this exists (and what it isn't)

Anode.team and UTSA already run explorers — the human-facing read path
is solved and can point at either DragonMonk's gRPC donor or a local
seat's `:9090`. This shim is for the *tooling* path: anything that
expects a `rest_endpoint` URL. It is deliberately a separate binary
from `junoclaw-attest-gateway` — chain-read compat and the customer
write surface have different blast radii.
