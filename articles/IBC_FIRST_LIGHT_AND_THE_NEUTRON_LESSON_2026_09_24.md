# First Light Over IBC — and Why the Neutron Attack Can't Happen Here

*September 24, 2026*

Two days ago the Cosmos Hub halted — not because Tendermint failed, but because
Neutron's on-chain governance did exactly what it was designed to do, for whoever
could afford the votes. While that was still being untangled, we quietly crossed
a milestone of our own: the first live IBC handshake between JunoClaw and a real
Cosmos chain, verified end-to-end by the BLS light client.

This article is about both. The engineering that made the handshake work, and the
reason the attack that just drained ~$9.4M from Neutron is structurally
impossible on JunoClaw.

## What just shipped

JunoClaw now relays real IBC messages to Osmosis. Not a mock, not a test harness —
a full `connection → channel → ICS-20` handshake where every counterparty proof
is verified on-chain by the `08-wasm` BLS light client contract.

The flow that now works:

1. `create-client` instantiates the `08-wasm` contract on Osmosis, anchored to a
   finalized JunoClaw height.
2. `conn-try` / `conn-confirm` open the connection — each carries a Merkle proof
   of JunoClaw's `ConnectionEnd`, verified against a BLS-signed consensus state.
3. `chan-try` / `chan-confirm` open the `transfer` channel the same way.
4. `recv-packet` delivers a real ICS-20 token transfer, minting the voucher on
   Osmosis after the contract verifies the packet-commitment proof. `jc-ack`
   then relays the acknowledgement back and clears the commitment on JunoClaw.

That last step is the one that took the longest to land — the voucher
(`ibc/31465B669D9903186E396E991E555F48ECDC3ADFBD3E46BA499258586C19C8F8`) is now
sitting in an Osmosis account, minted against a proof our own keeper had been
writing *wrong*. And because we re-ran the whole handshake on the *persistent*
chain after the upgrade work below, this voucher lives on state that survives a
restart — not a demo that evaporates. Which brings us to the bugs.

## Four bugs worth writing down

**Unfinalized proof heights.** JunoClaw's Merkle proofs are over the committed
state at `state_height`, but the proof is *anchored* at `state_height + 1` — the
block that carries that state root. That tip block isn't finalized yet when you
first ask for it. `assemble_membership_proof` now retries the block fetch until
`proof_height` actually finalizes instead of failing on the unfinalized tip.

**Consensus-state-at-proof-height.** ibc-go will only verify a membership proof
if the client already has a consensus state at that exact height. So every
proof-carrying command now calls `update_client_to(proof_height)` first —
submitting a `MsgUpdateClient` and *waiting for it to commit* via a new
`wait_tx` poll — before it ever sends the handshake message. No more racing the
client ahead of the proof.

**The `omitempty` landmine.** This one is subtle and worth a paragraph. ibc-go
marshals `clienttypes.Height` with `omitempty`, so a `revision_number` of `0` is
*dropped* from the JSON it sudo's into the contract. Our contract declared the
field required, so `VerifyMembership` failed to deserialize with a cryptic
"missing field `revision_number`". The correct fix is `#[serde(default)]` on the
`Height` fields — but re-storing the contract turned out to be impossible: the
gov `MsgStoreCode` for a ~513KB wasm needs ~67M gas and the per-tx cap is 60M.
So we shipped a `HEIGHT_REVISION_NUMBER = 1` workaround in the relayer: a
non-zero revision is never omitted, the field stays present, and the contract's
only check (`header.revision_number == latest_height.revision_number`) stays
consistent. The proper `serde(default)` build is ready to deploy the moment the
gas economics allow.

**The commitment-hash mismatch.** The subtlest bug was in JunoClaw's own
keeper, not the relayer. ibc-go's `CommitPacket` is
`sha256(timeout_ts ‖ rev_num ‖ rev_height ‖ sha256(data))` — it folds the packet
*data* through an inner hash so the preimage is fixed-length and non-malleable.
Our keeper appended the *raw* `packet_data` instead. The stored commitment could
therefore never equal ibc-go's recomputation, and `recv-packet` failed every
time with `couldn't verify counterparty packet commitment`. There is no
relayer-side fix — matching would require a sha256 preimage. The fix was a
one-line change in the keeper, but shipping it meant rebuilding the node image
and re-running the whole handshake, which surfaced a separate problem worth its
own paragraph.

**State that doesn't survive a restart — the upgrade path.** Rebuilding the
image wiped the chain: the node logged `No stored state — initializing from
genesis`. Getting a real *state-preserving upgrade* — the thing every Cosmos
chain does when it ships a new binary — took three separate fixes, and missing
any one of them desyncs or halts the chain:

- **App state.** `slay3rd` was compiled without its optional `rocksdb` feature,
  so it ran on an in-memory store and never wrote `/data/app`. One flag
  (`--features rocksdb`) and the node reloads committed state on boot.
