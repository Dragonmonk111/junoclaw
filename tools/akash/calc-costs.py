import urllib.request
import json

# Current wallet balance
ACT_BALANCE = 3.438618  # ACT
AKT_BALANCE = 2.275     # AKT
AKT_PRICE_USD = 0.50    # approximate
BME_COLLATERAL_RATIO = 1.505099317090988111
BLOCK_TIME_SEC = 6
BLOCKS_PER_MIN = 60 / BLOCK_TIME_SEC  # 10
DEPLOYMENT_DEPOSIT_ACT = 5  # 5,000,000 uact
GAS_AKT = 0.1  # approximate gas for all txs

# SDL offer prices (max we'll pay per block)
H100_SDL_PRICE = 50000   # uact/block
H200_SDL_PRICE = 40000   # uact/block

# Market reference: 4x H100 was 17,208 uact/block (Aug 2)
# 8x H100 might bid around 30000-40000 uact/block
# 8x H200 might bid around 30000-50000 uact/block

scenarios = [
    ("8x H100 (Kimi K2.6)", H100_SDL_PRICE, 35000, 40000),
    ("8x H200 (GLM-5.2 FP8)", H200_SDL_PRICE, 30000, 45000),
]

print("=" * 70)
print("AKASH GPU LEASE COST CALCULATOR")
print("=" * 70)
print()
print(f"Current wallet: akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt")
print(f"  Balance: {ACT_BALANCE:.2f} ACT + {AKT_BALANCE:.3f} AKT")
print(f"  AKT price: ~${AKT_PRICE_USD}")
print(f"  BME collateral ratio: {BME_COLLATERAL_RATIO:.4f}")
print(f"  Block time: ~{BLOCK_TIME_SEC}s ({BLOCKS_PER_MIN:.0f} blocks/min)")
print(f"  Deployment deposit: {DEPLOYMENT_DEPOSIT_ACT} ACT (refundable)")
print(f"  Gas estimate: ~{GAS_AKT} AKT")
print()

for name, sdl_price, bid_low, bid_high in scenarios:
    print(f"--- {name} ---")
    print(f"  SDL max offer: {sdl_price:,} uact/block")
    print(f"  Estimated market bid: {bid_low:,}-{bid_high:,} uact/block")
    print()
    
    for label, price in [("Worst case (SDL max)", sdl_price), ("Mid bid estimate", (bid_low+bid_high)//2), ("Low bid estimate", bid_low)]:
        act_per_min = price * BLOCKS_PER_MIN / 1_000_000
        act_per_hour = act_per_min * 60
        
        for hours in [2, 3, 4]:
            lease_act = act_per_hour * hours
            total_act = lease_act + DEPLOYMENT_DEPOSIT_ACT
            act_needed = max(0, total_act - ACT_BALANCE)
            akt_to_mint = act_needed * BME_COLLATERAL_RATIO
            usd_value = akt_to_mint * AKT_PRICE_USD
            
            if hours == 3:
                print(f"  {label}: {hours}h session")
                print(f"    Lease cost: {lease_act:.1f} ACT")
                print(f"    + Deposit: {DEPLOYMENT_DEPOSIT_ACT} ACT")
                print(f"    = Total: {total_act:.1f} ACT")
                print(f"    Current balance: {ACT_BALANCE:.2f} ACT")
                print(f"    ACT to mint: {act_needed:.1f} ACT")
                print(f"    AKT to send: {akt_to_mint:.1f} AKT (~${usd_value:.2f})")
                print()
    
    # Summary for 3hr at SDL max
    lease_3h = sdl_price * BLOCKS_PER_MIN * 60 * 3 / 1_000_000
    total_3h = lease_3h + DEPLOYMENT_DEPOSIT_ACT
    act_need_3h = max(0, total_3h - ACT_BALANCE)
    akt_need_3h = act_need_3h * BME_COLLATERAL_RATIO
    usd_3h = akt_need_3h * AKT_PRICE_USD
    
    print(f"  >>> 3hr WORST CASE SUMMARY <<<")
    print(f"  Need to send: {akt_need_3h:.0f} AKT (~${usd_3h:.2f})")
    print(f"  (This covers deposit + 3hr lease at SDL max price)")
    print()

print("=" * 70)
print("RECOMMENDATION: Send AKT to cover worst case, unused ACT is")
print("recoverable by burning ACT back to AKT (minus spread).")
print()
print("Wallet address for AKT transfer:")
print("  akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt")
print()
print("After receiving AKT, mint ACT via:")
print("  akash tx bme mint-act <uakt> --from akash-jlens --chain-id akashnet-2")
print(f"  (min mint: 10 ACT = {10*BME_COLLATERAL_RATIO:.2f} AKT)")
print("=" * 70)
