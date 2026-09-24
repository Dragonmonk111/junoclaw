# Validator Set, DKG & Security Model — Design + Plan

Status: design discussion → implementation plan. Converges at mainnet genesis.

## 1. How it works today (ground truth from code)

- `app/slay3rd/src/config.rs`: validator set is **static** — `validator_index`, peer list, BLS share all in `node.toml` + `keys.json`. Fixed at startup.
- `packages/golem/src/core.rs:120`: `ValidatorUpdate` is mocked — `TODO: whole mock validator story needs revisiting`.
- `tools/generate-testnet-keys`: generates ALL keys centrally — operator sees every BLS share. Devnet-only.
- Commonware consensus has `Epoch` type — epoch-based reconfiguration is the upstream pattern, not yet wired in.

**So today: validator change = halt → new keys → new config → restart.** No in-protocol set changes, no epochs wired.

## 2. Equal voting power — confirmed

BLS threshold: each validator holds one share of one group key. `n >= 3f+1`, quorum = 2f+1 shares. No stake weighting — one validator, one share, one vote. No ujclaw bonding exists anywhere in the protocol.

## 3. Is no-slashing safe?

Honest answer: it's a **consortium BFT model**, not PoS. Safety rests on:

- **Threshold honesty**: <1/3 of the set Byzantine. With n=7, tolerate 2 bad.
- **Admission control**: you choose who's in the set — the security parameter is vetting, not stake.
- **Attribution**: BLS shares identify exactly which validators signed a bad cert — provable misbehavior, publicly attributable. No slashing needed to *prove* fault.
- **Removal**: the punishment is ejection at next set change + reputational death. For known real-world operators (Ffern, Juno validators), reputation IS the bond.

**Can honesty be incentivized?** Yes, without full PoS:
- Fee revenue already flows to proposers — uptime = income.
- Optional later: require validators to post a **security bond in a contract** (jclaw-bond) — slashable by governance vote on proof of double-signing. This is "slashing as a contract," not protocol — much simpler to add than a staking module.
- Truth market (below) can carry operator bonds for adjudication work — same mechanism, different layer.

## 4. How the truth market watches validators

Q-Zeno adjudication layer (L4): staked operators vote on disputed claims. Applied to validators:

- Every consensus cert is attributable (BLS share → validator identity).
- A watchdog contract/service flags: double-signing (two certs, same view, different digests — cryptographic proof), extended downtime.
- Truth market resolves the dispute with operator stake; verdict feeds governance (L5) which decides ejection.
- **This is the enforcement layer that replaces slashing**: proof → market verdict → DAO ejects at next epoch boundary.

## 5. DKG tooling plan

Phase A — **Mainnet genesis (simplest sufficient)**:
- Each validator runs `keygen-share` locally → publishes Ed25519 pubkey + BLS public share + a signed attestation binding them.
- Coordinator (you) collects pubkeys → runs `assemble-genesis` → produces `node.toml` template + `genesis.json` + group threshold pubkey.
- Nobody ever sees another's secret share. ~30 min on a call.
- Extend `tools/generate-testnet-keys` into two commands: `keygen-share` (per-validator) and `assemble-genesis` (coordinator).

Phase B — **Epoch-based set changes (post-launch)**:
- Wire Commonware epochs: at epoch boundary, new set + new threshold key take effect.
- Set change = governance-approved config committed on-chain → nodes pick it up at boundary → new DKG among new set → old set signs handoff cert.
- Until Phase B ships: changes are halt-and-restart (documented, rehearsed).

Phase C — **Real DKG protocol** (if needed): distributed key generation where shares are never whole anywhere. Phase A's per-validator keygen is already 90% of the trust model — full DKG mainly removes the coordinator's assembly role.

## 6. Chain upgrades / halts

- Halt: validators stop at agreed height (governance or emergency comms). Static set = simple coordination.
- Upgrade: new binary + same keys → restart from state. BLS shares persist.
- Set change: halt → new DKG (Phase A tooling) → restart with new set. Post-Phase-B: in-protocol at epoch boundary.

## 7. BLS light client — the novel IBC path

Yes — this is the differentiator. JunoClaw certs are BLS threshold signatures; an `08-wasm` CosmWasm light client on Juno/Osmosis verifies them directly. No Tendermint needed on our side. Work items:
- Spec cert format → client update/verify logic
- CosmWasm BLS12-381 verify (host functions exist in our VM — check target chains' capabilities)
- Relayer: submit certs as client updates
- This is what makes "sovereign but interoperable" real.

## 8. Validator communication plan

1. Finish validator onboarding doc (setup guide).
2. Post DM to Juno validator group after their upgrade settles.
3. Collect commitments → pick n (recommend 7).
4. Schedule DKG ceremony call → Phase A tooling.
5. Genesis → launch → airdrop claims open.
