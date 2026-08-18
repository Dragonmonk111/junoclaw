# JunoClaw Truth Market Soak Test — LIVE on Akash

**🚀 The updated JunoClaw soak test is now running on Akash mainnet.**

This deployment includes **Layers 1–6** of the coordination stack in a 4-node real P2P mesh, running continuously for the next **6.5 days**.

## What is running

- **Layer 1 — Consensus:** P2P mesh consensus across 4 nodes
- **Layer 2 — Gate:** J-Lens truth auditing
- **Layer 4 — Moultbook:** Addendum/audit logging
- **Layer 5 — Executor:** Task submission to on-chain task ledger
- **Layer 6 — Truth Market:** CosmWasm contract tests for staking, verdicts, and epoch finalization
- **Layer 6 — Multi-Operator Gate:** Competitive evaluation with 2/3 majority consensus

## Live logs

All logs are served publicly from the Akash deployment:

**🔗 http://10sujobnch8gf1ec1nsgn49pmg.ingress.quanglong.org**

Available files:
- `soak-orchestrator.log` — main cycle log with PASS/FAIL status
- `consensus-cycle-1.log`
- `gate-cycle-1.log`
- `moult-cycle-1.log`
- `executor-cycle-1.log`
- `truth-market-cycle-1.log`
- `multi-gate-cycle-1.log`
- `soak-node-1.log` … `soak-node-4.log`
- `soak-status.json`

## First-cycle health check

```text
consensus-test: PASS
gate-test: PASS
moult-test: PASS
executor-test: PASS
truth-market-test: PASS
multi-gate-test: PASS
Health: cycle=2 p2p_nodes_alive=4/4 relayer_alive=no
```

## Deployment details

| Field | Value |
|-------|-------|
| Provider | `akash1sjwuwre4qprcaa34f6324yz7m8nn0awvc75gp5` |
| dseq | `28170405` |
| Bid price | 5 uact/block |
| Auto-close | ~Aug 21, 2026 01:34 UTC |
| Funds | 60 AKT minted → ACT for deposit + lease; 42 AKT remaining for gas |

## Notes

- The relayer is intentionally **not configured** in this soak (`MOULTBOOK_ADDR`, `TASK_LEDGER_ADDR`, `TRUTH_MARKET_ADDR`, and `RELAYER_KEY` are unset), so the on-chain submission paths are tested in unit/integration scope only.
- The build runs inside the Akash container using `rust:latest` with a `git clone` + startup-build approach — no pre-built Docker image needed.
- Truth Market contract tests now target the `contracts/` workspace manifest correctly.

## Follow the article

For the design behind Layer 6, see:
**"Truth Markets: When Evaluators Have Skin in the Game"** — `drafts/ARTICLE_TRUTH_MARKETS_2026_08_14.md`

---

*JunoClaw coordination stack — August 14, 2026*
