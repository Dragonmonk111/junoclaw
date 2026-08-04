#!/usr/bin/env python3
"""Extract per-token hidden states from a model's forward pass for J-Lens.

Output JSON matches the schema d1-probe.js (loadHiddenStates) expects:

{
  "probe_model": "<HF repo id, exact — must match the probe bank>",
  "layer": <int>,
  "states": [
    { "token": "<decoded token str>", "position": <int>, "vector": [...] },
    ...
  ]
}

Usage:
    python extract_hidden_states.py \
        --model Qwen/Qwen2.5-0.5B-Instruct \
        --text "the draft text to audit" \
        --layer 12 \
        --out hidden_states.json

--probe-model must exactly match the "probe_model" field the probe bank was
built with (d1-probe.js hard-fails on any mismatch — spec §3.5, probes do
not transfer across models).
"""

import argparse
import json
import sys

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer


def extract(model_id, text, layer, device):
    tokenizer = AutoTokenizer.from_pretrained(model_id)
    model = AutoModelForCausalLM.from_pretrained(model_id, torch_dtype=torch.float32)
    model.to(device)
    model.eval()

    inputs = tokenizer(text, return_tensors="pt").to(device)
    with torch.no_grad():
        out = model(**inputs, output_hidden_states=True)

    h = out.hidden_states[layer][0]  # [seq, hidden]
    token_ids = inputs["input_ids"][0]
    states = []
    for pos in range(h.shape[0]):
        token_str = tokenizer.decode([token_ids[pos]])
        states.append({
            "token": token_str,
            "position": pos,
            "vector": h[pos].float().cpu().numpy().tolist(),
        })

    return {"probe_model": model_id, "layer": layer, "states": states}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--text", required=True)
    ap.add_argument("--layer", type=int, required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--device", default="cpu")
    args = ap.parse_args()

    result = extract(args.model, args.text, args.layer, args.device)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2)
    print(f"[extract_hidden_states] wrote {args.out} — {len(result['states'])} tokens", file=sys.stderr)


if __name__ == "__main__":
    main()
