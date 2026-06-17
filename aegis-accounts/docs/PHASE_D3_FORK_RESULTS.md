# Phase D3 — Cosmos SDK fork: hybrid account key (results)

**Status:** core DONE (build + tests green). **Date:** 2026-06-17.

The ADR-007 hybrid (secp256k1 + ML-DSA-44) account key is folded into a real
Cosmos SDK fork as a `cryptotypes.PubKey`/`PrivKey`, proven to build and pass
tests inside the SDK module. This is the unit-testable D3 core; the codec/
keyring/CLI/gas wiring is the remaining, protoc-gated step.

## What was done

- **Fork:** `cosmos/cosmos-sdk` branch `release/v0.50.x`, shallow-cloned to
  `aegis-forks/cosmos-sdk` (gitignored; keeps its own git history).
- **New package `crypto/keys/hybrid`** (1:1 port of the verified `aegis-accounts`
  harness, re-expressed against the SDK interfaces):
  - `hybrid.go` — `PrivKey`/`PubKey` implementing `cryptotypes.PrivKey`/`PubKey`.
    The classical half **composes the SDK's own `crypto/keys/secp256k1`** (so it
    is byte-identical to classical accounts on the wire); the PQ half uses
    `cloudflare/circl` ML-DSA-44. `Address()` uses the SDK's `types/address.Hash`
    with a domain-separated tag.
  - `hybrid_test.go` — 9 tests (see below).
- **Dependency:** `go get github.com/cloudflare/circl@v1.6.1` (compiles cleanly
  under the SDK's `go 1.22.11` toolchain; no go-directive bump needed).

## The key design point (why no new AnteHandler)

`PubKey.VerifySignature(msg, sig)` returns true **iff BOTH** halves verify:
secp256k1 over `sig[:64]` AND ML-DSA-44 over `sig[64:]`. The SDK x/auth
AnteHandler authenticates a tx by calling `PubKey.VerifySignature`, so a hybrid
account plugs into the **existing** ante path with **no new decorator and no new
SignMode** — only a gas case is added. Sizes: PubKey 1345 B (33+1312), Sig
2484 B (64+2420).

## Verification (WSL, SDK toolchain go1.22.11)

```
cd aegis-forks/cosmos-sdk
go test ./crypto/keys/hybrid/ -v    # 9/9 PASS:
  TestInterfaces                    (compile-time cryptotypes impl)
  TestSizes                         (1345 / 2484 / 1312 / 2420)
  TestSignVerifyRoundTrip
  TestWrongKeyRejected
  TestBothHalvesRequired            (tamper/foreign either half -> reject)
  TestMalformedSignatureRejected
  TestAddressDeterministicAndLength (20-byte, deterministic, domain-separated)
  TestEqualsAndType
  TestDeterministicFromSeeds        (HD-from-mnemonic reproducibility)
go vet ./crypto/keys/hybrid/        # clean
```

## Remaining (gated on protoc/buf + devnet)

1. **Codec/proto:** define `hybrid.proto` (`PubKey{ bytes key }`, `PrivKey`),
   regenerate, register the `Any` interface impls so hybrid keys (de)serialize in
   txs and `AccountI`.
2. **Keyring/CLI:** add a `hybrid` `SignatureAlgo` + HD path so
   `keys add --algo hybrid` works end-to-end.
3. **Gas:** add a `hybrid` case to the sig-verify gas table (~ML-DSA-44 verify +
   secp256k1 verify), per ADR-007 §D4 / the measured Phase D2 numbers.
4. **Integration test** on the devnet: send a hybrid-signed tx, assert it
   authenticates and is charged the expected gas (overlaps Fable P4 tooling).
