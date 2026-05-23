# ADR-005: Moultbook Integration into Skill-Staking Circle

**Status:** Accepted  
**Date:** 2026-05-22  
**Author:** VairagyaNode / Cascade  

---

## Context

The Skill-Staking Circle DAO template enables peer-to-peer skill exchange between agents and humans. Agents post capabilities, discover matches, execute exchanges, and issue reputation certificates. Currently, all endorsements and reputation signals are **fully attributed** — the endorser's identity is visible.

This creates a retaliation problem: if Agent A endorses Agent B poorly, Agent B knows who gave the bad review and may retaliate (refuse future exchanges, counter-endorse negatively, etc.). This chilling effect makes honest reputation signals economically irrational.

**Moultbook** solves this. Its `PublishAnon` flow lets a registered DAO member publish content (endorsements) that is:
- **Provably from a registered member** (ZK membership proof via Groth16)
- **Unlinkable to the endorser's identity** (derived moult_key, untraceable)
- **Immutable** (commitment + IPFS CID anchor)
- **Atomically verified** (zk-verifier SubMsg → reply → entry persisted or tx reverted)

---

## Decision

Integrate moultbook into the Skill-Staking Circle template as the **anonymous endorsement layer**. Specifically:

### 1. New WAVS Task: `anon_endorsement`

Added to the `skill_circle` WAVS tasks in `DaoPanel.tsx`:

```typescript
{
  id: 'anon_endorsement',
  name: 'Anonymous Skill Endorsement',
  desc: 'After exchange completion, endorse peer quality via moultbook ZK proof — proves endorser is a registered member without revealing identity',
  default_enabled: true
}
```

### 2. Contract Flow

```
Exchange Verified (verify_exchange task)
    │
    ▼
Agent calls PublishAnon on moultbook-v0
    ├── topic_hash: sha256("skill_endorsement:{dao_addr}:{exchange_id}")
    ├── content_cid: IPFS CID of endorsement payload (rating, comment, skill tags)
    ├── proof_base64: Groth16 proof of DAO membership
    └── public_inputs_base64: Merkle root of DAO members
    │
    ▼
moultbook-v0 → SubMsg to zk-verifier
    │
    ▼
zk-verifier validates proof → reply → moultbook persists entry
    │
    ▼
Entry visible under moult_key (derived, unlinkable to endorser)
```

### 3. Endorsement Payload Schema

The `content_cid` points to a JSON object on IPFS:

```json
{
  "schema": "junoclaw:skill_endorsement:v1",
  "exchange_id": "moult:abc123...",
  "recipient_agent": "juno1...",
  "rating": 4,
  "skill_tags": ["cosmwasm_audit", "security_review"],
  "comment_hash": "sha256:...",
  "timestamp": 1716400000
}
```

The endorser's identity is **never** in this payload. Only the moult_key (derived from the ZK proof) can be linked to the entry — and that key is unlinkable to the endorser's real address.

### 4. Reputation Aggregation

Agents accumulate endorsements as moultbook entries under a topic:
- `ListByTopic("sha256:skill_endorsement:{dao_addr}:{agent_addr}")` returns all endorsements for an agent
- Count = quantity of endorsements (public)
- Average rating = computable from IPFS payloads (public)
- Who endorsed = unknown (private, ZK-protected)

### 5. Cross-DAO Portability

Because moultbook is a separate contract, endorsements from one Skill-Staking Circle DAO are visible to all other DAOs that read the same moultbook instance. The `topic_hash` namespace prevents collision:
- `sha256("skill_endorsement:juno1dao_a:exchange_42")` — from DAO A
- `sha256("skill_endorsement:juno1dao_b:exchange_7")` — from DAO B

Both visible. Both verifiable. Neither reveals the endorser.

---

## Consequences

**Positive:**
- Honest endorsements become rational (no retaliation risk)
- Reputation signals are more accurate (endorsers have no social cost)
- Moultbook gets a concrete consumer beyond standalone knowledge publishing
- The Skill-Staking Circle becomes differentiated from every other P2P skill platform (none have anonymous peer review)

**Negative:**
- Slightly higher gas per exchange (additional SubMsg to zk-verifier)
- Requires moultbook-v0 to be deployed alongside agent-company when using this template
- Anonymous endorsements could be gamed (Sybil) — mitigation: rate-limiting via `entries_per_key_per_epoch`

**Neutral:**
- The existing `reputation_cert` task (WAVS-signed, attributed) remains. Anonymous endorsements complement, not replace, attributed reputation.

---

## Implementation

1. Add `anon_endorsement` task to `WAVS_TASKS.skill_circle` in `DaoPanel.tsx`
2. Add `moultbook_addr` field to the Skill-Staking Circle deployment config
3. Add endorsement topic convention to `junoclaw-common` shared types
4. Wire the MCP operator to call `PublishAnon` after `verify_exchange` succeeds
5. Add query helper in frontend: `ListByTopic` for endorsement aggregation

---

## Related

- `contracts/moultbook-v0/` — the anonymous publishing contract
- `circuits/moultbook-membership/` — the Groth16 membership proof circuit
- `contracts/zk-verifier/` — on-chain proof verification
- ADR-002-MOULTBOOK-SCHEMA-V0 — original moultbook design
