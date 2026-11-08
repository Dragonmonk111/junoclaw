#!/usr/bin/env python3
"""jlens-serve — J-Lens generation-time probing sidecar.

Serves an open-weight model and probes its activations WHILE it
generates: prefill (prompt), reasoning, output and tool-call tokens.
Every generation returns a jlens.thought.v1 snapshot whose
activation_root commits to the captured vectors.

Endpoints:
  POST /generate_and_probe   {"prompt": "...", "model_id": "...",
                              "max_new_tokens": 64}
      -> {"response": "...", "integrity": 0.97, "gate": "green",
          "score_semantics": "jlens.integrity.v1",
          "thought": {...jlens.thought.v1...}}

  GET /health               -> {"status": "ok", "models": [...]}
  GET /models               -> registered serving models (public fields)

Auth: Bearer token via JLENS_SERVE_TOKEN (required).
Activation blobs: set JLENS_ACTIVATIONS_DIR to persist per-snapshot
  {snapshot_hash}.activations.json (int8 vectors + scales + leaves)
  for later re-audit with newer banks.

Run:
  JLENS_SERVE_TOKEN=$(openssl rand -hex 32) python serve.py
  # registry: JLENS_MODELS_FILE=../src/models.serve.json (default next to this file)

This is the T0 tier (operator-signed). T2 (TEE-attested) runs the same
probe inside a WAVS enclave and swaps the tier + signature source —
see the plan: drafts/PLAN_JLENS_V1_THOUGHT_ATTESTATION.md §2.3.
"""

import json
import os
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from thought import SCORE_SEMANTICS, canonical_hash

PORT = int(os.environ.get("JLENS_SERVE_PORT", "8200"))
HOST = os.environ.get("JLENS_SERVE_HOST", "127.0.0.1")
TOKEN = os.environ.get("JLENS_SERVE_TOKEN")
ACTIVATIONS_DIR = os.environ.get("JLENS_ACTIVATIONS_DIR")
MODELS_FILE = os.environ.get(
    "JLENS_MODELS_FILE",
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "models.serve.json"),
)

if not TOKEN:
    print("JLENS_SERVE_TOKEN is required", file=sys.stderr)
    sys.exit(1)

with open(MODELS_FILE, "r", encoding="utf-8") as f:
    REGISTRY = json.load(f)["models"]

# lazy-loaded probers: model_id -> GenerationTimeProber
_PROBERS = {}


def get_prober(model_id):
    if model_id in _PROBERS:
        return _PROBERS[model_id]
    if model_id not in REGISTRY:
        raise KeyError(f"unknown model_id: {model_id}")
    from probe_runtime import (
        GenerationTimeProber,
        compute_weights_hash,
        load_bank,
        resolve_model_revision,
    )
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer

    cfg = REGISTRY[model_id]
    dtype = {
        "float32": torch.float32,
        "bfloat16": torch.bfloat16,
        "float16": torch.float16,
    }.get(cfg.get("dtype", "float32"), torch.float32)
    device = cfg.get("device", "cpu")

    tokenizer = AutoTokenizer.from_pretrained(cfg["hf_model"])
    model = AutoModelForCausalLM.from_pretrained(
        cfg["hf_model"], torch_dtype=dtype
    ).to(device)
    model.eval()

    bank_path = os.path.join(
        os.path.dirname(os.path.abspath(MODELS_FILE)), cfg["probe_bank_path"]
    )
    bank = load_bank(bank_path)
    prober = GenerationTimeProber(model, tokenizer, bank, device=device)
    prober.weights_hash = compute_weights_hash(cfg["hf_model"])
    prober.model_revision = resolve_model_revision(cfg["hf_model"])
    _PROBERS[model_id] = prober
    return prober


def persist_blobs(snapshot_hash, blobs):
    if not ACTIVATIONS_DIR or not blobs:
        return
    os.makedirs(ACTIVATIONS_DIR, exist_ok=True)
    payload = {
        "schema": "jlens.activations.v1",
        "snapshot_hash": snapshot_hash,
        "vectors": [
            {
                "span": span,
                "scale": scale,
                "qbytes_b64": __import__("base64").b64encode(qb).decode(),
            }
            for span, qb, scale in blobs
        ],
    }
    path = os.path.join(ACTIVATIONS_DIR, f"{snapshot_hash}.activations.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(payload, f)


class Handler(BaseHTTPRequestHandler):
    def _send(self, status, body):
        data = json.dumps(body, indent=2).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _authed(self):
        auth = self.headers.get("Authorization", "")
        return auth == f"Bearer {TOKEN}"

    def do_GET(self):
        if self.path == "/health":
            self._send(200, {"status": "ok", "models": sorted(REGISTRY)})
            return
        if self.path == "/models":
            self._send(200, {
                "score_semantics": SCORE_SEMANTICS,
                "models": [
                    {
                        "model_id": mid,
                        "hf_model": cfg["hf_model"],
                        "layer": cfg.get("layer"),
                        "test_only": cfg.get("test_only", False),
                    }
                    for mid, cfg in REGISTRY.items()
                ],
            })
            return
        self._send(404, {"error": "not found"})

    def do_POST(self):
        if not self._authed():
            self._send(401, {"error": "unauthorized"})
            return
        if self.path != "/generate_and_probe":
            self._send(404, {"error": "not found"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            body = json.loads(self.rfile.read(length))
            prompt = body.get("prompt", "")
            model_id = body.get("model_id")
            max_new_tokens = int(body.get("max_new_tokens", 64))
            if not prompt or not model_id:
                self._send(400, {"error": "missing required fields: prompt, model_id"})
                return

            prober = get_prober(model_id)
            response, snapshot, blobs = prober.generate_and_probe(
                prompt, max_new_tokens=max_new_tokens
            )
            persist_blobs(snapshot["snapshot_hash"], blobs)
            self._send(200, {
                "response": response,
                "integrity": snapshot["integrity"],
                "gate": snapshot["gate"],
                "score_semantics": SCORE_SEMANTICS,
                "thought": snapshot,
            })
        except KeyError as e:
            self._send(400, {"error": str(e), "hint": "GET /models lists ids"})
        except Exception as e:  # noqa: BLE001
            self._send(500, {"error": str(e)})

    def log_message(self, fmt, *args):
        print(f"[jlens-serve] {fmt % args}", file=sys.stderr)


if __name__ == "__main__":
    print(f"jlens-serve listening on http://{HOST}:{PORT}", file=sys.stderr)
    print(f"models: {sorted(REGISTRY)}", file=sys.stderr)
    print(f"score semantics: {SCORE_SEMANTICS}", file=sys.stderr)
    print(f"activation blobs: {ACTIVATIONS_DIR or 'not persisted'}", file=sys.stderr)
    ThreadingHTTPServer((HOST, PORT), Handler).serve_forever()
