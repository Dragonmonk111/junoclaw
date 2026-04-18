#!/usr/bin/env tsx
/**
 * Upload new WASM code and instantiate a fresh agent-company contract.
 * Sets wasmd-level admin to the deployer so future migrations work.
 *
 * Usage:
 *   npx tsx src/deploy-fresh.ts <path-to-optimized.wasm>
 */

import { readFileSync, writeFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config, validateConfig } from "./config.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Existing contract addresses (from first deployment)
const EXISTING = {
  escrow: "juno1dh43lswg5ekv7q2p44s6hgays47k5mz67742vdwpd025p8q05kgs0azwrv",
  agentRegistry: "juno1qulyspwzjzsz7rq65v6ptzt278f9ta9uh0upxu6xa08gf4v5gzaqm676j7",
  taskLedger: "juno1agw6f05wxx5rm8d3etq7cejcm5g8e224s00dvykylaja7jlx3ljq6f0u46",
};

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    console.error("Usage: deploy-fresh <path-to-optimized.wasm>");
    process.exit(1);
  }

  validateConfig();

  const wasmPath = args[0];

  // Read WASM
  console.log(`\n[deploy] Reading WASM: ${wasmPath}`);
  const wasmCode = readFileSync(wasmPath);
  console.log(`[deploy] Size: ${wasmCode.length} bytes`);

  // Connect wallet
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();
  console.log(`[deploy] Deployer (Neo): ${account.address}`);

  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );

  const balance = await client.getBalance(account.address, config.denom);
  console.log(`[deploy] Balance: ${balance.amount} ${balance.denom}`);

  // Step 1: Upload code
  console.log(`\n[deploy] Step 1: Uploading code...`);
  const uploadResult = await client.upload(
    account.address,
    wasmCode,
    "auto",
    "JunoClaw agent-company v2 (WAVS attestation)"
  );
  console.log(`[deploy] Code ID: ${uploadResult.codeId}`);
  console.log(`[deploy] TX: ${uploadResult.transactionHash}`);
  console.log(`[deploy] Gas: ${uploadResult.gasUsed}`);

  // Step 2: Instantiate with wasmd admin set
  console.log(`\n[deploy] Step 2: Instantiating agent-company v2...`);
  const instantiateMsg = {
    name: "JunoClaw Core Team",
    admin: null, // contract-level admin defaults to sender
    governance: null,
    wavs_operator: account.address,
    escrow_contract: EXISTING.escrow,
    agent_registry: EXISTING.agentRegistry,
    task_ledger: EXISTING.taskLedger,
    nois_proxy: null,
    members: [
      {
        addr: account.address,
        role: "human",
        weight: 10000,
        alias: "Neo",
      },
    ],
    denom: "ujunox",
    voting_period_blocks: 100,
    quorum_percent: 51,
    adaptive_threshold_blocks: 10,
    adaptive_min_blocks: 13,
    verification: null, // uses default: WitnessAndWavs, 2-of-3, 200 block timeout
  };

  const instantiateResult = await client.instantiate(
    account.address,
    uploadResult.codeId,
    instantiateMsg,
    "JunoClaw Agent Company v2",
    "auto",
    {
      admin: account.address, // ← THIS sets the wasmd-level admin for future migrations
      memo: "JunoClaw agent-company v2 with WAVS TEE attestation support",
    }
  );
  console.log(`[deploy] Contract: ${instantiateResult.contractAddress}`);
  console.log(`[deploy] TX: ${instantiateResult.transactionHash}`);
  console.log(`[deploy] Gas: ${instantiateResult.gasUsed}`);

  console.log(`\n[deploy] Step 2b: Wiring task-ledger agent_company...`);
  const linkResult = await client.execute(
    account.address,
    EXISTING.taskLedger,
    {
      update_config: {
        admin: null,
        agent_registry: null,
        agent_company: instantiateResult.contractAddress,
      },
    },
    "auto",
    "Wire task-ledger agent_company"
  );
  console.log(`[deploy] Link TX: ${linkResult.transactionHash}`);
  console.log(`[deploy] Link Gas: ${linkResult.gasUsed}`);

  // Step 3: Verify
  console.log(`\n[deploy] Step 3: Verifying...`);
  const info = await client.getContract(instantiateResult.contractAddress);
  console.log(`[deploy] Address: ${info.address}`);
  console.log(`[deploy] Code ID: ${info.codeId}`);
  console.log(`[deploy] Admin: ${info.admin}`);
  console.log(`[deploy] Label: ${info.label}`);

  const cfg = await client.queryContractSmart(
    instantiateResult.contractAddress,
    { get_config: {} }
  );
  console.log(`[deploy] Config name: ${cfg.name}`);
  console.log(`[deploy] Config admin: ${cfg.admin}`);
  console.log(`[deploy] Task ledger: ${cfg.task_ledger}`);

  // Step 4: Update deployed.json
  const deployedPath = resolve(__dirname, "../../../deploy/deployed.json");
  try {
    const deployed = JSON.parse(readFileSync(deployedPath, "utf-8"));
    deployed["agent-company-v2"] = {
      code_id: uploadResult.codeId,
      store_tx: uploadResult.transactionHash,
      address: instantiateResult.contractAddress,
      instantiate_tx: instantiateResult.transactionHash,
      wasmd_admin: account.address,
      note: "v2 with WAVS TEE attestation support",
    };
    writeFileSync(deployedPath, JSON.stringify(deployed, null, 2));
    console.log(`\n[deploy] Updated deployed.json`);
  } catch {
    console.log(`\n[deploy] Could not update deployed.json (path: ${deployedPath})`);
  }

  console.log(`\n╔══════════════════════════════════════════════════════════════╗`);
  console.log(`║  ✅ DEPLOYMENT SUCCESSFUL                                    ║`);
  console.log(`╠══════════════════════════════════════════════════════════════╣`);
  console.log(`║  Contract: ${instantiateResult.contractAddress}`);
  console.log(`║  Code ID:  ${uploadResult.codeId}`);
  console.log(`║  Admin:    ${account.address}`);
  console.log(`║  Chain:    ${config.chainId}`);
  console.log(`╚══════════════════════════════════════════════════════════════╝`);
}

main().catch((err) => {
  console.error("\n[deploy] Failed:", err.message || err);
  process.exit(1);
});
