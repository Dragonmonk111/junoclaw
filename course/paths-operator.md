# After graduation — the operator track (agent → DAO → validator)

*For students who finished the capstone and want to go deeper: how a
registered agent's operator becomes infrastructure.*

The course taught your agent to work in the economy. This page maps the
path from *participant* to *operator* — the people and agents who run
the network itself.

## The ladder

```
agent registered + working          (lesson 3–5 — done)
        ↓
history + trust_score grow          (keep operating — every task counts)
        ↓
DAO membership via cw4              (governance weight — where direction is decided)
        ↓
observer seat                       (run read infrastructure — verify, don't trust)
        ↓
seat attestation                    (on-chain proof your node serves correctly)
        ↓
relay / validator candidacy         (consensus participation — the top of the ladder)
```

## Each rung, concretely

### 1. Keep working — your history is your application

Nothing on this ladder is applied for with a form. `total_tasks`,
`successful_tasks`, `trust_score`, and verdict history ARE the
application — that's why lesson 5's receipts matter more than any CV.
DAO admission looks at exactly this record.

### 2. DAO membership (cw4)

Membership lives in the `cw4-group` contract — a member list the DAO
core reads for voting weight. Check your standing:

```
Query the cw4-group contract on junoclaw-g1:
{ "member": { "addr": "<your-address>" } }
```

Validator registration alone does NOT grant voting power — membership
is a separate, deliberate act. That's the design: running
infrastructure ≠ governing.

### 3. Observer seat — the entry point to infrastructure

Run an observer node: sync state, serve reads, verify certificates.
It's the lowest-commitment way to operate real infrastructure — no
consensus responsibility, but you're part of the verifiable fabric.
This is also what powers the "second independent endpoint" exercise in
lesson 2 — observers are why cross-verification is possible at all.

### 4. Seat attestation — proof you serve correctly

The `seat-attestation` contract (wired to cw4 + DAO core) records
attestations that a seat performs its role. Check the live config:

```
Query seat-attestation on junoclaw-g1 with { "get_config": {} } —
Contract: juno1rhlajgcuufadqwhjdw29fh4tged8cyxrtry4lt9luv54ex6emvdq44t7qm
What does it require, and which contracts govern it?
```

Attested seats are how the network knows who's real — it's the
infrastructure analogue of your agent's work history: receipts, not
claims.

### 5. Relay / validator

The top of the ladder: relay operation (DAO-gated admission — the relay
reads the cw4 membership contract for its access list) and validator
seats (consensus participation — your key signs the threshold
certificates you verified back in lesson 2). Both are admission-by-
attestation: your track record + DAO standing, not self-nomination.

**The key discipline tightens at this rung.** The sealed-signer pattern
from lesson 3 scales up — the material always stays behind a boundary,
the handle travels instead:

- **Hot/cold separation.** The seat's hot key (ed25519 + BLS share +
  MAYO share) signs consensus traffic. Your cold `juno1` governance key
  holds DAO authority. The `seat-attestation` contract binds them
  on-chain — rotation is itself an attested act, with old keys recorded
  in key history. The chain remembers which key served which seat when.
- **`keys.json` never leaves its host.** Ceremony material obeys the
  same rule: `dealer.seed` is deleted once dealing completes, and
  private dealing files travel only to their named recipients — never
  by glob, never over an open directory.
- **One identity = one live process, everywhere.** Two processes holding
  one key produce equivocating signatures — the network treats it as a
  fault, because it is one.
- **No single key controls the chain.** Finality is a threshold of
  independent signatures — compromising one seat is a seat failure,
  never a chain failure. That's why the ladder exists: trust is spread
  across operators precisely so that no one, including you, is a single
  point of failure.

## Why this path exists at all

The economy needs three kinds of participants, and this course seeds
all three:

- **agents who work** — task pipeline, escrow, earning
- **operators who adjudicate** — truth-market seats, verdict history
- **operators who run** — observers, relay, validators, governance

Your capstone record is the same document that opens each door —
verified work → adjudication seats → infrastructure seats. The chain
remembers; the ladder climbs itself.
