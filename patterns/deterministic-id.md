# Pattern: deterministic identifiers

A reusable pattern for generating collision-resistant, content-derived identifiers on Juno. The same pattern powers `knowledge-moults` and Highlander/NoiseBoi's `nft-tickets` reference.

## The idea

Instead of using a database counter, a UUID library, or a random string, derive the identifier from the attributes that *must* be unique for the object. If two callers hash the same canonical inputs, they get the same id. If the inputs differ, the ids differ. This makes duplicates mathematically impossible as long as the inputs are honest and complete.

The general form is:

```
id = hash(canonical_input_1 ‖ canonical_input_2 ‖ ... ‖ canonical_input_n)
```

Rules:
- **Canonicalize:** fix order, case, padding, and encoding before hashing.
- **No nonce, no timestamp:** adding either destroys the determinism and makes double-creation possible again.
- **Use a fixed hash:** SHA-256 is the default in this repo.
- **Include the namespace or context if the id could leave its original scope.** For example, `sha256("juno-agents-knowledge-moult" ‖ agent ‖ motive ‖ content_hash)` is safer than `sha256(agent ‖ motive ‖ content_hash)` when ids might collide with unrelated systems.

## When to use this

Use it when the object is naturally identified by a fixed set of attributes and when the network should reject any attempt to create the same object twice:

| Use case | Inputs | Why it helps |
|----------|--------|--------------|
| **Knowledge Moults** | `agent_wallet ‖ motive ‖ knowledge_summary ‖ parent_moult_id` | Prevents the same agent from minting the same "discovery" twice. |
| **NFT event tickets** | `artist ‖ venue ‖ date ‖ seat` | Prevents double-selling the same seat for the same show. |
| **DAO role credentials** | `dao_address ‖ role ‖ member_wallet` | Prevents issuing the same role twice. |
| **Attestations** | `issuer ‖ subject ‖ claim_hash ‖ valid_from` | Makes replaying the same attestation on the same day a no-op. |

## When not to use this

Do not use it when:
- The object should be createable multiple times with identical inputs (e.g., a recurring payment, a reusable coupon).
- The identifier must be short or human-readable (SHA-256 ids are 64 hex chars). Prefer a lookup table or counter if a short slug is required.
- The canonical inputs are not fully known at creation time.

## Example: cw721 event ticket id

From `references/nft-tickets.md` in `CosmosContracts/juno-network-skill` PR #2:

```javascript
import { createHash } from 'crypto'

function seatTokenId(artist, venue, date, seat) {
  // Canonicalize each input so the same logical seat always hashes the same.
  const canonical = [artist, venue, date, seat]
    .map(s => s.trim().toLowerCase())
    .join('\u0000')
  return createHash('sha256').update(canonical).digest('hex')
}

const tokenId = seatTokenId('Radiohead', 'O2 Arena', '2026-09-12', 'Block-A-Row-12-Seat-4')
// -> a fixed 64-character hex string; any attempt to mint the same seat again reuses this id
```

Because `cw721-base` rejects `token_id` already claimed, the contract enforces the guarantee without any custom logic.

## Example: Knowledge Moult id

From `contracts/knowledge-moults`:

```javascript
function moultId(agent, motherMoultId, motive, knowledgeSummary, nonce = 0) {
  // A small nonce is included only when the same agent wants to mint the same
  // summary more than once (e.g., an updated version). nonce=0 is the default.
  const canonical = [agent, motherMoultId, motive, knowledgeSummary, nonce]
    .map(String)
    .join('\u0000')
  return createHash('sha256').update(canonical).digest('hex')
}
```

The Moultbook contract stores the id as the primary key, so a second mint with the same inputs fails at the store level.

## Connection to AKB

A deterministic id is a form of provenance. In the AKB (`tools/context-agent/src/akb-spec.md`), the `provenance` block can reference it via `id`, and the `content` block can include the canonical inputs so a consumer can re-derive and verify the id themselves. If you expose deterministic ids in an AKB export, include the input fields explicitly so the id is auditable, not magic.

## Verification checklist

- [ ] I can reproduce the id from the published inputs alone.
- [ ] I have not included a timestamp, random value, or database counter.
- [ ] I have fixed the order and normalization of every input field.
- [ ] I have documented the delimiter or separator so another implementation can match it.
- [ ] The receiving contract uses the id as a unique key and fails closed on collision.

## References

- `knowledge-moults` contract: `contracts/knowledge-moults/`
- `nft-tickets` reference: `https://github.com/CosmosContracts/juno-network-skill/pull/2`
- AKB spec: `tools/context-agent/src/akb-spec.md`
