#!/usr/bin/env python3
"""jlens-committee — reference verifier-seat service for thought-court.

Watches the on-chain dispute set; when this node's address sits on a
dispute's committee and it hasn't voted yet, it:

  1. locates the operator's DISCLOSURE bundle for the attestation root
     (snapshot + jlens.activations.v1.1 blobs + probe bank), from a local
     artifact directory the operator/retention layer must populate —
     disclosure availability is the thing being incentivized, so a
     missing bundle is itself evidence;
  2. runs challenge_recompute.py (the certified dispute harness);
  3. submits the verdict to the court via the mcp signing boundary
     (npx tsx src/jlens-court.ts --verdict ...), with evidence_hash =
     sha256 over the harness output + input artifact hashes.

Chain access: queries go through jlens-court.ts --query-* (junoclaw-g1
is gRPC-only, no REST gateway). Verdicts are signed by the enrolled
wallet, never by this script directly.

This is the REFERENCE implementation for validator seats — a WAVS
component should replace the poll loop in production; the dispute
semantics (seed sortition, quorum, slashing) live in the contract.

Usage:
  python committee_service.py --court juno1... --mcp-dir ../../mcp \
      --wallet jlens-ops --artifacts ./disclosures --bank bank.json \
      --model Qwen/Qwen2.5-0.5B-Instruct [--dtype bfloat16] [--once]
"""

import argparse
import base64
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time

POLL_SECS = 60


def npx(mcp_dir: str):
    exe = shutil.which("npx") or shutil.which("npx.cmd")
    if not exe:
        raise RuntimeError("npx not on PATH")
    def run(*a: str) -> str:
        r = subprocess.run(
            [exe, "tsx", "src/jlens-court.ts", *a],
            cwd=mcp_dir, capture_output=True, text=True, timeout=120,
        )
        if r.returncode != 0:
            raise RuntimeError(f"jlens-court failed: {r.stderr.strip() or r.stdout.strip()}")
        return r.stdout
    return run


def court_query(run, court: str, msg: dict):
    out = run("--contract", court, "--query", json.dumps(msg))
    return json.loads(out)


def our_address(mcp_dir: str, wallet: str) -> str:
    # Wallet files are ~/.junoclaw/wallets/<id>.enc — the mnemonic is
    # encrypted inside but metadata.address is public cleartext. Read
    # ONLY that field (field extraction of public data, same as the
    # wallet store's list()).
    import pathlib
    f = pathlib.Path.home() / ".junoclaw" / "wallets" / f"{wallet}.enc"
    if not f.exists():
        raise RuntimeError(f"wallet file not found: {f}")
    try:
        return json.loads(f.read_text())["metadata"]["address"]
    except (KeyError, json.JSONDecodeError) as e:
        raise RuntimeError(f"cannot read address metadata from {f}: {e}")


def find_bundle(art_dir: str, root: str):
    """Locate a disclosure bundle for an activation root.

    Layout: <artifacts>/<root>.json with keys
      {snapshot, blobs, bank?, model?} — paths relative to the manifest,
    or a directory <artifacts>/<root>/ containing
      snapshot.json + <hash>.activations.json.
    Returns (snapshot_path, blobs_path, bank_override, model_override).
    """
    manifest = os.path.join(art_dir, f"{root}.json")
    if os.path.exists(manifest):
        m = json.load(open(manifest))
        base = os.path.dirname(manifest)
        return (
            os.path.join(base, m["snapshot"]),
            os.path.join(base, m["blobs"]),
            m.get("bank"),
            m.get("model"),
        )
    d = os.path.join(art_dir, root)
    if os.path.isdir(d):
        snaps = [f for f in os.listdir(d) if f.endswith(".json")
                 and not f.endswith(".activations.json")]
        blobs = [f for f in os.listdir(d) if f.endswith(".activations.json")]
        if snaps and blobs:
            return os.path.join(d, snaps[0]), os.path.join(d, blobs[0]), None, None
    return None


def evidence_hash(snapshot_p, blobs_p, harness_out: str) -> str:
    h = hashlib.sha256()
    for p in (snapshot_p, blobs_p):
        h.update(open(p, "rb").read())
    h.update(harness_out.encode())
    return h.hexdigest()


def run_dispute(args, run, att_id: str, dispute: dict) -> str:
    """Recompute and return 'clean' | 'fraud' | 'skip:<reason>'."""
    found = find_bundle(args.artifacts, att_id)
    if not found:
        return "skip:no-disclosure-bundle (operator availability failure)"
    snap_p, blobs_p, bank, model = found
    cmd = [
        sys.executable,
        os.path.join(os.path.dirname(os.path.abspath(__file__)), "challenge_recompute.py"),
        "--snapshot", snap_p,
        "--blobs", blobs_p,
        "--bank", bank or args.bank,
        "--model", model or args.model,
        "--dtype", args.dtype,
        "--device", args.device,
    ]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=1800)
    ev = evidence_hash(snap_p, blobs_p, r.stdout + r.stderr)
    verdict = "clean" if r.returncode == 0 else "fraud"
    print(f"  [dispute {att_id[:12]}] harness rc={r.returncode} -> {verdict} "
          f"(evidence {ev[:16]}…)")
    run("--contract", args.court, "--verdict", att_id,
        "--verdict-kind", verdict, "--evidence-hash", ev)
    return verdict


def poll(args, run, me: str):
    challenged = court_query(run, args.court, {
        "list_attestations": {"status": "challenged", "limit": 100}
    }).get("attestations", [])
    for att in challenged:
        aid = att["id"]
        try:
            d = court_query(run, args.court, {"get_dispute": {"attestation_id": aid}})
        except Exception as e:
            print(f"  [dispute {aid[:12]}] query failed: {e}")
            continue
        if me not in [str(a) for a in d.get("committee", [])]:
            continue
        if any(v["verifier"] == me for v in d.get("votes", [])):
            continue
        if int(d["verdict_deadline"]) <= int(time.time()):
            continue
        print(f"  [dispute {aid[:12]}] seated — recomputing")
        try:
            run_dispute(args, run, aid, d)
        except Exception as e:
            # A failed recompute is abstention, not a vote — never guess.
            print(f"  [dispute {aid[:12]}] recompute error, abstaining: {e}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--court", required=True)
    ap.add_argument("--mcp-dir", required=True)
    ap.add_argument("--wallet", default="jlens-ops")
    ap.add_argument("--artifacts", required=True,
                    help="directory of disclosure bundles keyed by activation_root")
    ap.add_argument("--bank", required=True)
    ap.add_argument("--model", required=True)
    ap.add_argument("--dtype", default="bfloat16")
    ap.add_argument("--device", default="cpu")
    ap.add_argument("--poll-secs", type=int, default=POLL_SECS)
    ap.add_argument("--once", action="store_true")
    args = ap.parse_args()

    run = npx(args.mcp_dir)
    me = our_address(args.mcp_dir, args.wallet)
    print(f"jlens-committee online — seat {me} on {args.court}")
    while True:
        try:
            poll(args, run, me)
        except Exception as e:
            print(f"poll error: {e}")
        if args.once:
            return
        time.sleep(args.poll_secs)


if __name__ == "__main__":
    main()
