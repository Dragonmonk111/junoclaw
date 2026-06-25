# ADR-010: Staged account-key migration — opt-in registration + dual-signature enforcement (Project Aegis Phase D, migration track)

**Status:** Proposed — Phase D migration deliverable of [PROJECT_AEGIS_JUNO_FULL_PQC](./PROJECT_AEGIS_JUNO_FULL_PQC.md) §4.3
**Date:** 2026-06-25
**Authors:** Dragonmonk / VairagyaNodes (with Cascade)
**Scope:** the *process* by which an **existing** classical account becomes hybrid **without changing its address, losing funds, or requiring a flag day** (surfaces #5 account auth, #6 tx signatures, #9 migration UX)
**Depends on:** ADR-007 (the hybrid key **type** + address derivation + AnteHandler), this ADR specifies the **migration mechanism** ADR-007 deliberately left out

> ADR-007 defines *what* a hybrid account is (the key type, signature, address,
> HD derivation). This ADR defines *how an account already in use crosses over*:
> the on-chain registration message, the staged enforcement ladder, the rollback
> rules, and the failure modes. The two are complementary — ADR-007 is the
> destination, ADR-010 is the road.

---

## Context

ADR-007 introduces hybrid `secp256k1 + ML-DSA-44` accounts where the address is
`address.Hash("pqc/hybrid-secp256k1-mldsa44", secp33 || mldsa1312)[:20]`. That
address is derived from **both** key halves, so a brand-new hybrid account has a
**different** address from any pre-existing classical account.

This creates a migration gap that ADR-007 does not address: a user (or an agent
DAO member) with an existing `juno1…` classical address, holding funds and
governance weight, **cannot simply "become hybrid"** — the naive path would mint
a *new* address and force a fund/credential transfer, breaking every reference to
the old address (delegations, contract allow-lists, the JunoClaw member roster
keyed by `addr`, IBC escrow records, explorer history).

The agent DAO makes this acute: the governance credential is the **member
roster** keyed by wallet address (cw4-group semantics — see `JCLAW_TOKEN_DESIGN.md`).
Changing a member's address means a `WeightChange`/re-`Bud` dance and loses the
trust-tree edge. We need migration that **preserves the address**.

The tension: ADR-007's address binds *both* halves, but we want to *add* a PQC
half to an *existing* classical address whose 20 bytes were derived from the
classical key **alone** (`ripemd160(sha256(secp_pub))`). We cannot retroactively
fold a PQC key into an address that already exists on-chain without changing it.

**Resolution:** decouple the *authenticator* from the *address*. Keep the
existing classical address as the **account identity**, and register the PQC half
as an **associated authenticator** in a new state map, enforced by the
AnteHandler. The address never changes; what changes is *how many signatures the
AnteHandler demands* for that address.

---

## Decision

Migrate in **four enforcement stages** driven by an explicit, account-scoped
on-chain registration and a per-account enforcement flag. Each stage is opt-in
and reversible until the account commits to enforcement.

```
Stage 0  CLASSICAL        secp256k1 only (today)
Stage 1  REGISTERED       PQC pubkey registered, NOT yet required (advisory)
Stage 2  DUAL-REQUIRED    both secp256k1 AND ML-DSA-44 must verify (hybrid era)
Stage 3  PQC-ONLY         only ML-DSA-44 required (Phase H, post-classical)
```

### M1 — `MsgRegisterPqcKey`: associate a PQC half with an existing address

A new SDK message in a small `x/pqcauth` module (or an extension of `x/auth`):

```proto
message MsgRegisterPqcKey {
    string  address       = 1;   // bech32 of the EXISTING classical account
    bytes   mldsa44_pubkey = 2;  // 1,312 B FIPS 204 public key
    bytes   binding_sig    = 3;  // ML-DSA-44 signature over the binding message
}
```

**Binding message** (prevents an attacker registering *their* PQC key against
*your* address, and binds the PQC key to this specific account + chain):

```
binding_msg = sha256(
    "aegis/pqc/account-bind/v1" ||
    chain_id                    ||
    address                     ||   // 20-byte classical address
    secp256k1_pubkey(33)        ||   // proves the classical key is revealed/owned
    mldsa44_pubkey(1312)
)
```

The transaction carrying `MsgRegisterPqcKey` is **signed by the existing
classical key** (normal AnteHandler), AND the message body carries
`binding_sig` = the ML-DSA-44 signature over `binding_msg`. So registration
requires **proof of possession of both keys at once**:

- the classical signature (outer tx auth) proves control of the address,
- the `binding_sig` proves control of the PQC private key and binds it to this
  address + chain.

This is a **proof-of-possession** ceremony, not yet an authentication change.

**State written** (`x/pqcauth` keyed by address):

```
PqcAccount {
    mldsa44_pubkey:  bytes        // 1,312 B
    stage:           enum         // REGISTERED | DUAL_REQUIRED | PQC_ONLY
    registered_at:   block_height
    enforced_at:     block_height // 0 until stage >= DUAL_REQUIRED
}
```

After M1 the account is at **Stage 1 (REGISTERED)**. Nothing about how its
transactions authenticate has changed yet — the PQC key is on file but
**advisory**.

### M2 — `MsgEnablePqcEnforcement`: advance to DUAL-REQUIRED

A second message, **signed by the classical key AND carrying a fresh PQC
signature** over the enable message, flips the account to Stage 2:

```proto
message MsgEnablePqcEnforcement {
    string address       = 1;
    bytes  enable_sig    = 2;   // ML-DSA-44 sig over enable_msg
    uint64 grace_blocks  = 3;   // optional cooldown before enforcement bites
}
```

```
enable_msg = sha256("aegis/pqc/account-enable/v1" || chain_id || address || current_height)
```

Requiring a **dual signature on the enabling transaction itself** is the safety
gate: an account cannot enter DUAL-REQUIRED unless it has *just demonstrated*
that it can produce a valid hybrid signature **at enforcement time**. This
eliminates the catastrophic failure mode of locking yourself out with a
mis-registered or lost PQC key — you literally cannot enable enforcement without
proving the key works *now*.

`grace_blocks` writes `enforced_at = current_height + grace_blocks`, giving
tooling a window to roll out before the AnteHandler starts rejecting
classical-only signatures.

### M3 — AnteHandler enforcement ladder

The `SigVerificationDecorator` consults `x/pqcauth` for the signer's address and
branches on stage:

| Stage | secp256k1 sig | ML-DSA-44 sig | Accept iff |
|------:|:-------------:|:-------------:|------------|
| 0 CLASSICAL | required | absent | secp verifies (today) |
| 1 REGISTERED | required | **optional** | secp verifies (PQC ignored if present) |
| 2 DUAL-REQUIRED (≥ `enforced_at`) | required | required | **both** verify (ADR-007 hybrid rule) |
| 3 PQC-ONLY | optional | required | ML-DSA-44 verifies |

**How a transaction carries two signatures for one signer:** reuse the existing
`TxRaw.signatures` + `SignerInfo` machinery. The hybrid signature is the
ADR-007 ordered concatenation (64 B secp || 2,420 B ML-DSA-44) placed in the
single signature slot for that signer; the AnteHandler splits it by the
account's registered stage. **No new `SignMode`, no proto change to `Tx`** — the
signature bytes for a Stage-2 account are simply longer, and the decorator knows
to expect both halves because the *account's stage in state* says so. This is the
account-layer mirror of ADR-008 §F4's "resolve the verifier from state, not from
the wire."

**Stage 1 is deliberately permissive:** the PQC half is *accepted if present and
valid* but *not required*. This lets wallets and agents start producing hybrid
signatures and self-test the full path against a live chain **before** any
enforcement, with zero lock-out risk.

### M4 — Rollback rules

| From | To | Allowed? | How |
|------|----|---------:|-----|
| 1 REGISTERED | 0 CLASSICAL | ✅ | `MsgDeregisterPqcKey`, classical-signed (PQC was advisory) |
| 2 DUAL-REQUIRED | 1 REGISTERED | ✅ but **dual-signed** | `MsgDisablePqcEnforcement` requires both halves — you must still hold the PQC key to step down |
| 2 DUAL-REQUIRED | 0 CLASSICAL | ❌ direct | must go 2→1→0 |
| 3 PQC-ONLY | 2 | governance / break-glass only | Phase H is intended one-way; downgrade is an emergency lever, gated |

**Why downgrade from Stage 2 needs the PQC key:** if a single classical
signature could drop enforcement, then a quantum attacker who broke secp256k1
could **disable** the very protection that stops them, then forge freely. So
stepping *down* from DUAL-REQUIRED must itself satisfy the DUAL-REQUIRED rule.
This closes the obvious downgrade-attack vector.

### M5 — Key rotation within the PQC half

`MsgRotatePqcKey` (dual-signed at Stage ≥ 2) replaces `mldsa44_pubkey` with a new
one, binding the new key with the same proof-of-possession ceremony as M1. The
address is unchanged (it was always the classical address). This lets an account
cycle its PQC key on suspicion of compromise without touching its identity.

---

## Interaction with the JunoClaw agent DAO roster

The member roster (`MemberInput { addr, weight, role }`) is keyed by the
**classical address**, which ADR-010 **never changes**. So:

- A member migrates to hybrid by submitting `MsgRegisterPqcKey` +
  `MsgEnablePqcEnforcement` from their existing wallet. Their roster entry,
  weight, trust-tree edges, and `Bud` lineage are **untouched** — only the
  AnteHandler's signature demand changes.
- No `WeightChange`/re-`Bud` is needed; the credential is address-stable by
  construction.
- The DAO can set a **governance target** (e.g., "all members at Stage ≥ 2
  before height H") and track it by reading each member's `x/pqcauth` stage —
  the credential and the PQC status are orthogonal columns on the same address.

This is the payoff of decoupling authenticator from address (Context §): the
soulbound credential and the quantum-safety upgrade compose cleanly instead of
fighting over the address.

---

## Security analysis

- **Registration cannot be hijacked:** `binding_sig` over a message that
  includes the classical pubkey + chain_id + address means an attacker cannot
  register their PQC key against a victim address (they lack the victim's
  classical key to authorize the outer tx) and cannot replay a binding across
  chains (chain_id is bound).
- **Enforcement cannot lock you out:** M2 requires a *live* dual signature to
  enter Stage 2 — you prove the PQC key works before it becomes mandatory.
- **Downgrade is not a quantum bypass:** M4 requires the PQC key to leave
  Stage 2, so breaking only classical does not let an attacker disable
  enforcement.
- **Stage 1 is risk-free adoption:** advisory PQC means tooling can roll out and
  self-test with no possibility of bricking an account.
- **Address stability removes the migration's biggest footgun:** no fund moves,
  no credential re-issue, no delegation reset — the failure surface of "moved to
  a new address" is eliminated entirely.
- **Replay/sign-bytes:** all binding/enable/rotate messages include `chain_id`
  and a height (or are wrapped in the normal tx with sequence/account-number), so
  none replay across chains, accounts, or time.

---

## What this does NOT do (honest boundaries)

- **Does NOT change the address.** That is the whole point — the classical
  20-byte `juno1…` is the permanent identity; the PQC half is an attached
  authenticator, not part of the address. (This differs from ADR-007's
  *fresh* hybrid accounts, whose address binds both halves. Both coexist: new
  accounts use ADR-007 addresses; migrated accounts use ADR-010 attachment.)
- **Does NOT touch consensus, IBC, or contract auth.** Pure account-tx layer.
- **Does NOT force any account to migrate.** Stage 0 is the untouched default
  forever, until/unless governance proposes Phase H deprecation.
- **Does NOT introduce a new `SignMode` or break existing wallets** — Stage-0
  accounts sign exactly as today.

---

## Alternatives considered

- **Just use ADR-007 fresh hybrid addresses and tell users to move funds.**
  Rejected: breaks the agent-DAO roster, delegations, contract allow-lists, and
  every external reference to the old address — a migration UX disaster for
  long-lived agent identities.
- **Fold the PQC key into the existing address by re-deriving it.** Impossible:
  the address already exists on-chain derived from the classical key alone;
  changing the derivation changes the 20 bytes.
- **Single combined message for register+enforce (one step).** Rejected: removes
  the Stage-1 risk-free self-test window and the proof-that-it-works-now gate;
  re-introduces the lock-out footgun.
- **Allow classical-signed downgrade from Stage 2.** Rejected: opens a quantum
  downgrade-attack (break secp → disable enforcement → forge).
- **A `SIGN_MODE_HYBRID`.** Rejected for the same reason as ADR-007 §D5 — the
  account's *stage in state* already disambiguates the verifier; no wire
  negotiation needed.

---

## Consequences

**Positive**

- Existing accounts (and agent-DAO members) go quantum-safe **without changing
  address, moving funds, or re-issuing credentials.**
- Staged ladder with a risk-free advisory stage and a prove-it-works enable gate
  makes lock-out essentially impossible.
- Downgrade-attack closed by the dual-signed step-down rule.
- Composes cleanly with the soulbound roster credential (orthogonal columns).

**Negative / costs**

- A new `x/pqcauth` module (or `x/auth` extension): one state map + four messages
  + an AnteHandler branch. More surface than ADR-007's key type alone.
- Stage-2 transactions are ~2,484 B larger per signer (the ADR-007 cost) and the
  AnteHandler does one extra ML-DSA-44 verify (~101 µs — negligible).
- Two address provenances coexist (ADR-007 fresh-hybrid vs ADR-010 attached);
  explorers/wallets must understand that a classical-looking `juno1…` address
  may demand a hybrid signature (read its `x/pqcauth` stage).

---

## Implementation plan (M1 → M5)

1. **M-harness (in `aegis-accounts/`, no SDK fork):** extend the existing harness
   with the binding/enable message construction + verification and a state-machine
   model of the stage ladder, with tests for: binding-sig hijack rejection,
   cross-chain replay rejection, enable-without-working-PQC rejection, Stage-1
   advisory acceptance, Stage-2 dual enforcement, and downgrade-attack rejection.
2. **M-fork (gated on SDK fork + devnet):** `x/pqcauth` module (proto + keeper +
   msg server), AnteHandler decorator branch reading the stage, CLI
   (`junod tx pqcauth register|enable|disable|rotate`), and an end-to-end devnet
   test that takes a funded classical account through 0→1→2 and confirms a
   classical-only tx is rejected at Stage 2 but the hybrid tx succeeds — all at
   the **same address**.

The harness is the source of truth for the binding/enable crypto and the stage
state machine; the fork is the module-wiring exercise on top, exactly as
ADR-007 §D2→D3.

---

## References

- `ADR-007-PQC-HYBRID-ACCOUNTS.md` — the hybrid key **type** this ADR migrates *to*
- `ADR-008-PQC-HYBRID-CONSENSUS.md` §F4 — "resolve the verifier from state" pattern
- `ADR-009-PQC-HYBRID-IBC-LIGHTCLIENT.md` — address-stability invariant reused cross-chain
- `JCLAW_TOKEN_DESIGN.md` — the address-keyed member roster this preserves
- `PROJECT_AEGIS_JUNO_FULL_PQC.md` §4.3 (accounts), §6 (determinism)
- Cosmos SDK `x/auth/ante` `SigVerificationDecorator`, `TxRaw`/`SignerInfo`,
  `crypto/address`; FIPS 204 (ML-DSA-44), RFC 5869 (HKDF)
