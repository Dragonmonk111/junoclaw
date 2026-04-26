#!/usr/bin/env tsx
/**
 * Signing smoke test (write path) — post Ffern C-3
 *
 * - Loads WAVS_OPERATOR_MNEMONIC from ../wavs/.env
 * - Auto-enrols it into the wallet registry as `signing-smoke-uni7`
 *   (idempotent: no-op if already present)
 * - Signs via WalletStore — no raw mnemonic crosses any function
 *   boundary in tools/tx-builder.ts
 * - Sends 1 ujunox to self (minimal-impact on-chain TX)
 *
 * Prerequisites (choose one backend for the registry):
 *   - Passphrase backend: set JUNOCLAW_WALLET_PASSPHRASE (or _FILE)
 *     so the registry can derive the master DEK.
 *   - Keychain backend: leave the passphrase env var unset; the
 *     registry will use the OS credential manager (DPAPI / Keychain
 *     / libsecret). Requires the optional @napi-rs/keyring dep.
 *   - wavs/.env must contain WAVS_OPERATOR_MNEMONIC.
 *
 * Validates: enrolment -> registry decrypt -> signing client -> broadcast.
 */

import { readFileSync } from "fs";
import { resolve } from "path";

import { getChain } from "./resources/chains.js";
import { sendTokens } from "./tools/tx-builder.js";
import { getDefaultWalletStore } from "./wallet/store.js";

const SMOKE_WALLET_ID = "signing-smoke-uni7";

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

async function ensureSmokeWallet(): Promise<string> {
  const store = getDefaultWalletStore();
  const existing = await store.list();
  const found = existing.find((e) => e.id === SMOKE_WALLET_ID);
  if (found) {
    console.log(`Wallet "${SMOKE_WALLET_ID}" already registered (${found.address})`);
    return found.address;
  }

  console.log(`Enrolling "${SMOKE_WALLET_ID}" into wallet registry...`);
  const mnemonic = loadMnemonicFromEnvFile();
  const entry = await store.add(SMOKE_WALLET_ID, mnemonic, {
    bech32Prefix: "juno",
  });
  console.log(`✓ enrolled as ${entry.address}`);
  return entry.address;
}

async function main() {
  const chain = getChain("uni-7");
  if (!chain) throw new Error("Chain uni-7 not found");

  // Post-Phase-2, the registry has two backends. At least one must
  // be usable or enrolment/signing will fail with a clearer error
  // deeper in the stack. This guard surfaces the common misconfig
  // (no passphrase, no keychain driver) up front.
  const hasPassphrase =
    !!process.env.JUNOCLAW_WALLET_PASSPHRASE ||
    !!process.env.JUNOCLAW_WALLET_PASSPHRASE_FILE;
  const store = getDefaultWalletStore();
  const hasKeychain = store.listBackends().includes("keychain");
  if (!hasPassphrase && !hasKeychain) {
    throw new Error(
      "no wallet registry backend available. Set JUNOCLAW_WALLET_PASSPHRASE " +
        "(passphrase backend) or install @napi-rs/keyring (keychain backend)."
    );
  }
  const activeBackend =
    process.env.JUNOCLAW_WALLET_DEFAULT_BACKEND ??
    (hasPassphrase ? "passphrase" : "keychain");
  console.log(`Using wallet backend: ${activeBackend}`);

  const address = await ensureSmokeWallet();

  console.log(`Signer: ${address}`);

  const result = await sendTokens(
    chain.chainId,
    SMOKE_WALLET_ID,
    address,
    "1",
    chain.denom,
    "cosmos-mcp signing smoke test (wallet_id flow)"
  );

  console.log("\n✅ Signing flow validated via wallet registry");
  console.log(`TX hash:   ${result.txHash}`);
  console.log(`Gas used:  ${result.gasUsed}`);
  console.log(`Explorer:  ${result.explorerUrl}`);
}

main().catch((err: unknown) => {
  const msg = err instanceof Error ? err.message : String(err);
  console.error(`❌ Signing smoke test failed: ${msg}`);
  process.exit(1);
});
