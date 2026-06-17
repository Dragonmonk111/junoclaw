# Porting `aegis-accounts` into a Cosmos SDK fork (Project Aegis Phase D / D3)

This module is the **verified, fork-ready** form of the ADR-007 hybrid account.
It implements the crypto end to end (keygen, sign, verify, address, HD) so the
SDK change lands as a small, reviewable patch series rather than a rewrite — the
same relationship `aegis-transport/secretconn/` + its `PORTING.md` have to the
CometBFT fork in Phase C.

- **Spec:** `docs/ADR-007-PQC-HYBRID-ACCOUNTS.md`
- **This module:** `aegis-accounts/` (20 tests pass, `go vet` clean, Go 1.24)
- **Crypto:** `cloudflare/circl` ML-DSA-44 + decred `secp256k1/v4` + `cosmos/go-bip39`

---

## 0. The one fact that makes this small

Cosmos's `x/auth/ante.SigVerificationDecorator` authenticates a tx by calling

```go
pubKey.VerifySignature(signBytes, sig)
```

on whatever `cryptotypes.PubKey` the account carries. If our **hybrid
`PubKey.VerifySignature` requires both halves**, the *existing* AnteHandler
authenticates hybrid txs **with no new decorator and no new SignMode**. The only
ante-side change is a **gas** case. Everything else is key-type plumbing.

---

## 1. Function-by-function map

| `aegis-accounts` (harness) | Cosmos SDK target | What changes |
|----------------------------|-------------------|--------------|
| `HybridPubKey` | new proto `aegis.crypto.hybrid.PubKey` implementing `cryptotypes.PubKey` | Wrap `secp(33) \|\| mldsa(1312)`; implement `Address/Bytes/VerifySignature/Type/Equals`. |
| `HybridPrivKey` | `hybrid.PrivKey` implementing `cryptotypes.PrivKey` | Hold both halves; `Sign` returns the `64 \|\| 2420` concat. |
| `HybridPubKey.Verify(msg, sig)` | `hybrid.PubKey.VerifySignature(msg, sig)` | Split sig, verify **both** halves — this is what makes the ante path work unchanged (§0). |
| `HybridPrivKey.Sign(msg)` | `hybrid.PrivKey.Sign(msg)` | secp over `sha256(msg)` + ML-DSA over `msg`, concatenated. |
| `(*HybridPubKey).AddressBytes()` | `hybrid.PubKey.Address()` | Same domain-tagged `address.Hash("pqc/hybrid-secp256k1-mldsa44", …)[:20]`. |
| `Bech32Address()` | `sdk.AccAddress(pk.Address())` | Drop in-tree bech32; the SDK bech32-encodes the 20-byte `Address()`. |
| `NewHybridFromMnemonic` / `DeriveSecp256k1` + HKDF | a `keyring.SignatureAlgo` `hd.HybridSecp256k1MlDsa44` (`Derive()` + `Generate()`) | Port BIP-44 (SDK `crypto/hd.Secp256k1.Derive`) + the HKDF ML-DSA seed step. |
| `bech32.go` | — (delete) | SDK `types` already provides bech32. |
| `bip32.go` | — (delete) | SDK `crypto/hd` already provides BIP-32/44. |
| `mldsa44.NewKeyFromSeed` (circl) | the **vendored Aegis ML-DSA-44** | Use the *same* impl as consensus/transport/CosmWasm — not a second one. |

---

## 2. Intentional divergences (and what the fork must restore)

The harness is deliberately **dependency-light and self-contained** so it could
be tested in isolation. The SDK fork should drop the standalone scaffolding in
favour of the SDK's own, battle-tested equivalents:

1. **Bech32 + BIP-32/44 are in-tree here only** to avoid pulling the whole SDK
   into a tiny harness. The fork **deletes `bech32.go` and `bip32.go`** and uses
   `github.com/cosmos/cosmos-sdk/types` (bech32) and
   `github.com/cosmos/cosmos-sdk/crypto/hd` (BIP-44). The harness's
   `TestBIP32Vector1` (canonical xprv) is what proves the two derivations agree.
2. **ML-DSA-44 implementation.** Harness uses `cloudflare/circl`. The fork must
   use the **same vendored ML-DSA-44** as the rest of Aegis (transport node key,
   consensus, CosmWasm `ml_dsa_verify`) so there is exactly one ML-DSA in the
   trust base. The byte formats are FIPS 204 standard, so this is a swap of the
   `Sign`/`Verify`/`NewKeyFromSeed` calls, not a redesign.
