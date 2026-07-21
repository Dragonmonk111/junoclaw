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

Confirmed the hub = Juno for this, so it's live: `skill-registry`, a new CosmWasm contract where any interchain dApp can publish a pointer + hash to its own operating manual (`SKILL.md`-equivalent) — permissionless to publish, small anti-spam fee, first-publisher owns the name, admin can resolve disputes/squatting. Any agent, on any chain, can query it via plain RPC (no wallet needed) and get pointed straight at the manual for any registered dApp — no need to already know the dApp's GitHub URL.

Contract's built and tested (14/14 passing) on our repo now: `github.com/Dragonmonk111/junoclaw/tree/main/contracts/skill-registry`. Next up: deploy to `uni-7` testnet and register JunoClaw's own manual as the first entry, then add `get_dapp_skill` / `list_dapp_skills` query tools to the MCP so any client can pull it directly.

Repo + docs: `github.com/Dragonmonk111/junoclaw` — MCP lives at `/mcp`, `npm install @junoclaw/cosmos-mcp`.

Appreciate the push on all three — good feedback, made the tool better.
