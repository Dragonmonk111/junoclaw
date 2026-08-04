#!/usr/bin/env python3
"""
probe-batch-domain.py — J-Lens domain probe batch runner

Runs contrastive pairs from a domain pairs file against an Akash GPU
endpoint across multiple layers, computes separation scores per layer,
and saves full results including raw vectors for probe bank building.

Usage:
  python3 probe-batch-domain.py \
    --endpoint http://akash-endpoint:8000 \
    --pairs probe-banks/medical.pairs.json \
    --model "Qwen/Qwen2.5-14B-Instruct" \
    --layers 10,20,30,40,47 \
    --output probe-batch-medical-results.json
"""

import argparse
import json
import math
import sys
import urllib.request


def extract(endpoint, text, layer):
    body = json.dumps({"text": text, "layer": layer}).encode()
    req = urllib.request.Request(
        f"{endpoint.rstrip('/')}/extract_hidden_states",
        data=body,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=300) as r:
        return json.load(r)


def cosine(a, b):
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(x * x for x in b))
    return dot / (na * nb) if na and nb else 0.0


def norm(a):
    return math.sqrt(sum(x * x for x in a))


def main():
    parser = argparse.ArgumentParser(description="Run J-Lens domain probe batch on Akash GPU")
    parser.add_argument("--endpoint", required=True, help="Akash GPU endpoint")
    parser.add_argument("--pairs", required=True, help="Domain pairs JSON file")
    parser.add_argument("--model", required=True, help="Model identifier")
    parser.add_argument("--layers", required=True, help="Comma-separated layer indices")
    parser.add_argument("--output", required=True, help="Output results JSON path")
    args = parser.parse_args()

    layers = [int(x) for x in args.layers.split(",")]

    with open(args.pairs) as f:
        pairs_data = json.load(f)
    pairs = pairs_data["pairs"]
    domain = pairs_data.get("domain", "unknown")

    print(f"[batch] domain={domain} model={args.model} pairs={len(pairs)} layers={layers}")
    print(f"[batch] endpoint={args.endpoint}")
    print()

    results = {}
    for layer in layers:
        results[layer] = {
            "cos": [],
            "norm_t": [],
            "norm_d": [],
            "truth_vecs": [],
            "dec_vecs": [],
            "pair_names": [],
        }
        for pair in pairs:
            name = pair["name"]
            rt = extract(args.endpoint, pair["truth"], layer)
            rd = extract(args.endpoint, pair["deceptive"], layer)
            vt = rt["states"][-1]["vector"]
            vd = rd["states"][-1]["vector"]
            c = cosine(vt, vd)
            results[layer]["cos"].append(c)
            results[layer]["norm_t"].append(norm(vt))
            results[layer]["norm_d"].append(norm(vd))
            results[layer]["truth_vecs"].append(vt)
            results[layer]["dec_vecs"].append(vd)
            results[layer]["pair_names"].append(name)
            print(
                f"  [layer {layer:2d}] {name:30s} cos={c:.4f} |T|={norm(vt):7.1f} |D|={norm(vd):7.1f}",
                flush=True,
            )

    print()
    print(f"=== PER-LAYER SUMMARY (domain={domain}, n={len(pairs)} pairs) ===")
    print(f"{'layer':>5} {'mean_cos':>9} {'mean_|T|':>9} {'mean_|D|':>9} {'sep_score':>10}")
    summary = {}
    for layer in layers:
        r = results[layer]
        mc = sum(r["cos"]) / len(r["cos"])
        mt = sum(r["norm_t"]) / len(r["norm_t"])
        md = sum(r["norm_d"]) / len(r["norm_d"])
        dims = len(r["truth_vecs"][0])
        n = len(pairs)
        dvec = [
            sum(v[i] for v in r["truth_vecs"]) / n - sum(v[i] for v in r["dec_vecs"]) / n
            for i in range(dims)
        ]
        sep = norm(dvec) / ((mt + md) / 2)
        summary[layer] = {
            "mean_cos": mc,
            "mean_norm_truth": mt,
            "mean_norm_dec": md,
            "sep_score": sep,
            "vector_dim": dims,
        }
        print(f"{layer:>5} {mc:>9.4f} {mt:>9.1f} {md:>9.1f} {sep:>10.4f}")

    best = max(summary, key=lambda l: summary[l]["sep_score"])
    print(f"\nBest probing layer: {best} (sep_score={summary[best]['sep_score']:.4f})")

    output = {
        "domain": domain,
        "model": args.model,
        "endpoint": args.endpoint,
        "layers": layers,
        "pairs": len(pairs),
        "summary": summary,
        "best_layer": best,
        "raw_vectors": {
            str(layer): {
                "truth_vecs": results[layer]["truth_vecs"],
                "dec_vecs": results[layer]["dec_vecs"],
                "pair_names": results[layer]["pair_names"],
            }
            for layer in layers
        },
    }

    with open(args.output, "w") as f:
        json.dump(output, f, indent=2)
    print(f"\nSaved {args.output}")


if __name__ == "__main__":
    main()
