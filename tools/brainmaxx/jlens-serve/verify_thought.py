#!/usr/bin/env python3
"""jlens-verify — independent verifier for jlens.thought.v1 snapshots.

This is the stage-4 tool: a THIRD PARTY verifies a thought attestation
using only the committed artifacts — no model, no GPU, no trust in the
operator that produced it.

Inputs:
  --snapshot   jlens.thought.v1 JSON (as returned by /generate_and_probe)
  --blobs      jlens.activations.v1 JSON persisted by jlens-serve
               ({snapshot_hash}.activations.json)
  --bank       probe bank JSON whose hash must equal snapshot.bank_hash

Checks (each printed ok/FAIL):
  1. schema + score_semantics tags are the canonical values
  2. snapshot_hash recomputes over the canonical snapshot
  3. blob file's snapshot_hash field matches the snapshot
  4. activation_count matches the blob vector count
  5. activation_root recomputes from the int8-quantized vectors
  6. bank_hash matches the supplied bank file
  7. re-scoring dequantized vectors against the bank reproduces the
     recorded detection set {concept, span} (int8 tolerance applies —
     detScore may wobble, the SET must match)
  8. recomputed integrity is within int8 tolerance of the recorded value
  9. recomputed gate matches the recorded gate

Exit 0 on all-pass, 1 on any failure. Pure python + numpy.

Usage:
  python verify_thought.py --snapshot snap.json \
      --blobs <hash>.activations.json \
      --bank ../j-lens/probe_bank_tiny_gpt2.json
"""

import argparse
import base64
import hashlib
import json
import sys

import numpy as np

from thought import (
    GATE_THRESHOLDS,
    SCHEMA,
    SCORE_SEMANTICS,
    activation_leaf_hash,
    canonical_hash,
    merkle_root,
)

INT8_TOLERANCE = 0.02

FAIL = []


def check(name, cond, detail=""):
    suffix = f" ({detail})" if detail else ""
    print(f"  {'ok' if cond else 'FAIL'} - {name}{suffix}")
    if not cond:
        FAIL.append(name)


def dequantize(qbytes: bytes, scale: float) -> np.ndarray:
    if scale == 0.0:
        return np.zeros(len(qbytes), dtype=np.float32)
    return np.array([(b - 128) * scale for b in qbytes], dtype=np.float32)


def main():
    ap = argparse.ArgumentParser(description="verify a jlens.thought.v1 snapshot")
    ap.add_argument("--snapshot", required=True)
    ap.add_argument("--blobs", required=True)
    ap.add_argument("--bank", required=True)
    args = ap.parse_args()

    with open(args.snapshot, "r", encoding="utf-8") as f:
        snap = json.load(f)
    with open(args.blobs, "r", encoding="utf-8") as f:
        blobs = json.load(f)
    with open(args.bank, "rb") as f:
        bank_raw = f.read()
    bank = json.loads(bank_raw)

    print("jlens-verify — independent re-verification")

    # 1. canonical tags
    check("schema is jlens.thought.v1", snap.get("schema") == SCHEMA)
    check(
        "score_semantics is jlens.integrity.v1",
        snap.get("score_semantics") == SCORE_SEMANTICS,
    )

    # 2. snapshot_hash recomputes — canonical hash over all fields
    #    except snapshot_hash itself.
    recorded = snap.get("snapshot_hash")
    body = {k: v for k, v in snap.items() if k != "snapshot_hash"}
    recomputed = canonical_hash(body)
    check(
        "snapshot_hash recomputes",
        recomputed == recorded,
        f"{str(recomputed)[:16]}…",
    )

    # 3. blob belongs to this snapshot
    check(
        "blob file bound to snapshot",
        blobs.get("schema") == "jlens.activations.v1"
        and blobs.get("snapshot_hash") == recorded,
    )

    vectors = blobs.get("vectors", [])

    # 4. activation_count
    check(
        "activation_count matches blob count",
        snap.get("activation_count") == len(vectors),
        f"{len(vectors)} vectors",
    )

    # 5. activation_root from committed int8 bytes
    leaves = [
        activation_leaf_hash(base64.b64decode(v["qbytes_b64"]))
        for v in vectors
    ]
    check(
        "activation_root recomputes from blobs",
        merkle_root(leaves) == snap.get("activation_root"),
        f"{str(snap.get('activation_root'))[:16]}…",
    )

    # 6. bank hash
    bank_hash = hashlib.sha256(bank_raw).hexdigest()
    check(
        "bank_hash matches",
        bank_hash == snap.get("bank_hash"),
        f"{bank_hash[:16]}…",
    )

    # 7-9. re-score dequantized vectors → detection set, integrity, gate
    concepts = bank["concepts"]
    names = sorted(concepts.keys())
    bank_mat = np.stack(
        [np.asarray(concepts[n]["vector"], dtype=np.float32) for n in names],
        axis=1,
    )
    norms = np.linalg.norm(bank_mat, axis=0)

    span_vecs = {}
    for v in vectors:
        span_vecs.setdefault(v["span"], []).append(
            dequantize(base64.b64decode(v["qbytes_b64"]), v["scale"])
        )

    redetections = []
    for span, vecs in span_vecs.items():
        mat = np.stack(vecs).astype(np.float32)
        vn = np.linalg.norm(mat, axis=1, keepdims=True)
        scores = (mat @ bank_mat) / (vn * norms + 1e-9)
        for ci, c in enumerate(names):
            mx = float(scores[:, ci].max())
            if mx >= float(concepts[c]["threshold"]):
                redetections.append((c, span))

    orig_set = {(d["concept"], d["span"]) for d in snap.get("detections", [])}
    check(
        "detection set reproduces from int8",
        orig_set == set(redetections),
        f"{len(orig_set)} detections",
    )

    # Recompute integrity from re-scored maxima (closest re-audit value).
    redet_scores = []
    for c, span in redetections:
        mat = np.stack(span_vecs[span]).astype(np.float32)
        vn = np.linalg.norm(mat, axis=1, keepdims=True)
        ci = names.index(c)
        mx = float(((mat @ bank_mat) / (vn * norms + 1e-9))[:, ci].max())
        redet_scores.append(mx)
    re_integrity = max(0.0, 1.0 - max(redet_scores)) if redet_scores else 1.0
    rec_integrity = snap.get("integrity")
    check(
        "integrity within int8 tolerance",
        rec_integrity is not None
        and abs(re_integrity - rec_integrity) < INT8_TOLERANCE,
        f"re-audit {re_integrity:.4f} vs recorded {rec_integrity}",
    )

    gate = (
        "green"
        if re_integrity >= GATE_THRESHOLDS["green"]
        else "yellow"
        if re_integrity >= GATE_THRESHOLDS["yellow"]
        else "red"
    )
    check("gate reproduces", gate == snap.get("gate"), gate)

    print()
    if FAIL:
        print(f"FAILED: {len(FAIL)} check(s)")
        sys.exit(1)
    print(
        f"VERIFIED — snapshot {str(recorded)[:16]}… is self-consistent "
        f"and reproduces under independent re-audit"
    )


if __name__ == "__main__":
    main()
