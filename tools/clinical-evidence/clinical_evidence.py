"""clinical-evidence — auditable clinical decision-support evidence sidecar.

Wraps a decision-support run (synthetic case -> guideline retrieval -> open
model recommendation) in the JunoClaw evidence format: every step emits a
signed, hash-chained event; the model's activation readout at action time is
committed; the whole bundle collapses to one Merkle root that can anchor
on-chain. A third party recomputes the bundle and compares the root — a
contested decision produces a verifiable artifact, not a debate.

No PHI: cases are synthetic and commitments bind to hashes, not content.

Usage:
  py clinical_evidence.py keygen --out agent.key
  py clinical_evidence.py run --case cases/sepsis-01.json --key agent.key \
      --out bundle.json [--model Qwen/Qwen2.5-0.5B-Instruct] [--io-only]
  py clinical_evidence.py verify --bundle bundle.json
  py clinical_evidence.py tamper --bundle bundle.json --out tampered.json
"""

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent


def sha256(b) -> str:
    if isinstance(b, str):
        b = b.encode("utf-8")
    return hashlib.sha256(b).hexdigest()


def canon(obj) -> bytes:
    return json.dumps(obj, sort_keys=True, separators=(",", ":")).encode("utf-8")


def merkle_root(leaves) -> str:
    if not leaves:
        return sha256(b"")
    level = [bytes.fromhex(x) for x in leaves]
    while len(level) > 1:
        if len(level) % 2:
            level.append(level[-1])
        level = [hashlib.sha256(level[i] + level[i + 1]).digest()
                 for i in range(0, len(level), 2)]
    return level[0].hex()


def load_key(path):
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
    raw = bytes.fromhex(Path(path).read_text().strip())
    return Ed25519PrivateKey.from_private_bytes(raw)


def sign_hex(priv, msg: bytes) -> str:
    return priv.sign(msg).hex()


def pub_hex(priv) -> str:
    from cryptography.hazmat.primitives import serialization
    return priv.public_key().public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw).hex()


def token_score(query: str, doc: str) -> int:
    """Deterministic retrieval: summed term frequency of query words in doc."""
    stop = {"the", "a", "an", "of", "and", "or", "in", "to", "is", "for",
            "with", "on", "at", "be", "by", "as", "are", "was", "that"}
    q = {w.strip(".,;:()").lower() for w in query.split()} - stop
    dl = doc.lower()
    return sum(dl.count(w) for w in q if len(w) > 2)


def retrieve(case_text: str, corpus_dir: Path, top_k: int = 3):
    scored = []
    for p in sorted(corpus_dir.glob("*.md")):
        body = p.read_text(encoding="utf-8")
        scored.append({"doc_id": p.name, "doc_sha256": sha256(body),
                       "score": token_score(case_text, body)})
    scored.sort(key=lambda d: (-d["score"], d["doc_id"]))
    return scored[:top_k]


def make_event(priv, seq, etype, payload) -> dict:
    ev = {"seq": seq, "type": etype, "ts_ns": time.time_ns(),
          "payload_sha256": sha256(canon(payload)), "payload": payload}
    ev["sig"] = sign_hex(priv, canon({k: ev[k] for k in ("seq", "type", "ts_ns", "payload_sha256")}))
    return ev


