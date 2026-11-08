"""Generation-time probing runtime for J-Lens.

Wraps an open-weight HF causal LM and probes its activations WHILE it
generates — prompt tokens at prefill, reasoning/output/tool-call tokens
at every decode step. This is the core of "probe at the time of
thinking": we never re-read finished text through a second model.

Cost model: the probe is one matmul per step ([T x d] . [d x C]) —
negligible next to the forward pass that is already running.

Requires: torch, transformers, numpy.
"""

import hashlib
import json
import os

import numpy as np

from thought import (
    activation_leaf_hash,
    build_thought_snapshot,
    canonical_hash,
    char_range_to_token_range,
    merkle_root,
    quantize_int8,
    snapshot_gate,
    snapshot_integrity,
    split_spans,
    REASON_OPEN,
    REASON_CLOSE,
    TOOL_CALL_OPEN,
)


def load_bank(bank_path):
    """Load a probe bank + its hash. Bank shape (j-lens v0.1):
    {probe_version, probe_model, layer, concepts: {name: {vector, threshold}}}
    """
    with open(bank_path, "rb") as f:
        raw = f.read()
    bank = json.loads(raw)
    bank_hash = hashlib.sha256(raw).hexdigest()
    concepts = {}
    for name, spec in bank["concepts"].items():
        concepts[name] = (
            np.asarray(spec["vector"], dtype=np.float32),
            float(spec["threshold"]),
        )
    return {
        "probe_version": bank.get("probe_version", "unknown"),
        "probe_model": bank.get("probe_model", "unknown"),
        "layer": int(bank["layer"]),
        "concepts": concepts,
        "bank_hash": bank_hash,
        "method": "contrastive-dom-v0.1",
    }


def resolve_model_dir(model):
    """Local snapshot dir for a path or an HF hub id (cache only, no network)."""
    if os.path.isdir(model):
        return model
    from huggingface_hub import snapshot_download
    return snapshot_download(
        model, local_files_only=True, allow_patterns=["*.safetensors", "*.bin"]
    )


def compute_weights_hash(model):
    """Content pin of the exact weights: sha256 over sorted
    (file name, sha256(file bytes)) for the weight files transformers
    loads (*.safetensors, else *.bin). A fine-tune changes tensor bytes
    (not sizes), so it changes the hash and loses its bank. Fails closed
    when no weights can be found."""
    model_dir = resolve_model_dir(model)
    entries = os.listdir(model_dir)
    files = sorted(f for f in entries if f.endswith(".safetensors")) or \
        sorted(f for f in entries if f.endswith(".bin"))
    if not files:
        raise ValueError(f"no model weights under {model_dir}")
    h = hashlib.sha256()
    for f in files:
        fh = hashlib.sha256()
        with open(os.path.join(model_dir, f), "rb") as fp:
            for chunk in iter(lambda: fp.read(1 << 20), b""):
                fh.update(chunk)
        h.update(f.encode())
        h.update(b"\0")
        h.update(fh.digest())
    return h.hexdigest()