3. **secp256k1 signing.** Harness strips the recovery byte off `SignCompact` to
   get 64-byte `R\|\|S`. The fork should reuse the SDK's own
   `crypto/keys/secp256k1.PrivKey.Sign` (already 64-byte, low-S) for the
   classical half, for exact wire-compatibility with classical accounts.

None of these touch the ADR-007 security design — they align the harness to the
SDK's existing libraries.

---

## 3. Determinism rule (consensus-safety in the AnteHandler)

`PubKey.VerifySignature` runs inside the AnteHandler on **every validator**, so
it must be deterministic — every node must reach the same accept/reject.

- **ML-DSA-44 verify is integer-only with no verify-time RNG** (the same property
  relied on by consensus in Phase F and by the MAYO verifier). Safe.
- **Signing may be hedged** (client-side RNG) — irrelevant to consensus, it is
  off-chain. The harness does not depend on signature-byte determinism (only on
  keygen determinism for HD), and neither should the fork.
- Verification must **fail closed** on malformed length / wrong tag (the harness
  already rejects short halves and nil sigs).

---

## 4. Gas (the only ante-side change)

Extend `x/auth/ante.DefaultSigVerificationGasConsumer` with a case for the
hybrid pubkey type:

```go
case *hybrid.PubKey:
    // classical half
    meter.ConsumeGas(params.SigVerifyCostSecp256k1, "ante verify: secp256k1")
    // pq half
    meter.ConsumeGas(params.SigVerifyCostMlDsa44, "ante verify: ml-dsa-44")
    return nil
```

Add `SigVerifyCostMlDsa44` to `x/auth` params. Seed it **conservatively** from
the Phase B measurement (ML-DSA-44 verify ≈ 101 µs; ~270k Wasm-equivalent — the
native ante verify is far cheaper in wall-clock, so a conservative constant first,
tuned with a benchmark, is the right order). The classical half keeps its cost.

---

## 5. Concrete patch plan against the fork

1. **Pin the fork point.** Clone the Cosmos SDK tag Juno's target version uses;
   vendor the Aegis ML-DSA-44 impl.
2. **Proto.** Define `aegis.crypto.hybrid.{PubKey,PrivKey}` and
   `aegis.crypto.mldsa44.{PubKey,PrivKey}` (the hybrid is built from the latter).
   Register in the app `InterfaceRegistry`
   (`RegisterImplementations((*cryptotypes.PubKey)(nil), &hybrid.PubKey{})`) and
   in the legacy amino codec.
3. **Key types.** Implement `cryptotypes.PubKey` (Address via the §1 tag,
   `VerifySignature` = both halves per §0, `Bytes`, `Type`, `Equals`) and
   `cryptotypes.PrivKey` (`Sign`, `PubKey`). Port the harness logic 1:1.
4. **Keyring / HD.** Add a `keyring.SignatureAlgo` `hd.HybridSecp256k1MlDsa44`:
   `Derive()` = SDK BIP-44 secp + HKDF ML-DSA seed (port `hd.go`); `Generate()` =
   build a `hybrid.PrivKey` from the derived material. Register it in the
   keyring's supported-algos list.
5. **Gas.** Apply §4 (new param + consumer case). No new decorator, no new
   `SignMode` (reuse `SIGN_MODE_DIRECT`).
6. **CLI.** Add `--algo hybrid-secp256k1-mldsa44` to `keys add`; make `keys show`
   print the hybrid address + a hash of the ML-DSA pubkey (full bytes under
   `--output json`).
7. **Tests.** Port `hybrid_test.go` properties (round-trip, both-halves-required,
   tamper, wrong-key, HD determinism/independence, address collision) into the
   fork; add an **integration test**: create a hybrid account, send a `bank` tx,
   assert the AnteHandler accepts it and that a tampered signature is rejected.
8. **Devnet (the D-phase close-out, gated on a stable devnet).** Deploy the
   patched node, create a hybrid account from a mnemonic, send a real tx, confirm
   it lands in a block, and confirm a classical account is unaffected
   (opt-in / no flag-day).

---

## 6. Status

- [x] ADR-007 spec
- [x] **`aegis-accounts/` verified harness (this module) — 20 tests, vet clean**
- [x] Hybrid keygen / sign / verify (both-halves-required)
- [x] 20-byte domain-separated address + bech32
- [x] HD-from-mnemonic (BIP-44 secp + HKDF ML-DSA), canonical BIP-32 vector check
- [ ] Proto key types registered in the SDK fork (§5.2–5.3)
- [ ] `keyring.SignatureAlgo` + CLI (§5.4, §5.6)
- [ ] Gas param + consumer case (§4)
- [ ] Integration test + devnet hybrid-account tx (§5.7–5.8)
