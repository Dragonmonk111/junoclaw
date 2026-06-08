# CosmoWarp BN254 Demo — Frontend Plan

> A no-install, browser-based onboarding demo that connects to the
> `junoclaw-bn254-1` devnet and lets visitors verify Groth16 proofs
> against both the pure-Wasm and precompile zk-verifier contracts.

---

## Why this exists

The BN254 precompile is invisible — it lives inside `wasmvm`, not in a
contract users can click on. A demo frontend makes the gas reduction
*tangible*: a visitor uploads a proof, hits "Verify", and sees the
pure-Wasm cost (≈370K gas) side-by-side with the precompile cost
(≈203K gas) in real time.

This is **not** a production product. It is an onboarding tool for:
- Validator operators evaluating the software upgrade
- Developers who want to understand the contract interface
- Governance voters who want to see the precompile work before voting

---

## Tech stack

| Layer | Choice | Rationale |
|---|---|---|
| Framework | **React 18 + Vite** | Fast dev server, small bundle, no build-tool lock-in |
| Styling | **TailwindCSS** | Utility-first, no unused CSS shipped |
| Icons | **Lucide React** | Lightweight, consistent |
| Chain client | **CosmJS** (`@cosmjs/stargate`, `@cosmjs/cosmwasm-stargate`) | Official SDK, devnet-tested |
| Wallet connect | **Keplr + Leap** (CosmosKit or raw `window.keplr`) | Two-line integration, no backend needed |
| State | React `useState` / `useReducer` | Demo scope; no Redux/Zustand necessary |
| Build output | Static SPA (`dist/`) | Deploy to Netlify / Vercel / GitHub Pages in one click |

**Explicitly out of scope:** SSR, backend API, database, authentication,
WebAssembly in the browser (the proof is verified *on-chain*, not in the
browser).

---

## Screens

### 1. Landing / Connect

- Hero: "BN254 Precompile Demo — 1.82× cheaper Groth16 verification on Juno"
- Wallet connection button (Keplr / Leap)
- Chain selector: `junoclaw-bn254-1` devnet (hardcoded RPC)
- One-line warning: *"This connects to a local devnet. No real funds."*

### 2. Proof Upload

- Drag-and-drop JSON zone for the proof bundle
  - Schema: `{ "vk": "base64", "proof": "base64", "public_inputs": ["string"] }`
  - Size cap: 10 KB (client-side validation)
- Inline preview: parse and display `n_public_inputs`, `vk_size_bytes`
- "Load sample" button — injects a known-good fixture from
  `devnet/test-fixtures/` so the demo works even if the user has no
  proof JSON handy

### 3. A/B Verify

Two buttons, two cards:

| | Pure-Wasm | BN254 Precompile |
|---|---|---|
| Contract | `PURE_ADDR` | `PRECOMPILE_ADDR` |
| Action | `VerifyProof` | `VerifyProof` |
| Result card | Gas used, tx hash, block height | Gas used, tx hash, block height |
| Delta banner | — | "Saved **167,334 gas** (1.82×)" |

Both transactions are broadcast sequentially (not parallel — avoid
account-sequence mismatch). After each commits, query the contract's
`LastVerification` state to display `verified: true / false`.

### 4. Benchmark replay

- "Run 10 samples" button — re-runs the same proof 10 times against
  each contract, exactly like `devnet/scripts/benchmark.sh`
- Live table: per-tx gas, median, mean, min/max
- Export: download the results as the same markdown format as
  `BN254_BENCHMARK_RESULTS.md`

### 5. Contract state explorer

- Read-only queries:
  - `vk_status {}` → has_vk, vk_size_bytes
  - `last_verification {}` → verified, block_height
  - `config {}` → admin address
- Raw JSON view for power users

---

## Devnet integration

The demo is designed to point at the local devnet first. A `config.ts`
module exports RPC, chain ID, and contract addresses:

```typescript
// src/config.ts
export const DEVNET = {
  chainId: 'junoclaw-bn254-1',
  rpc: 'http://localhost:36657',
  rest: 'http://localhost:36658',
  contracts: {
    pure: 'juno1...pure',
    precompile: 'juno1...precompile',
  },
  gasPrice: '0.025ujuno',
}
```

For a public demo (optional future step), the same config can be
switched to `uni-7` testnet addresses.

---

## Directory structure (proposed)

```
frontend/
├── public/
│   └── test-fixtures/
│       └── sample-proof.json          # known-good Groth16 proof
├── src/
│   ├── config.ts                      # devnet RPC, contract addresses
│   ├── chain/
│   │   ├── client.ts                  # CosmJS SigningCosmWasmClient setup
│   │   ├── wallet.ts                  # Keplr / Leap connect
│   │   └── query.ts                   # read-only contract queries
│   ├── components/
│   │   ├── ConnectWallet.tsx
│   │   ├── ProofUploader.tsx
│   │   ├── VerifyCard.tsx
│   │   ├── BenchmarkRunner.tsx
│   │   └── StateExplorer.tsx
│   ├── hooks/
│   │   ├── useClient.ts
│   │   ├── useContractQuery.ts
│   │   └── useVerify.ts
│   └── App.tsx
├── index.html
├── vite.config.ts
├── tailwind.config.js
└── package.json
```

