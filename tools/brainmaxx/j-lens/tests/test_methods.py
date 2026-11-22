"""Fixture tests for the J2-J9 pathway — no model, no torch, no network.

Synthetic hidden states (dim=4) with planted concept directions exercise
every builder and the promotion harness deterministically.

Run from tools/brainmaxx/j-lens:  py -3 tests/test_methods.py
"""

import json
import math
import os
import subprocess
import sys
import tempfile

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, "methods"))

from common import auroc, cosine, fit_logistic, sigmoid  # noqa: E402

PY = sys.executable


def build_fixture(dirpath):
    """Two concepts, 4+4 tune / 4+4 eval examples each, 2 layers.
    Concept A lives in e0 at layer 1 and e1 at layer 2 (a *moving* signal);
    concept B lives in e2 at both layers. Negatives are near-zero noise.
    """
    texts = {}
    labels = {}
    for concept, vecs in (("alpha", ("a_pos", "a_neg")), ("beta", ("b_pos", "b_neg"))):
        for tag, sign in ((vecs[0], 1), (vecs[1], -1)):
            for i in range(8):
                text = f"{tag}_{i}"
                fold = "tune" if i % 2 == 0 else "eval"
                labels.setdefault(concept, {}).setdefault(fold, {"positive": [], "negative": []})
                labels[concept][fold]["positive" if sign == 1 else "negative"].append(text)
                rng = np.random.RandomState(abs(hash(text)) % (2**31))
                base = rng.normal(0, 0.01, 4)
                if concept == "alpha":
                    v1 = np.array([1.0, 0, 0, 0])
                    v2 = np.array([0, 1.0, 0, 0])
                else:
                    v1 = v2 = np.array([0, 0, 1.0, 0])
                texts[text] = {
                    "1": (base + (sign * 2 if sign == 1 else 0) * v1).tolist(),
                    "2": (base + (sign * 2 if sign == 1 else 0) * v2).tolist(),
                }
    suite = {
        "suite_version": "fixture-suite",
        "concepts": labels,
        "held_out_probes": [],
    }
    suite_path = os.path.join(dirpath, "suite.json")
    with open(suite_path, "w") as f:
        json.dump(suite, f)
    states_path = os.path.join(dirpath, "states.json")
    with open(states_path, "w") as f:
        json.dump(texts, f)
    return suite_path, states_path, labels


def run(args):
    r = subprocess.run([PY] + args, capture_output=True, text=True)
    if r.returncode != 0:
        raise AssertionError(f"command failed: {args}\n{r.stderr}")
    return r


