# Aegis Implementation Plan — F6 Consensus Rotation + D3 Account Wiring + IBC Phase G

*2026-06-25 · Project Aegis mainnet-gating work*

This plan assumes the **forks are already pushed** and the `aegis-forks/`
local clones are the working source of truth (CometBFT: `aegis-phase-cf-hybrid`,
Cosmos SDK: `aegis-phase-d3-hybrid`). The remaining work is **protoc-gated
message plumbing + ibc-go fork integration** — no new crypto, just wiring and
tests.

---

## Part 1 — F6: `MsgRotateConsensusKey` (Cosmos SDK + CometBFT)

**Goal:** allow a validator to rotate their **consensus** public key from
classical Ed25519 to hybrid Ed25519+ML-DSA-44 (or between any two
consensus-key types) without changing their 20-byte address or halting the chain.

**Why this is mainnet-gating:** without it, validators must re-create their
operator identity to adopt hybrid keys, which breaks delegations and is a
non-starter for a 300+ validator set.

### 1.1 Proto / state changes (Cosmos SDK fork)

**Step 1.1 — install the toolchain.**

```bash
sudo apt update && sudo apt install -y protobuf-compiler
# buf is the preferred generator for both forks
GOFLAGS=-mod=mod go install github.com/bufbuild/buf/cmd/buf@latest
```

**Step 1.2 — add `MsgRotateConsensusKey` to the staking proto.**

File: `proto/cosmos/staking/v1beta1/tx.proto` (in the SDK fork)

```protobuf
message MsgRotateConsensusKey {
  string           validator_address = 1;  // valoper1...
  google.protobuf.Any new_consensus_pubkey = 2;  // aegis-hybrid-ed25519-mldsa44
}

message MsgRotateConsensusKeyResponse {}
```

**Step 1.3 — add rate-limit state to the validator record.**

Extend `proto/cosmos/staking/v1beta1/staking.proto` `Validator`:

```protobuf
message Validator {
  // ... existing fields ...
  uint64 consensus_rotation_height = 42; // last rotate height; 0 = never
}
```

(Use the next available field number, not literal 42.)

**Step 1.4 — regenerate protos.**

```bash
cd aegis-forks/cosmos-sdk
make proto-gen   # or `buf generate` depending on SDK's Makefile
```

Verify `x/staking/types/tx.pb.go` contains the new message types and
`x/staking/types/validator.pb.go` contains the new field.

### 1.2 Message server implementation (SDK fork)

**Step 1.5 — register the handler.**

File: `x/staking/keeper/msg_server.go` (or wherever the other `Msg` handlers live)

Add:

```go
func (k msgServer) RotateConsensusKey(
    goCtx context.Context,
    msg *types.MsgRotateConsensusKey,
) (*types.MsgRotateConsensusKeyResponse, error) {
    ctx := sdk.UnwrapSDKContext(goCtx)

    valAddr, err := sdk.ValAddressFromBech32(msg.ValidatorAddress)
    if err != nil {
        return nil, err
    }

    val, found := k.GetValidator(ctx, valAddr)
    if !found {
        return nil, errorsmod.Wrap(types.ErrNoValidatorFound, msg.ValidatorAddress)
    }

    // Rate limit: 24h cooldown (approx 14,400 blocks at 6s, or use a time-based
    // module param). This prevents DoS via repeated validator-set hash changes.
    const minBlocksBetweenRotations = 14400
    if val.ConsensusRotationHeight != 0 &&
        ctx.BlockHeight()-int64(val.ConsensusRotationHeight) < minBlocksBetweenRotations {
        return nil, errorsmod.Wrap(types.ErrRotationTooFrequent, msg.ValidatorAddress)
    }

    // Decode the new consensus pubkey from Any.
    newPK, err := k.cdc.PubKeyFromProto(msg.NewConsensusPubkey)
    if err != nil {
        return nil, err
    }

    // Validate the key type is allowed by CometBFT ValidatorParams.
    if err := k.IsAllowedPubKeyType(newPK); err != nil {
        return nil, err
    }
    // ^ need to check against the current CometBFT consensus params; for v0.50
    // this is stored in x/consensus/params.

    // Address MUST stay the same (ADR-008 F1-b invariant). Reject any rotation
    // that would change the validator's 20-byte address.
    if !newPK.Address().Equals(val.GetConsAddr()) {
        return nil, errorsmod.Wrap(types.ErrRotationAddressMismatch, "address changed")
    }

    // Update the validator's consensus pubkey in state.
    val.ConsensusPubkey = msg.NewConsensusPubkey
    val.ConsensusRotationHeight = uint64(ctx.BlockHeight())
    k.SetValidator(ctx, val)

    // Emit an event so light clients / IBC relayers can react.
    ctx.EventManager().EmitEvent(
        sdk.NewEvent(
            "rotate_consensus_key",
            sdk.NewAttribute("validator", msg.ValidatorAddress),
            sdk.NewAttribute("height", strconv.FormatInt(ctx.BlockHeight(), 10)),
        ),
    )

    return &types.MsgRotateConsensusKeyResponse{}, nil
}
```

