/**
 * Smoke test for the `signing_paused` kill-switch — v0.x.y-security-2.
 *
 * Run:
 *   npm run signing-pause-test    (from mcp/)
 *   tsx src/wallet/signing-pause-test.ts
 *
 * Exits 0 on success, 1 on any failure. Uses a temp WALLET_ROOT under
 * the OS tmpdir; the operator's real registry is never touched.
 *
 * Coverage:
 *   - default state: kill-switch off, source null
 *   - setSigningPaused(true) flips state and records source
 *   - setSigningPaused(false) clears source
 *   - toggle round-trip is stable
 *   - signFor() throws SigningPausedError when paused
 *   - signFor() throws SigningPausedError even for non-existent
 *     walletId (gate-first ordering: no wallet-enumeration signal)
 *   - SigningPausedError carries walletId and chainId properties
 *   - add / list / verifyAddress / remove still work when paused
 *     (registry-management unaffected; the lever is signing-only)
 *   - parseSigningPausedEnv via defaultStore behavior is covered by
 *     the on-chain smoke (signing-pause-smoke.ts), not here.
 */

import { promises as fs } from "fs";
import { tmpdir } from "os";
import { join, resolve } from "path";

import type { ChainConfig } from "../resources/chains.js";
import { PassphraseKeyStore } from "./key-store.js";
import { SigningPausedError, WalletStore } from "./store.js";

// ──────────────────────────────────────────────
// Test runner (same shape as store-test.ts)
// ──────────────────────────────────────────────

let passed = 0;
let failed = 0;
const failures: string[] = [];

function test(name: string, fn: () => Promise<void>): Promise<void> {
  return fn().then(
    () => {
      console.log(`  PASS  ${name}`);
      passed++;
    },
    (e: Error) => {
      console.log(`  FAIL  ${name}\n        ${e.message}`);
      failed++;
      failures.push(name);
    }
  );
}

async function expectThrow(
  fn: () => Promise<unknown>,
  predicate: (e: Error) => boolean,
  description: string
): Promise<Error> {
  let threw: Error | null = null;
  try {
    await fn();
  } catch (e) {
    threw = e as Error;
  }
  if (!threw) {
    throw new Error(`expected throw matching "${description}", but no throw occurred`);
  }
  if (!predicate(threw)) {
    throw new Error(
      `expected throw matching "${description}", got: ${threw.constructor.name}: ${threw.message}`
    );
  }
  return threw;
}

// ──────────────────────────────────────────────
// Test fixtures
// ──────────────────────────────────────────────

// BIP-39 test vector (public, do not use for funds).
const TEST_MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

const PASSPHRASE = "correct horse battery staple";

// Minimal ChainConfig fake — signFor() never reaches the RPC when
// paused (the gate fires first), so unreachable endpoints are fine.
const FAKE_CHAIN: ChainConfig = {
  chainId: "test-chain-1",
  chainName: "Test Chain",
  rpcEndpoint: "http://127.0.0.1:1/unreachable",
  restEndpoint: "http://127.0.0.1:1/unreachable",
  denom: "utest",
  bech32Prefix: "cosmos",
  gasPrice: "0.025utest",
  slip44: 118,
  explorerTx: "http://example.test/tx/{hash}",
  isTestnet: true,
};

function makeStore(rootDir: string): WalletStore {
  const keyStore = new PassphraseKeyStore(rootDir, async () => PASSPHRASE);
  return new WalletStore(rootDir, keyStore);
}

console.log("\n━━━ signing_paused kill-switch (v0.x.y-security-2) ━━━\n");

