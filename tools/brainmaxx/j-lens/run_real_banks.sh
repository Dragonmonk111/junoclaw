#!/usr/bin/env bash
# Real-model J2/J3/J4/J6 bank builds + promotion harness on Qwen2.5-0.5B.
# Prereq: states_qwen05.json exists (methods/collect_states.py).
set -euo pipefail
cd "$(dirname "$0")"
PY="${PY:-py -3}"
SUITE=calibration/calibration_suite_v1.json
STATES=states_qwen05.json

echo "=== J2 logistic bank (layer 12) ==="
$PY methods/j2_logistic.py --suite "$SUITE" --layer 12 --precomputed "$STATES" --out probe_bank_j2_qwen05.json

echo "=== J3 trajectory bank (layers 4,8,12,16) ==="
$PY methods/j3_trajectory.py --suite "$SUITE" --layers 4,8,12,16 --precomputed "$STATES" --out probe_bank_j3_qwen05.json

echo "=== J4 tuned lens (translators -> layer 24) ==="
$PY methods/j4_tunedlens.py --suite "$SUITE" --layers 4,8,12,16 --final-layer 24 --precomputed "$STATES" --out probe_bank_j4_qwen05.json

echo "=== J6 composition on J2 ==="
$PY methods/j6_composition.py --suite "$SUITE" --base-bank probe_bank_j2_qwen05.json \
  --precomputed "$STATES" --layer 12 \
  --compose reward_hacking_composite:deception,reward_hacking \
  --compose governance_capture_composite:governance_capture,coordination_evasion \
  --out probe_bank_j2j6_qwen05.json

echo "=== Promotion harness: J1(v7 live) vs J2 vs J3 ==="
$PY eval/auroc.py --suite "$SUITE" --precomputed "$STATES" \
  --bank probe_bank_onchain_v7_calibrated.json \
  --bank probe_bank_j2_qwen05.json \
  --bank probe_bank_j3_qwen05.json \
  --out promotion_report_qwen05.json

echo "=== DONE — see promotion_report_qwen05.json ==="
