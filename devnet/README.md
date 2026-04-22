# BN254 precompile devnet

Single-validator, ephemeral Juno devnet that runs `junod` linked against a
`wasmvm` fork carrying the BN254 host functions from
`../wasmvm-fork/`. Used to produce the before/after gas numbers that the
Juno governance proposal points at.

## What this devnet is for

Exactly one thing: **get believable gas numbers for a Groth16 verification
that actually uses the BN254 host functions.** Once the benchmark runs
cleanly, the resulting table goes into `../docs/BN254_BENCHMARK_RESULTS.md`
and the devnet can be torn down.

The devnet is **not** a shared testnet, is **not** persistent, and is **not**
supposed to be used by anyone other than the benchmark harness.

## Prerequisites

- Docker 24+ with ≥ 6 GB memory allocated
- 15 GB free disk (the multi-stage build caches a Rust + Go toolchain)
- `junocli` (optional — the scripts also work with `docker exec junod ...`)

## Usage

```bash
# 1. Spin up.
./scripts/run-devnet.sh

# 2. Upload + instantiate the two zk-verifier flavours.
./scripts/deploy-zk-verifier.sh

# 3. Run the benchmark (writes ../docs/BN254_BENCHMARK_RESULTS.md).
./scripts/benchmark.sh

# 4. Tear down.
./scripts/stop-devnet.sh
```

The whole loop takes ~12 minutes cold (first `docker build`) and ~90
seconds warm.

## Layout

```
devnet/
├── README.md                       ← this file
├── Dockerfile                      ← multi-stage build: libwasmvm-bn254 + junod
├── docker-compose.yml              ← one-shot single-validator
├── genesis-template.json           ← single-validator genesis (account keys in scripts)
└── scripts/
    ├── run-devnet.sh               ← `docker compose up -d` + block-wait
    ├── stop-devnet.sh              ← `docker compose down -v`
    ├── init-genesis.sh             ← used inside the container on first boot
    ├── deploy-zk-verifier.sh       ← uploads both .wasm flavours
    └── benchmark.sh                ← 50 VerifyProof txs, records gas
```

## What happens inside the container

1. The Dockerfile clones `CosmWasm/cosmwasm` @ v2.2.0 and applies the
   three `../wasmvm-fork/patches/cosmwasm-*.patch` files, then builds
   the Rust side.
2. It clones `CosmWasm/wasmvm` @ v2.2.0 and applies the two
   `../wasmvm-fork/patches/wasmvm-*.patch` files, then builds
   `libwasmvm.a`.
3. It clones `CosmosContracts/juno` at the latest v29.x tag and builds
   `junod` linking against the patched `libwasmvm.a`.
4. On container start, `init-genesis.sh` creates a single validator
   (deterministic mnemonic stored in-image — safe because the chain is
   ephemeral) and launches `junod start` in the foreground.

## Accounts

Two pre-funded accounts are created on boot:

| Name       | Address                                    | Balance     |
|------------|--------------------------------------------|-------------|
| `admin`    | `juno1dev…` (derived from fixed mnemonic)  | 1 000 JUNO  |
| `verifier` | `juno1bench…`                              | 1 000 JUNO  |

Both mnemonics are in `scripts/init-genesis.sh` — they are **burned**
on every `stop-devnet.sh`; no funds flow onto a real network.

## Links

- Upstream CosmWasm: <https://github.com/CosmWasm/cosmwasm>
- Upstream wasmvm: <https://github.com/CosmWasm/wasmvm>
- Upstream juno: <https://github.com/CosmosContracts/juno>
- Gas analysis: `../docs/BN254_PRECOMPILE_CASE.md`
- Patches: `../wasmvm-fork/patches/`