**Step 1.6 — sign the message with the validator's operator key.**

The message must be signed by the validator's **operator** address (valoper1...)
not the consensus key. This is standard SDK staking authority: the operator key
owns the validator metadata. The signature is verified by the SDK's normal
AnteHandler before the message reaches `msgServer`.

### 1.3 CometBFT ordering constraint

**Step 1.7 — confirm CometBFT handles the rotation safely.**

CometBFT `VoteSignBytes` uses the validator's **address** (20 B), not the pubkey.
The pubkey is looked up from the `ValidatorSet` at the **vote's height**. So a
rotation at height `H` affects voting from `H+1` onward; votes at `H` still use
the old pubkey. No special CometBFT change is required — the existing
validator-set update mechanism handles it, provided the address is unchanged
(ADR-008 F1-b invariant).

**Step 1.8 — add a test simulating a rotation mid-consensus.**

In `x/staking/keeper/msg_server_test.go` or a new
`x/staking/keeper/rotation_test.go`:

1. Create a validator with a classical Ed25519 consensus key.
2. Have it sign a few votes with the old key (unit-level, not full consensus).
3. Submit `MsgRotateConsensusKey` to a new hybrid key.
4. Assert address unchanged, new pubkey in state, 24h rate limit enforced.
5. Submit a second rotation within 24h → assert rejected.

### 1.4 CLI

**Step 1.9 — add `junod tx staking rotate-cons-key`.**

File: `x/staking/client/cli/tx.go`

```go
cmd := &cobra.Command{
    Use:   "rotate-cons-key [validator-address] [path-to-hybrid-pubkey-json]",
    Short: "Rotate a validator's consensus key to a new hybrid pubkey",
    ...
}
```

The JSON file contains the SDK `Any` representation of the new hybrid pubkey, as
produced by the existing `aegis-bench/cmd/hybrid-consensus-keygen` tool (or a
new keygen subcommand).

---

## Part 2 — F7: Hybrid remote signer (`privval`)

**Goal:** the `PrivValidator` interface (used by `tmkms`, Horcrux, remote KMS)
carries the hybrid signature over the existing `privval` protobuf.

### 2.1 Proto changes (CometBFT fork)

**Step 2.1 — extend `proto/tendermint/privval/types.proto`.**

The `PubKeyResponse` and `SignVoteResponse` / `SignProposalResponse` already
carry `bytes signature` and `crypto.PublicKey`. The only proto addition is to
allow the `PublicKey` oneof to carry the hybrid variant — already done in
ADR-008 F4 (`aegis_hybrid_ed25519_mldsa44 = 3`). Regenerate with `make proto-gen`.

### 2.2 Signer server / SignerClient

**Step 2.2 — make `privval.SignerServer` return the hybrid signature.**

File: `privval/signer_server.go`

If the underlying `PrivValidator` is a `FilePV` that now holds a hybrid key (via
the sidecar `priv_validator_key_mldsa44.json` from ADR-008 F3), `SignVote` and
`SignProposal` already return the 2,491 B hybrid signature. The SignerServer
just forwards it. No logic change is needed if the FilePV change is complete.

**Step 2.3 — make `privval.SignerClient` accept the hybrid signature.**

File: `privval/signer_client.go`

Verify that `SignVote` / `SignProposal` do not truncate the signature (e.g., no
hard-coded 64 B length checks). Add a test with a mock hybrid signer that
returns a 2,491 B signature and assert it passes through unchanged.

**Step 2.4 — tmkms/Horcrux note.**

Out of scope for the first mainnet cut unless those tools upgrade. The
short-term path is: remote signer holds the **whole** hybrid key and returns the
combined signature. A future split-key HSM design (classical in one HSM, PQC in
another) is a Phase-H enhancement.

---

## Part 3 — D3: Account keyring / CLI / gas wiring (Cosmos SDK fork)

**Goal:** end users can create and use a hybrid `secp256k1+ML-DSA-44` account
from `junod keys` and `junod tx` without a flag day.

**Key insight (from memory: D3 core already done):** the hybrid account key is
already implemented in the SDK fork as `crypto/keys/hybrid` with a
`PubKey.VerifySignature` that returns true only if both halves verify. The
existing `x/auth/ante.SigVerificationDecorator` calls `pubKey.VerifySignature`,
so **no new decorator and no new SignMode are needed** — only a new gas case and
keyring/CLI plumbing.

