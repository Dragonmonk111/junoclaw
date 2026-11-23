"""Collect hidden states for every text in a calibration suite, once.

Writes states.json: {text: {"<layer>": [floats]}} — the --precomputed
input every bank builder and the promotion harness accepts. One model
load, one pass per text, resumable (existing texts are skipped).

Usage:
  py -3 methods/collect_states.py --suite calibration/calibration_suite_v1.json \
      --model Qwen/Qwen2.5-0.5B-Instruct --layers 4,8,12,16,24 \
      --out states_qwen05.json [--limit N] [--resume]
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", required=True)
    ap.add_argument("--model", required=True)
    ap.add_argument("--layers", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--device", default="cpu")
    ap.add_argument("--dtype", default="float32")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--resume", action="store_true")
    args = ap.parse_args()

    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer

    with open(args.suite, "r", encoding="utf-8") as f:
        suite = json.load(f)
    texts = sorted({t for c in suite["concepts"].values() for fold in ("tune", "eval")
                    for t in c[fold]["positive"] + c[fold]["negative"]})
    if args.limit:
        texts = texts[: args.limit]
    layers = [int(x) for x in args.layers.split(",")]

    out = {}
    if args.resume and os.path.exists(args.out):
        with open(args.out, "r", encoding="utf-8") as f:
            out = json.load(f)
        print(f"[collect] resuming: {len(out)} texts already present", file=sys.stderr)
    texts = [t for t in texts if t not in out]

    dt = {"float32": torch.float32, "bfloat16": torch.bfloat16, "float16": torch.float16}[args.dtype]
    print(f"[collect] loading {args.model} ...", file=sys.stderr)
    tok = AutoTokenizer.from_pretrained(args.model)
    model = AutoModelForCausalLM.from_pretrained(args.model, torch_dtype=dt)
    model.to(args.device)
    model.eval()

    t0 = time.time()
    for i, text in enumerate(texts):
        inputs = tok(text, return_tensors="pt").to(args.device)
        with torch.no_grad():
            r = model(**inputs, output_hidden_states=True)
        out[text] = {
            str(l): r.hidden_states[l][0, -1, :].float().cpu().numpy().tolist()
            for l in layers
        }
        if (i + 1) % 25 == 0 or i + 1 == len(texts):
            with open(args.out, "w", encoding="utf-8") as f:
                json.dump(out, f)
            rate = (i + 1) / max(1e-9, time.time() - t0)
            eta = (len(texts) - i - 1) / max(1e-9, rate) / 60
            print(f"[collect] {i + 1}/{len(texts)} texts ({rate:.2f}/s, eta {eta:.1f} min)", file=sys.stderr)

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(out, f)
    print(f"[collect] done: {len(out)} texts -> {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
