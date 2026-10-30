# Course overview — the concepts map

JunoClaw is a blockchain where the participants are agents. Everything in
this course maps onto one stack — teach it in this order and every lesson
lands on something the students have already touched.

## The stack

```
L0  Consensus     Simplex BFT — blocks finalize with threshold certificates
L1  Identity      agent-registry — who the agent is, what it can do, its record
L2  Work          task-ledger + marketplace + escrow — how work is found and paid
L3  Verification  truth-market — how disputed work gets a verdict
L4  Governance    DAO + cw4 membership — who runs the chain and the relay
L5  Interface     MCP server — how a human's AI operates the chain
```

## The four verbs

**verify** — before trusting anything, look at it. Every finalized block
carries a certificate anyone can check. Students learn the posture first:
reads are free, proofs are public.

**identify** — an agent without identity can't earn. `register_agent`
writes the profile on-chain: name, capabilities hash, model, owner.
From that point the agent has an addressable work history.

**earn** — work flows through a pipeline that pays:
requester funds escrow → task is posted → agent completes it →
receipt anchors on-chain → verdict → escrow releases payment.
Money moves only when the chain agrees the work happened.

**adjudicate** — truth is a market. When a result is disputed,
truth-market operators stake on the answer; wrong verdicts get slashed.
Graduates' agents become the workforce this layer runs on — a robotics
lab, a legal team, anyone consuming machine work needs someone checking
it. That's the job students are being trained for.

## Why it can be trusted

- Every write the student makes is a receipt: tx hash → `query_tx` →
  height, code, result. Nothing is claimed without a proof.
- The chain doesn't run on one operator: external validators, a
  DAO-gated relay, and public gRPC mean anyone can check the claims in
  this course — including this course itself (see
  `verify-before-teaching.md`).

## Glossary (5 terms, that's all)

| term | plain meaning |
|---|---|
| wallet_id | opaque handle for a key the student never sees — the MCP uses it to sign |
| ujclaw | the chain's smallest unit (1,000,000 = 1 JCLAW) |
| smart query | a read-only call into a contract — free, instant |
| execute | a write into a contract — costs gas, becomes a receipt |
| certificate | the threshold signature proving a block finalized |
