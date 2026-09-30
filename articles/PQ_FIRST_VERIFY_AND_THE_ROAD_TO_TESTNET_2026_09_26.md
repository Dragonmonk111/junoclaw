# Post-Quantum, On-Chain, Real: Where JunoClaw Stands on the Road to Testnet

*September 26, 2026 — revised September 28 after the devnet re-genesis*

Two days ago we wrote about first light over IBC — the first live handshake
between JunoClaw and a real Cosmos chain, every proof checked by our BLS light
client. This week the chain did something arguably stranger: it verified a
**post-quantum signature inside a CosmWasm contract**, on live consensus, in a
transaction that cost less gas than a uni-7 contract upload.

Since that draft, the engine got hardened (atomic finalize-driven execution,
a real mempool, bounded peer payloads), the devnet got wiped and regenerated
on purpose, the IBC bridge came back up in ninety seconds, and we lost — and
forensically accounted for — a funded relayer key. This is a status article.
What shipped, what the MAYO verification actually proves, where we're
honestly not solid yet, and what has to happen before we'd put a public
testnet in front of strangers.

## What shipped

**A native contract lifecycle.** JunoClaw decodes standard wasmd
`/cosmwasm.wasm.v1.*` messages straight into `WasmMsg` — no governance
proposal, no IBC hop. The relayer picked up five subcommands:

- `jc-store-code` / `jc-instantiate` / `jc-execute` — the full
  store → instantiate → execute path, signed by the deployer key.
- `jc-query` / `jc-contracts` — smart-contract and code-id lookups over gRPC.
- `--msg @file` — because MAYO public keys are 4–5KB and nobody should have
  to paste a JSON array of five thousand integers into a shell.

**`jclaw-credential` on devnet.** The credential-graph contract (weighted
membership tree with an optional post-quantum key hash per member) ran live:

- `code_id` 2, contract `juno17c5ucyukaf9heseh7gnyjgmwd3jtz6gegm039ezkjhnx6zx526hqq0738c`
  (on the pre-regenesis chain — see "The 48 hours nobody plans for")
- a `Bud` registered a child member with its **MAYO-2 public key** — the
  contract stores only the SHA-256 hash (`3f2453…db869a`), so any supported
  MAYO parameter set fits one field
- then the headline: `VerifyMayoAttestation` ran the **full MAYO-2
  verification inside the contract** — pure Rust compiled to Wasm — against a
  real KAT vector, and DeliverTx came back `success=true`,
  **`gas_used = 309 076`**

That number deserves a beat. Three hundred thousand gas for a post-quantum
signature verification, on-chain, without a precompile. For context, that's
the same order of magnitude as a small contract instantiate — and it's the
*unoptimized* path. The contract also supports MAYO-1/3/5 and ML-DSA
(44/65/87), and there's a `mayo-precompile` feature flag waiting for the
wasmvm-fork host function if we ever want verification in native code.

**Update, September 27 — the full MAYO sweep.** A v2 of the contract
(`code_id` 3, `juno10js5…hnhpek`) takes an explicit `variant` on
`VerifyMayoAttestation`. We ran MAYO-2, MAYO-3 and MAYO-5 end-to-end — Bud a
member with the variant's public key, then verify a signature against it —
plus a tampered MAYO-2 signature as a negative control. The transactions
below executed on the pre-regenesis devnet; the heights are historical but
the measurements are the deliverable:

| Step | Height | gas_used | Result |
|---|---|---|---|
| MAYO-2 verify | 460 286 | 309 396 | success |
| MAYO-2 verify, tampered signature | 460 866 | 309 573 | **rejected** |
| MAYO-3 Bud / verify | 462 414 / 462 422 | 200 212 / 400 245 | success |
| MAYO-5 Bud / verify | 462 562 / 462 574 | 294 132 / 725 804 | success |

These are whole-transaction numbers against a 4M gas limit. The worst case,
MAYO-5 at NIST's highest security category, lands at about **18% of the
limit**. The tampered signature costs the same gas as the valid one and
fails with `MAYO signature verification failed` — the verifier does the
work and says no, which is exactly what we want to see.

