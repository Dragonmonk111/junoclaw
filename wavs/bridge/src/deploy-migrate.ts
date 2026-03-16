#!/usr/bin/env tsx
/**
 * Upload new WASM code and migrate the existing agent-company contract.
 *
 * Usage:
 *   npx tsx src/deploy-migrate.ts <path-to-optimized.wasm> <contract-address>
 *
 * Example:
 *   npx tsx src/deploy-migrate.ts C:\Temp\junoclaw-wasm-target\agent_company_optimized.wasm juno12xayvf6uz0juj4rrm9p62626fjc2r289qz2kyzp9jpxd7d93fggsy7ja06
 */

import { readFileSync } from "fs";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config, validateConfig } from "./config.js";

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 2) {
    console.error(
      "Usage: deploy-migrate <path-to-wasm> <contract-address>"
    );
    process.exit(1);
  }

  validateConfig();

  const [wasmPath, contractAddress] = args;

  // Read WASM file
  console.log(`\n[migrate] Reading WASM from: ${wasmPath}`);
  const wasmCode = readFileSync(wasmPath);
  console.log(`[migrate] WASM size: ${wasmCode.length} bytes`);

  // Connect wallet
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();
  console.log(`[migrate] Signer: ${account.address}`);

  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );

  // Check balance
  const balance = await client.getBalance(account.address, config.denom);
  console.log(`[migrate] Balance: ${balance.amount} ${balance.denom}`);

  if (BigInt(balance.amount) < BigInt(1_000_000)) {
    console.error("[migrate] Insufficient funds for deployment");
    process.exit(1);
  }

  // Step 1: Upload new code
  console.log(`\n[migrate] Step 1: Uploading new code to ${config.chainId}...`);
  const uploadResult = await client.upload(
    account.address,
    wasmCode,
    "auto",
    "JunoClaw agent-company v2 (WAVS attestation support)"
  );
  console.log(`[migrate] New code_id: ${uploadResult.codeId}`);
  console.log(`[migrate] Upload TX: ${uploadResult.transactionHash}`);
  console.log(`[migrate] Gas used: ${uploadResult.gasUsed}`);

  // Step 2: Migrate existing contract
  console.log(
    `\n[migrate] Step 2: Migrating ${contractAddress} from old code → code_id ${uploadResult.codeId}...`
  );
  const migrateResult = await client.migrate(
    account.address,
    contractAddress,
    uploadResult.codeId,
    {}, // empty MigrateMsg — our migrate handler just updates cw2 version
    "auto",
    "Migrate to v2: WAVS attestation + trigger events"
  );
  console.log(`[migrate] Migrate TX: ${migrateResult.transactionHash}`);
  console.log(`[migrate] Gas used: ${migrateResult.gasUsed}`);

  // Step 3: Verify — query contract info
  console.log(`\n[migrate] Step 3: Verifying migration...`);
  const contractInfo = await client.getContract(contractAddress);
  console.log(`[migrate] Contract address: ${contractInfo.address}`);
  console.log(`[migrate] Code ID: ${contractInfo.codeId}`);
  console.log(`[migrate] Admin: ${contractInfo.admin}`);
  console.log(`[migrate] Label: ${contractInfo.label}`);

  if (contractInfo.codeId === uploadResult.codeId) {
    console.log(
      `\n✅ Migration successful! Contract ${contractAddress} now runs code_id ${uploadResult.codeId}`
    );
  } else {
    console.error(
      `\n❌ Migration may have failed — code_id is ${contractInfo.codeId}, expected ${uploadResult.codeId}`
    );
  }
}

main().catch((err) => {
  console.error("\n[migrate] Failed:", err.message || err);
  process.exit(1);
});
