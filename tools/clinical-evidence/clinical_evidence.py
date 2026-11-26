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
import os
import subprocess
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


def merkle_proof(leaves, index):
    level = [bytes.fromhex(x) for x in leaves]
    proof = []
    i = index
    while len(level) > 1:
        if len(level) % 2:
            level.append(level[-1])
        proof.append(level[i ^ 1].hex())
        level = [hashlib.sha256(level[j] + level[j + 1]).digest()
                 for j in range(0, len(level), 2)]
        i //= 2
    return proof


SCALE = 1_000_000  # thought-court READOUT_SCALE


def compute_claims(qv_leaf, bank):
    """ReadoutClaims for one int8 leaf against the int8 bank — the exact
    integers the contract recomputes: score=Σa·b, integrity=SCALE−score,
    detection=score≥threshold."""
    score = sum(a * b for a, b in zip(qv_leaf, bank["bank_vec"]))
    return {
        "score": score,
        "integrity": max(0, SCALE - score),
        "detection": score >= bank["threshold"],
    }


def load_key(path):
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
    raw = bytes.fromhex(Path(path).read_text().strip())
    return Ed25519PrivateKey.from_private_bytes(raw)


def sign_hex(priv, msg: bytes) -> str:
    return priv.sign(msg).hex()


def mayo_sign_root(msg_hex: str, mayo_sk: str, mayo_bin: str,
                   variant: str = "mayo2"):
    """Detached MAYO sig over raw message bytes via the hybrid-sign binary.
    Returns (sig_hex, pubkey_hex)."""
    r = subprocess.run(
        [mayo_bin, "mayo-sign", "--variant", variant,
         "--mayo-sk", mayo_sk, "--msg-hex", msg_hex],
        capture_output=True, text=True, timeout=300)
    if r.returncode != 0:
        raise RuntimeError(f"mayo-sign failed: {r.stderr.strip()}")
    kv = dict(l.split("=", 1) for l in r.stdout.strip().splitlines())
    return kv["sig"], kv["pubkey"]


def mayo_verify_root(msg_hex: str, sig_hex: str, pk_hex: str, mayo_bin: str,
                     variant: str = "mayo2") -> bool:
    """Verify a detached MAYO sig; returns False if the binary is missing
    or verification fails."""
    try:
        r = subprocess.run(
            [mayo_bin, "mayo-verify", "--variant", variant,
             "--mayo-pk", pk_hex, "--sig-hex", sig_hex, "--msg-hex", msg_hex],
            capture_output=True, text=True, timeout=300)
        return r.returncode == 0
    except FileNotFoundError:
        return False


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
                                "output_sha256": sha256(output)}, None
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
    # Machine-adjudicable readout: per-token int8 leaves at the probe layer
    # (layer 12 — the deception direction was calibrated there). Leaf bytes
    # are two's-complement int8; sha256 leaf + pair-chain root matches the
    # thought-court ResolveWithRecompute convention exactly.
    probe_layer = 12
    leaves, activations = [], []
    for step in out.hidden_states:
        v = step[probe_layer][0, -1].float().cpu().numpy().tolist()
        m = max(abs(x) for x in v)
        qv = [int(round(x * 127.0 / m)) for x in v] if m else [0] * len(v)
        activations.append(qv)
        leaves.append(hashlib.sha256(bytes((q & 0xFF) for q in qv)).hexdigest())
    adjudication = {
        "probe_layer": probe_layer,
        "activation_root": merkle_root(leaves),
        "leaf_count": len(leaves),
        "leaves": leaves,
        "activations": activations,
    }
    return prompt, output, readout, adjudication


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


