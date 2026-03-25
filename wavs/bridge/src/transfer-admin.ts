#!/usr/bin/env tsx
/**
 * Transfer wasmd admin of all JunoClaw contracts to a new address,
 * fund the new admin with gas money, and drain remaining tokens
 * back to the Mother wallet — leaving exactly 13 ujunox on Neo.
 *
 * This is Path B of the Dimi handoff: on-chain admin transfer, no mnemonic handoff.
 * See docs/DIMI_HANDOFF_PLAN.md for full procedure.
 *
 * Usage:
 *   npx tsx src/transfer-admin.ts <new-admin-juno1-address>
 *
 * What this script does (in order):
 *   Phase 1: Verify Neo is admin of all 5 contracts
 *   Phase 2: Transfer wasmd admin of all 5 contracts to Dimi
 *   Phase 3: Send 5 JUNOX to Dimi (gas money for WeightChange + future ops)
 *   Phase 4: Drain remaining JUNOX to Mother wallet, leave 13 ujunox on Neo
 *   Phase 5: Verify all transfers + final balances
 *
 * Prerequisites:
 *   - wavs/.env must contain WAVS_OPERATOR_MNEMONIC (Neo wallet)
 *   - Neo wallet must be the current wasmd admin of all contracts
 *   - Neo wallet must have JUNOX for gas + tokens to drain
 */

import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config } from "./config.js";

// Mother wallet — testnet treasury, receives drained tokens
const MOTHER_WALLET = "juno1scpm8wukdq52lqs2g9d9ulcza4yeyy5qxct7g2";

// How much to leave on Neo forever (symbolic: 13 ujunox = 13 genesis buds)
const NEO_KEEP_UJUNOX = 13n;

// How much to send Dimi for gas (5 JUNOX = 5,000,000 ujunox)
const DIMI_GAS_UJUNOX = 5_000_000n;

