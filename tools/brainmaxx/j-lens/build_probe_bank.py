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


def build_bank(model_id, examples_path, layer, device):
    with open(examples_path, "r", encoding="utf-8") as f:
        examples = json.load(f)

    print(f"[build_probe_bank] loading {model_id} ...", file=sys.stderr)
    tokenizer = AutoTokenizer.from_pretrained(model_id)
    model = AutoModelForCausalLM.from_pretrained(model_id, torch_dtype=torch.float32)
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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True, help="HF model id or local path")
    ap.add_argument("--examples", required=True, help="path to contrastive-examples JSON")
    ap.add_argument("--layer", type=int, required=True, help="hidden_states index to probe")
    ap.add_argument("--out", required=True, help="output probe_bank.json path")
    ap.add_argument("--device", default="cpu", help="cpu | cuda | mps")
    args = ap.parse_args()

    bank = build_bank(args.model, args.examples, args.layer, args.device)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(bank, f, indent=2)
    print(f"[build_probe_bank] wrote {args.out} — {len(bank['concepts'])} concepts", file=sys.stderr)


if __name__ == "__main__":
    main()
