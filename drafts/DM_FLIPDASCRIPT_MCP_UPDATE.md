# DM to FlipDAscript — Cosmos MCP update

*Draft reply to FlipDAscript's Cosmos-chat feedback (message types, install guide, on-chain skill repo "on the hub"). Send via Cosmos chat / Discord / wherever you normally reach them.*

---

Hey FlipDAscript,

Following up on your three points — shipped all of it, here's exactly what's live now:

**1. "Can it compose sign and broadcast any message types?"**

Short answer: yes, but gated, not wide open. Two layers:

- **Dedicated typed tools** for the common cases you're probably actually hitting: `vote_on_proposal`, `delegate_tokens`, `undelegate_tokens`, `redelegate_tokens`, `withdraw_rewards`. These sit alongside the existing `send_tokens`, `execute_contract`, `ibc_transfer`, etc. — 28 tools total now.
- **`compose_and_broadcast_msg`** — takes any Cosmos SDK message type URL + JSON value, so it genuinely can sign anything the chain accepts. But it's **disabled by default**. The operator has to explicitly set `JUNOCLAW_ALLOWED_MSG_TYPES` (a comma-separated allowlist of type URLs) before it'll broadcast anything. Reasoning: an always-on "sign literally anything" tool is a bad idea for an AI-driven signer — if the calling model gets prompt-injected or just hallucinates a bad message, fail-closed-by-default limits the blast radius to whatever the operator explicitly opted in ahead of time. Same posture as the admin kill-switch we already ship.

**2. Installation guide with easy examples**

Added `mcp/QUICKSTART.md` — 5-minute path: install → wire into Windsurf/Claude/Cursor/Cline/Zed/ChatGPT/local-GPU → first read-only query → wallet enrollment → first signed tx → contract execute → IBC transfer → governance vote → DAO scaffold. Linked at the top of the main README.

**3. On-chain skill repo "on the hub"**

Confirmed the hub = Juno for this, so it's live on **mainnet** now: `skill-registry` (codeId 5145, `juno1wp5fpcxukgjm9ag9u97a7yf7rtwe47m9h93vk7nfrnel9xelt6zs4hj09s`) — a CosmWasm contract where any interchain dApp can publish a pointer + SHA-256 hash to its own operating manual. Permissionless to publish, first-publisher owns the name, admin can resolve disputes. Any agent, on any chain, can query it via plain RPC (no wallet needed) and get pointed straight at the manual for any registered dApp.

JunoClaw self-registered `junoclaw-cosmos-mcp` as the first entry. `get_dapp_skill` / `list_dapp_skills` query tools are in the MCP now — any client can pull it directly.

Also shipped a **second-approval gate**: every fund-moving tool stages a tx and returns a preview + confirmation_id. Nothing broadcasts until a separate `confirm_transaction` call. Single-use, 5-min TTL, required by default. Makes the MCP safe to hand to an AI agent on mainnet.

Mainnet txs: store `56F71E...`, instantiate `1457C5...`, self-register `09E3A4...` — all on `juno-1`.

Repo + docs: `github.com/Dragonmonk111/junoclaw` — MCP lives at `/mcp`, `npm install @junoclaw/cosmos-mcp`.

Appreciate the push on all three — good feedback, made the tool better.