// All contracts deployed by Neo wallet on uni-7
const CONTRACTS = [
  {
    name: "agent-company v3",
    address: "juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6",
  },
  {
    name: "junoswap factory",
    address: "juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh",
  },
  {
    name: "escrow",
    address: "juno1dh43lswg5ekv7q2p44s6hgays47k5mz67742vdwpd025p8q05kgs0azwrv",
  },
  {
    name: "agent-registry",
    address: "juno1qulyspwzjzsz7rq65v6ptzt278f9ta9uh0upxu6xa08gf4v5gzaqm676j7",
  },
  {
    name: "task-ledger",
    address: "juno1agw6f05wxx5rm8d3etq7cejcm5g8e224s00dvykylaja7jlx3ljq6f0u46",
  },
];

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    console.error("Usage: transfer-admin <new-admin-juno1-address>");
    console.error("Example: transfer-admin juno1s33zct2zhhaf60x4a90cpe9yquw99jj0zen8pt");
    process.exit(1);
  }

  const newAdmin = args[0];

  // Validate new admin address format
  if (!newAdmin.startsWith("juno1") || newAdmin.length !== 43) {
    console.error(`[transfer] ERROR: Invalid address format: ${newAdmin}`);
    console.error(`[transfer] Expected: juno1... (43 characters)`);
    process.exit(1);
  }

  // Only need mnemonic — don't require AGENT_COMPANY_CONTRACT
  if (!config.mnemonic) {
    console.error("[transfer] ERROR: WAVS_OPERATOR_MNEMONIC not set in wavs/.env");
    process.exit(1);
  }

  // Connect with Neo wallet
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();
  console.log(`\n[transfer] Current admin (Neo): ${account.address}`);
  console.log(`[transfer] New admin (Dimi):    ${newAdmin}`);
  console.log(`[transfer] Chain:               ${config.chainId}`);

  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );

  // Check balance
  const balance = await client.getBalance(account.address, config.denom);
  console.log(`[transfer] Balance: ${(Number(balance.amount) / 1_000_000).toFixed(2)} JUNOX`);

  if (Number(balance.amount) < 100_000) {
    console.error("[transfer] WARNING: Low balance. Need ~0.08 JUNOX for 5 admin transfers.");
  }

  // Safety check: don't transfer to yourself
  if (newAdmin === account.address) {
    console.error("[transfer] ERROR: New admin is the same as current admin. Nothing to do.");
    process.exit(1);
  }

  console.log(`\n[transfer] ═══════════════════════════════════════════════════════`);
  console.log(`[transfer]  PHASE 1: Verify current admin status`);
  console.log(`[transfer] ═══════════════════════════════════════════════════════\n`);

  // Phase 1: Verify we are admin of all contracts
  const results: { name: string; address: string; currentAdmin: string; status: string; tx?: string }[] = [];

  for (const contract of CONTRACTS) {
    try {
      const info = await client.getContract(contract.address);
      const isAdmin = info.admin === account.address;
      console.log(`[transfer] ${contract.name.padEnd(20)} admin: ${info.admin} ${isAdmin ? "✅ WE ARE ADMIN" : "❌ NOT OUR ADMIN"}`);

      if (!isAdmin) {
        results.push({
          name: contract.name,
          address: contract.address,
          currentAdmin: info.admin || "(none)",
          status: "SKIPPED — not admin",
        });
      }
    } catch (err: any) {
      console.error(`[transfer] ${contract.name}: ERROR — ${err.message}`);
      results.push({
        name: contract.name,
        address: contract.address,
        currentAdmin: "ERROR",
        status: `FAILED: ${err.message}`,
      });
    }
  }

  // Filter to contracts we can actually transfer
  const transferable = CONTRACTS.filter((c) => {
    const result = results.find((r) => r.address === c.address);
    return !result; // If not in results, it passed the admin check
  });

  if (transferable.length === 0) {
    console.error("\n[transfer] No contracts to transfer. Are you using the right wallet?");
    process.exit(1);
  }

  console.log(`\n[transfer] ═══════════════════════════════════════════════════════`);
  console.log(`[transfer]  PHASE 2: Transfer admin (${transferable.length} contracts)`);
  console.log(`[transfer] ═══════════════════════════════════════════════════════\n`);

  // Phase 2: Transfer admin for each contract
  for (const contract of transferable) {
    try {
      console.log(`[transfer] Transferring ${contract.name}...`);
      const result = await client.updateAdmin(
        account.address,
        contract.address,
        newAdmin,
        1.5, // 1.5x multiplier on auto gas estimation
        `Transfer admin to Dimi (Path B handoff)`
      );
      console.log(`[transfer]   TX: ${result.transactionHash}`);
      console.log(`[transfer]   Gas: ${result.gasUsed}`);

      results.push({
        name: contract.name,
        address: contract.address,
        currentAdmin: newAdmin,
        status: "✅ TRANSFERRED",
        tx: result.transactionHash,
      });
    } catch (err: any) {
      console.error(`[transfer]   FAILED: ${err.message}`);
      results.push({
        name: contract.name,
        address: contract.address,
        currentAdmin: account.address,
        status: `❌ FAILED: ${err.message}`,
      });
    }
  }

  // ── Phase 3: Fund Dimi with gas money ──────────────────────

  console.log(`\n[transfer] ═══════════════════════════════════════════════════════`);
  console.log(`[transfer]  PHASE 3: Fund Dimi with gas money`);
  console.log(`[transfer] ═══════════════════════════════════════════════════════\n`);

  try {
    const preBalance = await client.getBalance(account.address, config.denom);
    const preBal = BigInt(preBalance.amount);
    console.log(`[transfer] Neo balance: ${(Number(preBal) / 1_000_000).toFixed(2)} JUNOX`);

    if (preBal < DIMI_GAS_UJUNOX + 500_000n) {
      console.log(`[transfer] ⚠️  Not enough to fund Dimi (need ${Number(DIMI_GAS_UJUNOX) / 1_000_000} JUNOX + gas). Skipping.`);
    } else {
      console.log(`[transfer] Sending ${Number(DIMI_GAS_UJUNOX) / 1_000_000} JUNOX to Dimi (${newAdmin})...`);
      const sendResult = await client.sendTokens(
        account.address,
        newAdmin,
        [{ denom: config.denom, amount: DIMI_GAS_UJUNOX.toString() }],
        "auto",
        "JunoClaw: gas money for new admin (Path B handoff)"
      );
      console.log(`[transfer]   TX: ${sendResult.transactionHash}`);
      console.log(`[transfer]   Gas: ${sendResult.gasUsed}`);
      console.log(`[transfer]   ✅ Dimi funded with ${Number(DIMI_GAS_UJUNOX) / 1_000_000} JUNOX`);
    }
  } catch (err: any) {
    console.error(`[transfer] ⚠️  Failed to fund Dimi: ${err.message}`);
    console.error(`[transfer]    (non-fatal — Dimi can get JUNOX from Mother wallet or faucet)`);
  }

  // ── Phase 4: Drain Neo → Mother wallet (leave 13 ujunox) ──

  console.log(`\n[transfer] ═══════════════════════════════════════════════════════`);
  console.log(`[transfer]  PHASE 4: Drain Neo → Mother wallet (leave 13 ujunox)`);
  console.log(`[transfer] ═══════════════════════════════════════════════════════\n`);

  try {
    // Re-check balance after funding Dimi
    const currentBalance = await client.getBalance(account.address, config.denom);
    const currentBal = BigInt(currentBalance.amount);
    console.log(`[transfer] Neo balance: ${(Number(currentBal) / 1_000_000).toFixed(6)} JUNOX (${currentBal} ujunox)`);

    // Reserve: 13 ujunox to keep + ~200,000 ujunox for this TX's gas
    const gasReserve = 200_000n;
    const drainAmount = currentBal - NEO_KEEP_UJUNOX - gasReserve;

    if (drainAmount <= 0n) {
      console.log(`[transfer] ⚠️  Balance too low to drain. Neo has ${currentBal} ujunox.`);
    } else {
      console.log(`[transfer] Draining to Mother wallet: ${MOTHER_WALLET}`);
      console.log(`[transfer] Amount:  ${(Number(drainAmount) / 1_000_000).toFixed(6)} JUNOX (${drainAmount} ujunox)`);
      console.log(`[transfer] Keeping: ${NEO_KEEP_UJUNOX} ujunox on Neo (symbolic: 13 genesis buds)`);

      const drainResult = await client.sendTokens(
        account.address,
        MOTHER_WALLET,
        [{ denom: config.denom, amount: drainAmount.toString() }],
        "auto",
        "JunoClaw: drain Neo wallet to Mother treasury (Path B handoff)"
      );
      console.log(`[transfer]   TX: ${drainResult.transactionHash}`);
      console.log(`[transfer]   Gas: ${drainResult.gasUsed}`);

      // Show final Neo balance (should be ~13 ujunox + leftover gas)
      const finalNeo = await client.getBalance(account.address, config.denom);
      console.log(`[transfer]   ✅ Neo final balance: ${finalNeo.amount} ujunox`);

      // Show Mother balance
      const motherBal = await client.getBalance(MOTHER_WALLET, config.denom);
      console.log(`[transfer]   ✅ Mother balance: ${(Number(motherBal.amount) / 1_000_000).toFixed(2)} JUNOX`);
    }
  } catch (err: any) {
    console.error(`[transfer] ⚠️  Failed to drain Neo: ${err.message}`);
    console.error(`[transfer]    (non-fatal — tokens remain on Neo, can drain manually later)`);
  }

  // ── Phase 5: Verify everything ─────────────────────────────

  console.log(`\n[transfer] ═══════════════════════════════════════════════════════`);
  console.log(`[transfer]  PHASE 5: Final verification`);
  console.log(`[transfer] ═══════════════════════════════════════════════════════\n`);

  // Verify admin transfers
  let allGood = true;
  for (const contract of CONTRACTS) {
    try {
      const info = await client.getContract(contract.address);
      const transferred = info.admin === newAdmin;
      console.log(`[transfer] ${contract.name.padEnd(20)} → ${info.admin} ${transferred ? "✅" : "❌"}`);
      if (!transferred) allGood = false;
    } catch (err: any) {
      console.error(`[transfer] ${contract.name.padEnd(20)} → ERROR: ${err.message}`);
      allGood = false;
    }
  }

  // Final balances
  const finalNeoBalance = await client.getBalance(account.address, config.denom);
  const finalDimiBalance = await client.getBalance(newAdmin, config.denom);
  const finalMotherBalance = await client.getBalance(MOTHER_WALLET, config.denom);

  // Summary
  console.log(`\n╔══════════════════════════════════════════════════════════════════╗`);
  if (allGood) {
    console.log(`║  ✅ PATH B HANDOFF COMPLETE                                      ║`);
  } else {
    console.log(`║  ⚠️  SOME TRANSFERS FAILED — check above for details               ║`);
  }
  console.log(`╠══════════════════════════════════════════════════════════════════╣`);
  console.log(`║  ADMIN TRANSFER                                                  ║`);
  console.log(`║    Old admin (Neo):  ${account.address}     ║`);
  console.log(`║    New admin (Dimi): ${newAdmin}     ║`);
  console.log(`║    Contracts:        ${transferable.length}/5 transferred                            ║`);
  console.log(`║                                                                  ║`);
  console.log(`║  TOKEN DRAIN                                                     ║`);
  console.log(`║    Neo:    ${finalNeoBalance.amount.padEnd(15)} ujunox (symbolic 13)              ║`);
  console.log(`║    Dimi:   ${finalDimiBalance.amount.padEnd(15)} ujunox (gas money)               ║`);
  console.log(`║    Mother: ${finalMotherBalance.amount.padEnd(15)} ujunox (treasury)               ║`);
  console.log(`║                                                                  ║`);
  console.log(`║  Chain: ${config.chainId.padEnd(55)} ║`);
  console.log(`╚══════════════════════════════════════════════════════════════════╝`);

  if (allGood) {
    console.log(`\n[transfer] NEXT STEPS:`);
    console.log(`[transfer]   1. Dimi verifies:  npx tsx src/verify-admin.ts ${newAdmin}`);
    console.log(`[transfer]   2. Dimi submits WeightChange proposal (governance weight)`);
    console.log(`[transfer]   3. Genesis destroys Neo mnemonic:`);
    console.log(`[transfer]      - Delete deploy/.env`);
    console.log(`[transfer]      - Delete any paper/digital backups`);
    console.log(`[transfer]      - Clear shell history`);
    console.log(`[transfer]   4. Neo wallet is now permanently inert.`);
    console.log(`[transfer]      ${finalNeoBalance.amount} ujunox remain as a tombstone — 13 for the 13 buds.`);
  }
}

main().catch((err) => {
  console.error("\n[transfer] Fatal:", err.message || err);
  process.exit(1);
});
