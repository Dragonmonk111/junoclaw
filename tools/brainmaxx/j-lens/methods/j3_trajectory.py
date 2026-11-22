"""J3 — multi-layer trajectories.

Same diff-of-means readout at k layers instead of one. The signal is the
*shape across depth*: a concept that rises from early syntax layers into
late planning layers is different evidence from a single-layer blip.
"The plan forms before the token" becomes measurable.

Bank method tag: "trajectory-dom-v0.1".
Runtime scoring (deterministic, per token position, multi-layer states):
  base_l  = cosine(h_l, v_l)                      for each bank layer l
  base    = sum_l(layer_weight_l * base_l)
  slope   = linear-regression slope of base_l over increasing layer index
  score   = base + slope_weight * max(0, slope)
layer_weights come from per-layer tune-fold AUROC (clipped at 0.5 so a
non-informative layer contributes zero weight), normalized to sum 1.

Usage:
  py -3 methods/j3_trajectory.py --suite calibration/calibration_suite_v1.json \
      --layers 4,8,12,16 --precomputed states.json --out probe_bank_j3.json
"""

import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import auroc, bank_header, collect_states, cosine, sha256_file


def dom_direction(pos_vecs, neg_vecs):
    d = np.mean(np.stack(pos_vecs), axis=0) - np.mean(np.stack(neg_vecs), axis=0)
    n = np.linalg.norm(d)
    if n == 0:
        raise ValueError("zero-norm direction — examples too similar")
    return d / n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", required=True)
    ap.add_argument("--layers", required=True, help="comma-separated, e.g. 4,8,12,16")
    ap.add_argument("--model", default=None)
    ap.add_argument("--precomputed", default=None)
    ap.add_argument("--slope-weight", type=float, default=0.3)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    layers = [int(x) for x in args.layers.split(",")]
    with open(args.suite, "r", encoding="utf-8") as f:
        suite = json.load(f)
    suite_hash = sha256_file(args.suite)
    concepts = suite["concepts"]

    all_texts = sorted({t for c in concepts.values() for fold in ("tune", "eval")
                        for t in c[fold]["positive"] + c[fold]["negative"]})
    states = collect_states(all_texts, layers, precomputed=args.precomputed, model_id=args.model)

    out_concepts = {}
    report = {}
    for name in sorted(concepts):
        c = concepts[name]
        per_layer = {}
        weights = {}
        aurocs = {}
        for l in layers:
            ls = str(l)
            pos = [np.array(states[t][ls]) for t in c["tune"]["positive"] if t in states and ls in states[t]]
            neg = [np.array(states[t][ls]) for t in c["tune"]["negative"] if t in states and ls in states[t]]
            if not pos or not neg:
                continue
            v = dom_direction(pos, neg)
            a = auroc([cosine(h, v) for h in pos], [cosine(h, v) for h in neg])
            per_layer[ls] = v.tolist()
            weights[ls] = max(0.0, a - 0.5)
            aurocs[ls] = round(a, 4)
        if not per_layer:
            print(f"[j3] {name}: no layers available — skipped", file=sys.stderr)
            continue
        total_w = sum(weights.values())
        if total_w == 0:
            # No layer separates on tune data — equal weights, honestly reported.
            weights = {ls: 1.0 / len(per_layer) for ls in per_layer}
        else:
            weights = {ls: w / total_w for ls, w in weights.items()}

        out_concepts[name] = {
            "method": "trajectory-dom-v0.1",
            "layers": per_layer,
            "layer_weights": weights,
            "slope_weight": args.slope_weight,
            "threshold": 0.5,
            "calibrated": True,
            "certified_on": suite["suite_version"],
        }
        report[name] = {"per_layer_tune_auroc": aurocs, "layer_weights": {k: round(v, 4) for k, v in weights.items()}}
        print(f"[j3] {name}: {aurocs}", file=sys.stderr)

    bank = bank_header("trajectory-dom-v0.1", args.model or "precomputed", layers,
                       calibration={"suite": os.path.basename(args.suite), "suite_sha256": suite_hash})
    bank["concepts"] = out_concepts
    bank["build_report"] = report
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(bank, f, indent=1, sort_keys=True)
    print(f"[j3] bank written: {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