**Smaller but load-bearing:** the chain's `BroadcastTx` response omits the
txhash, so the relayer now computes it locally (`sha256(tx_bytes)`, the
CometBFT convention) — which is how we could find the verify tx in the node
logs at all.

**September 28 — the engine work.** While the contract story above was the
visible part, four testnet-blocking items landed in the consensus/execution
path itself:

- **Atomic block commit.** `state_root` and block payload now commit in one
  store transaction at `finalize_block`. Previously the chain could execute
  speculatively at `certify()` — pre-finality — which is precisely the
  behaviour that made the old state unmigratable (below).
- **Decoupled execution.** Block execution no longer blocks the consensus
  voter.
- **Mempool v2.** Deduplication, peek-without-pop, remove-on-commit, and
  recheck — the difference between "transactions go in" and "transactions
  behave."
- **Bounded peer-pushed payloads.** Height-window and byte caps on
  unsolicited payloads from peers — closing a memory-DoS surface before
  strangers can find it.

## The 48 hours nobody plans for

We attempted a state-preserving binary swap: new image, old volumes. The
nodes stalled with `No stored payload for resume height`. Root cause: the old
binary executed at `certify()` and its per-node height counters drifted, so
the tip block on every node had no finalized payload record to resume from.
The honest options were an ugly migration shim or a clean regenesis. We took
the wipe — after archiving all four node volumes.

The rebuild is where the week stopped being a war story and started being
evidence:

- **Genesis is deterministic.** The re-genesised chain reproduces the Layer
  root contract byte-for-byte — same `code_id` 1, same checksum
  (`a1eda8bd…32236e`), same creator and contract address. We verified this
  by restoring the old node-0 volume into a probe container and querying
  `jc-codes` against both chains.
- **IBC rebuilt in ~1.5 minutes.** The scripted link (`ibc-rebuild.ps1`)
  stood up Osmosis `08-wasm-6` / `connection-3` / `channel-3` against
  JunoClaw `07-tendermint-0` / `connection-0` / `channel-0`, and a 1000
  ujclaw transfer landed as a voucher on the Osmosis side.
- **The relay daemon is now a daemon.** It runs unattended with
  catch_unwind per tick, exponential backoff, a heartbeat log, and a health
  endpoint serving stats JSON on `127.0.0.1:18080` — monitoring hooks, not
  hope.

None of the MAYO state above needed rebuilding — those were transactions,
not fixtures — but every artifact you'd want for a re-run is pinned with
SHA-256 checksums in `backups/junoclaw-state-inputs`.

## The key we lost

We also lost a funded relayer key (`osmo1aq995…`) that had been generated
ad-hoc and never persisted anywhere. The chain didn't lose it; our process
did. In the same week we found validator `keys.json` files — BLS and ed25519
private keys — tracked in a public branch, and confirmed they're additionally
derivable from a seeded RNG (`ChaCha8Rng::seed_from_u64(0)`). Devnet keys
being public is a feature for reproducibility; it's a liability only if we
pretend otherwise.

So now there is a key-management plan (`docs/KEY_MANAGEMENT.md`) with actual
teeth: `testnet-keys/` untracked and backed up, the Osmosis keyring archived,
env-var-only key handoff in the relayer launcher, and hard requirements for
testnet — DKG with real entropy for validators, a keyring-held deployer key,
and a dedicated low-balance relayer key with a backed-up mnemonic.

## The bug that wasn't the chain

For most of a day, every smart query to the credential contract failed with
`Error parsing into type jclaw_credential::msg::QueryMsg: Invalid type` —
while executes against the same contract worked fine. We traced the message
through the relayer, the gRPC handler, the keeper, the vendored VM and
`serde-json-wasm`, and threw 272 corrupted variants of the query at the
contract locally. None reproduced it.

The answer came from controlled inputs on the live chain: sending the string
`"x"` returned the error you'd get for a bare `x`. PowerShell was stripping
the `"` characters out of inline arguments before the relayer ever saw them.
`{"list_members":{}}` arrived as `{list_members:{}}`. Executes worked only
because MAYO vectors are too big for the command line and always went
through `--msg @file`.

