# DM to FlipDAscript / Rattadan — DEX Offer + Type-Mismatch Bug

*Re: FlipDAscript's offer to deploy a White Whale-based DEX on Juno (cosmowarp.vercel.app/dex, live at rewynd.vercel.app/dex), and Rattadan's `created_at` type-mismatch bug report during agent creation.*

---

Hey both,

**On the DEX offer** — appreciate it, but we're actually covered here. We already run our own AMM DEX on Juno mainnet:

- `junoswap-factory` (code ID 61) — pair factory, XYK constant-product model
- `junoswap-pair` (code ID 60) — individual pair contracts, 0.30% swap fee, WAVS hooks built in
- Live pairs: JUNOX/USDC, JUNOX/STAKE
- Clean rewrite from scratch — **not** a White Whale fork, no upstream dependency to track

So `rewynd.vercel.app/dex` would be redundant for us unless it unlocks specific pairs or liquidity we don't have. If you know of pairs/liquidity sources not covered by our factory, flag them and we'll take a look — otherwise we'll pass on redeploying a second DEX stack.

**On the `created_at` bug** — good catch, and it turns out the exact same mismatch was live in our own frontend too:

- Backend (`crates/junoclaw-core/src/types.rs`): `created_at: u64` (Unix ms)
- Frontend (`frontend/src/types.ts`): was `created_at: string` (ISO 8601)

Same root cause as what you found: `new Date().toISOString()` in `store.ts` was sending an ISO string where the daemon expects a `u64`. Fixed on our side:

- `types.ts` — `created_at`, `completed_at`, `timestamp` changed from `string` → `number` on `AgentInfo`, `DelegationRecord`, `ChatMessage`, `Task`
- `store.ts` — all `new Date().toISOString()` calls swapped for `Date.now()`

Ran `tsc --noEmit` clean after, no other call sites broke (all the `new Date(x)` render spots already accept epoch-ms numbers fine).

Good validation that we're both hitting the same interface contract in parallel — worth double-checking any other `string`-typed timestamp fields on your end that cross a Rust `u64` boundary, since this pattern likely repeats.

Repo: `github.com/Dragonmonk111/junoclaw`