async function run() {
  const sandbox = resolve(tmpdir(), `junoclaw-signing-pause-${Date.now()}`);
  const root = join(sandbox, "registry");
  await fs.mkdir(sandbox, { recursive: true });

  // ──────────────────────────────────────────
  // State machine
  // ──────────────────────────────────────────

  await test("default state: getSigningPaused() returns paused=false, source=null", async () => {
    const store = makeStore(root);
    const state = store.getSigningPaused();
    if (state.paused !== false) {
      throw new Error(`expected paused=false, got ${state.paused}`);
    }
    if (state.source !== null) {
      throw new Error(`expected source=null, got ${state.source}`);
    }
  });

  await test("setSigningPaused(true, source) flips state and records source", async () => {
    const store = makeStore(root);
    store.setSigningPaused(true, "test:setSigningPaused");
    const state = store.getSigningPaused();
    if (state.paused !== true) {
      throw new Error(`expected paused=true after arm, got ${state.paused}`);
    }
    if (state.source !== "test:setSigningPaused") {
      throw new Error(`expected source="test:setSigningPaused", got ${state.source}`);
    }
  });

  await test("setSigningPaused(false, source) clears source to null", async () => {
    const store = makeStore(root);
    store.setSigningPaused(true, "first");
    store.setSigningPaused(false, "second");
    const state = store.getSigningPaused();
    if (state.paused !== false) {
      throw new Error(`expected paused=false after disarm, got ${state.paused}`);
    }
    if (state.source !== null) {
      throw new Error(
        `expected source=null after disarm (not retained), got ${state.source}`
      );
    }
  });

  await test("toggle round-trip is stable (false → true → false → true)", async () => {
    const store = makeStore(root);
    if (store.getSigningPaused().paused !== false) throw new Error("step 0: not initially false");
    store.setSigningPaused(true, "a");
    if (store.getSigningPaused().paused !== true) throw new Error("step 1: not true");
    store.setSigningPaused(false, "b");
    if (store.getSigningPaused().paused !== false) throw new Error("step 2: not false");
    store.setSigningPaused(true, "c");
    const final = store.getSigningPaused();
    if (final.paused !== true) throw new Error("step 3: not true");
    if (final.source !== "c") throw new Error(`step 3: source=${final.source}, expected "c"`);
  });

  // ──────────────────────────────────────────
  // signFor gate
  // ──────────────────────────────────────────

  await test("signFor() throws SigningPausedError when paused (registered wallet)", async () => {
    const root1 = join(sandbox, "registry-1");
    const store = makeStore(root1);
    await store.add("alice", TEST_MNEMONIC, { bech32Prefix: "cosmos" });
    store.setSigningPaused(true, "test:gate");
    const err = await expectThrow(
      () => store.signFor("alice", FAKE_CHAIN),
      (e) => e instanceof SigningPausedError,
      "SigningPausedError for registered wallet"
    );
    if (!(err instanceof SigningPausedError)) throw new Error("not SigningPausedError instance");
  });

  await test("signFor() throws SigningPausedError even for non-existent walletId (gate-first, no enumeration leak)", async () => {
    const root2 = join(sandbox, "registry-2");
    const store = makeStore(root2);
    // Note: NO wallet added. Without the gate, this would throw
    // "wallet not found"; the gate must throw SigningPausedError
    // first so an attacker cannot probe wallet IDs while the
    // registry is paused.
    store.setSigningPaused(true, "test:enumeration-leak");
    await expectThrow(
      () => store.signFor("does-not-exist", FAKE_CHAIN),
      (e) => e instanceof SigningPausedError,
      "SigningPausedError for non-existent wallet (must outrank wallet-not-found)"
    );
  });

  await test("signFor() unpaused: gate does not fire (proceeds past pause check)", async () => {
    const root3 = join(sandbox, "registry-3");
    const store = makeStore(root3);
    await store.add("bob", TEST_MNEMONIC, { bech32Prefix: "cosmos" });
    // Note: store is NOT paused. signFor will still throw because
    // FAKE_CHAIN has bech32Prefix "cosmos" but the wallet was added
    // with bech32Prefix "cosmos" — wait, both are "cosmos" so the
    // prefix check passes. The signing client construction then
    // fails on the unreachable RPC. Either way, the error must
    // NOT be SigningPausedError.
    let caught: Error | null = null;
    try {
      await store.signFor("bob", FAKE_CHAIN);
    } catch (e) {
      caught = e as Error;
    }
    if (caught instanceof SigningPausedError) {
      throw new Error(
        `unpaused store should not throw SigningPausedError, but did: ${caught.message}`
      );
    }
    // We expect *some* error (network unreachable) but specifically
    // not a pause error. Pass.
  });

  // ──────────────────────────────────────────
  // SigningPausedError shape
  // ──────────────────────────────────────────

  await test("SigningPausedError carries walletId and chainId properties", async () => {
    const err = new SigningPausedError("alice", "uni-7");
    if (err.walletId !== "alice") {
      throw new Error(`expected walletId="alice", got ${err.walletId}`);
    }
    if (err.chainId !== "uni-7") {
      throw new Error(`expected chainId="uni-7", got ${err.chainId}`);
    }
    if (err.name !== "SigningPausedError") {
      throw new Error(`expected name="SigningPausedError", got ${err.name}`);
    }
    if (!(err instanceof Error)) {
      throw new Error("SigningPausedError is not an Error subclass");
    }
    if (!(err instanceof SigningPausedError)) {
      throw new Error("instanceof SigningPausedError check failed");
    }
    if (!err.message.includes("alice") || !err.message.includes("uni-7")) {
      throw new Error(`message missing identifiers: ${err.message}`);
    }
  });

  // ──────────────────────────────────────────
  // Registry management still works while paused
  // ──────────────────────────────────────────

  await test("add() works while paused (registry management is signing-paused-independent)", async () => {
    const root4 = join(sandbox, "registry-4");
    const store = makeStore(root4);
    store.setSigningPaused(true, "test:registry-mgmt");
    const entry = await store.add("carol", TEST_MNEMONIC, { bech32Prefix: "cosmos" });
    if (!entry.address.startsWith("cosmos1")) {
      throw new Error(`add returned bad address: ${entry.address}`);
    }
  });

  await test("list() works while paused", async () => {
    const root5 = join(sandbox, "registry-5");
    const store = makeStore(root5);
    await store.add("dave", TEST_MNEMONIC, { bech32Prefix: "cosmos" });
    store.setSigningPaused(true, "test:list-while-paused");
    const entries = await store.list();
    if (entries.length !== 1 || entries[0].id !== "dave") {
      throw new Error(`expected [dave], got ${JSON.stringify(entries)}`);
    }
  });

  await test("verifyAddress() works while paused", async () => {
    const root6 = join(sandbox, "registry-6");
    const store = makeStore(root6);
    const addEntry = await store.add("erin", TEST_MNEMONIC, { bech32Prefix: "cosmos" });
    store.setSigningPaused(true, "test:verify-while-paused");
    const verifiedAddr = await store.verifyAddress("erin");
    if (verifiedAddr !== addEntry.address) {
      throw new Error(
        `verifyAddress(${addEntry.address}) returned ${verifiedAddr}; mismatch`
      );
    }
  });

  await test("remove() works while paused", async () => {
    const root7 = join(sandbox, "registry-7");
    const store = makeStore(root7);
    await store.add("frank", TEST_MNEMONIC, { bech32Prefix: "cosmos" });
    store.setSigningPaused(true, "test:remove-while-paused");
    await store.remove("frank");
    const after = await store.list();
    if (after.length !== 0) {
      throw new Error(`expected empty list after remove, got ${JSON.stringify(after)}`);
    }
  });

  // ──────────────────────────────────────────
  // Cleanup
  // ──────────────────────────────────────────

  try {
    await fs.rm(sandbox, { recursive: true, force: true });
  } catch (e) {
    console.warn(`(cleanup warning: ${(e as Error).message})`);
  }

  console.log(`\n${passed} passed, ${failed} failed\n`);

  if (failed > 0) {
    console.log("Failed tests:");
    for (const name of failures) console.log(`  - ${name}`);
    process.exit(1);
  }

  process.exit(0);
}

run().catch((e) => {
  console.error("test runner crashed:", e);
  process.exit(1);
});
