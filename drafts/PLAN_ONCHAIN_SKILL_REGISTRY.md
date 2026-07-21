# Plan — On-Chain Skill Registry for Interchain dApps

> Prompted by FlipDAscript's feedback on Cosmos chat: "Let's put up an on-chain repo on the hub, where dApps can publish their skill.md easily. So any agent can easily retrieve the manual of any interchain dApp." This doc scopes what that means, where it can actually live, and how it composes with the existing `CosmosContracts/juno-network-skill` pattern and JunoClaw's `agent-registry` contract.

---

## 1. Clarifying "the hub"

**Cosmos Hub (`gaia`, chain-id `cosmoshub-4`) does not run CosmWasm.** There is no wasmd module on Hub mainnet. So a literal on-chain CosmWasm contract cannot be deployed there today. Two real options:

- **Option A — Juno as the registry chain.** Deploy the skill registry as a CosmWasm contract on Juno (`juno-1`), and make it IBC-queryable (or just publicly queryable via any Cosmos MCP client, since any chain's RPC/LCD is reachable from anywhere). "The hub" then means "the central registry any interchain agent checks," not literally Cosmos Hub the chain. This is almost certainly what a Cosmos-ecosystem-fluent person means when they say "the hub" informally — the term is often used loosely for "a shared place," not the specific `cosmoshub-4` chain.
- **Option B — Actual Cosmos Hub.** Would require either (a) Hub adopting wasmd (a long governance/engineering process, not happening soon), or (b) a native Golang module shipped via a Hub upgrade (far larger lift, not something we control), or (c) an Interchain Security consumer chain anchored to the Hub's validator set specifically for this registry (heavy infra for a registry).

**Recommendation: build Option A.** Deploy on Juno, make it queryable by any agent on any chain via IBC-ICQ or simple cross-chain RPC query (no bridge needed — reading Juno state doesn't require IBC, any client can just query Juno's LCD/gRPC directly). If "the Hub" is meant literally, we table it and revisit if/when Hub gets wasm support.

---

## 2. What already exists vs. what's missing

**Already shipped**, close cousins of this idea:

1. **`CosmosContracts/juno-network-skill`** — a GitHub repo with `SKILL.md` + `references/*.md` that any AI agent reads to learn how to operate on Juno. This is the "manual" pattern FlipDAscript is describing — but it's **off-chain** (a git repo), not a registry, and it's one skill (Juno itself), not a directory of many dApps' skills.
2. **`agent-registry` contract** (`@/c:/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/contracts/agent-registry/src/msg.rs`) — already stores a `capabilities_hash: String` per registered agent. This is *almost* the right shape, but it's scoped to JunoClaw agents, not third-party dApps, and it stores a hash, not the skill manual content/pointer itself.

**Missing**: a registry where **any interchain dApp** (not just JunoClaw agents) can publish a pointer to its own `SKILL.md`-equivalent, and any agent can query it on-chain without needing to know the dApp's GitHub URL in advance.

---

## 3. Proposed contract: `skill-registry`

A new CosmWasm crate, `skill-registry`, deployed on Juno. Minimal, permissionless-write, query-heavy — the opposite security posture from `agent-company` (which gates everything behind governance). Publishing your dApp's manual should be as frictionless as posting to a public bulletin board; querying should be free and walletless.

### Storage shape

```rust
pub struct SkillEntry {
    pub dapp_name: String,          // e.g. "osmosis-dex", "levana-perps"
    pub publisher: Addr,            // whoever registered/updated this entry
    pub chain_id: String,           // which chain the dApp actually runs on
    pub skill_uri: String,          // where the actual SKILL.md lives: ipfs://, https://, ar://
    pub skill_hash: String,         // sha256 of the fetched content, for integrity check
    pub version: u64,               // bump on every update
    pub updated_at: u64,            // block height
}
```

### Messages

```rust
pub enum ExecuteMsg {
    PublishSkill {
        dapp_name: String,
        chain_id: String,
        skill_uri: String,
        skill_hash: String,
    },
    UpdateSkill {
        dapp_name: String,
        skill_uri: String,
        skill_hash: String,
    },
    // Optional: allow a dApp's own governance/admin address to be the only
    // one who can update an entry once claimed, to prevent squatting/griefing.
    ClaimName {
        dapp_name: String,
    },
}

pub enum QueryMsg {
    GetSkill { dapp_name: String },
    ListSkills { start_after: Option<String>, limit: Option<u32> },
    SearchByChain { chain_id: String },
}
```

### Anti-squatting / anti-spam

- **Name claiming**: first `PublishSkill` for a `dapp_name` sets the `publisher`. Subsequent `UpdateSkill` calls on that name require `info.sender == publisher` (or a DAO-elected override for disputes).
- **Small registration fee** (mirrors `agent-registry.registration_fee_ujuno`) to deter spam registrations, refundable or burned per governance decision.
- **Integrity check pattern**: the registry stores `skill_hash`, not the content itself (keeps state small). Any agent fetching `skill_uri` re-hashes and compares — same trust model as the existing `attestation_hash` re-computation pattern already used in `agent-company`.

### Why store a URI + hash, not the raw markdown

- Keeps on-chain state small and gas cheap — this matches the honest architectural stance JunoClaw already takes elsewhere (e.g., zk-verifier stores proof + hash, not raw computation trace).
- `skill_uri` can point to IPFS (censorship-resistant, content-addressed — ideally the hash *is* the IPFS CID, which gives free integrity checking) or a plain HTTPS GitHub raw link for dApps that don't want IPFS ops overhead.
- Any agent — on any chain — reads the registry contract via a plain gRPC/LCD query (no wallet, no gas cost to read), fetches the URI, verifies the hash, and now has the dApp's operating manual.

---

## 4. MCP integration

Add two new **query tools** to the Cosmos MCP server (`@/c:/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/mcp/README.md`) once the contract is live:

| Tool | Description |
|---|---|
| `get_dapp_skill` | Fetch a dApp's skill manual pointer from the on-chain registry, fetch the content, verify the hash, return the parsed SKILL.md |
| `list_dapp_skills` | List all registered dApps in the skill registry, optionally filtered by chain |

This makes the registry genuinely useful for *any* MCP client, not just JunoClaw's own agents — exactly the "any agent can retrieve the manual of any interchain dApp" outcome FlipDAscript described.

---

## 5. Sequencing

1. Draft `skill-registry` crate (state, msg, contract, tests) — smallest possible surface, mirrors existing `agent-registry` patterns for consistency.
2. Deploy to `uni-7` testnet, self-register JunoClaw's own skill as the first entry (dogfooding — point it at `juno-network-skill`'s `SKILL.md` or a JunoClaw-specific one).
3. Add `get_dapp_skill` / `list_dapp_skills` MCP tools.
4. Reach out to 2-3 other Cosmos dApp teams to register (Osmosis, Levana, dao-dao itself) — social proof before asking for a formal governance proposal.
5. If adoption looks real, propose the registry as ecosystem infrastructure (similar framing to A18c-9 / #373), possibly co-signed with Juno core if they want to fold it into the `juno-network-skill` initiative officially.

---

## 6. Resolution (2026-07-21)

Confirmed: "the hub" = **Juno**, not Cosmos Hub literally. Option A proceeds as planned.

**Status: `skill-registry` contract implemented.** `contracts/skill-registry/` — `state.rs`, `error.rs`, `msg.rs`, `contract.rs`, `tests.rs`, wired into the workspace `Cargo.toml`. 14/14 tests passing (`cargo test -p skill-registry`): publish/update/remove lifecycle, fee gating, duplicate-name rejection, non-publisher update rejection, admin dispute-resolution transfer, chain-filtered search, listing.

**Next steps:**
1. Deploy to `uni-7` testnet, self-register JunoClaw's own `SKILL.md` as the first entry.
2. Add `get_dapp_skill` / `list_dapp_skills` MCP query tools (no wallet needed — pure reads).
3. Reach out to 2-3 other Cosmos dApp teams to register before proposing this as ecosystem infrastructure.
