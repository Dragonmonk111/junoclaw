#!/usr/bin/env python3
"""JunoClaw Cost Model Calculator — gas costs per robot per day at various fleet scales.

Usage:
    python cost_calculator.py --robots 1 10 100 1000
    python cost_calculator.py --robots 100 --batches-per-day 288 --gas-price 0.025
"""

import argparse
import sys

# ─── Constants ───

# Gas costs (measured on devnet + mainnet)
GAS_VERIFY_PROOF_PRECOMPILE = 203_000   # with BN254 precompiles
GAS_VERIFY_PROOF_PURE_WASM = 371_000    # pure CosmWasm (no precompiles)
GAS_TRIP_BREAKER = 60_000
GAS_SET_ENVELOPE = 100_000
GAS_ANCHOR_ROOT = 80_000
GAS_VERIFY_ATTESTATION = 50_000

# Juno block time
BLOCK_TIME_SECS = 2.8
BLOCKS_PER_DAY = int(86400 / BLOCK_TIME_SECS)  # ~30,857

# Default assumptions
DEFAULT_BATCHES_PER_DAY = 288  # one batch every 5 minutes
DEFAULT_GAS_PRICE_UJUNO = 0.025  # ujuno per gas unit
DEFAULT_JUNO_PRICE_USD = 0.25   # USD per JUNO


def calculate_costs(
    num_robots: int,
    batches_per_day: int = DEFAULT_BATCHES_PER_DAY,
    gas_price: float = DEFAULT_GAS_PRICE_UJUNO,
    juno_price: float = DEFAULT_JUNO_PRICE_USD,
    use_precompiles: bool = True,
    include_tee: bool = True,
) -> dict:
    """Calculate gas costs for a fleet of robots."""

    verify_gas = GAS_VERIFY_PROOF_PRECOMPILE if use_precompiles else GAS_VERIFY_PROOF_PURE_WASM
    total_verify_gas = verify_gas + (GAS_VERIFY_ATTESTATION if include_tee else 0)

    # Per robot per day
    gas_per_robot_per_day = total_verify_gas * batches_per_day
    # One-time setup costs (envelope + first root anchor)
    setup_gas = GAS_SET_ENVELOPE + GAS_ANCHOR_ROOT

    # Fleet totals
    total_daily_gas = gas_per_robot_per_day * num_robots
    total_setup_gas = setup_gas * num_robots
    total_monthly_gas = total_daily_gas * 30

    # Convert to ujuno and USD
    daily_cost_ujuno = total_daily_gas * gas_price
    daily_cost_usd = daily_cost_ujuno * juno_price / 1_000_000
    monthly_cost_ujuno = total_monthly_gas * gas_price
    monthly_cost_usd = monthly_cost_ujuno * juno_price / 1_000_000

    return {
        "num_robots": num_robots,
        "batches_per_day": batches_per_day,
        "use_precompiles": use_precompiles,
        "include_tee": include_tee,
        # Per robot
        "gas_per_robot_per_day": gas_per_robot_per_day,
        "setup_gas_per_robot": setup_gas,
        "daily_cost_per_robot_ujuno": gas_per_robot_per_day * gas_price,
        "daily_cost_per_robot_usd": gas_per_robot_per_day * gas_price * juno_price / 1_000_000,
        # Fleet
        "total_daily_gas": total_daily_gas,
        "total_monthly_gas": total_monthly_gas,
        "daily_cost_ujuno": daily_cost_ujuno,
        "daily_cost_usd": daily_cost_usd,
        "monthly_cost_ujuno": monthly_cost_ujuno,
        "monthly_cost_usd": monthly_cost_usd,
        "setup_cost_ujuno": total_setup_gas * gas_price,
        "setup_cost_usd": total_setup_gas * gas_price * juno_price / 1_000_000,
    }


