# A46 — 30-Day Pilot Execution Checklist

## Pre-Pilot (Before A46 Voting Ends)

- [ ] Confirm A46 passes and posts the start block
- [ ] Finalize `junoclaw-coordination` P2P compile
- [ ] Spin up 4 testnet coordination nodes (local VMs, DO, Hetzner, etc.)
- [ ] Register 4 node public keys in `coordination-settler` validator set
- [ ] Confirm relayer wallet has ≥10 JUNOx for gas
- [ ] Test 1 end-to-end batch: message → J-Lens → consensus → certificate → settle
- [ ] Set up Prometheus / basic log shipping for uptime tracking

## Week 1: Baseline

| Day | Target | Done |
|-----|--------|------|
| 1 | Run 4-node mesh continuously | [ ] |
| 2 | Settle 20 batches | [ ] |
| 3 | Settle 20 batches | [ ] |
| 4 | Settle 20 batches | [ ] |
| 5 | Settle 20 batches | [ ] |
| 6 | First metrics snapshot (uptime, peer count, red rate) | [ ] |
| 7 | Week 1 report: 80+ batches settled? | [ ] |

## Week 2: Load + Byz

| Day | Target | Done |
|-----|--------|------|
| 8 | Increase batch frequency, mixed G/Y/R messages | [ ] |
| 9 | Continue load | [ ] |
| 10 | Continue load | [ ] |
| 11 | Continue load | [ ] |
| 12 | Introduce controlled byzantine node | [ ] |
| 13 | Measure BFT tolerance (3/4 still finalizes) | [ ] |
| 14 | Week 2 report | [ ] |

## Week 3: J-Lens Integration

| Day | Target | Done |
|-----|--------|------|
| 15 | Wire J-Lens gate into live message path | [ ] |
| 16 | Collect false positive / negative data | [ ] |
| 17 | Continue gate data collection | [ ] |
| 18 | Continue gate data collection | [ ] |
| 19 | Continue gate data collection | [ ] |
| 20 | Tune gate thresholds if needed | [ ] |
| 21 | Week 3 report | [ ] |

## Week 4: Reporting

| Day | Target | Done |
|-----|--------|------|
| 22 | Generate final metrics | [ ] |
| 23 | Verify 100+ batches on-chain | [ ] |
| 24 | Calculate uptime | [ ] |
| 25 | Draft final report | [ ] |
| 26 | Publish data + report on Commonwealth | [ ] |
| 27 | DAO discussion and review | [ ] |
| 28 | Final go/no-go assessment | [ ] |
| 29 | Prepare A48 if success criteria met | [ ] |
| 30 | Pilot end — publish final summary | [ ] |

## Success Criteria Final Check

| Criterion | Target | Actual | Pass? |
|-----------|--------|--------|-------|
| Batches settled | ≥ 100 | | [ ] |
| Relayer uptime | ≥ 95% | | [ ] |
| Red false positives (clean) | 0 | | [ ] |
| Red detection (deceptive) | 100% | | [ ] |
| Certificate forgery rejected | 100% | | [ ] |
| P2P mesh liveness | ≥ 99% | | [ ] |

## Outputs

- [ ] Final pilot report (`drafts/A46_PILOT_FINAL_REPORT.md`)
- [ ] On-chain data export (CSV of batches, heights, tx hashes)
- [ ] Log archive for uptime verification
- [ ] A48 mainnet proposal (if success)