The fix is one validation: the relayer now checks `--msg` is JSON before
broadcasting and says, in plain words, that the shell probably ate your
quotes. The lesson is older than this chain: when every layer you own checks
out, test the layer you don't.

## Are we "PQC solid"?

Honest scorecard, because that's the only kind worth publishing:

**What's genuinely demonstrated:**
- MAYO-2, MAYO-3 and MAYO-5 verify on-chain at viable gas, and a tampered
  signature is rejected. Not simulated — real txs in finalized blocks.
- The trust model is coherent: the contract stores a *hash* of the PK, so a
  member can't swap keys after the fact; `VerifyMayoAttestation` re-derives
  the hash and rejects mismatches before touching the expensive verifier.
- The architecture degrades gracefully — in-contract verification today,
  precompile upgrade path later, without changing the contract interface.
- **PQ moved from the contract layer to the protocol layer.** Since this
  draft: `PubKey::HybridSecp256k1Mayo` landed in the chain's auth module —
  an account whose spend requires BOTH a valid secp256k1 signature AND a
  valid MAYO-1/2/3/5 signature over the same sign-doc hash. Security is
  max(classical, PQ): the account survives a secp256k1 break *and* a MAYO
  flaw. Chain-side verify is native (the same pure-Rust port, vendored into
  the node), so the PQ half costs +25k gas, not wasm-metered hundreds of
  thousands. Unit-verified against a reference-impl MAYO-2 vector; a live
  hybrid spend on devnet is the remaining proof — the signer tooling needs
  the C reference impl, so it runs in a container.

**What we have NOT proven:**
- **Test vectors, not adversarial inputs.** We verified KAT vectors for
  MAYO-2/3/5 and rejected one tampered signature. Nobody has fuzzed
  `junoclaw-mayo-verify` or thrown malformed signatures at the length/parse
  checks.
- **No external audit.** The verifier is our Rust port. MAYO won its NIST
  slot on the reference implementation's strength; "we ported it and it
  passes KATs" is a milestone, not a security property.
- **Gas under adversarial load.** Honest-path gas is now measured
  (309k / 400k / 726k for MAYO-2/3/5). What we haven't measured is the
  worst case a hostile caller can construct — a per-tx gas cap is still the
  main thing standing between an oversized execute and a stall.
- **gRPC completeness.** `Simulate` still returns `Unimplemented`.
  `GetTx` landed since the first draft — `tx-sender get-tx` now returns
  height, code, gas and events for a committed tx (verified live at
  height 71 870). One gap closed, one to go.

**New since the first draft:**
- Finalize-driven atomic commit means the execution layer can't fork itself
  the way the old speculative path did — and we have the re-genesis scar
  tissue to explain why that mattered.
- Deterministic genesis is *verified*, not assumed — same code, checksum,
  creator and contract address across a full wipe.
- One live lesson in operational reality: a funded key was lost to process,
  not to crypto. It's in the scorecard now, not under the rug.
- **Bulk backfill landed.** The B5 fault tests exposed that catch-up was a
  lazy one-RTT-per-block digest crawl. Nodes now learn the peer tip from any
  observed payload (even rejected pushes), and a 250 ms tick requests every
  missing height between the executed tip and that observation in
  ≤64-height range fetches — parallel, not sequential. Payload retention
  went 1k→64k heights so peers can actually serve deep gaps. Unit-tested;
  the live 10k-block outage test is the verification that remains. What it
  is NOT: snapshot state-sync — a brand-new validator still replays every
  block. That's the honest remainder.

So: competent enough to claim "first PQ signature verified on a Cosmos-style
chain we control end-to-end," and — as of this week — honest enough about
operations to survive our own devnet. Not competent enough — yet — to tell
strangers to trust it.

## The DAO vote with zero votes

