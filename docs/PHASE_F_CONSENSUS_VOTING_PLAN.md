# Phase F: Hybrid Consensus Voting — Detailed Framework Plan

> **Status:** Design complete, implementation pending  
> **Claim:** Juno will be the first Cosmos chain with post-quantum validator consensus signatures.  
> **Algorithm:** ML-DSA-44 (NIST FIPS 204)  
> **Approach:** Hybrid (classical Ed25519 + ML-DSA-44) with strict dual-verification

---

## 1. The Stakes: Why Phase F Is the Linchpin

Every other phase (A–E) upgrades a *single* cryptographic surface:
- **A/B:** Hashing / randomness
- **C:** P2P transport (node-to-node secrecy)
- **D:** Account keys (user transactions)
- **E:** IBC light-client (cross-chain verification)

**Phase F is different.** It touches the *heartbeat* of the chain: the validator signatures that commit every block. Without F, an attacker with a quantum computer can forge consensus votes, rewrite history, and double-spend. With F, the chain’s economic security guarantee extends into the post-quantum era.

This is the claim that separates **greenfield** PQC (Marius’s Shell, new chain from scratch) from **brownfield** PQC (Project Aegis, retrofitting a live chain). Greenfield chains simply *declare* their consensus algorithm. Brownfield chains must *migrate* it without halting.

---

## 2. The Core Problem: CometBFT Consensus Signatures

### 2.1 Current State

CometBFT (forked at v0.38.x for Juno) uses **Ed25519** for validator consensus keys:
- **Vote messages** (`Prevote`, `Precommit`) signed by validators
- **Block commit** (2/3+ signatures included in block)
- **Proposer block signatures**

Key sizes:
| Component | Ed25519 | ML-DSA-44 | Hybrid (Ed25519 + ML-DSA-44) |
|-----------|---------|-----------|------------------------------|
| Public key | 32 B | 1,312 B | 1,344 B |
| Signature | 64 B | 2,420 B | 2,484 B |

### 2.2 Impact of 39× Larger Signatures

A typical Cosmos block has ~100 validators, with 2/3+ (say 67) signing each commit:
- **Current commit size:** 67 × 64 B ≈ **4.3 KB**
- **Hybrid commit size:** 67 × 2,484 B ≈ **166 KB**

This is a **~39× increase in commit signature payload alone**.

| Metric | Current | Hybrid | Multiplier |
|--------|---------|--------|------------|
| Commit sigs (67 validators) | 4.3 KB | 166 KB | 39× |
| Full block with txs (1 MB) | 1 MB | ~1.16 MB | 1.16× |
| Block time target | 6s | 6s (unchanged) | — |
| Bandwidth per validator | ~170 KB/s | ~200 KB/s | 1.18× |

**Verdict:** Block size increase is manageable (~16% for a 1 MB block), but commit signature parsing and verification CPU time will increase significantly. This is the primary engineering risk.

### 2.3 Consensus Timing Constraints

CometBFT’s consensus timeout parameters (from `config.toml`):
```
timeout_propose = 3s
timeout_prevote = 1s
timeout_precommit = 1s
timeout_commit = 0s (finalize immediately)
```

Each validator must:
1. Receive a proposal
2. Verify its signature
3. Sign and broadcast a prevote
4. Collect 2/3+ prevotes
5. Verify all prevote signatures
6. Sign and broadcast a precommit
7. Collect 2/3+ precommits

**Critical path:** Signature verification happens on every message. If ML-DSA-44 verification takes ~1 ms (measured on devnet hardware), and a validator receives 100 prevotes, that’s **100 ms of pure verification time** — still within the 1-second prevote timeout, but leaving less headroom for network latency.

| Operation | Ed25519 (µs) | ML-DSA-44 (µs) | Hybrid total (µs) |
|-----------|--------------|----------------|-------------------|
| Sign | ~50 | ~800 | ~850 |
| Verify | ~100 | ~1,000 | ~1,100 |
| Verify 100 votes | ~10 ms | ~100 ms | ~110 ms |

**Verdict:** Feasible but tight. Must benchmark under load before mainnet deployment.

---

## 3. Implementation Architecture

### 3.1 Fork Strategy: Aegis CometBFT Fork

We maintain a fork of CometBFT at `v0.38.x` under `aegis-forks/cometbft/`.

**Changes required:**

#### 3.1.1 New Hybrid Consensus Key Type

Location: `aegis-forks/cometbft/crypto/hybrid/`

