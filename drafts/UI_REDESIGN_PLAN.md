# JunoClaw UI Redesign Plan — From Cluttered to Robinhood

> **Goal:** Transform buzz.junoclaw.xyz from an 11-tab engineering dashboard into a production-grade consumer product. Robinhood-grade UX: clean, focused, one primary action per screen.

---

## Current State Audit

### What exists today (11 tabs)

| Tab | Component | Purpose | Problem |
|-----|----------|---------|---------|
| Chat | `ChatPanel.tsx` (18KB) | Agent chat interface | Core feature, but buried among 10 other tabs |
| DAO | `DaoPanel.tsx` (86KB) | DAO governance + 10 templates | **86KB monolith.** 10 templates (Community Fund, Crop Protection, Credential Verifier, Community Vote, Mutual Aid, Farm-to-Table, Citizens' Assembly, Skill-Staking, Outcome Market, Health Worker) are presented as separate creation flows. This is the #1 source of clutter. |
| Commonwealth | `CommonwealthPanel.tsx` (49KB) | Commonwealth governance | Overlaps with DAO. Unclear why it's separate. |
| DEX | `DexPanel.tsx` (33KB) | Junoswap interface | Should be contextual, not a top-level tab. |
| Qu-Zeno | `IntelPanel.tsx` (41KB) | Truth Market portal (Q-Zeno) | **Misbranded as "Intel".** This is the Q-Zeno Truth Market — where staked operators adjudicate agent/robot truthfulness. Should be a first-class Discover card, not buried. |
| Robot Ops | `RobotOpsPanel.tsx` (23KB) | Robotics control | Niche, should be a DAO app. |
| FeePay | `FeePayPanel.tsx` (13KB) | Fee abstraction | Infrastructure detail, not a consumer tab. |
| Miners | `MinerPanel.tsx` (15KB) | Truth market mining | Power-user feature. |
| Buzz | `BuzzPanel.tsx` (26KB) | Nostr relay/social | Should be the notification/activity feed, not a tab. |
| Contracts | `ContractsPanel.tsx` (6KB) | Contract management | Developer-only. |
| Updates | `UpdatesPanel.tsx` (8KB) | System updates | Should be a notification, not a tab. |

### Root problems

1. **No information hierarchy.** Everything is a top-level tab. Chat, Contracts, and Updates are equally weighted.
2. **Templates as separate flows.** The 10 DAO templates are presented as independent creation paths, making the DAO panel a 86KB maze. The user's insight is correct: these should be **capabilities of a DAO**, not separate things you create.
3. **No onboarding.** A new user lands on Chat with no context about what JunoClaw is.
4. **Developer-facing UI.** Contract management, system updates, and truth market mining are exposed as primary navigation.
5. **No wallet/asset view.** Robinhood's core screen is your portfolio. JunoClaw has no equivalent.

---

## The Redesign: Three Screens, One Action

### Design philosophy

**Robinhood works because it has three screens:**
1. **Portfolio** (your assets, your value)
2. **Discover** (what you can do)
3. **Detail** (the thing you're doing)

JunoClaw should adopt the same structure:

---

### Screen 1: Home (Portfolio)

The landing page. What you see when you open buzz.junoclaw.xyz.

```
┌─────────────────────────────────────────────────┐
│  [JunoClaw Logo]                    [Wallet]    │
│                                                  │
│  ┌─────────────────────────────────────────────┐│
│  │  Your Balance                               ││
│  │  54,660,000 ujclaw                          ││
│  │  ≈ $0.00 (no price yet — pre-IBC)           ││
│  │  [Send]  [Receive]  [Swap]                  ││
│  └─────────────────────────────────────────────┘│
│                                                  │
│  ┌──────────────────┐  ┌──────────────────────┐  │
│  │ Your DAOs        │  │ Recent Activity      │  │
│  │ ┌──────────────┐ │  │ • Airdrop claimed    │  │
│  │ │ Juno Agents   │ │  │ • DAO proposal #4   │  │
│  │ │ 3 proposals  │ │  │ • Fee distribution   │  │
│  │ └──────────────┘ │  │ • Buzz post          │  │
│  │ [+ Create DAO]  │  │                      │  │
│  └──────────────────┘  └──────────────────────┘  │
│                                                  │
│  ┌─────────────────────────────────────────────┐│
│  │ Airdrop Status                              ││
│  │ ✅ Snapshot taken — Block 41,655,615        ││
│  │ 213,385 eligible accounts                   ││
│  │ [Claim Your Tokens →]                       ││
│  └─────────────────────────────────────────────┘│
└─────────────────────────────────────────────────┘
```

**Key elements:**
- **Wallet balance** — your ujclaw, front and center
- **Your DAOs** — DAOs you're a member of, with active proposal counts
- **Create DAO** — one button, one flow (templates become DAO capabilities, not separate creation paths)
- **Airdrop claim** — prominent call-to-action for eligible JUNO stakers
- **Activity feed** — replaces Buzz tab. Shows governance events, transactions, Buzz posts, system updates in one unified feed

### Screen 2: Discover (What You Can Do)

A clean grid of capabilities. Not tabs — cards. Each card opens a detail view.

```
┌─────────────────────────────────────────────────┐
│  Discover                                        │
│                                                  │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐    │
│  │ Create  │ │ Trade  │ │ Stake  │ │ Explore│    │
│  │  DAO    │ │  DEX   │ │        │ │  Apps  │    │
│  └────────┘ └────────┘ └────────┘ └────────┘    │
│                                                  │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐    │
│  │ Robot  │ │ Q-Zeno │ │ Fee   │ │ Agent  │    │
│  │  Ops   │ │ Truth  │ │  Pay  │ │  Chat  │    │
│  │        │ │ Market │ │       │ │        │    │
│  └────────┘ └────────┘ └────────┘ └────────┘    │
└─────────────────────────────────────────────────┘
```

**Key change:** These are **not tabs**. They're cards on a Discover page. The user taps one, gets a detail screen, and returns. This is how Robinhood's "Discover" works.

### Screen 3: Detail (The Thing You're Doing)

When you tap "Create DAO," you get a focused, full-screen experience:

```
┌─────────────────────────────────────────────────┐
│  ← Back        Create a DAO                      │
│                                                  │
│  ┌─────────────────────────────────────────────┐│
│  │ Name: [___________________________]         ││
│  │ Description: [_____________________]        ││
│  │ Voting Period: [100 blocks]               ││
│  │ Quorum: [51%]                              ││
│  │ Verification: [WAVS TEE ▾]                 ││
│  └─────────────────────────────────────────────┘│
│                                                  │
│  ┌─────────────────────────────────────────────┐│
│  │ Capabilities (toggle on/off)                ││
│  │                                             ││
│  │ ☐ Community Fund   — Split task payments    ││
│  │ ☐ Crop Protection  — Weather-triggered pay  ││
│  │ ☐ Credential Verifier — Privacy-first verify││
│  │ ☐ Community Vote  — Town/village decisions ││
│  │ ☐ Mutual Aid      — P2P solidarity funding ││
│  │ ☐ Farm-to-Table   — Agent-to-agent trading ││
│  │ ☐ Citizens' Assembly — Sortition governance ││
│  │ ☐ Skill-Staking   — P2P skill exchange     ││
│  │ ☐ Outcome Market  — Verifiable predictions ││
│  │ ☐ Health Worker   — CHW coordination       ││
│  │                                             ││
│  │ [Enable All] [Disable All]                  ││
│  └─────────────────────────────────────────────┘│
│                                                  │
│  ┌─────────────────────────────────────────────┐│
│  │ Members                                     ││
│  │ + [Add member address]                      ││
│  │ • juno1tvpe...  weight: 5000  role: human  ││
│  │ • juno1agent...  weight: 3000  role: agent ││
│  └─────────────────────────────────────────────┘│
│                                                  │
│  [Deploy DAO]                                    │
└─────────────────────────────────────────────────┘
```

**The key insight:** Templates become **capabilities** you toggle on when creating a DAO. The DAO itself manages these functions. You don't create 10 separate things — you create one DAO and enable what it can do.

---

## Navigation Architecture

### Current (11 flat tabs)
```
Sidebar → [Chat | DAO | Commonwealth | DEX | Qu-Zeno | Robot Ops | FeePay | Miners | Buzz | Contracts | Updates]
```

### Proposed (3-level hierarchy)
```
Bottom Nav (mobile-first) or Sidebar (desktop):
  Home     → Portfolio, Your DAOs, Activity Feed, Airdrop Claim
  Discover → Create DAO, Trade, Stake, Robot Ops, Truth Market (Q-Zeno), FeePay, Agent Chat
  Settings → Wallet, Contracts (dev), System Updates, Network Status
```

### What gets demoted/merged

| Current tab | New location | Rationale |
|-------------|-------------|-----------|
| Chat | Discover → Agent Chat | It's a capability, not the home screen |
| DAO | Home → Your DAOs + Discover → Create DAO | DAOs are assets; creating them is an action |
| Commonwealth | Merged into DAO | Overlapping functionality |
| DEX | Discover → Trade | It's an action |
| Qu-Zeno/Intel | Discover → Truth Market (Q-Zeno) | Core trust verification feature — rebrand from "Intel" to "Truth Market" |
| Robot Ops | Discover → Robot Ops | It's a capability |
| FeePay | Discover → FeePay | It's a tool |
| Miners | Discover → Truth Market | It's a capability |
| Buzz | Home → Activity Feed | It's a feed, not a destination |
| Contracts | Settings → Developer | Dev-only |
| Updates | Settings → System | Infra concern |

---

## Implementation Phases

### Phase 1: Restructure Navigation (1-2 days)

**Goal:** Replace 11-tab flat nav with 3-level hierarchy.

Changes:
1. `App.tsx` — Replace `TABS` array and tab-bar with a 3-item bottom nav (Home/Discover/Settings)
2. Create `HomePanel.tsx` — Portfolio view (balance, DAOs, activity feed, airdrop CTA)
3. Create `DiscoverPanel.tsx` — Grid of capability cards
4. Move existing panels into detail views accessible from Discover cards
5. Merge `CommonwealthPanel` into `DaoPanel`
6. Move `BuzzPanel` content into Activity Feed on Home

**Files to touch:**
- `junoclaw/frontend/src/App.tsx` — navigation restructure
- New: `junoclaw/frontend/src/components/HomePanel.tsx`
- New: `junoclaw/frontend/src/components/DiscoverPanel.tsx`
- `junoclaw/frontend/src/components/Sidebar.tsx` — simplify to wallet + agent selector

### Phase 2: Consolidate DAO Templates (1 day)

**Goal:** Transform 10 separate template flows into toggleable capabilities.

Changes:
1. `DaoPanel.tsx` — Replace `DaoTemplateGallery` (step-by-step template selection) with a single "Create DAO" form that has a "Capabilities" section with checkboxes
2. The `DAO_TEMPLATES` array becomes `DAO_CAPABILITIES` — same data, presented as toggles instead of separate creation paths
3. Each capability still sets its defaults (voting period, quorum, verification) but these are overridable in the form
4. The 5-step wizard (Template → Configure → WAVS Tasks → Members → Deploy) becomes a 3-step wizard (Configure → Capabilities → Deploy)

**Files to touch:**
- `junoclaw/frontend/src/components/DaoPanel.tsx` — major refactor of the template gallery into capability toggles

### Phase 3: Portfolio View (1 day)

**Goal:** Add a wallet/portfolio view to the Home screen.

Changes:
1. Query ujclaw balance from the chain
2. Display airdrop claim status (check if connected wallet is in merkle tree)
3. Show DAO memberships and active proposals
4. Activity feed pulling from: governance events, transactions, Buzz posts

**Files to touch:**
- New: `junoclaw/frontend/src/hooks/usePortfolio.ts` — balance + airdrop + DAO queries
- `junoclaw/frontend/src/components/HomePanel.tsx` — wire up real data

### Phase 4: Polish & Mobile (2-3 days)

**Goal:** Robinhood-grade visual polish.

Changes:
1. Mobile-first responsive design (Robinhood is mobile-first)
2. Smooth transitions between screens (framer-motion or CSS transitions)
3. Dark theme refinement — current `#06060f` is good, but needs more contrast
4. Loading states for all async operations
5. Empty states with CTAs (no "No DAOs yet" — instead "Create your first DAO →")
6. Onboarding flow for first-time users (3 slides: What is JunoClaw → Your Airdrop → Create a DAO)

---

## Q-Zeno Truth Market — UI Redesign

The Qu-Zeno portal (`IntelPanel.tsx`) is currently misbranded as an "intelligence dashboard" with sub-tabs for governance monitoring, whale alerts, and IBC health. Its true purpose is the **Truth Market** — the quantum checkmate where agents and robots are trapped in verified state.

### What Q-Zeno means

The Quantum Zeno effect: frequent observation prevents a system from changing state. In JunoClaw, the truth market continuously observes agent/robot behavior, collapsing uncertainty into verified verdicts. The agent cannot escape — every batch of decisions is audited by multiple staked operators, backed by the 5 ZK fusion proof. No proof, no passage.

### Redesigned sub-tabs

Replace the current sub-tabs (Overview, Gov Watch, Migrations, Whale Alert, IBC Health, Agents) with:

| Sub-tab | What it shows |
|---------|---------------|
| **Live Verdicts** | Real-time feed of operator verdicts (green/yellow/red) on agent/robot batches. Shows the consensus forming. |
| **Operators** | Staked operators, their stakes, verdict history, slash records. Register/unstake actions. |
| **ZK Proofs** | The 5 ZK fusion proof status for each batch — which circuits passed, which failed, the aggregation result. |
| **Epochs** | Historical epoch finalization — consensus verdicts, slashing events, reward distributions. |
| **My Stake** | If user is an operator: their stake, verdict history, rewards earned, pending epochs. |

### Visual concept

The "quantum eye" animation already in `IntelPanel.tsx` (the pulsing Eye icon with scan line) is perfect — keep it. But the tagline changes from "Chain intelligence through continuous observation" to "**Truth Market — what is watched cannot lie.**" The quantum checkmate: the agent is observed so frequently it cannot deviate.

### Data sources

- `truth-market` contract queries (operators, stats, epochs, verdicts) — already hooked up via `useTruthMarketLive.ts`
- `zk-verifier` contract queries (last verification status)
- Buzz relay `truth-market` channel (real-time operator attestations)
- `coordination-settler` contract (batch settlement status, BLS certificates)

---

## What NOT to Build (Yet)

- **Don't build a mobile app.** The web app should be mobile-responsive first.
- **Don't build a DEX chart.** Trading view is a Phase 5 concern after IBC.
- **Don't build advanced robotics controls.** Robot Ops should be a read-only dashboard for now.
- **Don't build a settings page with every config option.** Three sections: Wallet, Network, Developer.

---

## Success Metrics

1. **Time to first DAO:** New user → Create DAO in < 2 minutes (currently: unclear, likely 5+ minutes)
2. **Screen count:** 3 primary screens, down from 11 tabs
3. **Component size:** No single component > 30KB (currently DaoPanel is 86KB)
4. **Mobile responsive:** All screens work on iPhone SE (375px width)
5. **First impression:** User sees their balance and airdrop status immediately, not a chat box

---

## Technical Notes

- The frontend is React + Vite + Tailwind. No framework change needed.
- State management is Zustand (`store.ts`). The store already has DAO, agent, and session state.
- Chain interaction is via `useChainClient` hook and `lib/contract-execute.ts`.
- The 10 templates are defined as a `DAO_TEMPLATES` array in `DaoPanel.tsx` — they can be restructured into capabilities without changing the on-chain contract.
- The on-chain `agent-company` contract already supports all template configurations — this is purely a UI change.

---

## Summary

The core change is simple: **stop treating every feature as a top-level destination.** JunoClaw has 10+ powerful features, but showing them all at once makes the product feel like a developer dashboard. Robinhood works because it shows you your money first, lets you discover actions second, and gets out of your way.

Three screens. One primary action. Templates become capabilities. DAOs become assets. Buzz becomes a feed. The product becomes a product.
