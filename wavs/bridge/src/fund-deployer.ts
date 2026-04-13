#!/usr/bin/env tsx
/**
 * One-shot script: send JUNOX from a parliament MP wallet to the deploy wallet.
 * Reads parliament-state.json for the first MP's mnemonic.
 *
 * Usage: npx tsx src/fund-deployer.ts
 */

import { readFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { GasPrice } from "@cosmjs/stargate";
import { config } from "./config.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

const TARGET_WALLET = "juno1t08k74tqwukkxjyq5cwqrguzs7ktv4y7jfr4d6";
const SEND_AMOUNT = "100000000"; // 100 JUNOX (6 decimals)

async function main() {
  // Read parliament state for MP mnemonic
  const statePath = resolve(__dirname, "../parliament-state.json");
  const state = JSON.parse(readFileSync(statePath, "utf-8"));
  const mp = state.mps[0]; // The Builder
  console.log(`Funding from: ${mp.name} (${mp.address})`);
  console.log(`Target:       ${TARGET_WALLET}`);
  console.log(`Amount:       ${parseInt(SEND_AMOUNT) / 1e6} JUNOX\n`);

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mp.mnemonic, {
    prefix: "juno",
  });
  const [account] = await wallet.getAccounts();

  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString("0.075ujunox") }
  );

  const balance = await client.getBalance(account.address, config.denom);
  console.log(`Sender balance: ${parseInt(balance.amount) / 1e6} JUNOX`);

  const result = await client.sendTokens(
    account.address,
    TARGET_WALLET,
    [{ denom: config.denom, amount: SEND_AMOUNT }],
    "auto",
    "Fund zk-verifier deployer from parliament MP"
  );

  console.log(`\nTX: ${result.transactionHash}`);
  console.log(`Gas used: ${result.gasUsed}`);

  const newBal = await client.getBalance(TARGET_WALLET, config.denom);
  console.log(`\nDeployer balance: ${parseInt(newBal.amount) / 1e6} JUNOX`);
}

main().catch((err) => {
  console.error("Error:", err.message);
  process.exit(1);
});
