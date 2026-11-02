"""jlens.thought.v1 — the thought snapshot schema.

A thought snapshot is produced at GENERATION TIME by jlens-serve: it
captures what the model's activations looked like while it was deciding,
projects them onto a registered probe bank, and commits to the raw
vectors via a merkle root (activation_root) so any past decision can be
re-audited later with a newer bank — without re-running the model.

Only hashes/roots/scores go on-chain; the quantized activation blobs
stay off-chain (operator-held, optionally pinned to IPFS/Arweave).

Pure Python — no torch required. Testable standalone.
"""

import hashlib
import json
import math

SCHEMA = "jlens.thought.v1"
SCORE_SEMANTICS = "jlens.integrity.v1"
TIERS = ("t0-operator", "t2-tee")
REASON_OPEN = "<" + "think" + ">"
REASON_CLOSE = "</" + "think" + ">"
TOOL_CALL_OPEN = "<" + "tool_call" + ">"
TOOL_CALL_CLOSE = "</" + "tool_call" + ">"


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_json(obj) -> str:
    """Deterministic JSON: sorted keys, no whitespace, floats rounded."""
    def norm(v):
        if isinstance(v, float):
            if math.isnan(v) or math.isinf(v):
                raise ValueError("non-finite float in snapshot")
            return round(v, 6)
        if isinstance(v, dict):
            return {k: norm(x) for k, x in sorted(v.items())}
        if isinstance(v, (list, tuple)):
            return [norm(x) for x in v]
        return v
    return json.dumps(norm(obj), sort_keys=True, separators=(",", ":"))


def canonical_hash(obj) -> str:
    return sha256_hex(canonical_json(obj).encode("utf-8"))


def action_hash(intent: dict) -> str:
    """Deterministic action hash — binds a thought to exactly one action.

    Byte-identical to Rust `thought::action_hash`: sha256 over compact
    JSON with sorted keys of {action, controller_timestamp, params,
    robot_id, sensor_snapshot_hash}. Golden: j-lens/fixtures/action_hash.v1.json
    """
    v = {
        "action": intent["action"],
        "controller_timestamp": intent["controller_timestamp"],
        "params": intent["params"],
        "robot_id": intent["robot_id"],
        "sensor_snapshot_hash": intent["sensor_snapshot_hash"],
    }
    s = json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return sha256_hex(s.encode("utf-8"))


# ─── activation commitment ──────────────────────────────────────────

def quantize_int8(vec):
    """Per-vector symmetric int8 quantization.

    Returns (bytes, scale) with scale = max(abs(vec)) / 127.0 so that
    dequant(v) ≈ v. Deterministic — same vector in, same bytes out.
    """
    m = max(abs(x) for x in vec)
    if m == 0:
        return bytes([128] * len(vec)), 0.0  # offset encoding for zero
    scale = m / 127.0
    out = bytearray(len(vec))
    for i, x in enumerate(vec):
        q = int(round(x / scale))
        out[i] = (q + 128) & 0xFF  # signed → offset byte
    return bytes(out), scale


def activation_leaf_hash(qbytes: bytes) -> str:
    return sha256_hex(b"jlens.act.v1|" + qbytes)


def merkle_root(leaves) -> str:
    """Pairwise sha256 merkle root over leaf hashes (hex strings).

    Same rule as the on-chain merkle-verifier contract and the Rust
    ThoughtRecord anchor: node = sha256(left_bytes || right_bytes) over
    the raw 32-byte digests; odd level duplicates the last node.
    Empty → sha256(b"").
    """
    if not leaves:
        return sha256_hex(b"")
    level = [bytes.fromhex(x) for x in leaves]
    while len(level) > 1:
        if len(level) % 2 == 1:
            level.append(level[-1])
        level = [
            hashlib.sha256(level[i] + level[i + 1]).digest()
            for i in range(0, len(level), 2)
        ]
    return level[0].hex()