def run_inference(case, docs, model_name, io_only):
    """Return (output_text, readout). readout commits the model's internal
    state at action time; --io-only degrades to prompt/output hashes."""
    guideline_text = "\n\n".join(
        (HERE / "guidelines" / d["doc_id"]).read_text(encoding="utf-8")
        for d in docs)
    prompt = (
        "You are a clinical decision support assistant. Using ONLY the "
        "guideline excerpts provided, give a one-line recommendation.\n\n"
        f"GUIDELINES:\n{guideline_text}\n\n"
        f"SYNTHETIC PATIENT CASE:\n{case['narrative']}\n\n"
        "VITALS: " + json.dumps(case["vitals"]) + "\n"
        "LABS: " + json.dumps(case["labs"]) + "\n\n"
        "RECOMMENDATION:")
    if io_only:
        output = "[io-only mode: inference skipped]"
        return prompt, output, {"mode": "io_only",
                                "prompt_sha256": sha256(prompt),
                                "output_sha256": sha256(output)}
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer
    tok = AutoTokenizer.from_pretrained(model_name)
    model = AutoModelForCausalLM.from_pretrained(
        model_name, torch_dtype=torch.float32)
    model.eval()
    inputs = tok(prompt, return_tensors="pt")
    with torch.no_grad():
        out = model.generate(**inputs, max_new_tokens=48,
                             output_hidden_states=True,
                             return_dict_in_generate=True, do_sample=False)
    seq = out.sequences[0]
    output = tok.decode(seq[inputs["input_ids"].shape[1]:],
                        skip_special_tokens=True).strip()
    # Action-time readout: hidden state of the final generated token at the
    # model's last layer — committed, not published.
    last_layer = -1
    final_step = out.hidden_states[-1][last_layer][0, -1]
    readout = {
        "mode": "activation",
        "model": model_name,
        "layer": last_layer,
        "prompt_sha256": sha256(prompt),
        "output_sha256": sha256(output),
        "state_sha256": sha256(final_step.float().cpu().numpy().tobytes()),
        "n_generated": int(seq.shape[0] - inputs["input_ids"].shape[1]),
    }
    return prompt, output, readout


def cmd_keygen(a):
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
    from cryptography.hazmat.primitives import serialization
    priv = Ed25519PrivateKey.generate()
    Path(a.out).write_text(
        priv.private_bytes(serialization.Encoding.Raw,
                           serialization.PrivateFormat.Raw,
                           serialization.NoEncryption()).hex())
    print(f"agent key written -> {a.out}")
    print(f"pubkey: {pub_hex(priv)}")


def cmd_run(a):
    priv = load_key(a.key)
    case = json.loads(Path(a.case).read_text(encoding="utf-8"))
    case_hash = sha256(canon(case))
    docs = retrieve(case["narrative"] + " " + json.dumps(case["vitals"]),
                    HERE / "guidelines", top_k=3)
    prompt, output, readout = run_inference(
        case, docs, a.model, a.io_only)

    events = [
        make_event(priv, 0, "case", {"case_id": case["case_id"],
                                     "case_sha256": case_hash}),
        make_event(priv, 1, "retrieve",
                   {"query_sha256": sha256(case["narrative"]),
                    "docs": docs}),
        make_event(priv, 2, "readout", readout),
        make_event(priv, 3, "output", {"output_sha256": sha256(output)}),
    ]
    leaves = [ev["payload_sha256"] for ev in events]
    root = merkle_root(leaves)
    bundle = {
        "bundle_version": 1,
        "agent_pubkey": pub_hex(priv),
        "case_id": case["case_id"],
        "events": events,
        "prompt_sha256": sha256(prompt),
        "output": output,
        "output_sha256": sha256(output),
        "merkle_root": root,
        "bundle_sig": sign_hex(priv, bytes.fromhex(root)),
        "note": "synthetic case — no PHI; commitments bind to hashes",
    }
    Path(a.out).write_text(json.dumps(bundle, indent=2))
    print(f"case:    {case['case_id']}  ({case_hash[:16]}...)")
    print(f"docs:    {[d['doc_id'] for d in docs]}")
    print(f"output:  {output[:120]}")
    print(f"root:    {root}")
    print(f"bundle -> {a.out}")


