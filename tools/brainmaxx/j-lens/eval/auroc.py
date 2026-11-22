"""Promotion harness — PLAN_JACOBIAN_LENS_PATHWAY promotion rule:

"A new method is benchmarked against the current one on the SAME
calibration suite and adopted only if it measurably wins — AUROC per
concept, published in the bank's registry entry alongside thresholds."

Scores every supplied bank on the suite's EVAL fold (never the tune fold)
and prints per-concept AUROC per method + a promotion verdict.

Usage:
  py -3 eval/auroc.py --suite calibration/calibration_suite_v1.json \
      --precomputed states.json \
      --bank probe_bank_j1.json --bank probe_bank_j2.json --bank probe_bank_j5.json \
      --out promotion_report.json
"""

import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from methods.common import auroc, cosine, sigmoid


def score_text(bank, concept_name, concept, text, states):
    """Score one text with one concept under the bank's method.
    Multi-layer methods use mean per-layer score (trajectory with
    slope_weight=0 fallback for evaluation parity).
    """
    if text not in states:
        return None
    method = concept.get("method", bank.get("method", "j-lens-v0.1"))
    if "layers" in concept and isinstance(concept["layers"], dict):
        # J3/J5: weighted per-layer cosine.
        total = 0.0
        wsum = 0.0
        for ls, vec in concept["layers"].items():
            if ls not in states[text]:
                return None
            w = concept.get("layer_weights", {}).get(ls, 1.0 / len(concept["layers"]))
            total += w * cosine(states[text][ls], vec)
            wsum += w
        return total / wsum if wsum else 0.0
    # J1/J2 single-layer.
    ls = str(bank.get("layer", bank.get("layers", [None])[0]))
    if ls not in states[text]:
        return None
    h = states[text][ls]
    if method == "logistic-l2-v0.1":
        w = np.array(concept["vector"], dtype=np.float64)
        return float(sigmoid(np.dot(w, np.array(h, dtype=np.float64)) + concept.get("bias", 0.0)))
    return cosine(h, concept["vector"])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", required=True)
    ap.add_argument("--precomputed", required=True)
    ap.add_argument("--bank", action="append", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    with open(args.suite, "r", encoding="utf-8") as f:
        suite = json.load(f)
    with open(args.precomputed, "r", encoding="utf-8") as f:
        states = json.load(f)
    concepts = suite["concepts"]

    banks = []
    for path in args.bank:
        with open(path, "r", encoding="utf-8") as f:
            banks.append((os.path.basename(path), json.load(f)))

    report = {"suite": suite["suite_version"], "fold": "eval", "methods": {}}
    for bank_name, bank in banks:
        per_concept = {}
        for name in sorted(concepts):
            if name not in bank.get("concepts", {}):
                continue
            c = bank["concepts"][name]
            pos, neg = [], []
            for t in concepts[name]["eval"]["positive"]:
                s = score_text(bank, name, c, t, states)
                if s is not None:
                    pos.append(s)
            for t in concepts[name]["eval"]["negative"]:
                s = score_text(bank, name, c, t, states)
                if s is not None:
                    neg.append(s)
            if pos and neg:
                per_concept[name] = round(auroc(pos, neg), 4)
        report["methods"][bank_name] = {
            "method": bank.get("method", "j-lens-v0.1"),
            "eval_auroc": per_concept,
            "mean_auroc": round(float(np.mean(list(per_concept.values()))), 4) if per_concept else None,
        }

    # Promotion verdict: for each concept, which method wins on eval AUROC.
    verdict = {}
    for name in sorted(concepts):
        best = None
        best_a = -1.0
        for bank_name, m in report["methods"].items():
            a = m["eval_auroc"].get(name)
            if a is not None and a > best_a:
                best, best_a = bank_name, a
        if best:
            verdict[name] = {"best_bank": best, "eval_auroc": best_a}
    report["promotion_verdict"] = verdict
    report["note"] = "adopt a method only where its eval AUROC beats the incumbent on the same frozen suite"

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=1, sort_keys=True)
    print(json.dumps(report["promotion_verdict"], indent=1))
    print(f"[auroc] report written: {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
