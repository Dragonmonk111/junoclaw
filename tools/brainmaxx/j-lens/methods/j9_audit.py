"""J9 — cross-agent audit + J-Reef feedback.

Aggregates j_space_snapshots across the agent population: which concepts
recur, in which agents, at what rates. Recurring concept signatures become
candidate jreef: entries — the population-level analog of a single thought
attestation, for the DAO to inspect, challenge, and vote on.

Usage:
  py -3 methods/j9_audit.py --snapshots snaps/*.json --min-agents 2 \
      --out jreef_candidates.json
"""

import argparse
import glob
import json
import sys


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--snapshots", nargs="+", required=True,
                    help="snapshot files (glob ok); each may carry agent_id")
    ap.add_argument("--min-agents", type=int, default=2,
                    help="minimum distinct agents before a concept becomes a jreef candidate")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    files = []
    for pattern in args.snapshots:
        files.extend(sorted(glob.glob(pattern)))
    if not files:
        print("[j9] no snapshots matched", file=sys.stderr)
        sys.exit(1)

    per_agent = {}
    for path in files:
        with open(path, "r", encoding="utf-8") as f:
            snap = json.load(f)
        agent = snap.get("agent_id", path)
        det = {}
        for d in snap.get("detections", []):
            c = d["concept"]
            s = float(d.get("jacobian_score", d.get("score", 0.0)))
            det[c] = max(det.get(c, 0.0), s)
        per_agent[agent] = det

    concepts = sorted({c for det in per_agent.values() for c in det})
    candidates = []
    stats = {}
    for c in concepts:
        agents_with = sorted(a for a, det in per_agent.items() if c in det)
        scores = [per_agent[a][c] for a in agents_with]
        stats[c] = {
            "agents_with_detection": len(agents_with),
            "population": len(per_agent),
            "max_score": round(max(scores), 6),
            "mean_score": round(sum(scores) / len(scores), 6),
        }
        if len(agents_with) >= args.min_agents:
            candidates.append({
                "jreef_key": f"jreef:concept:{c}",
                "concept": c,
                "agents": agents_with,
                "evidence": stats[c],
                "status": "candidate — DAO inspect/challenge/vote, not an assertion",
            })

    report = {
        "audit_version": "cross-agent-audit-v0.1",
        "agents": len(per_agent),
        "snapshots": len(files),
        "concept_stats": stats,
        "jreef_candidates": candidates,
        "note": "population recurrence is a review signal, not a verdict on any agent",
    }
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=1, sort_keys=True)
    print(json.dumps({"agents": len(per_agent), "candidates": len(candidates), "out": args.out}, indent=1))


if __name__ == "__main__":
    main()
