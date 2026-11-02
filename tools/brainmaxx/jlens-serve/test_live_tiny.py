"""Live generation-time probe test on sshleifer/tiny-gpt2.

Loads the real model, generates, and probes its own activations during
generation. Verifies:
  - prompt + every generated token captured at the bank layer
  - activation_root commits to exactly those vectors (recomputable)
  - re-scoring the committed int8 vectors reproduces the detections
    deterministically (the re-audit path — no model re-run needed)
  - identical runs (greedy) produce identical snapshot hashes

Run: python test_live_tiny.py   (needs torch + transformers; ~1 min first run)
"""

import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from probe_runtime import GenerationTimeProber, load_bank  # noqa: E402
from thought import activation_leaf_hash, merkle_root, snapshot_integrity  # noqa: E402

FAIL = []


def check(name, cond):
    print(f"  {'ok' if cond else 'FAIL'} - {name}")
    if not cond:
        FAIL.append(name)


def main():
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer

    torch.manual_seed(0)
    bank = load_bank(os.path.join(HERE, "..", "j-lens", "probe_bank_tiny_gpt2.json"))
    tok = AutoTokenizer.from_pretrained("sshleifer/tiny-gpt2")
    model = AutoModelForCausalLM.from_pretrained("sshleifer/tiny-gpt2")
    model.eval()

    prober = GenerationTimeProber(model, tok, bank, device="cpu")
    prober.weights_hash = "test-weights"

    prompt = "The robot should move forward and"
    n_prompt = len(tok(prompt)["input_ids"])

    print("generation-time probe:")
    resp, snap, blobs = prober.generate_and_probe(prompt, max_new_tokens=8)

    check("schema", snap["schema"] == "jlens.thought.v1")
    check("semantics", snap["score_semantics"] == "jlens.integrity.v1")
    check("prompt vectors captured", sum(1 for b in blobs if b[0] == "prompt") == n_prompt)
    gen_count = sum(1 for b in blobs if b[0] != "prompt")
    check(f"decode vectors captured ({gen_count})", gen_count >= 1)
    check("activation_count matches", snap["activation_count"] == len(blobs))

    # activation_root recomputable from blobs alone
    leaves = [activation_leaf_hash(qb) for _, qb, _ in blobs]
    check("activation_root recomputes", merkle_root(leaves) == snap["activation_root"])

    # re-audit from committed int8 vectors (no model) reproduces detections
    names = prober.concept_names
    redets = []
    for span_name in ("prompt", "reasoning", "output", "tool_call"):
        vecs = [
            np.array([(b - 128) * scale for b in qb], dtype=np.float32)
            for s, qb, scale in blobs
            if s == span_name
        ]
        if not vecs:
            continue
        scores = prober._cosine_matrix(np.stack(vecs))
        for ci, c in enumerate(names):
            mx = float(scores[:, ci].max())
            if mx >= prober.thresholds[c]:
                redets.append({"concept": c, "span": span_name, "score": mx})
    orig_set = {(d["concept"], d["span"]) for d in snap["detections"]}
    re_set = {(d["concept"], d["span"]) for d in redets}
    check("re-audit from int8 reproduces detection set", orig_set == re_set)
    if redets:
        re_integ = snapshot_integrity(redets)
        check(
            f"re-audit integrity within int8 tolerance ({re_integ:.4f} vs {snap['integrity']:.4f})",
            abs(re_integ - snap["integrity"]) < 0.02,
        )

    # determinism: greedy decode twice → same hash
    _, snap2, _ = prober.generate_and_probe(prompt, max_new_tokens=8)
    check("deterministic snapshot_hash across runs", snap["snapshot_hash"] == snap2["snapshot_hash"])

    print(f"\n  response: {resp!r}")
    print(f"  integrity: {snap['integrity']}  gate: {snap['gate']}")
    print(f"  detections: {len(snap['detections'])}  activations: {snap['activation_count']}")
    print(f"  activation_root: {snap['activation_root'][:16]}...")
    print(f"  snapshot_hash:   {snap['snapshot_hash'][:16]}...")

    print()
    if FAIL:
        print(f"FAILED: {len(FAIL)}")
        sys.exit(1)
    print("live generation-time probe: all checks passed")


if __name__ == "__main__":
    main()