def print_report(robots: list[int], batches_per_day: int, gas_price: float, juno_price: float):
    """Print a cost report for multiple fleet sizes."""

    print("=" * 80)
    print("JunoClaw Cost Model — Gas Costs per Robot per Day")
    print("=" * 80)
    print()
    print(f"Assumptions:")
    print(f"  Batches per day:     {batches_per_day} (one every {86400/batches_per_day/60:.0f} min)")
    print(f"  Gas price:           {gas_price} ujuno/gas")
    print(f"  JUNO price:          ${juno_price}")
    print(f"  Block time:          {BLOCK_TIME_SECS}s ({BLOCKS_PER_DAY} blocks/day)")
    print(f"  Verification:        BN254 precompiles ({GAS_VERIFY_PROOF_PRECOMPILE:,} gas)")
    print(f"  TEE attestation:     {GAS_VERIFY_ATTESTATION:,} gas")
    print()

    # Header
    print(f"{'Robots':>8} | {'Daily Gas':>14} | {'Daily Cost':>12} | {'Monthly Cost':>14} | {'Cost/Robot/Day':>14}")
    print(f"{'':>8} | {'':>14} | {'(USD)':>12} | {'(USD)':>14} | {'(USD)':>14}")
    print("-" * 80)

    for n in robots:
        r = calculate_costs(
            num_robots=n,
            batches_per_day=batches_per_day,
            gas_price=gas_price,
            juno_price=juno_price,
        )
        print(
            f"{n:>8} | {r['total_daily_gas']:>14,} | "
            f"${r['daily_cost_usd']:>10.2f} | "
            f"${r['monthly_cost_usd']:>12.2f} | "
            f"${r['daily_cost_per_robot_usd']:>12.4f}"
        )

    print()
    print("Breakdown (per robot per batch):")
    print(f"  VerifyProof (precompile):  {GAS_VERIFY_PROOF_PRECOMPILE:>10,} gas")
    print(f"  VerifyProof (pure Wasm):   {GAS_VERIFY_PROOF_PURE_WASM:>10,} gas")
    print(f"  TEE attestation:           {GAS_VERIFY_ATTESTATION:>10,} gas")
    print(f"  Anchor root:               {GAS_ANCHOR_ROOT:>10,} gas")
    print(f"  Trip breaker:              {GAS_TRIP_BREAKER:>10,} gas")
    print(f"  Set envelope (one-time):   {GAS_SET_ENVELOPE:>10,} gas")
    print()

    # Comparison: precompiles vs pure Wasm
    print("Precompile vs Pure Wasm comparison (100 robots, 288 batches/day):")
    r_pre = calculate_costs(100, batches_per_day, gas_price, juno_price, use_precompiles=True)
    r_pure = calculate_costs(100, batches_per_day, gas_price, juno_price, use_precompiles=False)
    print(f"  Precompiles:  ${r_pre['monthly_cost_usd']:>10.2f}/month")
    print(f"  Pure Wasm:    ${r_pure['monthly_cost_usd']:>10.2f}/month")
    print(f"  Savings:      ${r_pure['monthly_cost_usd'] - r_pre['monthly_cost_usd']:>10.2f}/month ({(1 - r_pre['monthly_cost_usd']/r_pure['monthly_cost_usd'])*100:.1f}%)")
    print()


def main():
    parser = argparse.ArgumentParser(
        description="JunoClaw cost model calculator — gas costs per robot per day"
    )
    parser.add_argument(
        "--robots",
        type=int,
        nargs="+",
        default=[1, 10, 100, 1000, 10000],
        help="Fleet sizes to calculate (default: 1 10 100 1000 10000)",
    )
    parser.add_argument(
        "--batches-per-day",
        type=int,
        default=DEFAULT_BATCHES_PER_DAY,
        help=f"Batches per robot per day (default: {DEFAULT_BATCHES_PER_DAY})",
    )
    parser.add_argument(
        "--gas-price",
        type=float,
        default=DEFAULT_GAS_PRICE_UJUNO,
        help=f"Gas price in ujuno (default: {DEFAULT_GAS_PRICE_UJUNO})",
    )
    parser.add_argument(
        "--juno-price",
        type=float,
        default=DEFAULT_JUNO_PRICE_USD,
        help=f"JUNO price in USD (default: ${DEFAULT_JUNO_PRICE_USD})",
    )

    args = parser.parse_args()

    print_report(args.robots, args.batches_per_day, args.gas_price, args.juno_price)


if __name__ == "__main__":
    main()
