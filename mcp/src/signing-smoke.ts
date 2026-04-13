#!/usr/bin/env tsx
/**
 * Signing smoke test (write path)
 *
 * - Loads WAVS_OPERATOR_MNEMONIC from ../wavs/.env
 * - Derives signer address on uni-7
 * - Sends 1 ujunox to self (minimal-impact on-chain TX)
 *
 * Validates: mnemonic -> signing client -> broadcast -> tx result
 */

import { readFileSync } from "fs";
import { resolve } from "path";
import { getChain } from "./resources/chains.js";
import { getSigningClient } from "./utils/cosmos-client.js";
import { sendTokens } from "./tools/tx-builder.js";

function loadMnemonicFromEnvFile(): string {
  const envPath = resolve(process.cwd(), "../wavs/.env");
  const raw = readFileSync(envPath, "utf-8");
  const line = raw
    .split(/\r?\n/)
    .find((l) => l.trim().startsWith("WAVS_OPERATOR_MNEMONIC="));

  if (!line) {
    throw new Error("WAVS_OPERATOR_MNEMONIC not found in wavs/.env");
  }

  const value = line.slice("WAVS_OPERATOR_MNEMONIC=".length).trim();
  if (!value || value === "PASTE_YOUR_MNEMONIC_HERE") {
    throw new Error("WAVS_OPERATOR_MNEMONIC is empty/placeholder in wavs/.env");
  }

  return value;
}

async function main() {
  const chain = getChain("uni-7");
  if (!chain) throw new Error("Chain uni-7 not found");

  const mnemonic = loadMnemonicFromEnvFile();
  const { client, address } = await getSigningClient(chain, mnemonic);

  const balBefore = await client.getBalance(address, chain.denom);
  console.log(`Signer: ${address}`);
  console.log(`Balance before: ${balBefore.amount} ${chain.denom}`);

  const result = await sendTokens(
    chain.chainId,
    mnemonic,
    address,
    "1",
    chain.denom,
    "cosmos-mcp signing smoke test (self-transfer)"
  );

  const balAfter = await client.getBalance(address, chain.denom);

  console.log("\n✅ Signing flow validated");
  console.log(`TX hash: ${result.txHash}`);
  console.log(`Gas used: ${result.gasUsed}`);
  console.log(`Explorer: ${result.explorerUrl}`);
  console.log(`Balance after: ${balAfter.amount} ${chain.denom}`);
}

main().catch((err: unknown) => {
  const msg = err instanceof Error ? err.message : String(err);
  console.error(`❌ Signing smoke test failed: ${msg}`);
  process.exit(1);
});
