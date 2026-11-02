"""Pure-Python tests for jlens.thought.v1 (no torch required).

Run: python test_thought.py
"""

import sys

from thought import (
    SCHEMA,
    SCORE_SEMANTICS,
    activation_leaf_hash,
    build_thought_snapshot,
    canonical_hash,
    canonical_json,
    char_range_to_token_range,
    merkle_proof,
    merkle_root,
    quantize_int8,
    sha256_hex,
    snapshot_gate,
    snapshot_integrity,
    split_spans,
)

THINK_OPEN = "<" + "think" + ">"
THINK_CLOSE = "</" + "think" + ">"
TOOL_OPEN = "<" + "tool_call" + ">"
TOOL_CLOSE = "</" + "tool_call" + ">"

FAILURES = []


def check(name, cond):
    print(f"  {'ok' if cond else 'FAIL'} - {name}")
    if not cond:
        FAILURES.append(name)


def approx(a, b, eps=1e-6):
    return abs(a - b) < eps


# ─── quantization ───────────────────────────────────────────────────
def test_quantize():
    print("quantize_int8:")
    vec = [0.5, -0.25, 0.0, 0.75]
    qb, scale = quantize_int8(vec)
    check("deterministic bytes", qb == quantize_int8(vec)[0])
    check("scale positive", scale > 0)
    deq = [((b - 128) * scale) if scale else 0.0 for b in qb]
    check("round trip within 1/127", all(abs(a - b) <= scale for a, b in zip(vec, deq)))
    zero_qb, zero_scale = quantize_int8([0.0, 0.0])
    check("zero vector safe", zero_scale == 0.0 and len(zero_qb) == 2)


# ─── merkle ─────────────────────────────────────────────────────────
def test_merkle():
    print("merkle_root:")
    leaves = [activation_leaf_hash(bytes([i])) for i in range(5)]
    check("empty root is sha256('')", merkle_root([]) == sha256_hex(b""))
    check("single leaf passthrough", merkle_root([leaves[0]]) == leaves[0])
    r1 = merkle_root(leaves)
    r2 = merkle_root(leaves)
    check("deterministic", r1 == r2)
    r3 = merkle_root(leaves[:-1])
    check("order sensitive", r1 != r3)
    manual = sha256_hex(bytes.fromhex(leaves[0]) + bytes.fromhex(leaves[1]))
    check("pair rule = merkle-verifier bytes concat", merkle_root(leaves[:2]) == manual)

    # proof verifies with the merkle-verifier contract's algorithm
    import hashlib as _h
    for idx in range(len(leaves)):
        proof = merkle_proof(leaves, idx)
        cur = bytes.fromhex(leaves[idx])
        for level, sib in enumerate(proof):
            s = bytes.fromhex(sib)
            cur = _h.sha256(cur + s).digest() if ((idx >> level) & 1) == 0 else _h.sha256(s + cur).digest()
        check(f"proof[{idx}] verifies like merkle-verifier", cur.hex() == r1)


# ─── canonical json ─────────────────────────────────────────────────
def test_canonical():
    print("canonical_json:")
    a = {"b": 1, "a": [2.0, 1.0]}
    b = {"a": [2.0, 1.0], "b": 1}
    check("key order irrelevant", canonical_json(a) == canonical_json(b))
    check("floats rounded", canonical_json({"x": 1.23456789}) == '{"x":1.234568}')
    check("hash deterministic", canonical_hash(a) == canonical_hash(b))
    try:
        canonical_json({"x": float("nan")})
        check("nan rejected", False)
    except ValueError:
        check("nan rejected", True)


