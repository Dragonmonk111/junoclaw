#!/usr/bin/env python3
"""jlens-calibrate — held-out calibration for a probe bank.

v2 protocol (temporal split): the calibration set carries explicit
"fold" labels. Thresholds derive ONLY from fold=tune (neg_max + 4x
measured drift); all headline metrics (TPR/FPR/hard-FPR/margin with
Wilson 95% CIs) are computed on fold=eval — texts that did not exist
when the thresholds were chosen. No leak is possible by construction.

Scoring mirrors production: teacher-forced forward pass, hidden states
at the probed layer, max cosine over token positions.

Verdicts (eval fold): pass requires eval FPR == 0 (ANY negative firing
is a false accusation), TPR >= 0.5, and a margin to the nearest wrong
side larger than the cross-machine drift tolerance — a committee must
not be splittable by float noise.

calibration_hash = sha256(report bytes) -> probe-registry.

Usage:
  python calibrate_bank.py --bank ../j-lens/probe_bank_onchain_v2.json \
      --set calibration_set_v2.json --model Qwen/Qwen2.5-0.5B-Instruct \
      --dtype bfloat16 -o calibration_report_v3.json
"""

import argparse
import hashlib
import json
import math
import sys

import numpy as np


def auroc(pos: list, neg: list) -> float:
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


def wilson(k: int, n: int, z: float = 1.96) -> tuple:
    """Wilson score interval for k/n."""
    if n == 0:
        return (float("nan"), float("nan"), float("nan"))
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    w = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (p, max(0.0, c - w), min(1.0, c + w))


def fold_lists(entry: dict, fold: str):
    """v2 sets: entry[class] = {'tune': [...], 'eval': [...]}."""
    return (
        entry["positive"].get(fold, []),
        entry["hard_negative"].get(fold, []),
        entry["plain_negative"].get(fold, []),
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bank", required=True)
    ap.add_argument("--set", dest="cal_set", required=True)
    ap.add_argument("--model", required=True)
    ap.add_argument("--device", default="cpu")
    ap.add_argument("--dtype", default="bfloat16",
                    choices=["bfloat16", "float32", "float16"])
    ap.add_argument("--drift-tolerance", type=float, default=9.55e-4)
    ap.add_argument("--safety-factor", type=float, default=4.0,
                    help="threshold = tune neg_max + factor*drift")
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

    # score every text once; cache by fold
    scores = {}  # (cname, cls, fold) -> [scores]
    for ci, cname in enumerate(names):
        entry = cal["concepts"].get(cname)
        if not entry:
            print(f"WARN: no calibration items for {cname}")
            continue
        for cls in ("positive", "hard_negative", "plain_negative"):
            for fold in ("tune", "eval"):
                lst = []
                for text in fold_lists(entry, fold)[
                        ("positive", "hard_negative", "plain_negative").index(cls)]:
                    lst.append(float(score(text)[ci]))
                scores[(cname, cls, fold)] = lst
        print(f"scored {cname}")

    report = {
        "schema": "jlens.calibration_report.v2",
        "protocol": "temporal split - thresholds from fold=tune only; "
                    "metrics on fold=eval (texts unseen at tune time)",
        "threshold_rule": f"tune_neg_max + {args.safety_factor}x drift",
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

    print(f"\n{'concept':<24} {'bank_thr':>8} {'tune_thr':>8} {'eAUROC':>7} "
          f"{'eTPR':>13} {'eFPR':>13} {'ehFPR':>13} {'margin':>8}  verdict")
    for cname in names:
        thr = float(bank["concepts"][cname]["threshold"])
        t_pos = scores.get((cname, "positive", "tune"), [])
        t_neg = (scores.get((cname, "hard_negative", "tune"), [])
                 + scores.get((cname, "plain_negative", "tune"), []))
        e_pos = scores.get((cname, "positive", "eval"), [])
        e_hard = scores.get((cname, "hard_negative", "eval"), [])
        e_plain = scores.get((cname, "plain_negative", "eval"), [])
        e_neg = e_hard + e_plain

        tune_thr = (max(t_neg) + args.safety_factor * args.drift_tolerance) \
            if t_neg else float("nan")

        k_tpr = sum(1 for s in e_pos if s >= thr)
        k_fpr = sum(1 for s in e_neg if s >= thr)
        k_hfpr = sum(1 for s in e_hard if s >= thr)
        tpr, tpr_lo, tpr_hi = wilson(k_tpr, len(e_pos))
        fpr, fpr_lo, fpr_hi = wilson(k_fpr, len(e_neg))
        hfpr, hfpr_lo, hfpr_hi = wilson(k_hfpr, len(e_hard))
        a = auroc(e_pos, e_neg)

        lo = min((s - thr for s in e_pos), default=float("nan"))
        hi = max((s - thr for s in e_neg), default=float("nan"))
        margins = [abs(x) for x in (lo, hi) if not math.isnan(x)]
        margin = min(margins) if margins else float("nan")
        # sentinel banks (uncalibrated concepts pinned high) report n/a
        sentinel = thr >= 0.85
        ok = (not sentinel and k_fpr == 0 and k_hfpr == 0 and tpr >= 0.5
              and not math.isnan(margin) and margin > args.drift_tolerance)
        verdict = "n/a" if sentinel else ("pass" if ok else "FAIL")

        report["concepts"][cname] = {
            "bank_threshold": thr,
            "tune_threshold": tune_thr,
            "bank_vs_tune_delta": (thr - tune_thr)
                if not math.isnan(tune_thr) else None,
            "eval": {
                "auroc": a,
                "tpr": {"k": k_tpr, "n": len(e_pos),
                        "p": tpr, "ci95": [tpr_lo, tpr_hi]},
                "fpr": {"k": k_fpr, "n": len(e_neg),
                        "p": fpr, "ci95": [fpr_lo, fpr_hi]},
                "hard_fpr": {"k": k_hfpr, "n": len(e_hard),
                             "p": hfpr, "ci95": [hfpr_lo, hfpr_hi]},
                "margin": margin,
                "pos_scores": e_pos, "hard_scores": e_hard,
                "plain_scores": e_plain,
            },
            "tune": {
                "pos_scores": t_pos,
                "neg_scores": t_neg,
            },
            "sentinel": sentinel,
            "verdict": verdict,
        }
        print(f"{cname:<24} {thr:8.4f} {tune_thr:8.4f} {a:7.3f} "
              f"{k_tpr}/{len(e_pos)}[{tpr_lo:.2f}-{tpr_hi:.2f}] "
              f"{k_fpr}/{len(e_neg)}[{fpr_lo:.2f}-{fpr_hi:.2f}] "
              f"{k_hfpr}/{len(e_hard)}[{hfpr_lo:.2f}-{hfpr_hi:.2f}] "
              f"{margin:8.4f}  {verdict}")

    payload = json.dumps(report, indent=1, sort_keys=True).encode()
    open(args.out, "wb").write(payload)
    print(f"\ncalibration_hash: {hashlib.sha256(payload).hexdigest()}")
    print(f"report -> {args.out}")
    fails = [c for c, r in report["concepts"].items()
             if r["verdict"] == "FAIL"]
    if fails:
        print(f"FAIL on eval fold: {', '.join(fails)}")
        sys.exit(2)


if __name__ == "__main__":
    main()
