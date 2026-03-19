#!/usr/bin/env tsx
/**
 * Deploy Junoswap v2 contracts (factory + pair) to Juno uni-7.
 *
 * Deploys in order:
 *   1. Upload junoswap_pair WASM → get pair_code_id
 *   2. Upload junoswap_factory WASM → get factory_code_id
 *   3. Instantiate factory with pair_code_id
 *   4. Create first pair (ujunox / uusdc-test) via factory
 *   5. Verify both contracts
 *
 * Usage:
 *   npx tsx src/deploy-junoswap.ts
 *
 * Requires:
 *   - WAVS_OPERATOR_MNEMONIC in .env (deployer wallet with JUNOX)
 *   - Optimized WASM files at C:\Temp\junoclaw-contracts-target\
 */

import { readFileSync, writeFileSync, existsSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config } from "./config.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

// WASM file paths (from cargo build + wasm-opt)
const WASM_DIR = "C:\\Temp\\junoclaw-contracts-target";
const PAIR_WASM = resolve(WASM_DIR, "junoswap_pair_small.wasm");
const FACTORY_WASM = resolve(WASM_DIR, "junoswap_factory_small.wasm");

// Existing JunoClaw contract (for WAVS hook integration)
const AGENT_COMPANY = "juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6";

async function main() {
  // Validate
  if (!config.mnemonic) {
    console.error("WAVS_OPERATOR_MNEMONIC not set in .env");
    process.exit(1);
  }

  if (!existsSync(PAIR_WASM)) {
    console.error(`Pair WASM not found: ${PAIR_WASM}`);
    console.error("Run: cargo build --release --target wasm32-unknown-unknown -p junoswap-pair");
    process.exit(1);
  }
  if (!existsSync(FACTORY_WASM)) {
    console.error(`Factory WASM not found: ${FACTORY_WASM}`);
    console.error("Run: cargo build --release --target wasm32-unknown-unknown -p junoswap-factory");
    process.exit(1);
  }

  // Connect wallet
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();
  console.log(`\n[junoswap] Deployer: ${account.address}`);

  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );

  const balance = await client.getBalance(account.address, config.denom);
  console.log(`[junoswap] Balance: ${(Number(balance.amount) / 1_000_000).toFixed(2)} JUNOX`);

  if (Number(balance.amount) < 5_000_000) {
    console.error("[junoswap] Need at least 5 JUNOX for deployment gas. Aborting.");
    process.exit(1);
  }

  // ── Step 1: Upload pair WASM ──
  console.log(`\n[junoswap] Step 1: Uploading junoswap-pair WASM...`);
  const pairWasm = readFileSync(PAIR_WASM);
  console.log(`[junoswap] Pair WASM size: ${(pairWasm.length / 1024).toFixed(0)}KB`);

  const pairUpload = await client.upload(
    account.address,
    pairWasm,
    "auto",
    "Junoswap v2 Pair (XYK AMM)"
  );
  console.log(`[junoswap] Pair code ID: ${pairUpload.codeId}`);
  console.log(`[junoswap] Pair upload TX: ${pairUpload.transactionHash}`);
  console.log(`[junoswap] Gas used: ${pairUpload.gasUsed}`);

  // ── Step 2: Upload factory WASM ──
  console.log(`\n[junoswap] Step 2: Uploading junoswap-factory WASM...`);
  const factoryWasm = readFileSync(FACTORY_WASM);
  console.log(`[junoswap] Factory WASM size: ${(factoryWasm.length / 1024).toFixed(0)}KB`);

  const factoryUpload = await client.upload(
    account.address,
    factoryWasm,
    "auto",
    "Junoswap v2 Factory"
  );
  console.log(`[junoswap] Factory code ID: ${factoryUpload.codeId}`);
  console.log(`[junoswap] Factory upload TX: ${factoryUpload.transactionHash}`);
  console.log(`[junoswap] Gas used: ${factoryUpload.gasUsed}`);

  // ── Step 3: Instantiate factory ──
  console.log(`\n[junoswap] Step 3: Instantiating factory...`);
  const factoryInitMsg = {
    pair_code_id: pairUpload.codeId,
    default_fee_bps: 30, // 0.30% swap fee
    junoclaw_contract: AGENT_COMPANY,
  };

  const factoryResult = await client.instantiate(
    account.address,
    factoryUpload.codeId,
    factoryInitMsg,
    "Junoswap v2 Factory",
    "auto",
    {
      admin: account.address,
      memo: "Junoswap v2 Factory — JunoClaw TEE-attested DEX",
    }
  );
  const factoryAddr = factoryResult.contractAddress;
  console.log(`[junoswap] Factory address: ${factoryAddr}`);
  console.log(`[junoswap] Factory TX: ${factoryResult.transactionHash}`);
  console.log(`[junoswap] Gas used: ${factoryResult.gasUsed}`);

  // ── Step 4: Create first pair (ujunox / ibc-usdc-test) ──
  console.log(`\n[junoswap] Step 4: Creating JUNOX/USDC pair...`);
  const createPairMsg = {
    create_pair: {
      token_a: { native: "ujunox" },
      token_b: { native: "ibc/EAC38D55372F38AA5A25FE2385764B04D0A7CEEE1A4F93856CBD9DAE68B1E1D0" },
      fee_bps: 30,
    },
  };

  let pairAddr: string;
  try {
    const pairResult = await client.execute(
      account.address,
      factoryAddr,
      createPairMsg,
      "auto",
      "Create JUNOX/USDC pair"
    );
    console.log(`[junoswap] Create pair TX: ${pairResult.transactionHash}`);
    console.log(`[junoswap] Gas used: ${pairResult.gasUsed}`);

    // Query the pair address
    const pairQuery = await client.queryContractSmart(factoryAddr, {
      pair: {
        token_a: { native: "ujunox" },
        token_b: { native: "ibc/EAC38D55372F38AA5A25FE2385764B04D0A7CEEE1A4F93856CBD9DAE68B1E1D0" },
      },
    });
    pairAddr = pairQuery.pair_addr;
    console.log(`[junoswap] Pair address: ${pairAddr}`);
  } catch (err: any) {
    console.log(`[junoswap] Note: Create pair failed (${err.message}). This is OK if no IBC USDC on testnet.`);
    console.log(`[junoswap] Creating ujunox/ujunox-b pair instead (for testing)...`);

    // Fallback: create a pair with two native denoms that exist on testnet
    const fallbackMsg = {
      create_pair: {
        token_a: { native: "ujunox" },
        token_b: { native: "ustake" },
        fee_bps: 30,
      },
    };
    const pairResult = await client.execute(
      account.address,
      factoryAddr,
      fallbackMsg,
      "auto",
      "Create JUNOX/STAKE test pair"
    );
    console.log(`[junoswap] Create pair TX: ${pairResult.transactionHash}`);

    const pairQuery = await client.queryContractSmart(factoryAddr, {
      pair: { token_a: { native: "ujunox" }, token_b: { native: "ustake" } },
    });
    pairAddr = pairQuery.pair_addr;
    console.log(`[junoswap] Pair address: ${pairAddr}`);
  }

  // ── Step 5: Verify ──
  console.log(`\n[junoswap] Step 5: Verifying...`);
  const factoryConfig = await client.queryContractSmart(factoryAddr, { config: {} });
  console.log(`[junoswap] Factory config:`, JSON.stringify(factoryConfig, null, 2));

  const pairCount = await client.queryContractSmart(factoryAddr, { pair_count: {} });
  console.log(`[junoswap] Pair count: ${pairCount}`);

  const pairInfo = await client.queryContractSmart(pairAddr, { pair_info: {} });
  console.log(`[junoswap] Pair info:`, JSON.stringify(pairInfo, null, 2));

  const poolState = await client.queryContractSmart(pairAddr, { pool: {} });
  console.log(`[junoswap] Pool state:`, JSON.stringify(poolState, null, 2));

  // ── Step 6: Save deployment info ──
  const deployedPath = resolve(__dirname, "../../../deploy/deployed.json");
  try {
    let deployed: any = {};
    if (existsSync(deployedPath)) {
      deployed = JSON.parse(readFileSync(deployedPath, "utf-8"));
    }
    deployed["junoswap-factory"] = {
      code_id: factoryUpload.codeId,
      store_tx: factoryUpload.transactionHash,
      address: factoryAddr,
      instantiate_tx: factoryResult.transactionHash,
      wasmd_admin: account.address,
    };
    deployed["junoswap-pair-code"] = {
      code_id: pairUpload.codeId,
      store_tx: pairUpload.transactionHash,
      note: "Pair instances created by factory",
    };
    deployed["junoswap-pair-1"] = {
      address: pairAddr,
      factory: factoryAddr,
    };
    writeFileSync(deployedPath, JSON.stringify(deployed, null, 2));
    console.log(`\n[junoswap] Updated deployed.json`);
  } catch {
    console.log(`\n[junoswap] Could not update deployed.json`);
  }

  console.log(`\n╔══════════════════════════════════════════════════════════════════╗`);
  console.log(`║  ✅ JUNOSWAP V2 DEPLOYMENT SUCCESSFUL                           ║`);
  console.log(`╠══════════════════════════════════════════════════════════════════╣`);
  console.log(`║  Factory: ${factoryAddr}`);
  console.log(`║  Pair:    ${pairAddr}`);
  console.log(`║  Pair Code ID: ${pairUpload.codeId}`);
  console.log(`║  Factory Code ID: ${factoryUpload.codeId}`);
  console.log(`║  Chain: ${config.chainId}`);
  console.log(`║  JunoClaw hook: ${AGENT_COMPANY}`);
  console.log(`╚══════════════════════════════════════════════════════════════════╝`);
}

main().catch((err) => {
  console.error("\n[junoswap] Deployment failed:", err.message || err);
  process.exit(1);
});