The proposal is open and the tally is empty. This is worth naming plainly:
**it is a bootstrap problem, not a code problem.** A credential graph with
one genesis member and no voters isn't insecure — it's uninhabited. The MAYO
attestation plumbing exists precisely so membership can grow by signed proof
rather than by whoever shows up first.

What would actually get votes: the Bud flow working end-to-end (it now does),
a human-readable way to see what you're attesting to (doesn't exist yet),
and — the boring one — people. That's a launch problem, and it's on the
checklist below, not in this article's "bugs" column.

## What stands between here and testnet

Ordered roughly by how much it would embarrass us in public. ~~Struck~~ items
shipped since the first draft:

1. **`Simulate` (and `GetTx` shipped).** `GetTx` is live — a committed tx
   is now queryable by hash with height/code/gas/events. `Simulate` is the
   remaining half: a chain where you can't dry-run still makes every
   integration partner's first hour miserable. Now the top item.
2. **wasm gas limits + metering audit.** One malformed or enormous execute
   shouldn't stall a block. Honest MAYO-5 is 726k gas; the open question is
   crafted inputs, not valid ones.
3. **Fuzz the PQ stack.** `junoclaw-mayo-verify`, the pk-hash path, Bud weight
   arithmetic — malformed inputs, boundary lengths, wrong variants.
4. **External eyes on the verifier.** Even a focused review of the MAYO port
   against the NIST reference; full audit can wait for more surface.
5. **~~Consensus atomicity + mempool + peer bounds.~~** Shipped this week —
   atomic commit, decoupled execution, mempool v2, bounded peer payloads.
6. **~~Bulk catch-up.~~** Height-range backfill shipped 2026-09-29; live
   10k-block verification + snapshot state-sync for validator joins are the
   remaining pieces.
7. **PQ auth at protocol level.** Hybrid secp256k1+MAYO accounts landed
   chain-side (above). Still to do: signer tooling for live spends, hybrid
   validator identities, and the hard one — a PQ or hybrid path for the
   consensus certificate itself (no standardized threshold PQ scheme exists;
   the realistic design is BLS-threshold AND k-of-n PQ sigs).
8. **Key ceremony.** The devnet's seeded keys are now a documented choice,
   not an accident — testnet needs the DKG path (`OsRng`, per-operator
   shares), a real deployer key, and keyring-held relayer keys with
   backed-up mnemonics. `docs/KEY_MANAGEMENT.md`.
9. **Genesis ceremony + onboarding.** We now know re-genesis reproduces
   state deterministically; `VALIDATOR_ONBOARDING.md` still needs a dry run
   by someone who didn't write it. Faucet story for `ujclaw`.
10. **Relayer supervision.** The daemon is crash-safe with a health
    endpoint; what remains is a process manager in front of strangers
    (compose `restart:` or a service unit — both sketched in
    `ICS20_LOCAL_OSMOSIS_DEMO.md`).
11. **A block explorer view of the above.** Even a thin status page:
    height, peers, last finality, contract list. "Trust us, check the node
    logs" is not a vibe.

## Is it novel? Is it solid?

Novel, honestly graded: a threshold-BLS consensus chain (not Tendermint, not
CometBFT) that natively speaks wasmd, runs a BLS light client *on* a
counterparty chain for IBC verification, and just verified a NIST
post-quantum signature inside a contract — that combination does not exist
elsewhere, as far as we know. Individually every piece has prior art; the
composition is ours.

Solid, honestly graded: the *crypto* is sound but unaudited; the *consensus*
finalized ~535k devnet heights on the old generation and, more importantly,
survived an audit of its own failure mode — we now *know* what speculative
execution costs and the new binary doesn't do it. The *IBC path* is proven
but narrow (one channel, one relayer, one counterparty); the *operational
surface* — querying, observability, onboarding — is still where we're
thinnest, because devnets optimize for "does it work" and testnets get
judged on "can I see why it failed."

The right summary sentence for this week: **the hard parts keep landing; the
boring parts are now the bottleneck — and the boring parts are what we're
shipping.**

Next article, probably: `Simulate`, so dry-runs stop returning
`Unimplemented` — and whatever breaks first when the fuzzer starts.