def merkle_proof(leaves, index):
    """Sibling path for leaves[index], verifiable by merkle-verifier
    VerifyProof {leaf_hash, leaf_index, proof}."""
    level = [bytes.fromhex(x) for x in leaves]
    proof = []
    i = index
    while len(level) > 1:
        if len(level) % 2 == 1:
            level.append(level[-1])
        sib = i ^ 1
        proof.append(level[sib].hex())
        level = [
            hashlib.sha256(level[j] + level[j + 1]).digest()
            for j in range(0, len(level), 2)
        ]
        i //= 2
    return proof


# ─── spans ─────────────────────────────────────────────────────────

def split_spans(generated: str) -> dict:
    """Split generated text into reasoning / output / tool_call spans.

    Char ranges are over `generated`. Reasoning models emit explicit
    think tags; tool calls carry tool_call tags; everything else is
    plain output.
    """
    spans = {"reasoning": None, "output": None, "tool_call": None}

    def find(tag_open, tag_close):
        if tag_open in generated:
            s = generated.index(tag_open)
            e = generated.index(tag_close) + len(tag_close) if tag_close in generated else len(generated)
            return (s, e)
        return None

    spans["reasoning"] = find(REASON_OPEN, REASON_CLOSE)
    spans["tool_call"] = find(TOOL_CALL_OPEN, TOOL_CALL_CLOSE)

    covered = sorted(sp for sp in (spans["reasoning"], spans["tool_call"]) if sp)
    pos = 0
    for s, e in covered:
        if s > pos:
            spans["output"] = (pos, s)
            break
        pos = max(pos, e)
    else:
        if pos < len(generated):
            spans["output"] = (pos, len(generated))
    return spans


def char_range_to_token_range(char_range, offsets):
    """Map a (start_char, end_char) range to (start_tok, end_tok) using
    token char offsets (list of per-token start offsets + total)."""
    if char_range is None:
        return None
    s, e = char_range
    toks = [i for i, off in enumerate(offsets) if off >= s]
    end = [i for i, off in enumerate(offsets) if off < e]
    if not toks or not end:
        return None
    return (toks[0], end[-1] + 1)


# ─── snapshot assembly ──────────────────────────────────────────────

def build_thought_snapshot(
    *,
    model_id, model_revision, weights_hash,
    bank_hash, method, layers,
    prompt_hash, response_hash, action_hash=None,
    span_maxima=None,
    detections=None,
    integrity=None, gate=None,
    activation_root=None, activation_count=None,
    tier="t0-operator", signer=None, timestamp=None,
):
    """Assemble jlens.thought.v1 and compute snapshot_hash."""
    if tier not in TIERS:
        raise ValueError(f"tier must be one of {TIERS}, got {tier}")
    snap = {
        "schema": SCHEMA,
        "score_semantics": SCORE_SEMANTICS,
        "model_id": model_id,
        "model_revision": model_revision,
        "weights_hash": weights_hash,
        "bank_hash": bank_hash,
        "method": method,
        "layers": layers,
        "prompt_hash": prompt_hash,
        "response_hash": response_hash,
        "action_hash": action_hash,
        "spans": span_maxima or {},
        "detections": detections or [],
        "integrity": round(integrity, 6) if integrity is not None else None,
        "gate": gate,
        "activation_root": activation_root,
        "activation_count": activation_count,
        "tier": tier,
        "signer": signer,
        "timestamp": timestamp,
    }
    snap["snapshot_hash"] = canonical_hash(snap)
    return snap


def snapshot_integrity(detections):
    """integrity = 1 - max(detection cosine). No detections -> 1.0.

    Same formula as computeSeparationScore() in chain-superintelligence.js
    and the Rust gate — jlens.integrity.v1 everywhere.
    """
    if not detections:
        return 1.0
    return max(0.0, 1.0 - max(d["score"] for d in detections))


GATE_THRESHOLDS = {"green": 0.30, "yellow": 0.15}


def snapshot_gate(integrity):
    """Gate from integrity — same boundaries as Rust gate + JS fixture."""
    if integrity >= GATE_THRESHOLDS["green"]:
        return "green"
    if integrity >= GATE_THRESHOLDS["yellow"]:
        return "yellow"
    return "red"