```go
package hybrid

import (
    "github.com/cometbft/cometbft/crypto/ed25519"
    "github.com/cloudflare/circl/sign/mldsa/mldsa44"
)

const (
    PubKeySize  = ed25519.PubKeySize + mldsa44.PublicKeySize  // 32 + 1312 = 1344
    PrivKeySize = ed25519.PrivKeySize + mldsa44.PrivateKeySize // 64 + 2560 = 2624
    SignatureSize = ed25519.SignatureSize + mldsa44.SignatureSize // 64 + 2420 = 2484
)

type PubKey struct {
    Ed25519Key ed25519.PubKey    // 32 bytes
    MLDS44Key  []byte            // 1312 bytes
}

type PrivKey struct {
    Ed25519Key ed25519.PrivKey
    MLDS44Key  mldsa44.PrivateKey
}
```

**Key design decisions:**
1. **Concatenated encoding:** PubKey and Signature are simply the Ed25519 component followed by the ML-DSA-44 component. No nested protobuf wrappers — keeps parsing fast and deterministic.
2. **Deterministic key derivation:** Use the Ed25519 private key seed to deterministically generate the ML-DSA-44 keypair via SHA-512 expansion. This ensures a single 32-byte mnemonic generates both halves.
3. **Dual verification:** A signature is valid **only if both** Ed25519.Verify() **and** ML-DSA-44.Verify() pass. No fallback, no partial acceptance.

#### 3.1.2 Amino Registration

Register in `aegis-forks/cometbft/crypto/encoding/`:
```go
// In the amino codec init
cdc.RegisterConcrete(&hybrid.PubKey{},  "tendermint/PubKeyHybridEd25519MLDSA44", nil)
cdc.RegisterConcrete(&hybrid.PrivKey{}, "tendermint/PrivKeyHybridEd25519MLDSA44", nil)
```

#### 3.1.3 Protobuf Definition

New file: `aegis-forks/cometbft/proto/tendermint/crypto/hybrid/keys.proto`

```protobuf
syntax = "proto3";
package tendermint.crypto.hybrid;

option go_package = "github.com/cometbft/cometbft/crypto/hybrid";

// PubKeyHybrid is a hybrid post-quantum public key for CometBFT consensus.
message PubKeyHybrid {
  bytes ed25519_key = 1;   // 32 bytes
  bytes mldsa44_key = 2;   // 1312 bytes
}

// PrivKeyHybrid is a hybrid post-quantum private key.
message PrivKeyHybrid {
  bytes ed25519_key = 1;   // 64 bytes (seed + pubkey)
  bytes mldsa44_key = 2;   // 2560 bytes
}

// SignatureHybrid is a dual signature.
message SignatureHybrid {
  bytes ed25519_sig = 1;   // 64 bytes
  bytes mldsa44_sig = 2;   // 2420 bytes
}
```

Generate with:
```bash
cd aegis-forks/cometbft
buf generate proto/tendermint/crypto/hybrid
```

#### 3.1.4 Validator Key File Migration

Validators store consensus keys in `priv_validator_key.json`:
```json
{
  "address": "...",
  "pub_key": {
    "type": "tendermint/PubKeyEd25519",
    "value": "..."
  },
  "priv_key": {
    "type": "tendermint/PrivKeyEd25519",
    "value": "..."
  }
}
```

**Migration path:**
1. Validator runs `junod keys migrate-consensus --new-algo hybrid` (new CLI command)
2. CLI generates the ML-DSA-44 key deterministically from the Ed25519 seed
3. Writes new file:
```json
{
  "address": "...",
  "pub_key": {
    "type": "tendermint/PubKeyHybridEd25519MLDSA44",
    "value": "<base64(1344 bytes)>"
  },
  "priv_key": {
    "type": "tendermint/PrivKeyHybridEd25519MLDSA44",
    "value": "<base64(2624 bytes)>"
  }
}
```
4. Old file backed up to `priv_validator_key.json.ed25519.backup`

#### 3.1.5 Consensus Message Signing

Modify `aegis-forks/cometbft/types/priv_validator.go`:

```go
func (pv *FilePV) SignVote(chainID string, vote *Vote) error {
    // Existing: sign with Ed25519
    // New: sign with both Ed25519 and ML-DSA-44
    
    ed25519Sig, err := pv.Key.Ed25519Key.Sign(vote.SignBytes(chainID))
    if err != nil { return err }
    
    mldsa44Sig, err := pv.Key.MLDS44Key.Sign(nil, vote.SignBytes(chainID), nil)
    if err != nil { return err }
    
    vote.Signature = append(ed25519Sig, mldsa44Sig...)
    return nil
}
```