def produce_bundle(case, priv, model_name, io_only, mayo_sk=None, mayo_bin=None):
    """Full pipeline for one case: retrieve -> infer -> signed events ->
    Merkle root -> adjudication block. Returns (bundle, docs)."""
    case_hash = sha256(canon(case))
    docs = retrieve(case["narrative"] + " " + json.dumps(case["vitals"]),
                    HERE / "guidelines", top_k=3)
    prompt, output, readout, adjudication = run_inference(
        case, docs, model_name, io_only)

    events = [
        make_event(priv, 0, "case", {"case_id": case["case_id"],
                                     "case_sha256": case_hash}),
        make_event(priv, 1, "retrieve",
                   {"query_sha256": sha256(case["narrative"]),
                    "docs": docs}),
        make_event(priv, 2, "readout",
                   {**readout, "activation_root": adjudication["activation_root"]}
                   if adjudication else readout),
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
    # PQ half: MAYO signature over the same root — the bundle stays valid
    # even if ed25519 is later broken (AND'd, like the chain's certs).
    if mayo_sk:
        msig, mpk = mayo_sign_root(root, mayo_sk, mayo_bin or "hybrid-sign")
        bundle["mayo_variant"] = "mayo2"
        bundle["mayo_sig"] = msig
        bundle["mayo_pubkey"] = mpk
    if adjudication:
        bank = json.loads((HERE / "bank_deception_int8.json").read_text())
        leaf_index = adjudication["leaf_count"] - 1
        qv = adjudication["activations"][leaf_index]
        claims = compute_claims(qv, bank)
        bundle["adjudication"] = {
            "bank": bank["name"],
            "bank_commitment": bank["bank_commitment"],
            "threshold": bank["threshold"],
            "activation_root": adjudication["activation_root"],
            "leaf_count": adjudication["leaf_count"],
            "leaf_index": leaf_index,
            "merkle_path": merkle_proof(adjudication["leaves"], leaf_index),
            "claims": claims,
            "activations_leaf": qv,
        }
    return bundle, docs


def cmd_run(a):
    priv = load_key(a.key)
    case = json.loads(Path(a.case).read_text(encoding="utf-8"))
    bundle, docs = produce_bundle(case, priv, a.model, a.io_only,
                                  a.mayo_sk, a.mayo_sign_bin)
    Path(a.out).write_text(json.dumps(bundle, indent=2))
    print(f"case:    {case['case_id']}")
    print(f"docs:    {[d['doc_id'] for d in docs]}")
    print(f"output:  {bundle['output'][:120]}")
    print(f"root:    {bundle['merkle_root']}")
    print(f"bundle -> {a.out}")


# Triage: which of N runs actually need a human. Two provable signals —
# every flag points at evidence, not vibes.
def triage_verdict(case, bundle, docs, bank):
    reasons = []
    retrieved = {d["doc_id"] for d in docs}
    for req in case.get("required_docs", []):
        if req not in retrieved:
            reasons.append(
                f"required protocol '{req}' never entered context "
                f"(answered from memory, not protocol)")
    adj = bundle.get("adjudication")
    if adj and adj["claims"]["score"] < bank["threshold"]:
        reasons.append(
            f"readout below threshold (score {adj['claims']['score']} "
            f"< {bank['threshold']})")
    return ("FLAG", reasons) if reasons else ("CLEAR", [])


def cmd_triage(a):
    """Run every case in the manifest and emit the review queue — the
    lane-departure warning: which of the N runs a human must check,
    with the evidence-backed reason for each flag."""
    priv = load_key(a.key)
    manifest = json.loads(Path(a.manifest).read_text(encoding="utf-8"))
    bank = json.loads((HERE / "bank_deception_int8.json").read_text())
    outdir = Path(a.outdir); outdir.mkdir(exist_ok=True)
    results = []
    for entry in manifest["cases"]:
        case = json.loads((HERE / "cases" / entry).read_text(encoding="utf-8"))
        if a.rescore:
            bundle = json.loads(
                (outdir / f"{case['case_id']}.json").read_text())
            ret = next(e for e in bundle["events"]
                       if e["type"] == "retrieve")["payload"]
            docs = ret["docs"]
        else:
            bundle, docs = produce_bundle(case, priv, a.model, a.io_only,
                                          getattr(a, "mayo_sk", None),
                                          getattr(a, "mayo_sign_bin", None))
            (outdir / f"{case['case_id']}.json").write_text(
                json.dumps(bundle, indent=2))
        verdict, reasons = triage_verdict(case, bundle, docs, bank)
        results.append({
            "case_id": case["case_id"],
            "bundle_root": bundle["merkle_root"],
            "verdict": verdict,
            "reasons": reasons,
            "retrieved": [d["doc_id"] for d in docs],
            "readout_score": (bundle.get("adjudication") or {})
                              .get("claims", {}).get("score"),
            "output": bundle["output"],
        })
        print(f"  {case['case_id']:<20} {verdict:<6} "
              + ("; ".join(reasons) if reasons else "all checks pass"))
    flagged = [r for r in results if r["verdict"] == "FLAG"]
    report = {
        "triage_root": merkle_root([r["bundle_root"] for r in results]),
        "n_cases": len(results),
        "n_flagged": len(flagged),
        "review_queue": [r["case_id"] for r in flagged],
        "results": results,
        "note": "flags are evidence-backed: required protocol absent "
                "from committed retrieval, or below-threshold readout — "
                "recompute any bundle against its committed root to check",
    }
    (outdir / "triage-report.json").write_text(json.dumps(report, indent=2))
    print(f"\nREVIEW QUEUE: {len(flagged)} of {len(results)} need a human: "
          f"{report['review_queue']}")
    print(f"triage_root: {report['triage_root']}")
    print(f"report -> {outdir / 'triage-report.json'}")


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
    # 6. MAYO half (when present): must verify or the bundle is invalid —
    # a stripped/downgraded bundle must not pass silently.
    if "mayo_sig" in bundle:
        ok = mayo_verify_root(
            bundle["merkle_root"], bundle["mayo_sig"], bundle["mayo_pubkey"],
            getattr(a, "mayo_sign_bin", None) or os.environ.get(
                "HYBRID_SIGN_BIN", "hybrid-sign"),
            bundle.get("mayo_variant", "mayo2"))
        if not ok:
            fails.append("mayo signature invalid or unverifiable")

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
    adj = bundle.get("adjudication")
    if adj:
        # machine-adjudicable: activation_root is the int8-leaf tree root and
        # claims make the attestation resolvable via ResolveWithRecompute
        msg = {"anchor_attestation": {
            "id": bundle["merkle_root"],
            "activation_root": adj["activation_root"],
            "claims": adj["claims"] | {"bank_commitment": adj["bank_commitment"]},
        }}
    else:
        activation_root = readout.get("state_sha256", readout["output_sha256"])
        msg = {"anchor_attestation": {"id": bundle["merkle_root"],
                                      "activation_root": activation_root}}
    print("contract: juno1w2z0nq7az8t0sgnxuft8y40u4tawl52nv7jqasmdgxzcy03p3n0q76qjfe")
    print("msg:      " + json.dumps(msg))
    print("exec:     bls-relayer jc-execute --contract <addr> --msg '<msg>' "
          "--funds 1000000ujclaw --jc-key-file <agent-key>")
    print("verify:   bls-relayer jc-query --contract <addr> "
          f"--msg '{{\"get_attestation\":{{\"attestation_id\":\"{bundle['merkle_root']}\"}}}}'")


def cmd_resolve(a):
    """Emit the ResolveWithRecompute msg — the contract itself recomputes
    Σa·b over the committed leaf + bank and convicts or clears the declared
    claims. No committee, no trust."""
    bundle = json.loads(Path(a.bundle).read_text(encoding="utf-8"))
    adj = bundle["adjudication"]
    bank = json.loads((HERE / "bank_deception_int8.json").read_text())
    msg = {"resolve_with_recompute": {
        "attestation_id": bundle["merkle_root"],
        "activations": adj["activations_leaf"],
        "bank_vec": bank["bank_vec"],
        "threshold": bank["threshold"],
        "leaf_index": adj["leaf_index"],
        "merkle_path": adj["merkle_path"],
    }}
    print("exec: bls-relayer jc-execute --contract <addr> --msg '<msg>' "
          "--jc-key-file <any-key>   # permissionless resolve")
    print(json.dumps(msg))


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
    r.add_argument("--mayo-sk", default=os.environ.get("CLINICAL_MAYO_SK"),
                   help="MAYO secret-seed hex — dual-signs the bundle (PQ)")
    r.add_argument("--mayo-sign-bin", default=os.environ.get("HYBRID_SIGN_BIN", "hybrid-sign"),
                   help="path to hybrid-sign binary (default: PATH)")
    v = sub.add_parser("verify"); v.add_argument("--bundle", required=True)
    v.add_argument("--mayo-sign-bin", default=os.environ.get("HYBRID_SIGN_BIN", "hybrid-sign"),
                   help="path to hybrid-sign binary for MAYO verification")
    t = sub.add_parser("tamper"); t.add_argument("--bundle", required=True)
    t.add_argument("--out", required=True)
    an = sub.add_parser("anchor"); an.add_argument("--bundle", required=True)
    rs = sub.add_parser("resolve"); rs.add_argument("--bundle", required=True)
    tr = sub.add_parser("triage")
    tr.add_argument("--manifest", required=True)
    tr.add_argument("--key", required=True)
    tr.add_argument("--outdir", default="triage-out")
    tr.add_argument("--model", default="Qwen/Qwen2.5-0.5B-Instruct")
    tr.add_argument("--io-only", action="store_true")
    tr.add_argument("--mayo-sk", default=os.environ.get("CLINICAL_MAYO_SK"))
    tr.add_argument("--mayo-sign-bin", default=os.environ.get("HYBRID_SIGN_BIN", "hybrid-sign"))
    tr.add_argument("--rescore", action="store_true",
                    help="re-evaluate saved bundles in outdir, no inference")
    a = ap.parse_args()
    {"keygen": cmd_keygen, "run": cmd_run, "anchor": cmd_anchor,
     "resolve": cmd_resolve, "verify": cmd_verify, "tamper": cmd_tamper,
     "triage": cmd_triage}[a.cmd](a)


if __name__ == "__main__":
    main()
