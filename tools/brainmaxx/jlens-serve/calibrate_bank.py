#!/usr/bin/env python3
"""jlens-calibrate — held-out calibration for a probe bank.

Scores every text in a calibration set against every concept direction
in the bank, the same way production does: teacher-forced forward pass,
hidden states at the probed layer, max cosine over token positions.

For each concept reports:
  - AUROC (positives vs all negatives)
  - TPR/FPR at the registered threshold
  - hard-negative FPR separately (the number that actually matters)
  - margin: distance between threshold and the nearest misclassed side,
    compared against the measured cross-machine drift tolerance so a
    committee can't be split by float noise near the boundary

Emits a report JSON; calibration_hash = sha256(report bytes) is what
goes on chain in the probe registry.

Labels: positive=1, hard_negative and plain_negative=0. A concept whose
hard negatives score above threshold is NOT calibrated no matter how
pretty its AUROC is.

Usage:
  python calibrate_bank.py --bank ../j-lens/probe_bank_onchain.json \
      --set calibration_set_v1.json --model Qwen/Qwen2.5-0.5B-Instruct \
      --dtype bfloat16 -o calibration_report_v1.json
"""

import argparse
import hashlib
import json
import math
import sys

import numpy as np


def auroc(pos: list, neg: list) -> float:
    """Rank-based AUROC; ties count half."""
    n_pos, n_neg = len(pos), len(neg)
    if not pos or not neg:
        return float("nan")
    wins = 0.0
    for p in pos:
        for n in neg:
            if p > n:
                wins += 1.0
            elif p == n:
                wins += 0.5
    return wins / (n_pos * n_neg)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bank", required=True)
    ap.add_argument("--set", dest="cal_set", required=True)
    ap.add_argument("--model", required=True)
    ap.add_argument("--device", default="cpu")
    ap.add_argument("--dtype", default="bfloat16",
                    choices=["bfloat16", "float32", "float16"])
    ap.add_argument("--drift-tolerance", type=float, default=9.55e-4,
                    help="measured cross-machine cosine drift (batch-3)")
    ap.add_argument("-o", "--out", required=True)
    args = ap.parse_args()

    bank_raw = open(args.bank, "rb").read()
    bank = json.loads(bank_raw)
    cal_raw = open(args.cal_set, "rb").read()
    cal = json.loads(cal_raw)
    layer = int(bank["layer"])
    names = sorted(bank["concepts"].keys())
    bank_mat = np.stack(
        [np.asarray(bank["concepts"][n]["vector"], dtype=np.float32) for n in names],
        axis=1,
    )
    norms = np.linalg.norm(bank_mat, axis=0)

    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer

    tok = AutoTokenizer.from_pretrained(args.model)
    dt = {"bfloat16": torch.bfloat16, "float32": torch.float32,
          "float16": torch.float16}[args.dtype]
    model = AutoModelForCausalLM.from_pretrained(
        args.model, torch_dtype=dt
    ).to(args.device)
    model.eval()

    def score(text: str) -> np.ndarray:
        ids = tok(text, return_tensors="pt")["input_ids"].to(args.device)
        with torch.no_grad():
            out = model(ids, output_hidden_states=True)
        v = out.hidden_states[layer][0].detach().float().cpu().numpy()
        vn = np.linalg.norm(v, axis=1, keepdims=True)
        return ((v @ bank_mat) / (vn * norms + 1e-9)).max(axis=0)

    # per-concept score lists
    pos_s = {n: [] for n in names}
    hard_s = {n: [] for n in names}
    plain_s = {n: [] for n in names}
    for ci, cname in enumerate(names):
        entry = cal["concepts"].get(cname)
        if not entry:
            print(f"WARN: no calibration items for {cname}")
            continue
        for text in entry["positive"]:
            pos_s[cname].append(float(score(text)[ci]))
        for text in entry["hard_negative"]:
            hard_s[cname].append(float(score(text)[ci]))
        for text in entry["plain_negative"]:
            plain_s[cname].append(float(score(text)[ci]))
        print(f"scored {cname}: {len(entry['positive'])}+"
              f"{len(entry['hard_negative'])}+{len(entry['plain_negative'])}")

    report = {
        "schema": "jlens.calibration_report.v1",
        "model": args.model,
        "model_revision": bank.get("model_revision"),
        "weights_hash": bank.get("weights_hash"),
        "bank_hash": hashlib.sha256(bank_raw).hexdigest(),
        "set_hash": hashlib.sha256(cal_raw).hexdigest(),
        "layer": layer,
        "dtype": args.dtype,
        "device": args.device,
        "drift_tolerance": args.drift_tolerance,
        "concepts": {},
    }

    print(f"\n{'concept':<24} {'thr':>6} {'AUROC':>6} {'TPR':>6} {'FPR':>6} "
          f"{'hFPR':>6} {'margin':>9}  verdict")
    for cname in names:
        thr = float(bank["concepts"][cname]["threshold"])
        pos, hard, plain = pos_s[cname], hard_s[cname], plain_s[cname]
        neg = hard + plain
        tpr = sum(1 for s in pos if s >= thr) / max(len(pos), 1)
        fpr = sum(1 for s in neg if s >= thr) / max(len(neg), 1)
        hfpr = sum(1 for s in hard if s >= thr) / max(len(hard), 1)
        a = auroc(pos, neg)
        # margin: how far the threshold sits from the nearest wrong side
        lo = min((s - thr for s in pos), default=float("nan"))   # pos below thr
        hi = max((s - thr for s in neg), default=float("nan"))   # neg above thr
        margin = min(abs(x) for x in (lo, hi) if not math.isnan(x)) \
            if pos and neg else float("nan")
        ok = (hfpr == 0.0 and tpr >= 0.5 and
              not math.isnan(margin) and margin > args.drift_tolerance)
        verdict = "pass" if ok else "FAIL"
        report["concepts"][cname] = {
            "threshold": thr, "auroc": a, "tpr": tpr, "fpr": fpr,
            "hard_fpr": hfpr, "margin": margin, "verdict": verdict,
            "pos_scores": pos, "hard_scores": hard, "plain_scores": plain,
        }
        print(f"{cname:<24} {thr:6.3f} {a:6.3f} {tpr:6.2f} {fpr:6.2f} "
              f"{hfpr:6.2f} {margin:9.4f}  {verdict}")

    payload = json.dumps(report, indent=1, sort_keys=True).encode()
    open(args.out, "wb").write(payload)
    print(f"\ncalibration_hash: {hashlib.sha256(payload).hexdigest()}")
    print(f"report -> {args.out}")
    fails = [c for c, r in report["concepts"].items() if r["verdict"] != "pass"]
    if fails:
        print(f"NOT calibrated for activation: {', '.join(fails)}")
        sys.exit(2)


if __name__ == "__main__":
    main()
