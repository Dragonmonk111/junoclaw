#!/usr/bin/env python3
"""JunoClaw Audit Trail Export — export on-chain attestations as CSV/PDF for regulators.

Usage:
    python audit_export.py --robot-id warehouse-bot-01 --from-block 1000 --to-block 5000 --format csv --output audit.csv
    python audit_export.py --robot-id warehouse-bot-01 --format pdf --output audit.pdf
    python audit_export.py --fleet --format csv --output fleet_audit.csv
"""

import argparse
import csv
import json
import sys
from datetime import datetime, timezone
from typing import Optional

# ─── Constants ───

DEFAULT_CHAIN_RPC = "http://localhost:26657"
DEFAULT_LCD_URL = "http://localhost:1317"


def query_tx_events(lcd_url: str, robot_id: str, from_block: int, to_block: int) -> list[dict]:
    """Query on-chain transactions for a robot's attestations."""
    import urllib.request

    events = []
    # Query wasm events for proof verification
    url = (
        f"{lcd_url}/cosmos/tx/v1beta1/txs?"
        f"events=wasm.verify_proof.robot_id='{robot_id}'"
        f"&page=1&limit=100"
    )

    try:
        req = urllib.request.Request(url)
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = json.loads(resp.read())
            for tx in data.get("txs", []):
                events.append(parse_tx(tx, robot_id))
    except Exception as e:
        print(f"Warning: query failed: {e}", file=sys.stderr)

    return events


def parse_tx(tx: dict, robot_id: str) -> dict:
    """Parse a transaction into an audit record."""
    return {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "robot_id": robot_id,
        "tx_hash": tx.get("txhash", ""),
        "block_height": tx.get("height", ""),
        "gas_used": tx.get("gas_used", ""),
        "proof_valid": True,  # would parse from logs
        "circuit_breaker_state": "closed",
        "envelope_version": 1,
        "merkle_root": "",
        "cycle_count": 0,
    }


def generate_sample_records(robot_id: str, count: int = 50) -> list[dict]:
    """Generate sample audit records for demonstration."""
    records = []
    base_time = datetime(2026, 8, 19, 0, 0, 0, tzinfo=timezone.utc)

    for i in range(count):
        ts = base_time.replace(hour=i % 24, minute=(i * 5) % 60)
        violated = i == 42  # one violation in the batch

        records.append({
            "timestamp": ts.isoformat(),
            "robot_id": robot_id,
            "tx_hash": f"0x{hash(f'{robot_id}-{i}') & 0xFFFFFFFFFFFFFFFF:016x}",
            "block_height": 1000000 + i * 100,
            "gas_used": 203000 if not violated else 203000 + 60000,
            "proof_valid": not violated,
            "circuit_breaker_state": "tripped" if violated else "closed",
            "envelope_version": 1,
            "merkle_root": f"0x{hash(f'merkle-{i}') & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF:064x}",
            "cycle_count": 1000,
            "violated_invariants": "max_speed,min_collision_distance" if violated else "",
        })

    return records


def export_csv(records: list[dict], output: str):
    """Export audit records as CSV."""
    if not records:
        print("No records to export", file=sys.stderr)
        return

    fields = [
        "timestamp", "robot_id", "tx_hash", "block_height", "gas_used",
        "proof_valid", "circuit_breaker_state", "envelope_version",
        "merkle_root", "cycle_count", "violated_invariants",
    ]

    with open(output, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=fields)
        writer.writeheader()
        writer.writerows(records)

    print(f"CSV exported: {output} ({len(records)} records)")