#### 3.1.6 Vote Verification in Consensus Reactor

Modify `aegis-forks/cometbft/consensus/state.go`:

```go
func (cs *State) tryAddVote(vote *types.Vote, peerID p2p.ID) (bool, error) {
    // Existing: verify Ed25519 signature
    // New: verify hybrid signature
    
    validatorPubKey := cs.Validators.GetByAddress(vote.ValidatorAddress).PubKey
    hybridPubKey, ok := validatorPubKey.(hybrid.PubKey)
    if !ok {
        return false, errors.New("expected hybrid consensus key")
    }
    
    if !hybridPubKey.VerifyVote(vote) {
        return false, errors.New("invalid hybrid consensus signature")
    }
    
    // ... rest of consensus logic
}
```

### 3.2 Fork Strategy: Aegis Cosmos SDK Fork

The Cosmos SDK fork (`aegis-forks/cosmos-sdk/`) needs complementary changes:

#### 3.2.1 Proto Registration for `cosmos.crypto.hybrid`

New file: `aegis-forks/cosmos-sdk/proto/cosmos/crypto/hybrid/keys.proto`

```protobuf
syntax = "proto3";
package cosmos.crypto.hybrid;

option go_package = "github.com/cosmos/cosmos-sdk/crypto/keys/hybrid";

// PubKey for Cosmos SDK accounts (different from consensus keys).
// This is the account-level hybrid key (secp256k1 + ML-DSA-44),
// already implemented in aegis-accounts.
message PubKey {
  bytes secp256k1_key = 1;  // 33 bytes
  bytes mldsa44_key   = 2;  // 1312 bytes
}
```

#### 3.2.2 Codec Registration

Update `aegis-forks/cosmos-sdk/crypto/codec/proto.go`:

```go
import "github.com/cosmos/cosmos-sdk/crypto/keys/hybrid"

func RegisterInterfaces(registry codectypes.InterfaceRegistry) {
    // ... existing registrations ...
    registry.RegisterImplementations(pk, &hybrid.PubKey{})
    registry.RegisterImplementations(priv, &hybrid.PrivKey{})
}
```

Update `aegis-forks/cosmos-sdk/crypto/codec/amino.go`:

```go
func RegisterCrypto(cdc *codec.LegacyAmino) {
    // ... existing registrations ...
    cdc.RegisterConcrete(&hybrid.PubKey{}, hybrid.PubKeyName, nil)
    cdc.RegisterConcrete(&hybrid.PrivKey{}, hybrid.PrivKeyName, nil)
}
```

#### 3.2.3 Keyring Algorithm Registration

Update `aegis-forks/cosmos-sdk/crypto/keyring/keyring_linux.go`:

```go
var DefaultOptions = Options{
    SupportedAlgos:       SigningAlgoList{hd.Secp256k1, hybrid.KeyType},
    SupportedAlgosLedger: SigningAlgoList{hd.Secp256k1},
    // ...
}
```

Or more dynamically, add an `Option` to append hybrid:

```go
func WithHybridAlgo() Option {
    return func(opts *Options) {
        if !opts.SupportedAlgos.Contains(hybrid.KeyType) {
            opts.SupportedAlgos = append(opts.SupportedAlgos, hybrid.KeyType)
        }
    }
}
```

#### 3.2.4 CLI Commands

Add to `junoclaw` CLI:

```bash
# Generate a new hybrid account key
junod keys add mykey --algo hybrid

# Migrate existing secp256k1 account to hybrid
junod keys migrate-to-hybrid mykey

# Generate/migrate consensus key to hybrid
junod tendermint gen-validator --algo hybrid
junod tendermint migrate-consensus-key
```

---

## 4. Coordination and Upgrade Mechanism

### 4.1 The Hard Problem: Coordinated Migration

Unlike account keys (each user migrates independently), **consensus keys affect all validators simultaneously**.

If one validator switches to hybrid but others don’t, the chain halts because:
- Hybrid votes have different PubKey types
- Old validators can’t verify hybrid signatures
- New validators reject old Ed25519-only signatures

### 4.2 On-Chain Governance Upgrade Path

**Recommended approach: Coordinated software upgrade via governance**

