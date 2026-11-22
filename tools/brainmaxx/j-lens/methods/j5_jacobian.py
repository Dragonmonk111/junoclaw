"""J5 — the Jacobian lens: causal gradient directions.

v_{c,l} = E_x[ d logprob_c(f(x)) / d h_l(x) ]

The expected gradient of the concept's target-phrase log-prob with respect
to the hidden state at layer l — the direction in activation space that
*causally moves the output toward the concept*, measured through the
frozen model's own downstream layers. J1/J2 find where classes differ;
J5 finds what changes the answer.

Autograd happens once, at bank build, on the positive calibration inputs.
At inference the readout is still a dot product — runtime cost unchanged.

Bank method tag: "jacobian-v0.1" (multi-layer; runtime shares the
trajectory scorer with slope_weight=0 and uniform layer weights).

Real path: --model runs forward+backward over the suite's positive examples.
Test path: --precomputed-grads grads.json maps text -> {layer: [gradient]}
(fixture gradients exercise the normalization/averaging math without a GPU).

Usage:
  py -3 methods/j5_jacobian.py --suite calibration/calibration_suite_v1.json \
      --layers 4,8,12,16 --model Qwen/Qwen2.5-0.5B-Instruct --out probe_bank_j5.json
"""

import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import bank_header, sha256_file


def jacobian_grads(model_id, texts, layers, target_phrase, device="cpu", dtype="float32"):
    """Mean gradient of target-phrase log-prob at the last position wrt
    hidden states at the requested layers. Lazy torch import."""
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer

    dt = {"float32": torch.float32, "bfloat16": torch.bfloat16, "float16": torch.float16}[dtype]
    tok = AutoTokenizer.from_pretrained(model_id)
    model = AutoModelForCausalLM.from_pretrained(model_id, torch_dtype=dt)
    model.to(device)
    model.eval()
    target_ids = tok(target_phrase, add_special_tokens=False)["input_ids"]

    grads = {}
    for text in texts:
        inputs = tok(text, return_tensors="pt").to(device)
        out = model(**inputs, output_hidden_states=True)
        kept = [out.hidden_states[l] for l in layers]
        for h in kept:
            h.retain_grad()
        logits = model.lm_head(out.hidden_states[-1]) if hasattr(model, "lm_head") else out.logits
        last = logits[0, -1, :]
        # Log-prob of the target phrase's first token as the concept scalar.
        logp = torch.log_softmax(last, dim=-1)[target_ids[0]]
        model.zero_grad()
        logp.backward()
        grads[text] = {
            str(l): h.grad[0, -1, :].float().cpu().numpy().tolist()
            for l, h in zip(layers, kept)
        }
    return grads


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", required=True)
    ap.add_argument("--layers", required=True)
    ap.add_argument("--model", default=None)
    ap.add_argument("--precomputed-grads", default=None)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    layers = [int(x) for x in args.layers.split(",")]
    with open(args.suite, "r", encoding="utf-8") as f:
        suite = json.load(f)
    suite_hash = sha256_file(args.suite)
    concepts = suite["concepts"]

    if args.precomputed_grads:
        with open(args.precomputed_grads, "r", encoding="utf-8") as f:
            grad_data = json.load(f)
    else:
        grad_data = None

    out_concepts = {}
    for name in sorted(concepts):
        positives = concepts[name]["tune"]["positive"]
        if grad_data is not None:
            grads = {t: grad_data[t] for t in positives if t in grad_data}
        else:
            if not args.model:
                raise ValueError("need --model or --precomputed-grads")
            grads = jacobian_grads(args.model, positives, layers, name.replace("_", " "))
        if not grads:
            print(f"[j5] {name}: no gradients available — skipped", file=sys.stderr)
            continue

        per_layer = {}
        for l in layers:
            ls = str(l)
            vecs = [np.array(g[ls], dtype=np.float64) for g in grads.values() if ls in g]
            if not vecs:
                continue
            d = np.mean(np.stack(vecs), axis=0)
            n = np.linalg.norm(d)
            if n == 0:
                continue
            per_layer[ls] = (d / n).tolist()
        if not per_layer:
            continue
        out_concepts[name] = {
            "method": "jacobian-v0.1",
            "layers": per_layer,
            "layer_weights": {ls: 1.0 / len(per_layer) for ls in per_layer},
            "slope_weight": 0.0,
            "threshold": 0.5,
            "calibrated": True,
            "certified_on": suite["suite_version"],
            "causal_note": "expected gradient of target-phrase log-prob wrt h_l; normalized",
        }
        print(f"[j5] {name}: {len(per_layer)} layers, {len(grads)} examples", file=sys.stderr)

    bank = bank_header("jacobian-v0.1", args.model or "precomputed", layers,
                       calibration={"suite": os.path.basename(args.suite), "suite_sha256": suite_hash})
    bank["concepts"] = out_concepts
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(bank, f, indent=1, sort_keys=True)
    print(f"[j5] bank written: {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
