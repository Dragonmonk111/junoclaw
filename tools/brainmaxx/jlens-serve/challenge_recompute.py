#!/usr/bin/env python3
"""jlens-challenge — reference dispute-recompute harness.

This is the committee-side machinery of "optimistic activation
verification" (drafts/DESIGN_JLENS_COMMUNITY_GPU_VERIFICATION.md):
given the DISCLOSURE payload (prompt text, exact response token ids,
committed activation blobs) and the pinned weights, recompute the
forward pass independently and decide whether the committed activations
are consistent with it.

Key design point (doc section 6): the recompute is ONE teacher-forced
pass over prompt+response, no autoregression, covering EVERY token
position — so a single challenge verifies the whole batch, and
soundness comes from the batch-level challenge rate, not intra-batch
position sampling.

Checks (each printed ok/FAIL):
  1. prompt text re-tokenizes and hashes to snapshot.prompt_hash
  2. response_ids decode and canonical-hash to snapshot.response_hash
  3. teacher-forced capture count == committed leaf count
  4. per-position recomputed int8 leaves match the committed leaves
     (the report shows max LSB divergence — the number that calibrates
     the dispute tolerance)
  5. detection set / integrity / gate reproduce from the RECOMPUTED
     vectors (the load-bearing check)

Adversarial self-test modes (for harness certification):
  --tamper N       rewrite leaf N (must FAIL)
  --launder        replace every detected position's bytes with the
                   first prompt position's bytes (must FAIL)

Usage:
  python challenge_recompute.py --snapshot snap.json \
      --blobs <hash>.activations.json --bank bank.json \
      --model Qwen/Qwen2.5-0.5B-Instruct [--device cpu] [--dtype bfloat16]
"""

import argparse
import base64
import json
import sys

import numpy as np

from thought import (
    GATE_THRESHOLDS,
    activation_leaf_hash,
    canonical_hash,
    merkle_root,
    quantize_int8,
    snapshot_integrity,
)

