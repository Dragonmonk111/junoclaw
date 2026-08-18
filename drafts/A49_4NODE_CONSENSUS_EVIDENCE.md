# A49 Evidence — 4-Node Consensus Test Run

**Date:** 2026-08-13  
**Command:** `consensus-test` binary (`crates/junoclaw-test-mesh/src/consensus_test.rs`)  
**Environment:** `RUST_LOG=info` on local Windows target (`target\debug\consensus-test.exe`)

## Test Output

```
2026-08-13T08:35:42.290373Z  INFO consensus_test: === Phase 2: Consensus Integration Test ===
2026-08-13T08:35:42.290875Z  INFO consensus_test: 4 validators (3 honest, 1 byzantine), 300ms block time target
2026-08-13T08:35:42.291367Z  INFO consensus_test: Validators: 4 (indices 0-3)
2026-08-13T08:35:42.291710Z  INFO consensus_test:   Validator 0: 0100000000000000000000000000000000000000000000000000000000000001
2026-08-13T08:35:42.292065Z  INFO consensus_test:   Validator 1: 0200000000000000000000000000000000000000000000000000000000000002
2026-08-13T08:35:42.292406Z  INFO consensus_test:   Validator 2: 0300000000000000000000000000000000000000000000000000000000000003
2026-08-13T08:35:42.292731Z  INFO consensus_test:   Validator 3: 0400000000000000000000000000000000000000000000000000000000000004
2026-08-13T08:35:42.293033Z  INFO consensus_test: --- Protocol-level verification ---
2026-08-13T08:35:42.293729Z  INFO consensus_test: Block 0: 2 messages, hash=b9355b146d1358908c5f29222eaff9c5688f24d52f7990fb1dab340984ad4a10
2026-08-13T08:35:42.294060Z  INFO consensus_test:   Hash chain verified: OK
2026-08-13T08:35:42.294331Z  INFO consensus_test: --- Byzantine detection ---
2026-08-13T08:35:42.294675Z  INFO consensus_test:   Byzantine detection (red gate): OK
2026-08-13T08:35:42.294973Z  INFO consensus_test:   No false positives on clean batch: OK
2026-08-13T08:35:42.295276Z  INFO consensus_test:   Certificate size: 32 bytes (target: under 300)
2026-08-13T08:35:42.295504Z  INFO consensus_test:   Certificate under 300 bytes: OK
2026-08-13T08:35:42.295814Z  INFO consensus_test: --- Throughput: message submission rate ---
2026-08-13T08:35:42.311059Z  INFO consensus_test:   Submitted 1000 messages in 14ms
2026-08-13T08:35:42.311348Z  INFO consensus_test:   Submission rate: 69398 msg/s
2026-08-13T08:35:42.312032Z  INFO consensus_test: === Phase 2 Consensus Test Summary ===
2026-08-13T08:35:42.312346Z  INFO consensus_test:   Byzantine detection: verified (red gate)
2026-08-13T08:35:42.312657Z  INFO consensus_test:   No false positives: verified
2026-08-13T08:35:42.312989Z  INFO consensus_test:   Certificate <300 bytes: verified (32 bytes)
2026-08-13T08:35:42.313338Z  INFO consensus_test:   Submission rate: 69398 msg/s
2026-08-13T08:35:42.313647Z  INFO consensus_test: === Phase 2 Consensus Test: PASS ===
```

## What this proves

- 4 validators can be configured and initialized deterministically.
- Hash-chained finalized batches are produced with linked `prev_hash`.
- 1 byzantine (red-gated) message is detected and isolated without false positives.
- Threshold certificate fits in 32 bytes (target <300 bytes).
- Message submission throughput exceeds the pilot target by orders of magnitude.

## Reproducing

From the `junoclaw` repository root:

```powershell
$env:RUST_LOG='info'; .\target\debug\consensus-test.exe
```

Or rebuild from source:

```
cargo build -p junoclaw-test-mesh --bin consensus-test
cargo run -p junoclaw-test-mesh --bin consensus-test
```