def main():
    tmp = tempfile.mkdtemp()
    suite, states, labels = build_fixture(tmp)

    # --- J1 sanity (math core) ---
    assert abs(cosine([1, 0], [1, 0]) - 1.0) < 1e-12
    assert abs(auroc([0.9, 0.8], [0.1, 0.2]) - 1.0) < 1e-12
    assert abs(auroc([0.1], [0.9]) - 0.0) < 1e-12
    w, b = fit_logistic(np.array([[1.0], [2.0], [-1.0], [-2.0]]), np.array([1, 1, 0, 0]), steps=2000)
    assert sigmoid(w * 1.5 + b) > 0.5 and sigmoid(w * -1.5 + b) < 0.5
    print("core math: OK")

    # --- J2: logistic bank ---
    j2_path = os.path.join(tmp, "j2.json")
    run([os.path.join(ROOT, "methods", "j2_logistic.py"),
         "--suite", suite, "--layer", "1", "--precomputed", states,
         "--out", j2_path])
    j2 = json.load(open(j2_path))
    assert j2["method"] == "logistic-l2-v0.1"
    assert "alpha" in j2["concepts"]
    for name, r in j2["auroc_report"].items():
        assert r["tune_auroc"] >= 0.99, f"J2 {name} tune_auroc too low: {r}"
    print("J2 logistic bank: OK (tune AUROC >= 0.99 on planted signal)")

    # --- J3: trajectory bank ---
    j3_path = os.path.join(tmp, "j3.json")
    run([os.path.join(ROOT, "methods", "j3_trajectory.py"),
         "--suite", suite, "--layers", "1,2", "--precomputed", states,
         "--out", j3_path])
    j3 = json.load(open(j3_path))
    assert j3["method"] == "trajectory-dom-v0.1"
    for name, c in j3["concepts"].items():
        assert set(c["layers"].keys()) == {"1", "2"}
        assert abs(sum(c["layer_weights"].values()) - 1.0) < 1e-9
        for ls, r in j3["build_report"][name]["per_layer_tune_auroc"].items():
            assert r >= 0.99, f"J3 {name}@{ls} auroc {r}"
    print("J3 trajectory bank: OK (both layers separate, weights normalized)")

    # --- J4: tuned lens affine fit ---
    j4_path = os.path.join(tmp, "j4.json")
    run([os.path.join(ROOT, "methods", "j4_tunedlens.py"),
         "--suite", suite, "--layers", "1", "--final-layer", "2",
         "--precomputed", states, "--out", j4_path])
    j4 = json.load(open(j4_path))
    tr = j4["translators"]["1"]
    A = np.array(tr["A"])
    b = np.array(tr["b"])
    # On alpha positives the map should carry e0(layer1) toward e1(layer2).
    text = "a_pos_0"
    st = json.load(open(states))
    mapped = np.array(st[text]["1"]) @ A.T + b
    target = np.array(st[text]["2"])
    assert np.linalg.norm(mapped - target) < 0.2, f"J4 mapping error too high: {np.linalg.norm(mapped - target)}"
    print(f"J4 tuned lens: OK (alpha map residual {np.linalg.norm(mapped - target):.4f})")

    # --- J5: jacobian normalization over fixture gradients ---
    grads = {}
    for text, layers in st.items():
        grads[text] = {ls: (np.array(v) / max(1e-9, np.linalg.norm(v))).tolist() for ls, v in layers.items()}
    grads_path = os.path.join(tmp, "grads.json")
    json.dump(grads, open(grads_path, "w"))
    j5_path = os.path.join(tmp, "j5.json")
    run([os.path.join(ROOT, "methods", "j5_jacobian.py"),
         "--suite", suite, "--layers", "1,2", "--precomputed-grads", grads_path,
         "--out", j5_path])
    j5 = json.load(open(j5_path))
    assert j5["method"] == "jacobian-v0.1"
    for name, c in j5["concepts"].items():
        for ls, v in c["layers"].items():
            assert abs(np.linalg.norm(v) - 1.0) < 1e-9
    print("J5 jacobian bank: OK (unit-normalized causal directions)")

    # --- J6: composition ---
    j6_path = os.path.join(tmp, "j6.json")
    run([os.path.join(ROOT, "methods", "j6_composition.py"),
         "--suite", suite, "--base-bank", j2_path, "--precomputed", states,
         "--layer", "1", "--compose", "alpha_composite:alpha", "--out", j6_path])
    j6 = json.load(open(j6_path))
    comp = j6["compositions"]["alpha_composite"]
    assert comp["folds"], "J6 produced no folds"
    print(f"J6 composition: OK ({len(comp['folds'])} folds)")

    # --- J7: temporal windows ---
    agent_dir = os.path.join(tmp, "agents")
    for turn in range(4):
        snap = {"snapshot_hash": f"h{turn}", "detections": [
            {"concept": "alpha", "jacobian_score": 0.1 * (turn + 1), "token": "t", "position": 0}]}
        sp = os.path.join(tmp, f"snap{turn}.json")
        json.dump(snap, open(sp, "w"))
        run([os.path.join(ROOT, "methods", "j7_windows.py"),
             "--agent", "a1", "--snapshot", sp, "--cache-dir", agent_dir,
             "--rise-threshold", "0.05"])
    rep = json.load(open(os.path.join(agent_dir, "a1", "j7_report.json")))
    assert rep["rising_intent"] and rep["rising_intent"][0]["concept"] == "alpha"
    print(f"J7 windows: OK (rising slope detected: {rep['rising_intent'][0]['per_turn_slope']})")

    # --- J8: steering dry-run ---
    j8_path = os.path.join(tmp, "j8.json")
    run([os.path.join(ROOT, "methods", "j8_steering.py"),
         "--j5-bank", j5_path, "--precomputed", states, "--out", j8_path])
    j8 = json.load(open(j8_path))
    assert "dry-run-only" in j8["mode"]
    assert j8["dry_run_deltas"], "J8 produced no dry-run deltas"
    print("J8 steering: OK (dry-run spec + deltas, nothing applied)")

    # --- J9: cross-agent audit ---
    s1 = os.path.join(tmp, "s1.json")
    s2 = os.path.join(tmp, "s2.json")
    json.dump({"agent_id": "a1", "detections": [{"concept": "alpha", "jacobian_score": 0.8}]}, open(s1, "w"))
    json.dump({"agent_id": "a2", "detections": [{"concept": "alpha", "jacobian_score": 0.7}]}, open(s2, "w"))
    j9_path = os.path.join(tmp, "j9.json")
    run([os.path.join(ROOT, "methods", "j9_audit.py"),
         "--snapshots", s1, s2, "--min-agents", "2", "--out", j9_path])
    j9 = json.load(open(j9_path))
    assert len(j9["jreef_candidates"]) == 1
    assert j9["jreef_candidates"][0]["jreef_key"] == "jreef:concept:alpha"
    print("J9 cross-agent audit: OK (population recurrence -> jreef candidate)")

    # --- Promotion harness ---
    promo_path = os.path.join(tmp, "promo.json")
    run([os.path.join(ROOT, "eval", "auroc.py"),
         "--suite", suite, "--precomputed", states,
         "--bank", j2_path, "--bank", j3_path, "--out", promo_path])
    promo = json.load(open(promo_path))
    for name, v in promo["promotion_verdict"].items():
        assert v["eval_auroc"] >= 0.99, f"promotion eval auroc low: {name} {v}"
    print("Promotion harness: OK (eval-fold AUROC reported per method)")

    print("\nALL J2-J9 FIXTURE TESTS PASSED")


if __name__ == "__main__":
    main()
