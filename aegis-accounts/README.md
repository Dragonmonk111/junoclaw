# aegis-accounts

**Project Aegis Phase D / D2 harness** — a standalone, dependency-light Go
implementation of **hybrid post-quantum Cosmos accounts** per
[ADR-007](../docs/ADR-007-PQC-HYBRID-ACCOUNTS.md).

A hybrid account key is **secp256k1 AND ML-DSA-44 (FIPS 204) at once**. A
transaction is accepted only if **both** signatures verify, so an account is
forgeable only if *both* the classical and the post-quantum primitive are
broken. This is the account-layer analogue of the ADR-006 transport handshake.

This module validates the ADR-007 crypto end to end **without forking the Cosmos
SDK**. The SDK fork (D3) ports this logic 1:1 — the same relationship
`aegis-transport/secretconn/` + `PORTING.md` have to the CometBFT fork in
Phase C.

## What it implements

| Capability | File | ADR-007 |
|-----------|------|---------|
| Hybrid keygen (random + from-seeds) | `hybrid.go` | §D1 |
| Hybrid sign / verify (both halves over one message) | `hybrid.go` | §D2 |
| 20-byte domain-separated address + bech32 | `hybrid.go`, `bech32.go` | §D3 |
| HD-from-mnemonic (BIP-44 secp + HKDF ML-DSA) | `hd.go`, `bip32.go` | §D4 |

## Design (one mnemonic, both halves)

```
bip39_seed = BIP-39(mnemonic, passphrase)                       # 64 B

# classical half — standard BIP-44, Juno coin type 118
secp256k1_priv = BIP32-CKD(bip39_seed, m/44'/118'/0'/0/0)

# post-quantum half — ML-DSA has no BIP-32, derive via HKDF, path-bound
mldsa_seed   = HKDF-SHA256(bip39_seed, info="aegis/pqc/mldsa44/v1:"+path, 32)
mldsa44_priv = ML-DSA-44.KeyGen(mldsa_seed)

address = bech32("juno",
                 sha256( sha256("pqc/hybrid-secp256k1-mldsa44")
                         || secp_pub(33) || mldsa_pub(1312) )[:20])
```

The classical half is the *exact* key any standard Cosmos wallet derives from
the mnemonic, so it stays recoverable by tools that know nothing about ML-DSA.

## Sizes

| Item | Bytes |
|------|------:|
| secp256k1 pubkey / sig | 33 / 64 |
| ML-DSA-44 pubkey / sig | 1,312 / 2,420 |
| **Hybrid pubkey / sig** | **1,345 / 2,484** |

## Dependencies

- `github.com/cloudflare/circl` — ML-DSA-44 (FIPS 204)
- `github.com/decred/dcrd/dcrec/secp256k1/v4` — secp256k1 (the Cosmos curve lib)
- `github.com/cosmos/go-bip39` — BIP-39 mnemonics
- Go 1.24 stdlib `crypto/hkdf`; bech32 is implemented in-tree (no dep)

## Build & test

Requires **Go 1.24** (for `crypto/hkdf`).

```bash
cd aegis-accounts
go mod tidy        # fetches circl / decred / go-bip39 + writes go.sum
go test ./...
go vet ./...
```

## Test coverage

- **Reference vectors:** BIP-32 Test Vector 1 (classical HD matches the
  standard bit-for-bit), BIP-173 bech32 vector.
- **Security properties:** sign/verify round-trip, tampered-message rejection,
  **both-halves-required** (foreign classical OR foreign PQ half is rejected),
  wrong-pubkey rejection, malformed-signature rejection.
- **Determinism / HD:** same mnemonic ⇒ same account; passphrase changes the
  account; different HD index ⇒ independent secp **and** ML-DSA keys; address
  collision resistance.

## Status

D2 harness — the source of truth for the ADR-007 crypto. D3 (Cosmos SDK fork:
`cryptotypes.PubKey` impls, `SigVerificationDecorator`, gas, keyring/CLI) is the
downstream wiring exercise and is gated on a fork + a stable devnet. The
function-by-function map for that fork is in [`PORTING.md`](./PORTING.md) — note
that because Cosmos's AnteHandler authenticates via `PubKey.VerifySignature`, a
hybrid `VerifySignature` that requires both halves needs **no new decorator and
no new SignMode** (only a gas case).