### 3.1 Gas accounting

**Step 3.1 — add `SigVerifyCostMlDsa44` to auth params.**

File: `proto/cosmos/auth/v1beta1/auth.proto` (or `params.proto` depending on
SDK version). Add a parameter for the extra gas per hybrid signature. Regenerate.

**Step 3.2 — update `DefaultSigVerificationGasConsumer`.**

File: `x/auth/ante/sigverify.go`

Locate the switch over pubkey types and add:

```go
case *hybrid.PubKey:
    // classical secp256k1 cost is already metered; add the PQC half
    return ctx.GasMeter().ConsumeGas(
        params.SigVerifyCostMlDsa44,
        "signature verification ML-DSA-44",
    )
```

Use the measured on-chain verify cost (~270k gas equivalent for the pure
Wasm path; the native Go verify is much faster, so set a conservative constant
to start, e.g., 300k, then tune with a devnet benchmark). Do **not** make it
free; a hybrid signature is 2,484 B and verification is ~100 µs.

### 3.2 Keyring support

**Step 3.3 — add `keyring` record type.**

The SDK keyring stores `types.NewLocalKeyRecord` with a `*codectypes.Any`
PubKey. Because the hybrid `PubKey` is already registered in the codec, a hybrid
key can be stored as an `Any` just like a secp256k1 key. No new keyring type is
required — only a **new keyring algorithm** entry for key derivation.

File: `crypto/keyring/keyring.go` (or `crypto/keyring/options.go` depending on
version)

Add to the HD path algorithm list:

```go
const (
    // existing
    Secp256k1Algo = "secp256k1"
    // new
    HybridSecp256k1MlDsa44Algo = "hybrid-secp256k1-mldsa44"
)
```

And implement the `Derive` function:

```go
func (k hd) Derive(keyring.Algo, mnemonic, bip39Passphrase, hdPath string) ([]byte, error)
```

For the hybrid algorithm, the returned bytes are the concatenation of the
secp256k1 BIP-32 private key (32 B) and the ML-DSA-44 seed (32 B), total 64 B.
The `HybridPrivKey` then unmarshals that as `[secp256k1_priv(32) || mldsa44_seed(32)]`.

### 3.3 CLI

**Step 3.4 — `junod keys add --hybrid`.**

File: `client/keys/add.go`

Add a `--hybrid` flag that sets the algorithm to `HybridSecp256k1MlDsa44Algo`.
The rest of the key generation path is identical to classical; the keyring
stores the 64 B hybrid privkey as a `*hybrid.PrivKey` Any.

**Step 3.5 — `junod keys show --output json`.**

Verify that the hybrid pubkey displays correctly. The ML-DSA-44 pubkey is
1,312 B, so default to showing its hash and provide `--show-pubkey` for the full
bytes.

**Step 3.6 — signing a transaction.**

The existing `SIGN_MODE_DIRECT` works unchanged. `x/auth/tx.signTx` signs the
serialized `SignDoc` with the account's key type. For a hybrid account, the
`PrivKey.Sign()` produces the 2,484 B hybrid signature (secp256k1 64 B ||
ML-DSA-44 2,420 B). The signer does not need to know it is hybrid; it just
signs with the registered `PrivKey`.

### 3.4 Integration test

**Step 3.7 — devnet test.**

1. `junod keys add alice --hybrid`
2. Fund alice.
3. `junod tx bank send alice <bob> 1ujuno --from alice --fees 5000ujuno`
4. Assert tx succeeds, gas includes the `SigVerifyCostMlDsa44` charge.

---

## Part 4 — IBC Phase G: hybrid-aware `07-tendermint` client (`ibc-go` fork)

**Goal:** a counterparty chain's `07-tendermint` light client can verify headers
from a chain whose validator set is migrating to hybrid consensus keys.

**Key insight:** no new IBC client type is needed. The IBC `07-tendermint`
module calls `VerifyCommitLight` / `VerifyCommitLightTrusting` from the CometBFT
fork. Once the CometBFT fork's `crypto/encoding/codec.go` knows the hybrid oneof
variant (ADR-008 F4, already done), the IBC client inherits the dispatch.
The only IBC-side work is a **header-size bound bump** and an end-to-end test.

### 4.1 Fork and pin

**Step 4.1 — fork ibc-go at Juno's pinned version.**

Juno v29 likely uses `ibc-go v8.x` on Cosmos SDK v0.50. Create a fork branch:
`aegis-phase-g-hybrid-client`.

**Step 4.2 — replace CometBFT with the Aegis fork.**

In `go.mod`:

```go
replace github.com/cometbft/cometbft => github.com/Dragonmonk111/cometbft aegis-phase-cf-hybrid
```

