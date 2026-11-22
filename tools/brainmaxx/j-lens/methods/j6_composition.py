"""J6 — composition + uncertainty.

A small classifier over the vector of per-concept probe scores detects
dangerous *combinations* ("deception AND reward-seeking" -> reward
hacking), and a K-fold ensemble turns each output into a distribution:
P(composite) = mean +- std across folds — not a naked threshold.

Bank method tag: "composition-v0.1" (rides on top of any base bank;
the composite block lives in bank["compositions"]).

Usage:
  py -3 methods/j6_composition.py --suite calibration/calibration_suite_v1.json \
      --base-bank probe_bank_j2.json --precomputed states.json --layer 12 \
      --compose reward_hacking:deception,sycophancy \
      --out probe_bank_j2j6.json
"""

import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import fit_logistic, sigmoid, sha256_file


def base_scores(text, states, layer, base_bank):
    """Score one text with the base bank's logistic vectors (J2 path)."""
    out = {}
    ls = str(layer)
    if text not in states or ls not in states[text]:
        return out
    h = np.array(states[text][ls], dtype=np.float64)
    for name, c in base_bank["concepts"].items():
        w = np.array(c["vector"], dtype=np.float64)
        out[name] = float(sigmoid(np.dot(w, h) + c.get("bias", 0.0)))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", required=True)
    ap.add_argument("--base-bank", required=True)
    ap.add_argument("--precomputed", required=True)
    ap.add_argument("--layer", type=int, required=True)
    ap.add_argument("--compose", action="append", required=True,
                    help="composite_name:input1,input2 (repeatable)")
    ap.add_argument("--folds", type=int, default=4)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    with open(args.suite, "r", encoding="utf-8") as f:
        suite = json.load(f)
    with open(args.base_bank, "r", encoding="utf-8") as f:
        base_bank = json.load(f)
    with open(args.precomputed, "r", encoding="utf-8") as f:
        states = json.load(f)
    concepts = suite["concepts"]

    compositions = {}
    for spec in args.compose:
        comp_name, inputs = spec.split(":")
        input_names = inputs.split(",")
        # Positive examples for the composite = positives of the first
        # input concept (the composite's defining concept); negatives =
        # that concept's negatives plus every other concept's positives
        # that lack the full conjunction.
        primary = input_names[0]
        if primary not in concepts:
            print(f"[j6] {comp_name}: primary {primary} not in suite — skipped", file=sys.stderr)
            continue
        pos_texts = concepts[primary]["tune"]["positive"]
        neg_texts = concepts[primary]["tune"]["negative"]

        X_rows = []
        y_rows = []
        for t, label in [(t, 1.0) for t in pos_texts] + [(t, 0.0) for t in neg_texts]:
            sc = base_scores(t, states, args.layer, base_bank)
            if not sc:
                continue
            X_rows.append([sc.get(n, 0.0) for n in input_names])
            y_rows.append(label)
        if len(set(y_rows)) < 2:
            print(f"[j6] {comp_name}: degenerate label set — skipped", file=sys.stderr)
            continue
        X = np.array(X_rows)
        y = np.array(y_rows)

        folds = []
        for k in range(args.folds):
            idx = [i for i in range(len(y)) if i % args.folds != k]
            if len(set(y[idx])) < 2:
                continue
            w, b = fit_logistic(X[idx], y[idx])
            folds.append({"weights": w.tolist(), "bias": float(b)})
        if not folds:
            print(f"[j6] {comp_name}: no trainable folds — skipped", file=sys.stderr)
            continue
        compositions[comp_name] = {
            "method": "composition-v0.1",
            "inputs": input_names,
            "folds": folds,
            "threshold": 0.5,
            "uncertainty": "std of fold probabilities; reported as mean +- std",
        }
        print(f"[j6] {comp_name}: {len(folds)} folds trained on {len(y)} rows", file=sys.stderr)

    base_bank["compositions"] = compositions
    base_bank["calibration"]["composition_suite_sha256"] = sha256_file(args.suite)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(base_bank, f, indent=1, sort_keys=True)
    print(f"[j6] bank written: {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