- **The consensus journal.** The Commonware runtime defaulted to a *random temp
  dir* in the container's writable layer — surviving a `restart` but wiped by a
  `--force-recreate`. The journal now lives under the persistent
  `/data/consensus`, so the engine remembers where it was.
- **Resume height.** The consensus node tracked its height in memory and was
  hardcoded to start at `0`, so even with state intact it proposed block 1
  against a persisted height-N app — a `BadBlockHeight` desync. It now resumes
  from the app's persisted height.

The result: `docker compose up -d --force-recreate` with a new image now resumes
finalizing at the correct height, no desync, no halt. That is the actual
halt-and-upgrade / rolling-upgrade path a sovereign chain needs in production —
proven in devnet before it's needed in anger.

**Priced block space.** While the persistence work settled, we also closed the
spam vector: a validator-local `min_gas_price` floor enforced at mempool
admission (`check_tx`), plus a calibrated 100M-gas block cap. Fee-less spam is
now rejected at the door instead of being packed for free — the same
`minimum-gas-prices` semantics Cosmos validators run, but as a first-class part
of the app rather than an afterthought.

## The Neutron lesson — and why it doesn't apply here

The Neutron incident was not a reentrancy bug and not a stolen key. It was
governance working *as designed*:

- **Proposal 9** (an "AI governance experiment") smuggled in 11 messages that
  reassigned the admins of the Astroport and Drop contracts.
- Neutron tallies voting power **at vote close, not at ballot cast**. The
  attacker voted with ~100 NTRN, then bought 31.6M NTRN for ~$20k *after* voting,
  delegated it, and closed the window holding **84.74% of YES**.
- **wasmd lets chain governance reassign any contract's admin.** The attacker
  migrated ten contracts to code they had uploaded with a withdraw-everything
  function and swept ~$9.4M. The stolen ATOM flowed over IBC to the Hub, which
  halted to isolate it.

As one researcher put it: *"there wasn't an exploit per se, just wasmd working
as currently designed."* Every link in that chain is a feature Cosmos sells.

JunoClaw is built so that chain cannot exist:

- **Voting power is not a token.** JunoClaw governance is the agent-company
  member roster — a soulbound, non-transferable credential with cw4-group
  semantics. There is no NTRN to buy on a DEX, no balance to delegate at the last
  second. You cannot purchase 84% of YES for $20k because the thing that would be
  for sale does not exist. Membership is granted and pruned by the trust tree,
  not bought on the market the proposal is about to drain.
- **No tally-at-close vote buying.** Because weight lives in the roster rather
  than in a coin balance, there is no "buy the token after you vote" move. The
  Neutron trick — vote cheap, then acquire the stake that counts — has no
  analogue when the weight was never a liquid asset.
- **Admin/migration is not a coin vote away.** The Neutron drain worked because a
  token-weighted proposal could rewrite contract admins. On JunoClaw, that
  authority sits behind the same soulbound roster — it cannot be captured by
  buying liquidity in the very pools being targeted.
- **Sovereign BLS consensus, not a shared-security hub.** JunoClaw finalizes its
  own state with a BLS aggregate signature over a fixed validator set. A
  governance failure on a connected chain is that chain's problem — it does not
  propagate back and force JunoClaw to halt, the way Neutron's stolen ATOM forced
  the Hub down. IBC is a bridge we verify, not a shared fate.

The deeper point Jae Kwon has been making for years — and that September 22 made
concrete — is that the customization points are the attack surface. Cheap,
tokenized, DEX-purchasable governance is a customization point. JunoClaw's answer
is to make governance a credential you earn, not an asset you buy.

## What's left

The devnet milestone is complete: a persistent sovereign chain that survives a
binary-swap upgrade, priced block space, and the full `connection → channel →
ICS-20` flow against local Osmosis — escrow on JunoClaw, a verified packet
commitment, a minted voucher, a cleared commitment on ack. What remains is
turning a proven devnet into a public network:

- **Harden the relayer loop** — automatic update-client cadence plus packet
  ack/timeout handling, so the bridge runs without babysitting.
- **Ship the `serde(default)` contract build** once the store-gas economics are
  resolved.
- **Point the same machinery at a public testnet** and stand up a persistent
  JunoClaw net with a real validator set — the independently verifiable evidence
  the Juno Agents DAO has asked for before it endorses a direction.
- **Report back to the DAO.** JunoClaw is sovereign — it doesn't need
  permission — but the Juno Agents DAO is where the work becomes legible and
  accountable. The next step is a bounded mandate for the public testnet, with
  Juno remaining the settlement layer and the fallback.

The light client verifies signatures. The architecture verifies incentives. This
week both got a real-world test — one passed on our chain, the other failed on
someone else's.
