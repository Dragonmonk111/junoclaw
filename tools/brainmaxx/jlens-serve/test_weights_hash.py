"""weights_hash must pin weight CONTENTS, not names/sizes.

Run: python test_weights_hash.py
"""

import os
import tempfile

from probe_runtime import compute_weights_hash


def _model_dir(root, name, payload):
    d = os.path.join(root, name)
    os.makedirs(d)
    with open(os.path.join(d, "model.safetensors"), "wb") as f:
        f.write(payload)
    return d


def test_finetune_same_size_changes_hash():
    with tempfile.TemporaryDirectory() as root:
        base = _model_dir(root, "base", b"\x01" * 4096)
        tuned = _model_dir(root, "tuned", b"\x01" * 4095 + b"\x02")
        assert os.path.getsize(os.path.join(base, "model.safetensors")) == \
            os.path.getsize(os.path.join(tuned, "model.safetensors"))
        assert compute_weights_hash(base) != compute_weights_hash(tuned)


def test_deterministic():
    with tempfile.TemporaryDirectory() as root:
        d = _model_dir(root, "m", b"weights")
        assert compute_weights_hash(d) == compute_weights_hash(d)


def test_no_weights_fails_closed():
    with tempfile.TemporaryDirectory() as root:
        os.makedirs(os.path.join(root, "empty"))
        try:
            compute_weights_hash(os.path.join(root, "empty"))
        except ValueError:
            return
        raise AssertionError("expected ValueError for a dir without weights")


if __name__ == "__main__":
    for name, fn in list(globals().items()):
        if name.startswith("test_"):
            fn()
            print(f"ok - {name}")
