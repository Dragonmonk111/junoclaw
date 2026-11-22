"""J8 — steering (intervention). GOVERNANCE-GATED. Dry-run only.

Subtracting a concept direction inside the forward pass changes model
behavior — a different product claim with a different liability profile
than measurement. This tool ONLY:
  1. derives a steering spec from a J5 causal bank (directions + gains),
  2. emits the attestation record every applied steering must carry,
  3. computes the dry-run delta on precomputed states (what WOULD be
     subtracted) — it never touches a live model or a production path.

Applying steering in a real forward pass is a separate governance
decision (PLAN_JACOBIAN_LENS_PATHWAY §J8). Do not wire this into the
extraction pipeline by default.

Usage:
  py -3 methods/j8_steering.py --j5-bank probe_bank_j5.json --gain 0.5 \
      --precomputed states.json --out steering_spec.json
"""

import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import sha256_file


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--j5-bank", required=True)
    ap.add_argument("--gain", type=float, default=0.5)
    ap.add_argument("--precomputed", default=None)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    with open(args.j5_bank, "r", encoding="utf-8") as f:
        bank = json.load(f)

    spec = {
        "steering_version": "steering-v0.1",
        "mode": "dry-run-only — governance-gated; never applied by this tool",
        "source_bank_sha256": sha256_file(args.j5_bank),
        "probe_model": bank["probe_model"],
        "gain": args.gain,
        "directions": {
            name: {"layers": c["layers"], "gain": args.gain}
            for name, c in sorted(bank.get("concepts", {}).items())
        },
        "attestation_required": [
            "steering_spec_hash", "concept", "layer", "gain", "operator",
            "governance_ref", "applied_at",
        ],
    }

    # Dry-run delta on precomputed states: report projection magnitude
    # that WOULD be removed, per concept per layer (no states modified).
    if args.precomputed:
        with open(args.precomputed, "r", encoding="utf-8") as f:
            states = json.load(f)
        deltas = {}
        for name, c in sorted(bank.get("concepts", {}).items()):
            for ls, v in sorted(c["layers"].items()):
                vec = np.array(v, dtype=np.float64)
                mags = []
                for t, layers in sorted(states.items()):
                    if ls in layers:
                        h = np.array(layers[ls], dtype=np.float64)
                        mags.append(round(float(args.gain * np.dot(h, vec)), 6))
                if mags:
                    deltas[f"{name}@{ls}"] = {"mean_abs_projection_removed": round(float(np.mean(np.abs(mags))), 6)}
        spec["dry_run_deltas"] = deltas

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(spec, f, indent=1, sort_keys=True)
    print(f"[j8] steering spec written: {args.out} (dry-run only)", file=sys.stderr)


if __name__ == "__main__":
    main()
