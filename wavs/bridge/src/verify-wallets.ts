#!/usr/bin/env tsx
/**
 * Verify wallet addresses derived from the deploy mnemonic.
 * 
 * This script helps confirm:
 * 1. Neo wallet (juno1...) - deployer, will be destroyed
 * 2. Mother wallet (juno1...) - treasury, must be preserved
 * 3. Akash wallet (akash1...) - compute lease, inherited by Dimi
 * 
 * CRITICAL: If Mother wallet is derived from the same mnemonic as Neo,
 * destroying the Neo mnemonic will also destroy access to Mother.
 * 
 * Safe practice: Mother should have a SEPARATE mnemonic.
 */

import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import dotenv from "dotenv";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
dotenv.config({ path: resolve(__dirname, "../../.env") });

const EXPECTED_NEO = "juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m";
const EXPECTED_MOTHER = "juno1scpm8wukdq52lqs2g9d9ulcza4yeyy5qxct7g2";
const EXPECTED_AKASH = "akash1tvpe72amnd3arnh4nhlf3hztx5aqznu6vt64ta";

async function main() {
  const mnemonic = process.env.WAVS_OPERATOR_MNEMONIC;
  if (!mnemonic) {
    console.error("[verify] ERROR: WAVS_OPERATOR_MNEMONIC not set in wavs/.env");
    process.exit(1);
  }

  console.log("\n[verify] Deriving addresses from deploy mnemonic...\n");

  // Derive Juno address (Neo)
  const junoWallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: "juno" });
  const [junoAccount] = await junoWallet.getAccounts();
  
  // Derive Akash address
  const akashWallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: "akash" });
  const [akashAccount] = await akashWallet.getAccounts();

  const neoMatch = junoAccount.address === EXPECTED_NEO;
  const motherMatch = junoAccount.address === EXPECTED_MOTHER;

  console.log(`Neo (deployer):     ${junoAccount.address}`);
  console.log(`  Expected:         ${EXPECTED_NEO}`);
  console.log(`  Match:            ${neoMatch ? "✅ YES" : "❌ NO"}\n`);

  console.log(`Mother (treasury):  ${EXPECTED_MOTHER}`);
  console.log(`  Derived from Neo: ${motherMatch ? "⚠️  YES — SAME MNEMONIC" : "✅ NO — SEPARATE MNEMONIC"}\n`);

  console.log(`Akash (compute):    ${akashAccount.address}`);
  console.log(`  Expected:         ${EXPECTED_AKASH}`);
  console.log(`  Match:            ${akashAccount.address === EXPECTED_AKASH ? "✅ YES" : "❌ NO"}\n`);

  if (motherMatch) {
    console.log("╔═══════════════════════════════════════════════════════════════╗");
    console.log("║  ⚠️  CRITICAL: Mother wallet uses the SAME mnemonic as Neo    ║");
    console.log("║                                                               ║");
    console.log("║  Destroying the Neo mnemonic will also destroy Mother access ║");
    console.log("║                                                               ║");
    console.log("║  SAFE PRACTICE:                                               ║");
    console.log("║  1. Create a NEW mnemonic for Mother wallet                   ║");
    console.log("║  2. Transfer all funds: Neo Mother → New Mother               ║");
    console.log("║  3. Update records with new Mother address                    ║");
    console.log("║  4. THEN destroy Neo mnemonic                                 ║");
    console.log("╚═══════════════════════════════════════════════════════════════╝\n");
    process.exit(1);
  } else {
    console.log("✅ SAFE: Mother wallet has a separate mnemonic.");
    console.log("   Destroying Neo mnemonic will NOT affect Mother access.\n");
  }
}

main().catch((err) => {
  console.error("\n[verify] Fatal:", err.message || err);
  process.exit(1);
});
