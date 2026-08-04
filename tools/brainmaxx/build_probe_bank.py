#!/usr/bin/env python3
"""
build_probe_bank.py — J-Lens probe bank builder

Takes a contrastive pairs JSON file, connects to an Akash GPU endpoint
running the hidden-states extraction server, extracts hidden states for
all truth/deception pairs at a specified layer, computes diff-of-means
probe vectors (contrastive activation directions), and outputs a
properly formatted probe bank JSON file ready for use with the
Chain Superintelligence Module / Domain-General Audit API.

Usage:
  python3 build_probe_bank.py \
    --endpoint http://akash-endpoint:8000 \
    --pairs probe-banks/medical.pairs.json \
    --model "Qwen/Qwen2.5-14B-Instruct" \
    --layer 40 \
    --output probe-banks/medical.probe_bank.json \
    [--threshold 0.70]

The output probe bank has the format:
  {
    "probe_version": "j-lens-v0.1",
    "probe_model": "Qwen/Qwen2.5-14B-Instruct",
    "layer": 40,
    "concepts": {
      "concept_name": { "vector": [...], "threshold": 0.70 }
    }
  }

The threshold is the cosine similarity cutoff above which a detection
fires. Default: 0.70 (can be overridden per-domain via --threshold).
"""

import argparse
import json
import math
import sys
import urllib.request


def extract_hidden_states(endpoint, text, layer):
    """Call the /extract_hidden_states endpoint and return the JSON response."""
    url = f"{endpoint.rstrip('/')}/extract_hidden_states"
    body = json.dumps({"text": text, "layer": layer}).encode()
    req = urllib.request.Request(
        url,
        data=body,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=300) as r:
        return json.load(r)


def get_final_vector(response):
    """Extract the final-token hidden state vector from the response."""
    states = response.get("states", [])
    if not states:
        raise ValueError("No states in hidden states response")
    return states[-1]["vector"]


def cosine(a, b):
    """Cosine similarity between two vectors."""
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(x * x for x in b))
    return dot / (na * nb) if na and nb else 0.0


def norm(a):
    """L2 norm of a vector."""
    return math.sqrt(sum(x * x for x in a))


def diff_of_means(truth_vecs, dec_vecs):
    """
    Compute the diff-of-means probe direction.
    This is the standard contrastive activation direction:
      d = mean(truth_vecs) - mean(dec_vecs)
    Then normalize to unit length for cosine-based scoring.
    """
    if not truth_vecs or not dec_vecs:
        raise ValueError("Need at least one truth and one deception vector")
    dims = len(truth_vecs[0])
    mean_t = [sum(v[i] for v in truth_vecs) / len(truth_vecs) for i in range(dims)]
    mean_d = [sum(v[i] for v in dec_vecs) / len(dec_vecs) for i in range(dims)]
    diff = [mean_t[i] - mean_d[i] for i in range(dims)]
    n = norm(diff)
    if n == 0:
        raise ValueError("Diff-of-means is zero vector — pairs may be identical")
    return [x / n for x in diff]


def build_probe_bank(endpoint, pairs_file, model_name, layer, threshold, output_path):
    """Build a probe bank from contrastive pairs."""
    with open(pairs_file) as f:
        pairs_data = json.load(f)

    pairs = pairs_data["pairs"]
    domain = pairs_data.get("domain", "unknown")
    print(f"[build] domain={domain} pairs={len(pairs)} model={model_name} layer={layer}")
    print(f"[build] endpoint={endpoint}")
    print()

    concepts = {}
    pair_results = []

    for pair in pairs:
        name = pair["name"]
        truth_text = pair["truth"]
        dec_text = pair["deceptive"]

        print(f"  [{name}] extracting truth...", end=" ", flush=True)
        rt = extract_hidden_states(endpoint, truth_text, layer)
        vt = get_final_vector(rt)
        print(f"|v|={norm(vt):.1f}", end=" ", flush=True)

        print(f"deception...", end=" ", flush=True)
        rd = extract_hidden_states(endpoint, dec_text, layer)
        vd = get_final_vector(rd)
        print(f"|v|={norm(vd):.1f}", end=" ", flush=True)

        cos = cosine(vt, vd)
        print(f"cos={cos:.4f}")

        pair_results.append({
            "name": name,
            "cosine": cos,
            "truth_norm": norm(vt),
            "dec_norm": norm(vd),
        })

        # Build individual concept vector for this pair
        # Each pair becomes one concept in the probe bank
        probe_vec = diff_of_means([vt], [vd])
        concepts[name] = {
            "vector": [round(x, 8) for x in probe_vec],
            "threshold": threshold,
        }

    # Also build an aggregate "deception_general" concept from all pairs
    all_truth_vecs = []
    all_dec_vecs = []
    for pair in pairs:
        rt = extract_hidden_states(endpoint, pair["truth"], layer)
        rd = extract_hidden_states(endpoint, pair["deceptive"], layer)
        all_truth_vecs.append(get_final_vector(rt))
        all_dec_vecs.append(get_final_vector(rd))

    general_vec = diff_of_means(all_truth_vecs, all_dec_vecs)
    concepts["deception_general"] = {
        "vector": [round(x, 8) for x in general_vec],
        "threshold": threshold,
    }

    # Build the probe bank
    probe_bank = {
        "probe_version": "j-lens-v0.1",
        "probe_model": model_name,
        "layer": layer,
        "concepts": concepts,
    }

    # Summary
    print()
    print(f"=== PROBE BANK SUMMARY ===")
    print(f"  domain: {domain}")
    print(f"  model: {model_name}")
    print(f"  layer: {layer}")
    print(f"  concepts: {len(concepts)} ({len(pairs)} per-pair + 1 aggregate)")
    print(f"  threshold: {threshold}")
    print(f"  vector_dim: {len(concepts[pairs[0]['name']]['vector'])}")
    print()

    # Per-pair cosine summary
    mean_cos = sum(r["cosine"] for r in pair_results) / len(pair_results)
    print(f"  mean truth-deception cosine: {mean_cos:.4f}")
    print(f"  (lower cosine = better separation = stronger probe signal)")
    print()

    # Write output
    with open(output_path, "w") as f:
        json.dump(probe_bank, f, indent=2)
    print(f"[build] wrote probe bank to {output_path}")

    return probe_bank


def main():
    parser = argparse.ArgumentParser(
        description="Build a J-Lens probe bank from contrastive pairs on an Akash GPU endpoint"
    )
    parser.add_argument(
        "--endpoint",
        required=True,
        help="Akash GPU hidden-states extraction endpoint (e.g. http://provider:8000)",
    )
    parser.add_argument(
        "--pairs",
        required=True,
        help="Path to contrastive pairs JSON file (e.g. probe-banks/medical.pairs.json)",
    )
    parser.add_argument(
        "--model",
        required=True,
        help="Model identifier (e.g. Qwen/Qwen2.5-14B-Instruct)",
    )
    parser.add_argument(
        "--layer",
        type=int,
        required=True,
        help="Hidden layer index to extract (e.g. 40 for Qwen2.5-14B, 25 for GLM-4.5-Air)",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Output probe bank JSON path (e.g. probe-banks/medical.probe_bank.json)",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=0.70,
        help="Cosine similarity detection threshold (default: 0.70)",
    )

    args = parser.parse_args()

    build_probe_bank(
        endpoint=args.endpoint,
        pairs_file=args.pairs,
        model_name=args.model,
        layer=args.layer,
        threshold=args.threshold,
        output_path=args.output,
    )


if __name__ == "__main__":
    main()