TOL_ABS = 0.02          # integrity wobble budget (int8 + float noise)
# Measured on same-stack teacher-forced recompute of anchored batch 3:
# int8 leaves drift up to 2 LSB at 6/61 positions (KV-cache decode vs
# parallel matmul ordering), so leaf-hash EQUALITY cannot be the dispute
# criterion. The binding check is the dequantized vector cosine, bounded
# well below any real laundering signal.
MAX_COS_DIST = 0.001    # ||cos(recomputed, published) - 1|| budget


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--snapshot", required=True)
    ap.add_argument("--blobs", required=True)
    ap.add_argument("--bank", required=True)
    ap.add_argument("--model", required=True, help="HF id or local path (resolved cache-only)")
    ap.add_argument("--device", default="cpu")
    ap.add_argument("--dtype", default="bfloat16", choices=["bfloat16", "float32", "float16"])
    ap.add_argument("--tamper", type=int, default=None, help="self-test: rewrite leaf N")
    ap.add_argument("--launder", action="store_true",
                    help="self-test: overwrite detected positions with benign bytes")
    args = ap.parse_args()

    snap = json.load(open(args.snapshot, "r", encoding="utf-8"))
    blobs = json.load(open(args.blobs, "r", encoding="utf-8"))
    bank = json.load(open(args.bank, "r", encoding="utf-8"))
    layer = int(bank["layer"])

    print("jlens-challenge — teacher-forced dispute recompute")

    FAIL = []

    def check(name, cond, detail=""):
        suffix = f" ({detail})" if detail else ""
        print(f"  {'ok' if cond else 'FAIL'} - {name}{suffix}")
        if not cond:
            FAIL.append(name)

    # ── disclosure checks ──────────────────────────────────────────
    prompt = blobs.get("prompt")
    prompt_ids = blobs.get("prompt_ids")
    response_ids = blobs.get("response_ids")
    check(
        "disclosure payload present (prompt, prompt_ids, response_ids)",
        prompt is not None and prompt_ids is not None and response_ids is not None,
    )
    if FAIL:
        print("FAILED: bundle predates jlens.activations.v1.1 — "
              "operator did not disclose recompute material")
        sys.exit(1)

    # 1. prompt binding
    check(
        "prompt_hash binds prompt text",
        canonical_hash(prompt) == snap.get("prompt_hash"),
    )

    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer

    tok = AutoTokenizer.from_pretrained(args.model)
    dt = {"bfloat16": torch.bfloat16, "float32": torch.float32, "float16": torch.float16}[args.dtype]
    model = AutoModelForCausalLM.from_pretrained(args.model, torch_dtype=dt).to(args.device)
    model.eval()

    # 2. response binding: ids decode to the hashed text
    ids = [int(t) for t in response_ids]
    resp_text = tok.decode(ids, skip_special_tokens=True)
    check(
        "response_hash binds decoded response_ids",
        canonical_hash(resp_text) == snap.get("response_hash"),
    )
    enc_prompt = tok(prompt)["input_ids"]
    check(
        "prompt_ids match re-tokenized prompt",
        enc_prompt == [int(t) for t in prompt_ids],
    )

    full = torch.tensor([enc_prompt + ids], device=args.device)
    n_prompt = len(enc_prompt)

    # ── the single teacher-forced pass ──────────────────────────────
    with torch.no_grad():
        out = model(full, output_hidden_states=True)
    hs = out.hidden_states  # tuple per layer, [1, seq, d]
    pos_vecs = hs[layer][0, :, :].detach().float().cpu().numpy()

    committed = blobs["vectors"]
    n_prompt_leaves = sum(1 for v in committed if v["span"] == "prompt")
    # autoregressive capture has no hidden state for the final sampled
    # token (it is never fed back), so G = len(response_ids) - 1
    expected = n_prompt + (len(ids) - 1)
    check(
        "capture count matches committed leaf count",
        n_prompt_leaves == n_prompt and len(committed) == expected,
        f"{len(committed)} committed vs {expected} expected "
        f"({len(ids)} response ids; last token has no hidden state)",
    )

    # adversarial self-tests substitute the PUBLISHED vectors, because the
    # binding criterion is per-position vector agreement, not leaf bytes:
    # a dispute must fail both of these
    attacks: dict = {}
    if args.tamper is not None:
        attacks[args.tamper] = np.zeros(int(pos_vecs.shape[1]), dtype=np.float32)
    if args.launder:
        cqb0 = base64.b64decode(committed[0]["qbytes_b64"])
        benign = np.array([(b - 128) * committed[0]["scale"] for b in cqb0], dtype=np.float32)
        gen_spans = [v["span"] for v in committed[len(enc_prompt):]]
        first_seen: dict = {}
        for i, s in enumerate(gen_spans):
            if s not in first_seen:
                first_seen[s] = i
        for d in snap.get("detections", []):
            span, p = d["span"], d["position"]
            abs_i = p if span == "prompt" else len(enc_prompt) + first_seen.get(span, 0) + p
            if abs_i < len(committed):
                attacks[abs_i] = benign

    # ── per-position comparison (committed positions only) ──────────
    max_lsb = 0
    lsb_hist = {0: 0, 1: 0, "2+": 0}
    requant_leaves = []
    requant_vecs = []
    pub_vecs = []
    for i, vec in enumerate(pos_vecs[: len(committed)]):
        qb, _scale = quantize_int8(vec.tolist())
        requant_leaves.append(activation_leaf_hash(qb))
        requant_vecs.append(vec)
        cqb = base64.b64decode(committed[i]["qbytes_b64"])
        cscale = committed[i]["scale"]
        decoded = np.array([(b - 128) * cscale for b in cqb], dtype=np.float32)
        pub_vecs.append(attacks.get(i, decoded))
        drift = max(abs(a - b) for a, b in zip(qb, cqb)) if len(qb) == len(cqb) else 256
        max_lsb = max(max_lsb, drift)
        lsb_hist[0 if drift == 0 else 1 if drift == 1 else "2+"] += 1

    # exact: the disclosed blob bytes must hash to the anchored root —
    # any fabrication shows here even when float noise would forgive it
    pub_leaves = [
        activation_leaf_hash(base64.b64decode(v["qbytes_b64"])) for v in committed
    ]
    check(
        "disclosed blobs hash exactly to anchored activation_root",
        merkle_root(pub_leaves) == snap.get("activation_root"),
    )
    print(
        f"  info - operator-vs-recompute int8 drift (LSB): {lsb_hist}; "
        f"exact reproduction of root from verifier's own requantization: "
        f"{'yes' if merkle_root(requant_leaves) == snap.get('activation_root') else 'no (expected: float noise, see tolerance model)'}"
    )
    # 4. binding check on dequantized vectors, not leaf equality
    max_cos = 0.0
    bad_pos = []
    for i, (rv, pv) in enumerate(zip(requant_vecs, pub_vecs)):
        rvv = rv.astype(np.float64)
        pvv = pv.astype(np.float64)
        c = float(np.dot(rvv, pvv) / (np.linalg.norm(rvv) * np.linalg.norm(pvv) + 1e-12))
        d = abs(1.0 - c)
        max_cos = max(max_cos, d)
        if d > MAX_COS_DIST:
            bad_pos.append((i, d))
    check(
        "per-position recompute matches committed vectors",
        not bad_pos,
        f"max cosine divergence {max_cos:.2e} (budget {MAX_COS_DIST})"
        + (f"; {len(bad_pos)} positions over budget: {[p for p, _ in bad_pos][:8]}" if bad_pos else ""),
    )

    # 5. detection set / integrity / gate from RECOMPUTED vectors
    names = sorted(bank["concepts"].keys())
    bank_mat = np.stack([np.asarray(bank["concepts"][n]["vector"], dtype=np.float32) for n in names], axis=1)
    norms = np.linalg.norm(bank_mat, axis=0)

    from thought import split_spans, char_range_to_token_range
    toks_text = [tok.decode(t, skip_special_tokens=True) for t in ids]
    offsets, pos = [], 0
    for t in toks_text:
        offsets.append(pos)
        pos += len(t)
    spans = split_spans(resp_text)
    span_tok = {k: char_range_to_token_range(v, offsets + [len(resp_text)]) for k, v in spans.items()}

    n_gen = len(committed) - n_prompt
    gen_vecs = pos_vecs[n_prompt : n_prompt + n_gen]
    groups = {"prompt": pos_vecs[:n_prompt]}
    for sname in ("reasoning", "output", "tool_call"):
        r = span_tok.get(sname)
        groups[sname] = gen_vecs[r[0]:r[1]] if r else np.zeros((0, gen_vecs.shape[1]))

    redet = []
    for span, vecs in groups.items():
        if vecs.shape[0] == 0:
            continue
        v = vecs.astype(np.float32)
        vn = np.linalg.norm(v, axis=1, keepdims=True)
        scores = (v @ bank_mat) / (vn * norms + 1e-9)
        for ci, c in enumerate(names):
            mx = float(scores[:, ci].max())
            if mx >= float(bank["concepts"][c]["threshold"]):
                redet.append({"concept": c, "span": span, "score": mx})
    orig = {(d["concept"], d["span"]) for d in snap.get("detections", [])}
    new = {(d["concept"], d["span"]) for d in redet}
    check("detection set reproduces from recompute", orig == new, f"{len(new)} detections")

    re_integrity = snapshot_integrity(redet)
    check(
        "integrity within tolerance",
        abs(re_integrity - snap.get("integrity")) < TOL_ABS,
        f"recomputed {re_integrity:.4f} vs recorded {snap.get('integrity')}",
    )
    gate = ("green" if re_integrity >= GATE_THRESHOLDS["green"]
            else "yellow" if re_integrity >= GATE_THRESHOLDS["yellow"] else "red")
    check("gate reproduces", gate == snap.get("gate"), gate)

    print()
    if FAIL:
        print(f"DISPUTE VALID — {len(FAIL)} check(s) failed against commitments")
        sys.exit(1)
    print(f"COMMITMENTS CONSISTENT — snapshot {str(snap.get('snapshot_hash'))[:16]}… "
          f"survives independent recompute (max drift {max_lsb} LSB)")


if __name__ == "__main__":
    main()
