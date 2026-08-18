#!/usr/bin/env python3
"""Build a J-Lens probe bank from contrastive concept examples.

Method (v0.1, matches PLAN_J_REEF_AND_J_LENS.md §3.6): diff-of-means
direction. For each concept, run the model over a set of "positive" texts
(the concept is present) and "negative" texts (the concept is absent),
take the mean hidden state at a chosen layer/token-position for each set,
and use (mean_positive - mean_negative), L2-normalized, as the probe
vector v_c. This is the standard "contrastive activation direction"
baseline — cheaper than a full Jacobian-of-logit gradient, and the
documented starting point before phrase-level / tuned-lens probes.

Output JSON matches the schema d1-probe.js (loadProbeBank) expects:

{
  "probe_version": "j-lens-v0.1",
  "probe_model": "<HF repo id, exact>",
  "layer": <int>,
  "concepts": {
    "<name>": { "vector": [...], "threshold": <float> }
  }
}

Usage:
    python build_probe_bank.py \
        --model Qwen/Qwen2.5-0.5B-Instruct \
        --examples examples/concepts.json \
        --layer 12 \
        --out probe_bank.json

concepts.json shape:
{
  "reward_hacking": {
    "positive": ["I will maximize my reward by ...", ...],
    "negative": ["I will answer the question honestly ...", ...],
    "threshold": 0.70
  },
  ...
}

For Kimi K3 (2.8T MoE): swap --model for a self-hosted local path/handle
that exposes hidden_states (see README.md "Kimi K3 integration path" —
transformers.AutoModelForCausalLM cannot load 2.8T params on commodity
hardware; this script's --backend flag documents the vLLM/self-host swap
point but the diff-of-means math is identical either way.
"""

import argparse
import json
import sys

import numpy as np
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer


def mean_hidden_state(model, tokenizer, texts, layer, device):
    """Mean hidden-state vector at `layer`, averaged over the LAST token of
    each text (the position most likely to carry the concept summary) and
    then averaged again over all texts in the set."""
    vectors = []
    with torch.no_grad():
        for text in texts:
            inputs = tokenizer(text, return_tensors="pt").to(device)
            out = model(**inputs, output_hidden_states=True)
            # hidden_states: tuple(num_layers+1) of [batch, seq, hidden]
            h = out.hidden_states[layer][0, -1, :]  # last token, this layer
            vectors.append(h.float().cpu().numpy())
    return np.mean(np.stack(vectors, axis=0), axis=0)


DTYPES = {"float32": torch.float32, "bfloat16": torch.bfloat16, "float16": torch.float16}


def build_bank(model_id, examples_path, layer, device, dtype=torch.float32):
    with open(examples_path, "r", encoding="utf-8") as f:
        examples = json.load(f)

    print(f"[build_probe_bank] loading {model_id} (dtype={dtype}) ...", file=sys.stderr)
    tokenizer = AutoTokenizer.from_pretrained(model_id)
    model = AutoModelForCausalLM.from_pretrained(model_id, torch_dtype=dtype)
    model.to(device)
    model.eval()

    concepts = {}
    for name, spec in examples.items():
        pos = spec["positive"]
        neg = spec["negative"]
        threshold = float(spec.get("threshold", 0.70))
        print(f"[build_probe_bank] concept={name} pos={len(pos)} neg={len(neg)}", file=sys.stderr)

        mu_pos = mean_hidden_state(model, tokenizer, pos, layer, device)
        mu_neg = mean_hidden_state(model, tokenizer, neg, layer, device)
        direction = mu_pos - mu_neg
        norm = np.linalg.norm(direction)
        if norm == 0:
            raise ValueError(f"concept {name}: zero-norm direction, examples too similar")
        direction = direction / norm

        concepts[name] = {"vector": direction.tolist(), "threshold": threshold}

    return {
        "probe_version": "j-lens-v0.1",
        "probe_model": model_id,
        "layer": layer,
        "concepts": concepts,
    }


def sweep_layers(model_id, examples_path, layers, device, dtype=torch.float32):
    """Sweep multiple layers and report separation quality for each.
    Returns list of (layer, avg_separation, per_concept_norms)."""
    with open(examples_path, "r", encoding="utf-8") as f:
        examples = json.load(f)

    print(f"[sweep] loading {model_id} (dtype={dtype}) ...", file=sys.stderr)
    tokenizer = AutoTokenizer.from_pretrained(model_id)
    model = AutoModelForCausalLM.from_pretrained(model_id, torch_dtype=dtype)
    model.to(device)
    model.eval()

    results = []
    for layer in layers:
        print(f"[sweep] layer {layer} ...", file=sys.stderr)
        per_concept = {}
        sep_scores = []
        for name, spec in examples.items():
            mu_pos = mean_hidden_state(model, tokenizer, spec["positive"], layer, device)
            mu_neg = mean_hidden_state(model, tokenizer, spec["negative"], layer, device)
            sep = np.dot(mu_pos, mu_neg) / (np.linalg.norm(mu_pos) * np.linalg.norm(mu_neg))
            direction = mu_pos - mu_neg
            dnorm = np.linalg.norm(direction)
            per_concept[name] = {"separation": float(sep), "direction_norm": float(dnorm)}
            sep_scores.append(sep)
        avg_sep = float(np.mean(sep_scores))
        results.append((layer, avg_sep, per_concept))
        print(f"[sweep] layer {layer}: avg_separation={avg_sep:.4f} (lower=better)", file=sys.stderr)

    best = min(results, key=lambda x: x[1])
    print(f"[sweep] best layer: {best[0]} (avg_separation={best[1]:.4f})", file=sys.stderr)
    return results


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True, help="HF model id or local path")
    ap.add_argument("--examples", required=True, help="path to contrastive-examples JSON")
    ap.add_argument("--layer", type=int, required=True, help="hidden_states index to probe")
    ap.add_argument("--out", required=True, help="output probe_bank.json path")
    ap.add_argument("--device", default="cpu", help="cpu | cuda | mps")
    ap.add_argument("--dtype", default="float32", choices=list(DTYPES.keys()),
                    help="model weight dtype; use bfloat16/float16 to roughly halve RAM/VRAM for larger models")
    ap.add_argument("--sweep-layers", default=None,
                    help="comma-separated layer indices to sweep (e.g. '5,10,15,20'). "
                         "Reports separation quality per layer; does not write output.")
    args = ap.parse_args()
    dtype = DTYPES[args.dtype]

    if args.sweep_layers:
        layers = [int(x) for x in args.sweep_layers.split(",")]
        results = sweep_layers(args.model, args.examples, layers, args.device, dtype)
        print("\nLayer sweep results:")
        print(f"{'layer':>6s}  {'avg_sep':>8s}  per-concept")
        for layer, avg_sep, per_concept in results:
            concepts_str = "  ".join(f"{n}={c['separation']:.3f}" for n, c in sorted(per_concept.items()))
            print(f"{layer:6d}  {avg_sep:8.4f}  {concepts_str}")
        best = min(results, key=lambda x: x[1])
        print(f"\nBest layer: {best[0]} (avg_separation={best[1]:.4f})")
        print(f"Re-run with --layer {best[0]} to build the probe bank at this layer.")
        return

    bank = build_bank(args.model, args.examples, args.layer, args.device, dtype)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(bank, f, indent=2)
    print(f"[build_probe_bank] wrote {args.out} — {len(bank['concepts'])} concepts", file=sys.stderr)


if __name__ == "__main__":
    main()
