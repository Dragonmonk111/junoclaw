#!/usr/bin/env tsx
/**
 * Wallet utility for JunoClaw WAVS bridge.
 *
 * Commands:
 *   npx tsx src/wallet.ts generate     — Create a new wallet (shows mnemonic ONCE)
 *   npx tsx src/wallet.ts balance      — Check balance of configured wallet
 *   npx tsx src/wallet.ts address      — Show address of configured wallet
 */

import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { GasPrice } from "@cosmjs/stargate";
import { config } from "./config.js";

async function generate() {
  const wallet = await DirectSecp256k1HdWallet.generate(24, {
    prefix: "juno",
  });
  const [account] = await wallet.getAccounts();
  const mnemonic = wallet.mnemonic;

  console.log("╔══════════════════════════════════════════════════════════════╗");
  console.log("║  NEW WALLET GENERATED — SAVE THE MNEMONIC BELOW SECURELY!  ║");
  console.log("╠══════════════════════════════════════════════════════════════╣");
  console.log(`║  Address: ${account.address}`);
  console.log("╠══════════════════════════════════════════════════════════════╣");
  console.log(`║  Mnemonic (WRITE THIS DOWN — shown ONCE):`);
  console.log(`║`);
  console.log(`║  ${mnemonic}`);
  console.log(`║`);
  console.log("╠══════════════════════════════════════════════════════════════╣");
  console.log("║  Next steps:");
  console.log("║  1. Copy the mnemonic to a SAFE location");
  console.log("║  2. Put it in wavs/.env as WAVS_OPERATOR_MNEMONIC");
  console.log("║  3. Get testnet tokens: faucet or ask a validator");
  console.log("╚══════════════════════════════════════════════════════════════╝");
}

async function showAddress() {
  if (!config.mnemonic) {
    console.error("WAVS_OPERATOR_MNEMONIC not set in .env");
    process.exit(1);
  }
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: "juno",
  });
  const [account] = await wallet.getAccounts();
  console.log(account.address);
}

async function showBalance() {
  if (!config.mnemonic) {
    console.error("WAVS_OPERATOR_MNEMONIC not set in .env");
    process.exit(1);
  }
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: "juno",
  });
  const [account] = await wallet.getAccounts();

  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );

  const balance = await client.getBalance(account.address, config.denom);
  console.log(`Address: ${account.address}`);
  console.log(`Balance: ${balance.amount} ${balance.denom}`);
}

const command = process.argv[2];
switch (command) {
  case "generate":
    generate().catch(console.error);
    break;
  case "address":
    showAddress().catch(console.error);
    break;
  case "balance":
    showBalance().catch(console.error);
    break;
  default:
    console.log("Usage: npx tsx src/wallet.ts <generate|address|balance>");
}
