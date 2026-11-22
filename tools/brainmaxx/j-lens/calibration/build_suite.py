"""Calibration suite v1 — the frozen, hash-addressed labeled dataset that
every J2-J9 method is benchmarked against (PLAN_JACOBIAN_LENS_PATHWAY §2).

Sources merged (all repo-local, human-checked upstream):
  * examples/concepts.json          — contrastive pairs, 6 core concepts
  * examples/concepts-onchain.json  — the 4 governance concepts
  * examples/onchain_probe_questions.json — 19 labeled probes (held out)

Split discipline (the promotion rule depends on it):
  * tune fold  — threshold/method selection only
  * eval fold  — reported AUROC; never used for selection
  * held_out   — the 19 on-chain probes stay held out entirely (they were
                 the v7 certification set; reusing them for selection would
                 silently invalidate the earlier certification)

Output: calibration_suite_v1.json + .sha256 (frozen by hash; the hash is
what goes into bank.calibration so every bank names the suite it was
certified against).

Run: py -3 calibration/build_suite.py   (from tools/brainmaxx/j-lens)
"""

import hashlib
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)


def load(name):
    with open(os.path.join(ROOT, "examples", name), "r", encoding="utf-8") as f:
        return json.load(f)


def canonical_hash(obj):
    return hashlib.sha256(json.dumps(obj, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def main():
    examples = {}
    examples.update(load("concepts.json"))
    examples.update(load("concepts-onchain.json"))

    concepts = {}
    for name in sorted(examples):
        spec = examples[name]
        pos = list(spec["positive"])
        neg = list(spec["negative"])
        # Deterministic interleave-split: odd-indexed items -> eval fold.
        # Never random — same suite, byte-identical, on every rebuild.
        concepts[name] = {
            "positive": pos,
            "negative": neg,
            "tune": {"positive": pos[::2], "negative": neg[::2]},
            "eval": {"positive": pos[1::2], "negative": neg[1::2]},
        }

    probes = load("onchain_probe_questions.json")
    suite = {
        "suite_version": "calibration-suite-v1",
        "policy": {
            "tune": "threshold/method selection only",
            "eval": "reported AUROC only; never used for selection",
            "held_out": "on-chain probes excluded from selection and eval; reported separately",
        },
        "sources": ["concepts.json", "concepts-onchain.json", "onchain_probe_questions.json"],
        "concepts": concepts,
        "held_out_probes": probes,
    }
    suite["suite_hash"] = canonical_hash({k: v for k, v in suite.items() if k != "suite_hash"})

    out = os.path.join(ROOT, "calibration", "calibration_suite_v1.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(suite, f, indent=1, sort_keys=True)
    digest = hashlib.sha256(open(out, "rb").read()).hexdigest()
    with open(out + ".sha256", "w") as f:
        f.write(digest + "  calibration_suite_v1.json\n")

    n_pos = sum(len(c["positive"]) for c in concepts.values())
    n_neg = sum(len(c["negative"]) for c in concepts.values())
    print(f"suite: {len(concepts)} concepts, {n_pos} positive / {n_neg} negative examples")
    print(f"held-out probes: {len(probes)}")
    print(f"sha256: {digest}")
    print(f"written: {out}")


if __name__ == "__main__":
    main()