def export_pdf(records: list[dict], output: str, robot_id: str):
    """Export audit records as a simple PDF (text-based)."""
    # Generate a text-based PDF without external dependencies
    lines = []
    lines.append("JunoClaw Compliance Audit Trail")
    lines.append(f"Robot: {robot_id}")
    lines.append(f"Generated: {datetime.now(timezone.utc).isoformat()}")
    lines.append(f"Records: {len(records)}")
    lines.append("=" * 80)
    lines.append("")

    violations = [r for r in records if not r["proof_valid"]]
    operational = [r for r in records if r["proof_valid"]]

    lines.append(f"Summary:")
    lines.append(f"  Total attestations: {len(records)}")
    lines.append(f"  Valid proofs: {len(operational)}")
    lines.append(f"  Violations: {len(violations)}")
    lines.append(f"  Circuit breaker trips: {len(violations)}")
    lines.append(f"  Total gas consumed: {sum(int(r['gas_used']) for r in records):,}")
    lines.append("")

    if violations:
        lines.append("Safety Violations:")
        for v in violations:
            lines.append(f"  [{v['timestamp']}] Block {v['block_height']}")
            lines.append(f"    Violated: {v['violated_invariants']}")
            lines.append(f"    Breaker: {v['circuit_breaker_state']}")
            lines.append("")

    lines.append("Attestation Log:")
    for r in records:
        status = "OK" if r["proof_valid"] else "VIOLATION"
        lines.append(
            f"  [{r['timestamp']}] Block {r['block_height']} | "
            f"Gas {r['gas_used']:,} | {status} | "
            f"Breaker: {r['circuit_breaker_state']}"
        )

    # Write as text file with .pdf extension placeholder
    # In production, use reportlab or weasyprint for real PDF
    text_output = output.replace(".pdf", ".txt")
    with open(text_output, "w") as f:
        f.write("\n".join(lines))

    print(f"PDF (text format) exported: {text_output} ({len(records)} records)")
    print("Note: For production PDF, install reportlab: pip install reportlab")


def export_json(records: list[dict], output: str):
    """Export audit records as JSON."""
    with open(output, "w") as f:
        json.dump(records, f, indent=2)
    print(f"JSON exported: {output} ({len(records)} records)")


def main():
    parser = argparse.ArgumentParser(
        description="JunoClaw audit trail export — export on-chain attestations for regulators"
    )
    parser.add_argument(
        "--robot-id",
        default="warehouse-bot-01",
        help="Robot ID to export audit trail for",
    )
    parser.add_argument(
        "--fleet",
        action="store_true",
        help="Export for entire fleet (all robots)",
    )
    parser.add_argument(
        "--from-block",
        type=int,
        default=0,
        help="Starting block height",
    )
    parser.add_argument(
        "--to-block",
        type=int,
        default=0,
        help="Ending block height (0 = latest)",
    )
    parser.add_argument(
        "--format",
        choices=["csv", "pdf", "json"],
        default="csv",
        help="Output format (default: csv)",
    )
    parser.add_argument(
        "--output",
        default="audit_export",
        help="Output file path (extension added automatically)",
    )
    parser.add_argument(
        "--lcd-url",
        default=DEFAULT_LCD_URL,
        help="LCD endpoint URL",
    )
    parser.add_argument(
        "--sample",
        action="store_true",
        help="Generate sample data (no chain connection needed)",
    )
    parser.add_argument(
        "--sample-count",
        type=int,
        default=50,
        help="Number of sample records (with --sample)",
    )

    args = parser.parse_args()

    # Fetch records
    if args.sample:
        print(f"Generating {args.sample_count} sample audit records for {args.robot_id}...")
        records = generate_sample_records(args.robot_id, args.sample_count)
    else:
        print(f"Querying on-chain attestations for {args.robot_id}...")
        records = query_tx_events(args.lcd_url, args.robot_id, args.from_block, args.to_block)
        if not records:
            print("No on-chain records found. Use --sample for demo data.")
            return

    # Determine output path
    ext = {"csv": ".csv", "pdf": ".pdf", "json": ".json"}[args.format]
    output = args.output if args.output.endswith(ext) else args.output + ext

    # Export
    if args.format == "csv":
        export_csv(records, output)
    elif args.format == "pdf":
        export_pdf(records, output, args.robot_id)
    elif args.format == "json":
        export_json(records, output)

    print(f"\nAudit trail export complete.")
    print(f"  Robot: {args.robot_id}")
    print(f"  Records: {len(records)}")
    print(f"  Format: {args.format.upper()}")
    print(f"  Output: {output}")


if __name__ == "__main__":
    main()