class GenerationTimeProber:
    """Probe a model's own activations during generation."""

    def __init__(self, model, tokenizer, bank, device="cpu"):
        self.model = model
        self.tokenizer = tokenizer
        self.bank = bank
        self.device = device

        names = sorted(bank["concepts"].keys())
        self.concept_names = names
        # [d x C] matrix, deterministic column order
        self.bank_matrix = np.stack(
            [bank["concepts"][n][0] for n in names], axis=1
        ).astype(np.float32)
        self.bank_norms = np.linalg.norm(self.bank_matrix, axis=0)
        self.thresholds = {n: bank["concepts"][n][1] for n in names}

    def _cosine_matrix(self, vectors):
        """vectors [T x d] vs bank [d x C] -> [T x C] cosine."""
        if vectors.shape[0] == 0:
            return np.zeros((0, len(self.concept_names)))
        v = vectors.astype(np.float32)
        vn = np.linalg.norm(v, axis=1, keepdims=True)
        dot = (v @ self.bank_matrix) / (vn * self.bank_norms + 1e-9)
        return dot

    def generate_and_probe(self, prompt, max_new_tokens=64):
        """Generate + probe. Returns (response_text, snapshot, blobs).

        blobs = list of (span, quantized_bytes, scale) per token — the
        off-chain activation commitment material.
        """
        import torch

        inputs = self.tokenizer(prompt, return_tensors="pt").to(self.device)
        n_prompt = inputs["input_ids"].shape[1]

        out = self.model.generate(
            **inputs,
            max_new_tokens=max_new_tokens,
            return_dict_in_generate=True,
            output_hidden_states=True,
            do_sample=False,
        )

        # sequences[0] = prompt + generated tokens
        seq = out.sequences[0]
        response_ids = seq[n_prompt:]
        response_text = self.tokenizer.decode(
            response_ids, skip_special_tokens=True
        )

        # ── collect per-token vectors at the bank layer ──
        # out.hidden_states: tuple per step; step 0 covers the prompt
        # (positions 0..n_prompt-1, last position is the first sampled
        # token's input), steps 1.. cover each subsequent decode token.
        layer = self.bank["layer"]
        hs = out.hidden_states

        prompt_vecs = (
            hs[0][layer][0, :n_prompt, :].float().cpu().numpy()
            if hs and hs[0] and len(hs[0]) > layer
            else np.zeros((0, 1))
        )

        gen_vecs = []
        for step in range(1, len(hs)):
            if len(hs[step]) > layer:
                gen_vecs.append(hs[step][layer][0, -1, :].float().cpu().numpy())
        gen_vecs = np.stack(gen_vecs, axis=0) if gen_vecs else np.zeros((0, 1))

        # ── span mapping over the generated text ──
        # offsets: cumulative char start per generated token
        toks_text = [
            self.tokenizer.decode(t, skip_special_tokens=True)
            for t in response_ids
        ]
        offsets = []
        pos = 0
        for t in toks_text:
            offsets.append(pos)
            pos += len(t)

        spans = split_spans(response_text)
        span_tok = {
            k: char_range_to_token_range(v, offsets + [len(response_text)])
            for k, v in spans.items()
        }

        def span_slice(name):
            r = span_tok.get(name)
            if not r:
                return np.zeros((0, gen_vecs.shape[1]))
            s, e = r
            return gen_vecs[s:e]

        # ── projections ──
        prompt_scores = self._cosine_matrix(prompt_vecs)
        span_scores = {
            name: self._cosine_matrix(span_slice(name))
            for name in ("reasoning", "output", "tool_call")
        }

        # ── detections + per-span maxima ──
        detections = []
        span_maxima = {}

        def record(span_name, scores):
            if scores.shape[0] == 0:
                return
            per_concept = {}
            for ci, cname in enumerate(self.concept_names):
                col = scores[:, ci]
                am = int(np.argmax(col))
                mx = float(col[am])
                per_concept[cname] = {"max": round(mx, 6), "argmax": am}
                if mx >= self.thresholds[cname]:
                    detections.append({
                        "concept": cname,
                        "span": span_name,
                        "position": am,
                        "score": round(mx, 6),
                        "threshold": self.thresholds[cname],
                    })
            span_maxima[span_name] = per_concept

        record("prompt", prompt_scores)
        for name in ("reasoning", "output", "tool_call"):
            record(name, span_scores[name])

        integrity = snapshot_integrity(detections)
        gate = snapshot_gate(integrity)

        # ── activation commitment: int8-quantized vectors, merkle root ──
        blobs = []
        leaves = []
        for vec in prompt_vecs:
            qb, scale = quantize_int8(vec.tolist())
            blobs.append(("prompt", qb, scale))
            leaves.append(activation_leaf_hash(qb))
        for i, vec in enumerate(gen_vecs):
            span = "output"
            for name in ("reasoning", "tool_call"):
                r = span_tok.get(name)
                if r and r[0] <= i < r[1]:
                    span = name
                    break
            qb, scale = quantize_int8(vec.tolist())
            blobs.append((span, qb, scale))
            leaves.append(activation_leaf_hash(qb))

        activation_root = merkle_root(leaves)

        snapshot = build_thought_snapshot(
            model_id=self.bank["probe_model"],
            model_revision=getattr(self.model.config, "_name_or_path", "unknown"),
            weights_hash=self.weights_hash,
            bank_hash=self.bank["bank_hash"],
            method=self.bank["method"],
            layers=[self.bank["layer"]],
            prompt_hash=canonical_hash(prompt),
            response_hash=canonical_hash(response_text),
            span_maxima=span_maxima,
            detections=detections,
            integrity=integrity,
            gate=gate,
            activation_root=activation_root,
            activation_count=len(leaves),
            tier="t0-operator",
        )
        return response_text, snapshot, blobs
