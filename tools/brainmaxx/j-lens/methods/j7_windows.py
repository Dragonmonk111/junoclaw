"""J7 — temporal windows: slow-forming intent across turns.

Single-generation probes see flashes; some intent only exists as a
*trajectory across turns*. J7 keeps a per-agent rolling cache of concept
max-scores and detects concepts that rise steadily across a window even
when no single turn crosses a threshold.

Operator-side state: agents/<agent_id>/windows.jsonl (append-only,
DAO-visible). Deterministic: EWMA + least-squares slope over the last N
turns, fixed order, no randomness.

Usage:
  py -3 methods/j7_windows.py --agent agent007 --snapshot snap.json \
      --cache-dir agents --window 8 --rise-threshold 0.05
  (run once per turn; the snapshot is a normal j_space_snapshot)
"""

import argparse
import json
import os
import sys

import numpy as np


def snapshot_max_scores(snapshot):
    """Max detection score per concept in one snapshot (0 when absent)."""
    out = {}
    for d in snapshot.get("detections", []):
        c = d["concept"]
        s = float(d.get("jacobian_score", d.get("score", 0.0)))
        out[c] = max(out.get(c, 0.0), s)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--agent", required=True)
    ap.add_argument("--snapshot", required=True)
    ap.add_argument("--cache-dir", default="agents")
    ap.add_argument("--window", type=int, default=8)
    ap.add_argument("--rise-threshold", type=float, default=0.05,
                    help="min per-turn slope to flag as rising intent")
    ap.add_argument("--alpha", type=float, default=0.35, help="EWMA smoothing")
    args = ap.parse_args()

    with open(args.snapshot, "r", encoding="utf-8") as f:
        snap = json.load(f)
    scores = snapshot_max_scores(snap)

    os.makedirs(args.cache_dir, exist_ok=True)
    path = os.path.join(args.cache_dir, args.agent, "windows.jsonl")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps({"snapshot_hash": snap.get("snapshot_hash"), "scores": scores}, sort_keys=True) + "\n")

    with open(path, "r", encoding="utf-8") as f:
        rows = [json.loads(line) for line in f if line.strip()]
    window = rows[-args.window:]

    concepts = sorted({c for r in window for c in r["scores"]})
    flags = []
    ewma = {}
    for c in concepts:
        series = [r["scores"].get(c, 0.0) for r in window]
        e = series[0]
        for s in series[1:]:
            e = args.alpha * s + (1 - args.alpha) * e
        ewma[c] = round(e, 6)
        if len(series) >= 3:
            x = np.arange(len(series), dtype=np.float64)
            slope = float(np.polyfit(x, np.array(series), 1)[0])
            if slope >= args.rise_threshold:
                flags.append({"concept": c, "per_turn_slope": round(slope, 6),
                              "turns": len(series), "ewma": round(e, 6)})

    report = {
        "agent": args.agent,
        "turns_seen": len(rows),
        "window": len(window),
        "ewma": ewma,
        "rising_intent": flags,
        "note": "rising trajectory across turns — not proof of intent; a flag for stricter review",
    }
    out = os.path.join(args.cache_dir, args.agent, "j7_report.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=1, sort_keys=True)
    print(json.dumps({"flags": len(flags), "report": out}, indent=1))


if __name__ == "__main__":
    main()
