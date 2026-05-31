# JunoClaw — Testnet Safe Reference

*Safe to copy physically, encrypt locally, or print. Contains ONLY public addresses, code IDs,
and transaction hashes — no mnemonics, no private keys, no secrets.*

*Mnemonics for these wallets are stored separately: encrypted in wallet registry + physically
written down. This file is the address book, not the key ring.*

*Network: uni-7 (Juno testnet). All tokens are `ujunox` — zero monetary value.*

---

## Deployed Contracts (uni-7)

| Contract | Code ID | Address |
|---|---|---|
| agent-company v7 | 75 | (query: `junod q wasm list-contract-by-code 75 --node https://juno.rpc.t.stavr.tech`) |
| zk-verifier | 64 | `juno1ydxksv...lse7ekem` |
| junoswap-pair | 61 | live on uni-7 |
| moultbook-v0 | 76 | `juno1lahsc7ef0manp3czjx806l8v2erqzzlxhr7z9z7090h5k99vdd2qjhdh53` |
| ibc-task-host | 77 | `juno1hskkxy5wlfdgc0ht595plwrhc2zqmrkcer2v9sehxf44nv3upa4sgu9cag` |
| task-ledger | TBD (post v30) | — |
| escrow | TBD (post v30) | — |
| agent-registry | TBD (post v30) | — |
| builder-grant | TBD (post v30) | — |
| junoswap-factory | TBD (post v30) | — |
| faucet | TBD (post v30) | — |

---

## Governance Proposals (Juno mainnet)

| Proposal | Description | Result |
|---|---|---|
| #373 | JunoClaw recognition, WAVS-Junoswap, Akash sidecar | **Passed** — March 8, 2025 |
| #374 | BN254 precompile endorsement | **Passed** — 80% yes |

Links:
- `https://ping.pub/juno/gov/373`
- `https://ping.pub/juno/gov/374`

---

## Key Transaction Hashes (uni-7)

| Event | TX Hash |
|---|---|
| First TEE attestation (SGX, block 11,735,127) | `6EA1AE79D373BE7E57A8492A089E543ADA40B30CB5F7E69B177E607879D26B22` |
| First ZK verify (Groth16, 371,486 gas) | `F6D5774EE2073E2DD011399A7E96889BA026ED67C6A510D208FD5C575080F4DA` |
| Wallet C-3 fix proof — passphrase backend | `E2FB05A213D0C65C02EF0D5FAB1C7F8D4AF34BF275B9F0005F0B5A86FF9AED10` |
| Wallet C-3 fix proof — DPAPI keychain backend | `00D9AC4706A5AB923B7D45E0E97CE166F2EBD34402ECEB113DC7DC9A4702AF18` |
| signing_paused gate proof — disarmed TX | `346CC7FF418019A4FBA68D7847112954E2D8D9ECE3E27B314357408E8AE42B6A` |

Explorer: `https://testnet.mintscan.io/juno-testnet/tx/<HASH>`

---

## OCI Artifacts

| Artifact | Digest | Size |
|---|---|---|
| `ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0` | `sha256:90daab7a...05618` | 494 KB |
| Wasm hash | `sha256:2ab42cc3...3612c` | — |

Cosign signing: pending (browser OIDC flow — `cosign sign ghcr.io/dragonmonk111/junoclaw/verifier:0.1.0`)

---

## Parliament MP Wallet Addresses (uni-7 testnet, no mainnet value)

| # | Address | Role |
|---|---|---|
| 1 | `juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z` | MP |
| 2 | `juno15jcxytf0sya6l3f6c22zuhp8rwqxwtl5ppq66z` | MP |
| 3 | `juno1d4533s7a7srke0n7jlz6zrwzdaxeypu3gwje23` | MP |
| 4 | `juno1pwuttlm2fd3gg9f28lnehfwqyy6yttv7lty49m` | MP |
| 5 | `juno1eu4kxk0t35d67mn9ta3hpvu9pn7nfhzs65ujep` | MP |
| 6 | `juno1k0xezwajmfl5lmk62ppph904g3sa4zjz4s0pxe` | MP |
| 7 | `juno1n6h88sehc8c5ugvu3crhxlqach2smur9cmaw8n` | MP |

*Mnemonics: stored in `wavs/bridge/parliament-state.json` (gitignored). Before mainnet: migrate to encrypted wallet registry.*

---

## WAVS Operator

| Key | Value |
|---|---|
| Mnemonic location | `wavs/.env` → `WAVS_OPERATOR_MNEMONIC` (gitignored, testnet only) |
| Privkey location | `wavs/bridge/.env` → `WAVS_OPERATOR_PRIVKEY` (gitignored, testnet only) |
| Mainnet action | Migrate both to wallet registry; retire testnet key; never reuse on mainnet |

---

## GitHub + Repository

| Resource | Value |
|---|---|
| Repo | `https://github.com/Dragonmonk111/junoclaw` |
| Stars | 3 (as of 2026-05-20) |
| juno-network-skill PR #1 | `https://github.com/CosmosContracts/juno-network-skill/pull/1` |

---

## Security GHSAs (published on repo)

| ID | Severity | Finding |
|---|---|---|
| `GHSA-fvq5-79h6-952c` | HIGH (CVSS 8.4) | plugin-shell shell-injection bypass (C-1) |
| `GHSA-gpvm-3chf-2649` | HIGH (CVSS 8.4) | plugin-shell shell-metacharacter injection (C-2) |
| `GHSA-j75q-8xvm-6c48` | CRITICAL (CVSS 9.8) | MCP mnemonic exposure (C-3) |
| `GHSA-rw59-34hw-pmwp` | HIGH (CVSS 8.5) | upload_wasm path traversal (C-4) |
| `GHSA-q545-mvjf-q9pg` | HIGH (CVSS 8.2) | SSRF in WAVS computeDataVerify (H-3) |

All five closed in `v0.x.y-security-1`. CVEs assigned by GitHub CNA.

---

*Apache-2.0. Created 2026-05-20. Safe to copy physically — no secrets in this file.*
