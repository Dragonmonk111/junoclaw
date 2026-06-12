# Bud Invitation Template — The 13

*Use this to invite a candidate into the JunoClaw root ring (depth-1 bud).*
*Companion to `docs/DIMI_HANDOFF_PLAN.md` (the full onboarding mechanics).*

---

## Short DM (first contact)

> Hey [name] — JunoClaw is moving from a single founder key to a 13-member
> root ring that holds governance + infra stewardship for the agent economy.
> Each seat is **soulbound** (bound to one wallet, non-transferable) and
> carries real voting weight: 7/13 for normal proposals, 9/13 for
> constitutional ones (code upgrades + weight changes).
>
> I'd like to offer you bud #[n] of 13. No token to buy, no cost — it's a
> trust seat. Are you in? If yes I'll send the encrypted onboarding pack.

---

## Full invitation (once they say yes)

### What you're being offered

A **depth-1 seat** in the JunoClaw root ring — one of 13. This is a governance
and infrastructure stewardship role, not a token purchase.

**What the seat gives you:**

- **Governance weight** — ~769/10000 bps. Normal proposals need 7/13;
  constitutional proposals (`CodeUpgrade`, `WeightChange`) need 9/13.
- **Infra co-stewardship** — shared access to deploy tooling, testnet ops,
  and server infrastructure (root ring only).
- **The right to bud** — you can later sponsor your own branch (depth-2+),
  which grants governance weight to your invitees.

**What the seat requires:**

- One wallet address you control and will keep (the seat is soulbound to it).
- Participation in governance votes.
- The seat rule: **if you ever sunset, you must pass your bud first.** No seat
  is ever lost; the tree always has 13 active members.

**What it is NOT:**

- Not a token you buy or sell. Not tradeable. Not an investment.
- Not a transfer of custody — your key is born on your machine and never
  touches anyone else's (see onboarding below).

### Onboarding (encrypted, zero custody chain)

We use `tools/bud-seal` (X25519 + ChaCha20-Poly1305) so nothing sensitive
travels in the clear and no one ever holds your private key.

```bash
# 1. You generate a keypair on YOUR machine:
cargo run --bin bud-seal -- keygen --name [your-handle]
#    -> [your-handle].key  (PRIVATE — never share, never leaves your machine)
#    -> [your-handle].pub  (share this with me)

# 2. Send me [your-handle].pub plus the juno1... address for your seat.

# 3. I seal your onboarding pack to your public key and send the .sealed file.

# 4. You open it with your private key:
cargo run --bin bud-seal -- open --key [your-handle].key --file pack.sealed --out pack.md
```

The onboarding pack contains your role brief, the current contract map, and
the infra access details for the root ring.

### Next step on-chain

Once we have all 13 addresses, a `WeightChange` proposal distributes weight
from Genesis to the 13. Genesis drops to a symbolic 3 bps and loses voting
power. From that point, **the 13 are the governance.**

---

## Seat tracker (fill as candidates confirm)

| # | Handle | juno1 address | .pub received | sealed sent | confirmed |
|---|--------|---------------|---------------|-------------|-----------|
| 1 | Dimi | `juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt` | — | — | tentative |
| 2 | | | | | |
| 3 | | | | | |
| 4 | | | | | |
| 5 | | | | | |
| 6 | | | | | |
| 7 | | | | | |
| 8 | | | | | |
| 9 | | | | | |
| 10 | | | | | |
| 11 | | | | | |
| 12 | | | | | |
| 13 | | | (reserved — Genesis may re-enter here if invited) | | |