1. **Governance Proposal:** Submit `SoftwareUpgradeProposal` with:
   - Target block height: `H_upgrade`
   - New binary: Juno v30 with hybrid consensus support
   - Migration parameters: `consensus_key_type = "hybrid"`

2. **Voting Period:** 14 days (standard)
   - Validators signal readiness by voting YES
   - Exchanges, explorers, relayers prepare for new binary

3. **Pre-Upgrade (H_upgrade - N blocks):**
   - Validators install new binary
   - Each validator runs `junod tendermint migrate-consensus-key` to generate hybrid key from existing Ed25519 seed
   - New `priv_validator_key.json` prepared but **not yet active**

4. **At H_upgrade:**
   - Chain halts automatically (standard Cosmos upgrade behavior)
   - `upgrade.BeginBlocker` executes consensus key migration:
     ```go
     // In upgrade handler
     func RunConsensusKeyMigration(ctx sdk.Context) {
         // Set consensus params to require hybrid keys
         consensusParams.ValidatorPubKeyTypes = []string{"hybrid"}
         
         // Validator set is NOT changed — same validators, new key type
         // Each validator’s new PubKey is read from their updated priv_validator_key.json
     }
     ```

5. **Post-Upgrade:**
   - Chain resumes with hybrid consensus signatures
   - First block after upgrade is signed with hybrid keys
   - Old Ed25519 signatures are rejected by new binary

### 4.3 Alternative: Gradual Rollout (Riskier)

**Not recommended for consensus keys**, but documented for completeness:

- Allow both Ed25519 and hybrid signatures during a transition period
- Require 2/3+ hybrid before finalizing the switch
- **Risk:** “Soft fork” where old and new validators form incompatible views

**Verdict:** Hard upgrade (4.2) is the only safe path. Cosmos SDK’s `x/upgrade` module is designed for exactly this.

---

## 5. Testing and Validation Plan

### 5.1 Devnet Testing (Multi-Validator)

Current devnet is single-validator. Phase F requires **minimum 4 validators** to test consensus.

**Devnet setup:**
```yaml
# docker-compose.yml
validators:
  - validator-0 (proposer)
  - validator-1
  - validator-2
  - validator-3
```

**Test scenarios:**
1. **Happy path:** All 4 validators with hybrid keys, blocks produced normally
2. **1/4 offline:** 3 validators still reach consensus (2/3+ threshold)
3. **Signature verification stress:** Rapid block production (1s blocks), measure CPU
4. **Network partition:** Split 2-2, ensure no double-signing on rejoin
5. **Rollback test:** Upgrade to hybrid, run 100 blocks, rollback binary, verify chain halt (expected)

### 5.2 Benchmark Metrics

| Test | Target | Measurement |
|------|--------|-------------|
| Block production rate | ≥ 6s/block | Timer |
| Vote verification (100 vals) | < 200 ms | CPU profiler |
| Memory per validator | < 2 GB | `top` / `pprof` |
| Commit size (100 vals) | < 250 KB | Block inspector |
| Signature parsing time | < 1 ms/vote | Custom benchmark |

### 5.3 Simulation: Large Validator Set

Before mainnet, run a **testnet with 100+ validators** (can be simulated with VMs or cloud instances):
- Use `cometbft/testnet` tool to generate genesis
- Deploy on AWS/GCP with geographically distributed regions
- Run for 72 hours continuous
- Measure bandwidth, CPU, memory, block propagation time

---

## 6. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ML-DSA-44 verification too slow | Medium | High | Benchmark early; optimize with AVX2 if needed; consider batch verification |
| Block size exceeds p2p limit | Low | High | Increase `max_packet_msg_payload_size` in CometBFT config |
| Validator fails to migrate key | Low | Critical | Provide automated migration script; test on devnet extensively |
| Governance proposal fails | Low | Critical | Pre-vote signaling; social coordination; have rollback plan |
| Ledger/HSM incompatibility | Medium | Medium | Phase F-v2: add Ledger app support for hybrid keys (future work) |
| Binary size too large | Low | Low | ML-DSA-44 + ML-KEM adds ~500 KB; acceptable |

---

## 7. Work Breakdown Structure (WBS)

### 7.1 Phase F.1: CometBFT Fork Core (Est. 3–4 days)
- [ ] **F.1.1** Create `crypto/hybrid/` package with PubKey, PrivKey, Sign, Verify
- [ ] **F.1.2** Amino registration in `crypto/encoding/`
- [ ] **F.1.3** Protobuf definition and code generation (`keys.proto`)
- [ ] **F.1.4** Modify `types/priv_validator.go` to sign with hybrid
- [ ] **F.1.5** Modify `consensus/state.go` to verify hybrid votes
- [ ] **F.1.6** Unit tests for all new crypto functions
- [ ] **F.1.7** Integration test: single validator with hybrid key produces blocks

