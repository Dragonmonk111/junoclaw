"""Shared math for the J-Lens Jacobian pathway (J2-J9 bank builders).

Every builder supports --precomputed <states.json> so the math is testable
without a model: the file maps text -> {"layers": {"<layer>": [floats]}}.
The model path (torch/transformers) is imported lazily, only when needed.

Determinism: numpy float64 everywhere, sorted iteration order over
concepts/examples, no stochastic steps anywhere.
"""

import json

import numpy as np


def cosine(a, b):
    a = np.asarray(a, dtype=np.float64)
    b = np.asarray(b, dtype=np.float64)
    na = np.linalg.norm(a)
    nb = np.linalg.norm(b)
    if na == 0 or nb == 0:
        return 0.0
    return float(np.dot(a, b) / (na * nb))


def sigmoid(z):
    return 1.0 / (1.0 + np.exp(-np.clip(z, -60.0, 60.0)))


def fit_logistic(X, y, l2=1.0, lr=0.1, steps=4000):
    """Deterministic L2-regularized logistic regression via full-batch GD.
    X: (n, d) float64. y: (n,) in {0, 1}. Returns (w, b).
    """
    X = np.asarray(X, dtype=np.float64)
    y = np.asarray(y, dtype=np.float64)
    n, d = X.shape
    w = np.zeros(d)
    b = 0.0
    for _ in range(steps):
        p = sigmoid(X @ w + b)
        gw = X.T @ (p - y) / n + l2 * w / n
        gb = float(np.sum(p - y)) / n
        w -= lr * gw
        b -= lr * gb
    return w, b


def auroc(scores_pos, scores_neg):
    """Exact AUROC via rank-sum (ties count 0.5). Deterministic."""
    scores_pos = list(scores_pos)
    scores_neg = list(scores_neg)
    pairs = [(s, 1) for s in scores_pos] + [(s, 0) for s in scores_neg]
    pairs.sort(key=lambda p: p[0])
    rank = 0.0
    i = 0
    n = len(pairs)
    while i < n:
        j = i
        while j < n and pairs[j][0] == pairs[i][0]:
            j += 1
        avg_rank = (i + 1 + j) / 2.0
        for k in range(i, j):
            if pairs[k][1] == 1:
                rank += avg_rank
        i = j
    np_ = len(scores_pos)
    nn = len(scores_neg)
    if np_ == 0 or nn == 0:
        return float("nan")
    return (rank - np_ * (np_ + 1) / 2.0) / (np_ * nn)


def collect_states(texts, layers, precomputed=None, model_id=None, device="cpu", dtype="float32"):
    """Uniform interface: precomputed file or model forward pass.
    Returns {text: {str(layer): [floats]}}.
    """
    if precomputed:
        with open(precomputed, "r", encoding="utf-8") as f:
            data = json.load(f)
        return {t: data[t] for t in texts if t in data}
    if not model_id:
        raise ValueError("either precomputed or model_id is required")
    return _model_states(model_id, texts, layers, device, dtype)


def _model_states(model_id, texts, layers, device="cpu", dtype="float32"):
    """Forward-pass path (lazy torch import). Returns dict text -> {layer: vec}."""
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer

    dt = {"float32": torch.float32, "bfloat16": torch.bfloat16, "float16": torch.float16}[dtype]
    tok = AutoTokenizer.from_pretrained(model_id)
    model = AutoModelForCausalLM.from_pretrained(model_id, torch_dtype=dt)
    model.to(device)
    model.eval()
    out = {}
    for text in texts:
        inputs = tok(text, return_tensors="pt").to(device)
        with torch.no_grad():
            r = model(**inputs, output_hidden_states=True)
        out[text] = {
            str(l): r.hidden_states[l][0, -1, :].float().cpu().numpy().tolist()
            for l in layers
        }
    return out


def bank_header(method, probe_model, layers, calibration=None):
    return {
        "probe_version": method,
        "probe_model": probe_model,
        "layers": sorted(layers),
        "method": method,
        "calibration": calibration or {},
    }


def sha256_file(path):
    import hashlib

    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()
