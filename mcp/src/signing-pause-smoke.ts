#!/usr/bin/env tsx
/**
 * On-chain proof for the `signing_paused` kill-switch — v0.x.y-security-2.
 *
 * Two-phase smoke:
 *   Phase A — arm the kill-switch, attempt to sign, expect
 *     SigningPausedError. Proves the gate refuses when armed.
 *   Phase B — disarm, attempt to sign, expect a real broadcast.
 *     Proves disarming restores the live signing path.
 *
 * Reuses the same `signing-smoke-uni7` wallet enrolled by
 * signing-smoke.ts. Idempotent: enrols if missing, no-op if present.
 *
 * Prerequisites — at least one wallet-registry backend usable
 * (passphrase env var or @napi-rs/keyring installed). Mirrors
 * signing-smoke.ts.
 *
 * NOTE: This script does NOT read JUNOCLAW_SIGNING_PAUSED. It flips
 * the kill-switch programmatically via setSigningPaused() so the
 * test can drive both phases in one process without a restart.
 * The env-var startup-time path is exercised by the unit tests
 * and by `JUNOCLAW_SIGNING_PAUSED=1 npm run signing-smoke` in CI.
 */

import { readFileSync } from "fs";
import { resolve } from "path";

import { getChain } from "./resources/chains.js";
import { sendTokens } from "./tools/tx-builder.js";
import {
  getDefaultWalletStore,
  SigningPausedError,
} from "./wallet/store.js";

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

  // Same backend-availability guard as signing-smoke.ts.
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

  // ──────────────────────────────────────────
  // Phase A — armed: expect SigningPausedError
  // ──────────────────────────────────────────

  console.log("\n━━━ Phase A: signing_paused=true, expect refusal ━━━");
  store.setSigningPaused(true, "smoke:phase-A");
  const stateA = store.getSigningPaused();
  console.log(`State: paused=${stateA.paused}, source=${stateA.source}`);

  let phaseACaught: Error | null = null;
  try {
    await sendTokens(
      chain.chainId,
      SMOKE_WALLET_ID,
      address,
      "1",
      chain.denom,
      "signing-paused smoke (armed)"
    );
  } catch (e) {
    phaseACaught = e as Error;
  }

  if (!phaseACaught) {
    throw new Error(
      "❌ Phase A FAILED: armed kill-switch did NOT block sendTokens (a TX broadcast). " +
        "The gate is not wired correctly."
    );
  }
  if (!(phaseACaught instanceof SigningPausedError)) {
    throw new Error(
      `❌ Phase A FAILED: armed kill-switch threw ${phaseACaught.constructor.name} ` +
        `instead of SigningPausedError. Message: ${phaseACaught.message}`
    );
  }
  console.log(
    `✓ Phase A: SigningPausedError raised as expected — wallet=${phaseACaught.walletId}, chain=${phaseACaught.chainId}`
  );

  // ──────────────────────────────────────────
  // Phase B — disarmed: expect successful broadcast
  // ──────────────────────────────────────────

  console.log("\n━━━ Phase B: signing_paused=false, expect broadcast ━━━");
  store.setSigningPaused(false, "smoke:phase-B");
  const stateB = store.getSigningPaused();
  console.log(`State: paused=${stateB.paused}, source=${stateB.source}`);

  const result = await sendTokens(
    chain.chainId,
    SMOKE_WALLET_ID,
    address,
    "1",
    chain.denom,
    "signing-paused smoke (disarmed)"
  );

  console.log("\n✅ Phase B: signing flow validated end-to-end after disarm");
  console.log(`TX hash:   ${result.txHash}`);
  console.log(`Gas used:  ${result.gasUsed}`);
  console.log(`Explorer:  ${result.explorerUrl}`);

  console.log("\n━━━ signing_paused on-chain proof complete ━━━");
  console.log("  Phase A: refused as expected (SigningPausedError)");
  console.log(`  Phase B: broadcast succeeded (TX ${result.txHash})`);
}

main().catch((err: unknown) => {
  const msg = err instanceof Error ? err.message : String(err);
  console.error(`❌ Signing-pause smoke failed: ${msg}`);
  process.exit(1);
});