### 7.2 Phase F.2: Cosmos SDK Fork Wiring (Est. 2–3 days)
- [ ] **F.2.1** Proto definition for `cosmos.crypto.hybrid`
- [ ] **F.2.2** Codec registration (proto + amino)
- [ ] **F.2.3** Keyring algorithm registration (`SupportedAlgos`)
- [ ] **F.2.4** CLI commands for key generation and migration
- [ ] **F.2.5** Unit tests for SDK-side integration

### 7.3 Phase F.3: Juno Node Integration (Est. 2–3 days)
- [ ] **F.3.1** Wire Juno `app.go` to use hybrid codec
- [ ] **F.3.2** Add `junod tendermint migrate-consensus-key` command
- [ ] **F.3.3** Upgrade handler for coordinated switch
- [ ] **F.3.4** Build and test Juno binary with hybrid consensus

### 7.4 Phase F.4: Multi-Validator Devnet (Est. 3–4 days)
- [ ] **F.4.1** Expand docker-compose to 4 validators
- [ ] **F.4.2** Genesis config for hybrid consensus keys
- [ ] **F.4.3** Run 48-hour stability test
- [ ] **F.4.4** Collect benchmarks (CPU, memory, bandwidth)
- [ ] **F.4.5** Document results in `PHASE_F4_DEVNET_RESULTS.md`

### 7.5 Phase F.5: Governance and Mainnet Prep (Est. 5–7 days)
- [ ] **F.5.1** Draft governance proposal template
- [ ] **F.5.2** Socialize with Juno validators (Discord, forums)
- [ ] **F.5.3** Publish testnet for validator rehearsal
- [ ] **F.5.4** Coordinate with block explorers (indexer updates for new key type)
- [ ] **F.5.5** Coordinate with exchanges (custody key migration)
- [ ] **F.5.6** Write final `PHASE_F5_MAINNET_READINESS.md`

### 7.6 Phase F.6: Communication (Est. 1–2 days)
- [ ] **F.6.1** Update `PROJECT_AEGIS_JUNO_FULL_PQC.md` with Phase F completion
- [ ] **F.6.2** Draft announcement tweet: “Juno consensus is now post-quantum”
- [ ] **F.6.3** Publish technical deep-dive on Medium
- [ ] **F.6.4** Update competitive analysis: Aegis is the only brownfield PQC consensus

---

## 8. Total Effort Estimate

| Phase | Estimated Effort |
|-------|-----------------|
| F.1 CometBFT Core | 3–4 days |
| F.2 SDK Wiring | 2–3 days |
| F.3 Juno Integration | 2–3 days |
| F.4 Multi-Validator Devnet | 3–4 days |
| F.5 Mainnet Prep | 5–7 days |
| F.6 Comms | 1–2 days |
| **Total** | **16–23 days** |
| **With buffer / review cycles** | **4–5 weeks** |

---

## 9. Immediate Next Actions (Start Today)

1. **Create F.1.1 branch:** `git checkout -b feat/f1-hybrid-consensus`
2. **Implement `crypto/hybrid/` package** in `aegis-forks/cometbft/`
3. **Write unit tests** for Sign/Verify/Address/Equals
4. **Generate proto** with `buf generate`
5. **Verify** single-validator devnet produces blocks with hybrid consensus key

---

## 10. How Phase F Compares to Other Approaches

| Approach | Validator Key Type | Upgrade Model | Status |
|----------|-------------------|---------------|--------|
| **Ethereum (ERC-4337)** | BLS (still classical) | Account abstraction layer | Not PQC yet |
| **Marius/Shell** | ML-DSA-44 (greenfield) | New chain, no migration | In development |
| **Aegis/Juno** | Hybrid Ed25519+ML-DSA-44 | Coordinated upgrade of live chain | **This plan** |
| **BTC/QRL (QRLLite)** | XMSS | New chain (greenfield) | Live but tiny |

**Aegis’s unique claim:** We are doing what no other Cosmos chain has attempted — migrating a live, production validator set to post-quantum signatures without halting the chain permanently or forcing users to move funds.

---

*Last updated: 2026-06-18*  
*Next review: After F.1.1 completion*