---

## Security considerations

| Concern | Mitigation |
|---|---|
| Fake RPC / MITM | Devnet only; no real value at stake. For public demo, pin HTTPS RPC. |
| Malicious proof JSON (XSS via UI) | Client-side JSON.parse only; no `eval` or `innerHTML`. Display values are escaped. |
| Wallet phishing | UI clearly labels "junoclaw-bn254-1 DEVNET"; no mainnet addresses hardcoded. |
| Large file DoS | 10 KB upload cap; proof JSON validated against schema before broadcast. |
| Mnemonic exposure | No mnemonic input field. Wallet connection is read-only prompt from Keplr. |
| Secret key leakage in build | `.env` files excluded from git; no private keys in `config.ts`. |

---

## Licensing considerations

The JunoClaw repo is **Apache-2.0**. The frontend code will be the same.

**Rattadan's CosmoWarp template** was mentioned as a possible starting
point. Before using it:

1. **Verify the license** of the CosmoWarp template repository. If it is
   NOT Apache-2.0 / MIT / BSD, do not copy code into this repo without
   explicit permission or a license-compatible wrapper.
2. **If compatible** (MIT / Apache-2.0 / BSD): credit the original
   author in `frontend/README.md` and `package.json` `contributors`.
3. **If incompatible / unclear**: write the frontend from scratch using
   the stack above. The CosmJS + React pattern is standard; no template
   is strictly necessary.
4. **Patent / trademark**: do not use "CosmoWarp" branding in the demo
   title without checking trademark policy.

**Decision:** Start from scratch with the stack above. The CosmoWarp
template (if ever used) is a visual reference only, not a code
dependency. This avoids all license ambiguity.

---

## Implementation phases

### Phase 0 — Scaffold (1–2 hours)
- `npm create vite@latest frontend -- --template react-ts`
- Add Tailwind, Lucide, CosmJS deps
- `src/config.ts` with devnet constants
- `src/chain/client.ts` + `src/chain/wallet.ts`

### Phase 1 — Read-only explorer (2–3 hours)
- Connect wallet
- Query `vk_status`, `config`, `last_verification`
- Render in simple cards

### Phase 2 — Verify flow (3–4 hours)
- Proof upload + validation
- `VerifyProof` execute against pure contract
- `VerifyProof` execute against precompile contract
- Side-by-side gas display

### Phase 3 — Benchmark replay (2–3 hours)
- Sequential 10-sample runner
- Live table + median calculation
- Export to markdown

### Phase 4 — Polish & deploy (1–2 hours)
- Mobile responsive pass
- Error states (wallet reject, tx timeout, devnet down)
- Deploy to Netlify / Vercel
- Link from `docs/BN254_PRECOMPILE_CASE.md`

**Total estimate:** ~12–15 hours of focused work.

---

## Deployment options

| Target | Command | Notes |
|---|---|---|
| Netlify | `netlify deploy --prod --dir=frontend/dist` | Drag-and-drop also works |
| Vercel | `vercel --prod frontend/` | GitHub integration auto-builds |
| GitHub Pages | GitHub Action → `actions/deploy-pages` | Free, but no server-side logic |
| JunoClaw repo (static) | Commit `dist/` to `gh-pages` branch | Simplest, no external account |

Recommended: **GitHub Pages** via a GitHub Action triggered on push to
`main` that builds `frontend/` and deploys to `gh-pages`. Zero cost,
zero external dependency, fully reproducible.

---

## Open questions

1. **Should the demo support mainnet `uni-7`?** The pure-Wasm contract is
   already live there (code_id 64). The precompile variant cannot run on
   mainnet until the software upgrade. A `uni-7` mode would only show the
   pure side. **Decision:** devnet-only for now; add `uni-7` toggle after
   the upgrade.

2. **Should the demo include the `StoreVk` step?** A `VerifyProof` requires
   a verifying key to be stored first. The devnet deploy script already
   stores it. For a fresh demo, the user would need to `StoreVk` before
   `VerifyProof`. **Decision:** include `StoreVk` as a prerequisite
   button that runs once per session.

3. **Should we show the raw tx bytes?** Power users like to inspect
   `MsgExecuteContract` payloads. **Decision:** collapsible "Advanced"
   section with raw JSON.

---

*Last updated: 2026-06-07. See `GOVERNANCE_WIRING_PLAN.md` for the
on-chain proposal timeline and `BN254_TRAJECTORY_UPDATE.md` for the
technical state of the precompile.*
