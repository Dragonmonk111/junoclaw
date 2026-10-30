# Verify before teaching

Run this pre-flight the morning of each session. Everything the course
claims is checkable — checking it yourself first is the lesson's spirit.

## 1. Chain is live

Ask the MCP (or any gRPC client): `query_block_height` on `junoclaw-g1`.
Expect a height ~7 figures, increasing between two calls a minute apart.

## 2. Endpoint reachable

`list_chains` must show `junoclaw-g1` with transport `junoclaw-grpc`
and your `JUNOCLAW_GRPC_ENDPOINT`. If the endpoint is filtered, a
query hangs ~15s then fails — ask the operator team to allow your IP.

## 3. Economy contracts alive

`query_contract` on the agent-registry with `{ "get_config": {} }` —
expect `registration_fee_ujuno`, `denom: "ujclaw"`, `max_agents`, and
the `registry` map (task_ledger + escrow addresses). Contract list and
verified addresses: `mcp/SKILL.md` §"Sovereign chain".

## 4. Wallet can sign (one test tx)

Pick one enrolled `wallet_id`, send 1 ujclaw to itself via
`send_tokens`, then `query_tx` on the returned hash: `code` must be 0.
This exercises the whole write path — decrypt → sign → broadcast →
commit — before students depend on it.

## 5. Funding state

`query_balance` each student wallet: ≥ ~1,100,000 ujclaw needed for
the registration lesson (fee + gas).

## Known limitations to warn about

- `query_tx` needs a hash — teach students to save them.
- Contract state enumeration (`AllContractState`) is not served.
- Block/node-info endpoints are not implemented yet; use
  `query_block_height` for liveness.
- The gRPC endpoint is plaintext — classroom traffic is not secret,
  but never send keys over it. Keys never leave the wallet store anyway.
