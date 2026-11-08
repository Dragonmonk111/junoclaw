#!/usr/bin/env python3
"""jlens-attest-packet — build the on-chain attestation packet.

Turns a verified jlens.thought.v1 snapshot into the on-chain anchoring
material: a ThoughtRecord leaf hash (byte-identical to Rust
`thought::ThoughtRecord::leaf_hash`), a merkle root + inclusion proof
(same rule as the merkle-verifier contract), and the ready-to-broadcast
AnchorRoot execute message.

The TS driver (mcp/src/jlens-attest.ts) takes this packet and runs
upload -> instantiate -> anchor_root -> verify_proof on junoclaw-g1.

Usage:
  python attest_packet.py --snapshot snap.json \
      --agent-id jlens:demo --action-hash <hex64> \
      --batch-height 1 --tier t0-operator [--dev-gated] \
      -o packet.json

If --action-hash is omitted the packet binds action_hash = snapshot's
prompt_hash (demo intent: "generate a response to this prompt"). For a
real gated action, pass the IntentMessage's action_hash explicitly.
"""

import argparse
import json
import sys

from thought import merkle_proof, merkle_root, sha256_hex


def thought_record_leaf(record: dict) -> str:
    """sha256 over canonical compact JSON — Rust leaf_hash identical.

    Keys sorted; integrity carried as integrity_micro = round(i * 1e6)
    (cross-language determinism — no float formatting variance).
    """
    v = {
        "action_hash": record["action_hash"],
        "activation_root": record["activation_root"],
        "agent_id": record["agent_id"],
        "bank_hash": record["bank_hash"],
        "batch_height": record["batch_height"],
        "dev_gated": record["dev_gated"],
        "gate": record["gate"],
        "integrity_micro": int(round(record["integrity"] * 1_000_000)),
        "snapshot_hash": record["snapshot_hash"],
        "tier": record["tier"],
    }
    s = json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return sha256_hex(s.encode("utf-8"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--snapshot", required=True, action="append",
                    help="jlens.thought.v1 JSON (repeat for a multi-thought batch)")
    ap.add_argument("--agent-id", required=True)
    ap.add_argument("--action-hash", default=None,
                    help="hex64 intent action_hash (single-snapshot batches only)")
    ap.add_argument("--batch-height", type=int, default=1)
    ap.add_argument("--tier", default="t0-operator",
                    choices=["t0-operator", "t2-tee"])
    ap.add_argument("--dev-gated", action="store_true")
    ap.add_argument("-o", "--out", required=True)
    args = ap.parse_args()

    if args.action_hash and len(args.snapshot) > 1:
        print("--action-hash is per-thought; omit it for multi-snapshot batches",
              file=sys.stderr)
        sys.exit(2)

    records = []
    for path in args.snapshot:
        with open(path, "r", encoding="utf-8") as f:
            snap = json.load(f)
        records.append({
            "agent_id": args.agent_id,
            "action_hash": args.action_hash or snap["prompt_hash"],
            "snapshot_hash": snap["snapshot_hash"],
            "bank_hash": snap["bank_hash"],
            "activation_root": snap["activation_root"],
            "integrity": snap["integrity"],
            "gate": snap["gate"],
            "tier": args.tier,
            "dev_gated": bool(args.dev_gated),
            "batch_height": args.batch_height,
        })

    leaves = [thought_record_leaf(r) for r in records]
    if len(set(leaves)) != len(leaves):
        print("duplicate thought records in batch", file=sys.stderr)
        sys.exit(2)
    root = merkle_root(leaves)

    entries = []
    for i, (record, leaf) in enumerate(zip(records, leaves)):
        proof = merkle_proof(leaves, i)
        entries.append({
            "thought_record": record,
            "leaf_hash": leaf,
            "leaf_index": i,
            "proof": proof,
            "verify_msg": {
                "verify_proof": {
                    "robot_id": "jlens:thoughts",
                    "batch_height": args.batch_height,
                    "leaf_hash": leaf,
                    "leaf_index": i,
                    "proof": proof,
                }
            },
        })

    packet = {
        "schema": "jlens.attest.v2",
        "merkle_root": root,
        "entries": entries,
        "anchor_msg": {
            "anchor_root": {
                "robot_id": "jlens:thoughts",
                "batch_height": args.batch_height,
                "merkle_root": root,
                "cycle_count": len(leaves),
            }
        },
    }

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(packet, f, indent=2)

    for e in entries:
        print(f"leaf[{e['leaf_index']}] {e['leaf_hash'][:24]}…  gate={e['thought_record']['gate']}")
    print(f"merkle root: {root[:24]}…  ({len(leaves)} thoughts)")
    print(f"packet -> {args.out}")


if __name__ == "__main__":
    main()