# ─── spans ──────────────────────────────────────────────────────────
def test_spans():
    print("split_spans:")
    text = f"{THINK_OPEN}plan something{THINK_CLOSE}Here is my answer."
    spans = split_spans(text)
    r = spans["reasoning"]
    check("reasoning found", r is not None and r[0] == 0)
    check("reasoning covers tag", text[r[0]:r[1]].startswith(THINK_OPEN))
    out = spans["output"]
    check("output after reasoning", out is not None and text[out[0]:out[1]].startswith("Here"))

    text2 = "Answer first."
    spans2 = split_spans(text2)
    check("no reasoning when absent", spans2["reasoning"] is None)
    check("output covers rest", spans2["output"] == (0, len(text2)))

    text3 = f"Reasoning...{TOOL_OPEN}call(x){TOOL_CLOSE}"
    spans3 = split_spans(text3)
    check("tool_call found", spans3["tool_call"] is not None)
    check("output before tool_call", spans3["output"] is not None and spans3["output"][1] <= spans3["tool_call"][0])


def test_token_mapping():
    print("char_range_to_token_range:")
    offsets = [0, 4, 8, 12, 20, 24]
    r = char_range_to_token_range((8, 20), offsets)
    check("maps to (2,4)", r == (2, 4))
    check("none passthrough", char_range_to_token_range(None, offsets) is None)


# ─── snapshot ───────────────────────────────────────────────────────
def base_kwargs(**over):
    kw = dict(
        model_id="qwen2.5-0.5b-instruct",
        model_revision="Qwen/Qwen2.5-0.5B-Instruct@rev1",
        weights_hash="w" * 64,
        bank_hash="b" * 64,
        method="contrastive-dom-v0.1",
        layers=[12],
        prompt_hash="p" * 64,
        response_hash="r" * 64,
        span_maxima={"output": {"deception": {"max": 0.8, "argmax": 3}}},
        detections=[{
            "concept": "deception", "span": "output", "position": 3,
            "score": 0.8, "threshold": 0.5,
        }],
        integrity=0.2,
        gate="red",
        activation_root="a" * 64,
        activation_count=42,
        tier="t0-operator",
        signer="operator-1",
        timestamp=1730000000,
    )
    kw.update(over)
    return kw


def test_snapshot():
    print("build_thought_snapshot:")
    dets = [
        {"concept": "deception", "span": "output", "position": 3, "score": 0.8, "threshold": 0.5},
        {"concept": "reward_hacking", "span": "prompt", "position": 1, "score": 0.4, "threshold": 0.5},
    ]
    integ = snapshot_integrity(dets)
    check("integrity = 1 - max", approx(integ, 0.2))
    check("clean = 1.0", snapshot_integrity([]) == 1.0)
    check("0.2 is yellow (fixture parity)", snapshot_gate(integ) == "yellow")
    strong = [{"concept": "deception", "span": "output", "position": 0, "score": 0.95, "threshold": 0.5}]
    check("strong detection is red", snapshot_gate(snapshot_integrity(strong)) == "red")
    check("gate yellow", snapshot_gate(0.2) == "yellow")
    check("gate green", snapshot_gate(0.5) == "green")
    check("gate boundary green", snapshot_gate(0.30) == "green")
    check("gate boundary yellow", snapshot_gate(0.15) == "yellow")

    snap = build_thought_snapshot(**base_kwargs())
    check("schema tag", snap["schema"] == SCHEMA)
    check("semantics tag", snap["score_semantics"] == SCORE_SEMANTICS)
    check("hash present", len(snap["snapshot_hash"]) == 64)

    snap2 = build_thought_snapshot(**base_kwargs())
    check("deterministic hash", snap["snapshot_hash"] == snap2["snapshot_hash"])

    snap3 = build_thought_snapshot(**base_kwargs(integrity=0.5, gate="yellow"))
    check("score changes hash", snap2["snapshot_hash"] != snap3["snapshot_hash"])

    try:
        build_thought_snapshot(
            model_id="m", model_revision="r", weights_hash="w", bank_hash="b",
            method="x", layers=[0], prompt_hash="p", response_hash="r",
            tier="t9-bogus",
        )
        check("bad tier rejected", False)
    except ValueError:
        check("bad tier rejected", True)


if __name__ == "__main__":
    test_quantize()
    test_merkle()
    test_canonical()
    test_spans()
    test_token_mapping()
    test_snapshot()
    print()
    if FAILURES:
        print(f"FAILED: {len(FAILURES)} checks")
        sys.exit(1)
    print("all thought.v1 checks passed")
