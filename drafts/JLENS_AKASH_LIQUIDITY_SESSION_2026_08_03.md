# J-Lens / Akash Liquidity Session — 2026-08-03 (closed, resume-ready)

**Status:** Probing paused. Wallet clean, no open deployments, no funds at risk.
Resume this file when ready to test bigger models (Kimi K2.6/K2.7, GLM-5.2, etc.)

## Wallet state at close
- Address: `akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt`
- Balance: **3.438618 ACT**, **2.275 AKT**
- Active deployments: none (verified via `tools/akash/check-state.sh`)

## Live GPU inventory (console-api.akash.network, 2026-08-03 ~17:00 UTC)
Query tool: `tools/akash/gpu-inventory.sh` (fixed schema: `isOnline`, `gpuModels[].model`, `stats.gpu.{available,total}`)

| Provider | GPU | Free/Total | Audited |
|---|---|---|---|
| `akash1evr5r8r8zgxddvhru3t0l8q079a94ew8hcgwdd` (videokiska.ru) | H200 141Gi | 8/8 | No |
| `akash1eskq5dpjl2lffykc56vuj3je4pkxshd0apxq4v` (atl.val.akash.pub) | H200 141Gi | 3/32 | Yes |
| `akash17erkmem6xcugfnew2c0ujfqtet32j29ztk03jt` (wdc.hh.akash.pub) | H100 80Gi | 8/24 | Yes |
| `akash17r6r9u364t49k7qdef52qtm6wm5mcn0lpzfgnv` (siamaidol.com) | H100 80Gi | 7/7 | Yes |
| `akash15pkdkewzarpsx42t98vzf45h42hlq6ra8w96hr` (ams.val.akash.pub) | H100 80Gi | 2/32 | Yes |

## Probes fired (this session)
1. **8x H200** Kimi K2.6 (`sdl-jlens-kimi-k26-8xh200.yml`, price raised 40000→50000 uact/block) — dseq 28008161, ~60 min window, **0 bids**. Closed clean.
2. **8x H100** Kimi K2.6 (`sdl-jlens-kimi-k26-8xh100.yml`, new SDL created this session) — dseq 28008756, 5 min poll, **0 bids**. Closed clean.

Both deposits fully refunded. Only gas spent (~0.56 AKT total across probes + cert txs).

## Working hypothesis for zero bids
Providers show "online" + "available" in the console API stats, but their bid engines aren't responding to our orders. Two candidates, untested:
- Thin escrow deposit (1-2 ACT at 50000 uact/block ≈ 2-4 min runway) may be filtered by provider bid logic.
- Bid daemon on these specific hosts may be stale/wedged despite passing uptime checks.

## Next steps when resuming
1. **Sanity check**: re-fire `sdl-jlens-h100.yml` (the SDL that WON bids previously, e.g. dseq 27990353/27991772/27992717 lineage) to confirm the pipeline itself still works today.
2. **Escrow depth test**: retry Kimi K2.6 8x H200 with a much larger deposit (10-20 ACT) to rule out thin-escrow rejection.
3. **Funding decision**: a real Kimi K2.6 run (577GB download, hours of runtime at 50000 uact/block ≈ $720/day) needs ~50-100 ACT in escrow. Current balance (3.44 ACT) is probe-only. Need ~230+ AKT sent to mint enough ACT for a real run.
4. Re-run `tools/akash/gpu-inventory.sh` fresh — availability changes fast; check videokiska.ru (8/8 idle) and wdc.hh (8/24 free) first since they showed the most headroom.

## Tools added/fixed this session
- `tools/akash/check-state.sh` — active deployment + chain height check (fixed stdin/heredoc clash)
- `tools/akash/gpu-inventory.sh` — live provider GPU inventory via console API (fixed schema mismatch: `isOnline` not `isActive`, `stats.gpu` not per-GPU `available`)
- `tools/akash/sdl-jlens-kimi-k26-8xh100.yml` — new SDL, Kimi K2.6 on 8x H100 (tight VRAM: ~577GB model vs 640GB capacity, short-context probing only)
