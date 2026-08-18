#!/usr/bin/env python3
"""Run the 19 on-chain probe questions through a built probe bank and
report per-concept cosine-similarity detections. This is the local,
non-Colab equivalent of the Colab notebook's "batch audit" cell — same
diff-of-means probe math, same cosine-similarity scoring, just driven
from the CLI against a local/HF model instead of a browser T4 session.

Usage:
    python score_onchain_probes.py \
        --model Qwen/Qwen2.5-0.5B-Instruct \
        --probe-bank probe_bank_onchain.json \
        --questions examples/onchain_probe_questions.json \
        --out j_space_snapshot_onchain.json

Output JSON (the "j_space_snapshot" for this run):
{
  "probe_version": "...",
  "probe_model": "...",
  "layer": <int>,
  "results": [
    {
      "id": "Q1",
      "expected_clean": false,
      "expected_concepts": ["fund_manipulation", ...],
      "detections": {"fund_manipulation": 0.81, ...},   # concepts over threshold
      "scores": {"fund_manipulation": 0.81, "deception": 0.42, ...},  # all scores
      "matched_expected": true
    },
    ...
  ],
  "summary": {"total": 19, "matched_expected": 15, "clean_correct": 4}
}
"""

import argparse
import json
import sys

import numpy as np
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer


def last_token_vector(model, tokenizer, text, layer, device):
    inputs = tokenizer(text, return_tensors="pt").to(device)
    with torch.no_grad():
        out = model(**inputs, output_hidden_states=True)
    h = out.hidden_states[layer][0, -1, :]
    return h.float().cpu().numpy()


def cosine(a, b):
    return float(np.dot(a, b) / (np.linalg.norm(a) * np.linalg.norm(b) + 1e-9))


DTYPES = {"float32": torch.float32, "bfloat16": torch.bfloat16, "float16": torch.float16}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--probe-bank", required=True)
    ap.add_argument("--questions", required=True, help="JSON: list of {id, prompt, expected_clean, expected_concepts}")
    ap.add_argument("--out", required=True)
    ap.add_argument("--device", default="cpu")
    ap.add_argument("--dtype", default="float32", choices=list(DTYPES.keys()))
    args = ap.parse_args()
    dtype = DTYPES[args.dtype]

    with open(args.probe_bank, "r", encoding="utf-8") as f:
        bank = json.load(f)
    with open(args.questions, "r", encoding="utf-8") as f:
        questions = json.load(f)

    if bank["probe_model"] != args.model:
        print(f"WARNING: probe bank built on {bank['probe_model']}, running against {args.model}", file=sys.stderr)

    layer = bank["layer"]
    concepts = bank["concepts"]

    print(f"[score_onchain_probes] loading {args.model} (dtype={dtype}) ...", file=sys.stderr)
    tokenizer = AutoTokenizer.from_pretrained(args.model)
    model = AutoModelForCausalLM.from_pretrained(args.model, torch_dtype=dtype)
    model.to(args.device)
    model.eval()

    results = []
    matched = 0
    clean_correct = 0
    clean_total = 0

    for q in questions:
        vec = last_token_vector(model, tokenizer, q["prompt"], layer, args.device)
        scores = {}
        detections = {}
        for name, spec in concepts.items():
            s = cosine(vec, np.array(spec["vector"]))
            scores[name] = round(s, 4)
            if s >= spec["threshold"]:
                detections[name] = round(s, 4)

        expected_concepts = set(q.get("expected_concepts", []))
        expected_clean = bool(q.get("expected_clean", False))
        fired = set(detections.keys())

        if expected_clean:
            clean_total += 1
            ok = len(fired) == 0
            if ok:
                clean_correct += 1
        else:
            ok = bool(fired & expected_concepts) if expected_concepts else True

        if ok:
            matched += 1

        results.append({
            "id": q["id"],
            "expected_clean": expected_clean,
            "expected_concepts": sorted(expected_concepts),
            "detections": detections,
            "scores": scores,
            "matched_expected": ok,
        })
        print(f"[score] {q['id']}: fired={sorted(fired)} expected={sorted(expected_concepts)} match={ok}", file=sys.stderr)

    snapshot = {
        "probe_version": bank["probe_version"],
        "probe_model": args.model,
        "layer": layer,
        "results": results,
        "summary": {
            "total": len(questions),
            "matched_expected": matched,
            "clean_total": clean_total,
            "clean_correct": clean_correct,
        },
    }

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(snapshot, f, indent=2)

    print(f"\n[score_onchain_probes] wrote {args.out}", file=sys.stderr)
    print(f"[score_onchain_probes] matched {matched}/{len(questions)} expected detections "
          f"({clean_correct}/{clean_total} clean prompts correctly quiet)", file=sys.stderr)


if __name__ == "__main__":
    main()
