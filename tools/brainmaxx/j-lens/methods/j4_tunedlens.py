"""J4 — tuned lens: per-layer affine translators into final residual space.

Fits A_l, b_l so that (A_l h_l + b_l) approximates h_final on the suite
texts — ridge regression, closed form, deterministic. Decoding an
intermediate state then uses the model's own unembedding U (public model
weights, pinned by unembed_hash): U @ (A_l h_l + b_l) gives a vocabulary
distribution at any layer — "what token would the model say *now*".

Evidence is human-readable: the top-k tokens per layer per span, not a
scalar score. Still learned, not causal (J5 is the causal tier).

Bank method tag: "tuned-lens-v0.1". The bank stores only translators;
U never enters the bank (it is the model's own weight matrix).

Usage:
  py -3 methods/j4_tunedlens.py --suite calibration/calibration_suite_v1.json \
      --layers 4,8,12,16 --final-layer 24 --precomputed states.json --out probe_bank_j4.json
"""

import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import bank_header, collect_states, sha256_file


def fit_affine(H, T, ridge=1e-3):
    """Ridge fit minimizing ||H @ A_row + b - T||^2 (row-wise samples).
    Closed form: A_row = (H^T H + ridge I)^-1 H^T T.
    Returns (A_col, b) with column convention: y = A_col @ h + b, i.e.
    A_col = A_row.T. Callers applying H @ A.T + b get the same map.
    """
    n = H.shape[0]
    d_in = H.shape[1]
    Hc = H - H.mean(axis=0, keepdims=True)
    Tc = T - T.mean(axis=0, keepdims=True)
    G = Hc.T @ Hc + ridge * n * np.eye(d_in)
    A = np.linalg.solve(G, Hc.T @ Tc)
    b = T.mean(axis=0) - H.mean(axis=0) @ A
    return A.T, b


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", required=True)
    ap.add_argument("--layers", required=True)
    ap.add_argument("--final-layer", type=int, required=True)
    ap.add_argument("--model", default=None)
    ap.add_argument("--precomputed", default=None)
    ap.add_argument("--unembed-hash", default=None, help="sha256 of the model's unembedding matrix file (pin)")
    ap.add_argument("--ridge", type=float, default=1e-3)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    layers = [int(x) for x in args.layers.split(",")]
    with open(args.suite, "r", encoding="utf-8") as f:
        suite = json.load(f)
    suite_hash = sha256_file(args.suite)
    concepts = suite["concepts"]

    all_texts = sorted({t for c in concepts.values() for fold in ("tune", "eval")
                        for t in c[fold]["positive"] + c[fold]["negative"]})
    states = collect_states(all_texts, layers + [args.final_layer],
                            precomputed=args.precomputed, model_id=args.model)
    fl = str(args.final_layer)

    translators = {}
    report = {}
    for l in layers:
        ls = str(l)
        H = []
        T = []
        for t in all_texts:
            if t in states and ls in states[t] and fl in states[t]:
                H.append(states[t][ls])
                T.append(states[t][fl])
        if len(H) < 2:
            print(f"[j4] layer {l}: insufficient paired states — skipped", file=sys.stderr)
            continue
        H = np.array(H, dtype=np.float64)
        T = np.array(T, dtype=np.float64)
        A, b = fit_affine(H, T, ridge=args.ridge)
        resid = H @ A.T + b - T
        rel = float(np.linalg.norm(resid) / max(1e-12, np.linalg.norm(T)))
        translators[ls] = {"A": A.tolist(), "b": b.tolist()}
        report[ls] = {"samples": len(H), "relative_residual": round(rel, 6)}
        print(f"[j4] layer {l}: fit on {len(H)} samples, relative residual {rel:.4f}", file=sys.stderr)

    bank = bank_header("tuned-lens-v0.1", args.model or "precomputed", layers,
                       calibration={"suite": os.path.basename(args.suite), "suite_sha256": suite_hash})
    bank["final_layer"] = args.final_layer
    bank["translators"] = translators
    bank["unembed_hash"] = args.unembed_hash
    bank["build_report"] = report
    bank["evidence_note"] = (
        "Decode via U @ (A_l h_l + b_l) using the model's own unembedding "
        "(pinned by unembed_hash). Evidence = top-k vocabulary tokens per "
        "layer per span — human-readable, learned not causal."
    )
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(bank, f, indent=1, sort_keys=True)
    print(f"[j4] bank written: {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
