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
    ap.add_argument("--snapshot", required=True, help="jlens.thought.v1 JSON")
    ap.add_argument("--agent-id", required=True)
    ap.add_argument("--action-hash", default=None, help="hex64 intent action_hash")
    ap.add_argument("--batch-height", type=int, default=1)
    ap.add_argument("--tier", default="t0-operator",
                    choices=["t0-operator", "t2-tee"])
    ap.add_argument("--dev-gated", action="store_true")
    ap.add_argument("-o", "--out", required=True)
    args = ap.parse_args()

    with open(args.snapshot, "r", encoding="utf-8") as f:
        snap = json.load(f)

    if args.tier not in ("t0-operator", "t2-tee"):
        print("bad tier", file=sys.stderr)
        sys.exit(2)

    record = {
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
    }

    leaf = thought_record_leaf(record)
    leaves = [leaf]
    root = merkle_root(leaves)
    proof = merkle_proof(leaves, 0)

    packet = {
        "schema": "jlens.attest.v1",
        "thought_record": record,
        "leaf_hash": leaf,
        "leaf_index": 0,
        "merkle_root": root,
        "proof": proof,
        "anchor_msg": {
            "anchor_root": {
                "robot_id": "jlens:thoughts",
                "batch_height": args.batch_height,
                "merkle_root": root,
                "cycle_count": len(leaves),
            }
        },
        "verify_msg": {
            "verify_proof": {
                "robot_id": "jlens:thoughts",
                "batch_height": args.batch_height,
                "leaf_hash": leaf,
                "leaf_index": 0,
                "proof": proof,
            }
        },
    }

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(packet, f, indent=2)

    print(f"thought_record leaf: {leaf[:24]}…")
    print(f"merkle root:         {root[:24]}…")
    print(f"packet -> {args.out}")


if __name__ == "__main__":
    main()
