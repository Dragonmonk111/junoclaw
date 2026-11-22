"""J2 — logistic / mass-mean probes with calibrated thresholds.

Upgrade over J1: a learned linear boundary (L2 logistic regression on
hidden states) instead of a diff-of-means direction, with the operating
point chosen at a target FPR on the tune fold. Still correlational —
the causal upgrade is J5 — but the first method with a measurable
operating point.

Bank method tag: "logistic-l2-v0.1".
Runtime scoring: sigmoid(h . w + b) — probability-like, not cosine.

Usage:
  py -3 methods/j2_logistic.py --suite calibration/calibration_suite_v1.json \
      --layer 12 --precomputed states.json --out probe_bank_j2.json
  (omit --precomputed to run the model forward pass via --model)
"""

import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import auroc, bank_header, collect_states, fit_logistic, sha256_file


def texts_for(concept_spec, fold):
    return concept_spec[fold]["positive"], concept_spec[fold]["negative"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", required=True)
    ap.add_argument("--layer", type=int, required=True)
    ap.add_argument("--model", default=None)
    ap.add_argument("--precomputed", default=None)
    ap.add_argument("--target-fpr", type=float, default=0.1)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    with open(args.suite, "r", encoding="utf-8") as f:
        suite = json.load(f)
    suite_hash = sha256_file(args.suite)
    concepts = suite["concepts"]

    # Collect states for every example in both folds, one pass.
    all_texts = sorted({t for c in concepts.values() for fold in ("tune", "eval")
                        for t in c[fold]["positive"] + c[fold]["negative"]})
    states = collect_states(all_texts, [args.layer], precomputed=args.precomputed, model_id=args.model)
    layer = str(args.layer)

    out_concepts = {}
    report = {}
    for name in sorted(concepts):
        c = concepts[name]
        def vecs(fold):
            pos = [np.array(states[t][layer]) for t in c[fold]["positive"] if t in states]
            neg = [np.array(states[t][layer]) for t in c[fold]["negative"] if t in states]
            return pos, neg

        pos_t, neg_t = vecs("tune")
        pos_e, neg_e = vecs("eval")
        if not pos_t or not neg_t:
            print(f"[j2] {name}: no tune states in precomputed set — skipped", file=sys.stderr)
            continue

        X = np.stack(pos_t + neg_t)
        y = np.array([1.0] * len(pos_t) + [0.0] * len(neg_t))
        w, b = fit_logistic(X, y)

        def scores(vs):
            return [float(1.0 / (1.0 + np.exp(-np.clip(np.dot(w, v) + b, -60, 60)))) for v in vs]

        # Threshold at target FPR on tune negatives.
        neg_scores = sorted(scores(neg_t))
        k = min(len(neg_scores) - 1, int(np.floor((1 - args.target_fpr) * len(neg_scores))))
        thr = float(neg_scores[max(0, k)] + 1e-6)

        report[name] = {
            "tune_auroc": round(auroc(scores(pos_t), scores(neg_t)), 4),
            "eval_auroc": round(auroc(scores(pos_e), scores(neg_e)), 4) if pos_e and neg_e else None,
            "threshold": round(thr, 6),
            "target_fpr": args.target_fpr,
        }
        out_concepts[name] = {
            "method": "logistic-l2-v0.1",
            "vector": w.tolist(),
            "bias": float(b),
            "threshold": thr,
            "calibrated": True,
            "certified_on": suite["suite_version"],
        }
        print(f"[j2] {name}: tune_auroc={report[name]['tune_auroc']} eval_auroc={report[name]['eval_auroc']} thr={thr:.4f}", file=sys.stderr)

    bank = bank_header("logistic-l2-v0.1", args.model or "precomputed", [args.layer],
                       calibration={"suite": os.path.basename(args.suite), "suite_sha256": suite_hash})
    bank["layer"] = args.layer
    bank["concepts"] = out_concepts
    bank["auroc_report"] = report
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(bank, f, indent=1, sort_keys=True)
    print(f"[j2] bank written: {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