Run `go mod tidy` and resolve any API drift between the two versions.

### 4.2 Header size bounds

**Step 4.3 — find the client-created checks.**

Files likely in `modules/light-clients/07-tendermint/`

- `client_state.go` or `update.go` has a `MaxExpectedTimePerBlock` and/or header
  size validation. Look for a constant like `MaxTrustLevel` or `maxClockDrift`.
- The header size check may be implicit in the Tendermint `Header.ValidateBasic()`.

Update the bound to allow the larger hybrid validator set + commit:

- Classical 100-validator set header: ~56 KB commit.
- Hybrid 100-validator set header: ~250 KB commit + ~134 KB validator pubkeys.

Set a conservative ceiling, e.g., 2 MB, as a governance parameter default, not a
hard-coded protocol constant. If the code hard-codes it, raise it to 2 MB and
add a comment referencing ADR-009.

### 4.3 End-to-end hybrid update test

**Step 4.4 — write a unit test simulating the migration.**

File: `modules/light-clients/07-tendermint/update_test.go` (new file if needed)

The test constructs:

1. A trusted header with a **heterogeneous** validator set: 2 classical Ed25519
   validators + 2 hybrid validators, total voting power 100.
2. A new header at `trusted+1` signed by all four.
3. The `MsgUpdateClient` path calls `VerifyCommitLight` and succeeds.

Then:

4. A second new header at `trusted+10` with a **rotated** hybrid validator
   (same address, new hybrid pubkey) — the skipping path
   `VerifyCommitLightTrusting` must count the rotated validator's power toward
   the trust overlap because its address is unchanged (ADR-009 G3).

Finally:

5. A header with a **sub-threshold** overlap (<1/3 of trusted power) must be
   rejected with `ErrNotEnoughVotingPowerSigned`.

### 4.4 Misbehaviour test

**Step 4.5 — test hybrid double-sign freezing.**

File: `modules/light-clients/07-tendermint/misbehaviour_test.go`

Construct two valid headers at the same height with different `BlockID`s, both
signed by the hybrid validator. Submit `MsgSubmitMisbehaviour` and assert the
client is frozen. This proves the IBC client detects hybrid-key equivocation.

### 4.5 Relayer note

**Step 4.6 — update the IBC relayer compatibility section of ADR-009.**

Document the required `ibc-go` fork tag and the new `MaxHeaderSize` default for
Hermes / Go relayer operators. No relayer code change is needed beyond the
vendored CometBFT version.

---

## Part 5 — Order of attack, dependencies, and risk

| Phase | Work | Effort | Depends on | Risk |
|-------|------|--------|------------|------|
| D3 | gas + keyring + CLI | 2–3 days | protoc installed | Low — pure SDK fork plumbing |
| F6 | proto + msg server + CLI | 3–4 days | protoc + D3 (for keygen key format) | Medium — touches validator state |
| F7 | remote signer signature passthrough | 1 day | F6 file-PV hybrid signing | Low — mostly removing length checks |
| G | ibc-go fork + bounds + tests | 2–3 days | F4 proto + CometBFT fork | Medium — ibc-go API drift |

**Recommended order:**

1. **D3 first** — it gives the team the keyring/CLI infrastructure and is the
   lowest risk. It also validates the hybrid key codec end-to-end, which is
   needed for F6 keygen output.
2. **F6 next** — once the key format is proven in D3, implement the validator
   rotation message.
3. **F7 in parallel** — a small remote-signer test can be written while F6 is
   being reviewed.
4. **G last** — it depends on the CometBFT fork being stable, which is true
   after F6 is wired.

**Biggest risk:** API drift. If Juno v29.1 pins `ibc-go v8.3.x` while the Aegis
CometBFT fork is at `v0.38.x`, check that the dependency tree resolves cleanly.
The `replace` directive in `go.mod` is the primary tool; but if `ibc-go` imports
a CometBFT type that changed between its expected version and the fork, you may
need to rebase the fork onto the exact CometBFT tag Juno uses.

---

## Part 6 — Commits and pushes

Use the **fast-export fast-import method** for the two forks (the local clones
have the pack-corruption issue). For each fork:

```cmd
cd /d aegis-forks\cometbft
mkdir C:\temp\aegis-push\cometbft
git fast-export aegis-phase-cf-hybrid | git -C C:\temp\aegis-push\cometbft fast-import --quiet
cd C:\temp\aegis-push\cometbft
git checkout aegis-phase-cf-hybrid
git push --force origin aegis-phase-cf-hybrid
```

Repeat for the SDK fork. Do not push from WSL (no SSH key there). Record the
new remote SHAs in `progress.txt`.

---

*This plan is the source of truth for the protoc-gated and IBC work. Convert
each numbered step into a TODO or GitHub issue before execution.*
