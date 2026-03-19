#!/usr/bin/env tsx
/**
 * Upload agent-company v3 (with CodeUpgrade proposal kind) and migrate
 * the live contract on uni-7.
 *
 * Usage:
 *   npx tsx src/migrate-v3.ts
 */

import { readFileSync, writeFileSync, existsSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config, validateConfig } from "./config.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

const WASM_PATH = "C:\\Temp\\junoclaw-contracts-target\\agent_company_v3b.wasm";
const LIVE_CONTRACT = "juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6";

async function main() {
  validateConfig();

  if (!existsSync(WASM_PATH)) {
    console.error(`WASM not found: ${WASM_PATH}`);
    process.exit(1);
  }

  // Connect wallet
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();
  console.log(`\n[migrate] Deployer (Genesis): ${account.address}`);

  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );

  const balance = await client.getBalance(account.address, config.denom);
  console.log(`[migrate] Balance: ${(Number(balance.amount) / 1_000_000).toFixed(2)} JUNOX`);

  // Verify we are the wasmd admin of the live contract
  const contractInfo = await client.getContract(LIVE_CONTRACT);
  console.log(`[migrate] Live contract admin: ${contractInfo.admin}`);
  if (contractInfo.admin !== account.address) {
    console.error(`[migrate] ERROR: We are not the wasmd admin of ${LIVE_CONTRACT}`);
    console.error(`[migrate] Admin is: ${contractInfo.admin}, we are: ${account.address}`);
    process.exit(1);
  }

  // Step 1: Upload new code
  console.log(`\n[migrate] Step 1: Uploading agent-company v3...`);
  const wasmCode = readFileSync(WASM_PATH);
  console.log(`[migrate] WASM size: ${(wasmCode.length / 1024).toFixed(0)}KB`);

  const uploadResult = await client.upload(
    account.address,
    wasmCode,
    "auto",
    "JunoClaw agent-company v3 (CodeUpgrade + Junoswap + supermajority)"
  );
  console.log(`[migrate] New code ID: ${uploadResult.codeId}`);
  console.log(`[migrate] Upload TX: ${uploadResult.transactionHash}`);
  console.log(`[migrate] Gas used: ${uploadResult.gasUsed}`);

  // Step 2: Migrate
  console.log(`\n[migrate] Step 2: Migrating ${LIVE_CONTRACT}...`);
  const migrateMsg = {}; // MigrateMsg is empty

  const migrateResult = await client.migrate(
    account.address,
    LIVE_CONTRACT,
    uploadResult.codeId,
    migrateMsg,
    "auto",
    "Migrate to v3: CodeUpgrade proposal kind, supermajority quorum, Junoswap wiring"
  );
  console.log(`[migrate] Migrate TX: ${migrateResult.transactionHash}`);
  console.log(`[migrate] Gas used: ${migrateResult.gasUsed}`);

  // Step 3: Verify
  console.log(`\n[migrate] Step 3: Verifying...`);
  const newInfo = await client.getContract(LIVE_CONTRACT);
  console.log(`[migrate] Code ID: ${newInfo.codeId} (was ${contractInfo.codeId})`);
  console.log(`[migrate] Admin: ${newInfo.admin}`);

  const cfg = await client.queryContractSmart(LIVE_CONTRACT, { get_config: {} });
  console.log(`[migrate] Config name: ${cfg.name}`);
  console.log(`[migrate] Supermajority quorum: ${cfg.supermajority_quorum_percent ?? 'not set (will default on next instantiate)'}`);
  console.log(`[migrate] DEX factory: ${cfg.dex_factory ?? 'not set (will be wired via CodeUpgrade proposal)'}`);
  console.log(`[migrate] Members: ${cfg.members.length}`);

  // Step 4: Update deployed.json
  const deployedPath = resolve(__dirname, "../../../deploy/deployed.json");
  try {
    const deployed = JSON.parse(readFileSync(deployedPath, "utf-8"));
    deployed["agent-company-v3"] = {
      code_id: uploadResult.codeId,
      store_tx: uploadResult.transactionHash,
      address: LIVE_CONTRACT,
      migrate_tx: migrateResult.transactionHash,
      wasmd_admin: account.address,
      note: "v3: CodeUpgrade proposal kind, supermajority quorum, Junoswap + Akash integration",
    };
    writeFileSync(deployedPath, JSON.stringify(deployed, null, 2));
    console.log(`[migrate] Updated deployed.json`);
  } catch {
    console.log(`[migrate] Could not update deployed.json`);
  }

  console.log(`\n╔══════════════════════════════════════════════════════════════════════╗`);
  console.log(`║  ✅ MIGRATION TO V3 SUCCESSFUL                                       ║`);
  console.log(`╠══════════════════════════════════════════════════════════════════════╣`);
  console.log(`║  Contract:     ${LIVE_CONTRACT}`);
  console.log(`║  Old Code ID:  ${contractInfo.codeId}`);
  console.log(`║  New Code ID:  ${uploadResult.codeId}`);
  console.log(`║  Admin:        ${account.address}`);
  console.log(`║  Chain:        ${config.chainId}`);
  console.log(`║  New features: CodeUpgrade proposals, 67% supermajority, SetDexFactory`);
  console.log(`╚══════════════════════════════════════════════════════════════════════╝`);

  console.log(`\n[migrate] Next: Submit CodeUpgrade proposal to wire Junoswap factory`);
  console.log(`[migrate] Factory: juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh`);
}

main().catch((err) => {
  console.error("\n[migrate] Failed:", err.message || err);
  process.exit(1);
});
