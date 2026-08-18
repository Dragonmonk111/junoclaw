# A46 — Response to A45 NO Voters

## What the NO vote likely meant

A45 was rejected. We assume the NO voters (especially juno-ai) had at least one of these concerns:

1. **Scope was still too broad** — architecture ratification + sidecar model + BN254 question all in one.
2. **Not enough real testnet data** — only 2 batches settled, no 4-node mesh running.
3. **Mainnet commitment felt premature** — gating language was still there.
4. **Validator sidecar ask was too soon** — no product proof yet.
5. **BN254 still felt like a mandate** — even as an "open question" it took up space.

A46 addresses each one explicitly.

---

## How A46 Fixes Each Concern

### 1. Scope too broad?
**A46 removes everything except one thing:** run the existing testnet contract for 30 days.
- No architecture ratification.
- No production target.
- No token, no funds, no membership changes.

### 2. Not enough testnet data?
**A46 is designed to produce 100+ batches of data.**
- The relayer already works (2 batches settled).
- The pilot runs for 30 days specifically to gather data.
- Success criteria are public and testable.

### 3. Mainnet commitment premature?
**A46 explicitly gates mainnet.**
- The proposal text states: "Mainnet deployment, validator sidecars, and BN254 remain separate, future proposals."
- No mainnet contract is mentioned.
- No chain upgrade is mentioned.

### 4. Validator sidecar too soon?
**A46 removes sidecars entirely.**
- The pilot uses a DAO-appointed 4-node testnet set.
- Sidecars are deferred to A48, after the pilot succeeds.

### 5. BN254 still a concern?
**A46 removes BN254 entirely.**
- Not mentioned.
- Not a question.
- Not an open ask.

---

## If You Voted NO on A45, Consider This

A46 is not "A45 with different wording." It is a fundamentally different proposal. The only ask is:

> Let the builders run the existing testnet code for 30 days, settle 100+ batches, and produce a public report.

If that works, the DAO can decide what to do next. If it doesn't work, the DAO does nothing further. The risk is testnet gas only (paid by builders, not the DAO).

---

## What Would Make You Vote YES?

A YES on A46 does not commit you to:
- mainnet
- sidecars
- BN254
- architecture ratification
- any future proposal

It only says: "Run a 30-day testnet pilot and report back." If that is something the DAO can support, vote YES.
