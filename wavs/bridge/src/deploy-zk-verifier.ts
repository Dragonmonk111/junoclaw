#!/usr/bin/env tsx
/**
 * Deploy zk-verifier contract to uni-7 and measure Groth16 verification gas cost.
 *
 * Usage:
 *   npx tsx src/deploy-zk-verifier.ts            # full deploy
 *   npx tsx src/deploy-zk-verifier.ts --dry-run   # offline validation only
 *
 * Requires WAVS_OPERATOR_MNEMONIC in .env (not needed for --dry-run)
 * Wasm file: ../../contracts/zk-verifier (built + optimized)
 */

import { readFileSync } from "fs";
import { tmpdir } from "os";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { SigningCosmWasmClient, CosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { GasPrice } from "@cosmjs/stargate";
import { config } from "./config.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

// ── Generate test proof data offline (pre-computed from arkworks) ──
// We'll use a simpler approach: generate the VK, proof, and public inputs
// in a helper Rust binary, then paste them here as base64.
// For now, we use the contract's own test to validate, and focus on
// upload + instantiate + gas measurement.

const DRY_RUN = process.argv.includes("--dry-run");

async function main() {
  if (DRY_RUN) {
    console.log("═══ DRY RUN MODE ═══");
    console.log("Validating proof data and WASM without chain interaction.\n");
  }

  if (!DRY_RUN && !config.mnemonic) {
    throw new Error("WAVS_OPERATOR_MNEMONIC not set in .env (use --dry-run to skip chain)");
  }

  // ── Validate WASM file ──
  const wasmPath = process.env.ZK_WASM_PATH || resolve(__dirname, "../../../contracts/zk-verifier/zk_verifier_optimized.wasm");
  let wasmBytes: Uint8Array;
  try {
    wasmBytes = readFileSync(wasmPath);
  } catch {
    console.error(`Missing WASM: ${wasmPath}`);
    console.error("Build it: cargo +stable build -p zk-verifier --target wasm32-unknown-unknown --release");
    console.error("Then:     wasm-opt -Oz ... -o zk_verifier_optimized.wasm");
    process.exit(1);
  }
  console.log(`WASM size: ${wasmBytes.length} bytes (${(wasmBytes.length / 1024).toFixed(1)} KB)`);

  // ── Validate proof data ──
  const proofJsonPath = process.env.ZK_PROOF_PATH || resolve(tmpdir(), "groth16_proof.json");
  let proofData: { vk_base64: string; proof_base64: string; public_inputs_base64: string };
  try {
    proofData = JSON.parse(readFileSync(proofJsonPath, "utf-8"));
  } catch {
    console.error(`Missing ${proofJsonPath} — run: cargo +stable run -p zk-verifier --example generate_proof`);
    process.exit(1);
  }
  console.log(`Proof data: VK=${proofData.vk_base64.length} chars, Proof=${proofData.proof_base64.length} chars, Inputs=${proofData.public_inputs_base64.length} chars`);

  if (DRY_RUN) {
    console.log("\n════════════════════════════════════════════════════");
    console.log("  DRY RUN VALIDATION PASSED");
    console.log("════════════════════════════════════════════════════");
    console.log(`  WASM:   ${wasmBytes.length} bytes ✓`);
    console.log(`  VK:     ${proofData.vk_base64.length} base64 chars ✓`);
    console.log(`  Proof:  ${proofData.proof_base64.length} base64 chars ✓`);
    console.log(`  Inputs: ${proofData.public_inputs_base64.length} base64 chars ✓`);
    console.log("════════════════════════════════════════════════════");
    console.log("\nTo deploy for real: remove --dry-run and set WAVS_OPERATOR_MNEMONIC in wavs/.env");
    return;
  }

  // ── Chain interaction (requires mnemonic) ──
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();
  console.log(`\nDeployer: ${account.address}`);

  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString("0.075ujunox") }
  );

  const balance = await client.getBalance(account.address, config.denom);
  console.log(`Balance: ${balance.amount} ${balance.denom}`);
  if (BigInt(balance.amount) === 0n) {
    throw new Error(
      `Wallet ${account.address} has 0 ${config.denom}. Fund this wallet on ${config.chainId} before deployment.`
    );
  }

  // ── Step 1: Upload WASM ──
  console.log("\n── Step 1: Upload WASM ──");
  const uploadResult = await client.upload(account.address, wasmBytes, "auto");
  console.log(`Code ID: ${uploadResult.codeId}`);
  console.log(`Upload TX: ${uploadResult.transactionHash}`);
  console.log(`Upload gas used: ${uploadResult.gasUsed}`);

  // ── Step 2: Instantiate ──
  console.log("\n── Step 2: Instantiate ──");
  const instantiateResult = await client.instantiate(
    account.address,
    uploadResult.codeId,
    { admin: null },
    "junoclaw-zk-verifier-poc",
    "auto",
    { admin: account.address }
  );
  console.log(`Contract: ${instantiateResult.contractAddress}`);
  console.log(`Instantiate TX: ${instantiateResult.transactionHash}`);
  console.log(`Instantiate gas used: ${instantiateResult.gasUsed}`);

  // ── Step 3: Store VK ──
  console.log("\n── Step 3: Store VK ──");
  const storeVkResult = await client.execute(
    account.address,
    instantiateResult.contractAddress,
    { store_vk: { vk_base64: proofData.vk_base64 } },
    "auto"
  );
  console.log(`StoreVk TX: ${storeVkResult.transactionHash}`);
  console.log(`StoreVk gas used: ${storeVkResult.gasUsed}`);

  // ── Step 4: Verify proof (THE KEY MEASUREMENT) ──
  console.log("\n── Step 4: Verify Groth16 proof (BN254 pairing) ──");
  const verifyResult = await client.execute(
    account.address,
    instantiateResult.contractAddress,
    {
      verify_proof: {
        proof_base64: proofData.proof_base64,
        public_inputs_base64: proofData.public_inputs_base64,
      },
    },
    "auto"
  );
  console.log(`VerifyProof TX: ${verifyResult.transactionHash}`);
  console.log(`VerifyProof gas used: ${verifyResult.gasUsed}`);

  // ── Step 5: Query status ──
  const queryClient = await CosmWasmClient.connect(config.rpcEndpoint);
  const vkStatus = await queryClient.queryContractSmart(
    instantiateResult.contractAddress,
    { vk_status: {} }
  );
  const lastVerify = await queryClient.queryContractSmart(
    instantiateResult.contractAddress,
    { last_verify: {} }
  );
  console.log("\nVK Status:", JSON.stringify(vkStatus));
  console.log("Last Verify:", JSON.stringify(lastVerify));

  // ── Summary ──
  console.log("\n════════════════════════════════════════════════════");
  console.log("  ZK-VERIFIER GAS MEASUREMENT RESULTS");
  console.log("════════════════════════════════════════════════════");
  console.log(`  Code ID:          ${uploadResult.codeId}`);
  console.log(`  Contract:         ${instantiateResult.contractAddress}`);
  console.log(`  Upload gas:       ${uploadResult.gasUsed}`);
  console.log(`  Instantiate gas:  ${instantiateResult.gasUsed}`);
  console.log(`  StoreVk gas:      ${storeVkResult.gasUsed}`);
  console.log(`  VerifyProof gas:  ${verifyResult.gasUsed}  ← BN254 PAIRING COST`);
  console.log("════════════════════════════════════════════════════");
  console.log("\nComparison:");
  console.log(`  Hash storage (current):     ~200,000 gas`);
  console.log(`  With BN254 precompile:      ~187,000 gas`);
  console.log(`  Pure CosmWasm (this test):  ${verifyResult.gasUsed} gas`);
  const ratio = (parseInt(String(verifyResult.gasUsed)) / 187000).toFixed(1);
  console.log(`  Ratio (pure/precompile):    ${ratio}x more expensive`);
  console.log("════════════════════════════════════════════════════");
}

main().catch((err) => {
  console.error("Error:", err);
  process.exit(1);
});