def cmd_verify(a):
    bundle = json.loads(Path(a.bundle).read_text(encoding="utf-8"))
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
    pub = Ed25519PublicKey.from_public_bytes(bytes.fromhex(bundle["agent_pubkey"]))
    fails = []

    # 1. every event: payload hashes to payload_sha256, sig checks vs agent key
    for ev in bundle["events"]:
        if sha256(canon(ev["payload"])) != ev["payload_sha256"]:
            fails.append(f"event seq={ev['seq']} payload hash mismatch")
        msg = canon({k: ev[k] for k in ("seq", "type", "ts_ns", "payload_sha256")})
        try:
            pub.verify(bytes.fromhex(ev["sig"]), msg)
        except Exception:
            fails.append(f"event seq={ev['seq']} bad signature")
    # 2. retrieved doc hashes match the live corpus
    ret = next(e for e in bundle["events"] if e["type"] == "retrieve")["payload"]
    for d in ret["docs"]:
        live = HERE / "guidelines" / d["doc_id"]
        if not live.exists() or sha256(live.read_text(encoding="utf-8")) != d["doc_sha256"]:
            fails.append(f"doc {d['doc_id']} hash mismatch vs corpus")
    # 3. output hash matches
    if sha256(bundle["output"]) != bundle["output_sha256"]:
        fails.append("output hash mismatch")
    # 4. merkle root recomputes
    leaves = [ev["payload_sha256"] for ev in bundle["events"]]
    if merkle_root(leaves) != bundle["merkle_root"]:
        fails.append("merkle root mismatch")
    # 5. bundle signature over the root
    try:
        pub.verify(bytes.fromhex(bundle["bundle_sig"]),
                   bytes.fromhex(bundle["merkle_root"]))
    except Exception:
        fails.append("bundle signature invalid")

    if fails:
        print("VERIFY FAIL:")
        for f in fails:
            print(f"  - {f}")
        sys.exit(1)
    print(f"VERIFY PASS - root {bundle['merkle_root'][:16]}... "
          f"recomputed, all signatures valid, docs match corpus")


def cmd_anchor(a):
    """Emit the on-chain anchor: the exact MsgExecuteContract the agent
    broadcasts (with its operator bond) to thought-court on g1."""
    bundle = json.loads(Path(a.bundle).read_text(encoding="utf-8"))
    readout = next(e for e in bundle["events"]
                   if e["type"] == "readout")["payload"]
    activation_root = readout.get("state_sha256", readout["output_sha256"])
    msg = {"anchor_attestation": {"id": bundle["merkle_root"],
                                  "activation_root": activation_root}}
    print("contract: juno1w2z0nq7az8t0sgnxuft8y40u4tawl52nv7jqasmdgxzcy03p3n0q76qjfe")
    print("msg:      " + json.dumps(msg))
    print("exec:     bls-relayer jc-execute --contract <addr> --msg '<msg>' "
          "--funds 1000000ujclaw --jc-key-file <agent-key>")
    print("verify:   bls-relayer jc-query --contract <addr> "
          f"--msg '{{\"get_attestation\":{{\"attestation_id\":\"{bundle['merkle_root']}\"}}}}'")


def cmd_tamper(a):
    bundle = json.loads(Path(a.bundle).read_text(encoding="utf-8"))
    out = bundle["output"]
    bundle["output"] = out[:-1] + ("x" if out[-1] != "x" else "y")
    Path(a.out).write_text(json.dumps(bundle, indent=2))
    print(f"tampered output written -> {a.out} (verify should now fail)")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    k = sub.add_parser("keygen"); k.add_argument("--out", required=True)
    r = sub.add_parser("run")
    r.add_argument("--case", required=True); r.add_argument("--key", required=True)
    r.add_argument("--out", required=True)
    r.add_argument("--model", default="Qwen/Qwen2.5-0.5B-Instruct")
    r.add_argument("--io-only", action="store_true")
    v = sub.add_parser("verify"); v.add_argument("--bundle", required=True)
    t = sub.add_parser("tamper"); t.add_argument("--bundle", required=True)
    t.add_argument("--out", required=True)
    an = sub.add_parser("anchor"); an.add_argument("--bundle", required=True)
    a = ap.parse_args()
    {"keygen": cmd_keygen, "run": cmd_run, "anchor": cmd_anchor,
     "verify": cmd_verify, "tamper": cmd_tamper}[a.cmd](a)


if __name__ == "__main__":
    main()
