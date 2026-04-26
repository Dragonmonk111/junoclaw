/**
 * Smoke test for KeychainKeyStore + WalletStore keychain integration.
 *
 * Run:
 *   npm run keychain-store-test     (from mcp/)
 *
 * Coverage:
 *   - InMemoryKeyringDriver semantics (used by tests + as a reference)
 *   - KeychainKeyStore: first-use generation, idempotent retrieval, length check
 *   - WalletStore + keychain: full add/verifyAddress/remove path
 *   - Tamper detection on a keychain-backed wallet file
 *   - Two backends in one store: per-wallet dispatch by recorded backend
 *   - Real DPAPI / Keychain / libsecret round-trip via the native driver
 *     (skipped gracefully if @napi-rs/keyring isn't installed)
 *
 * No real RPC is touched. No real funds at risk.
 */

import { promises as fs } from "fs";
import { tmpdir } from "os";
import { join, resolve } from "path";

import { PassphraseKeyStore, type KeyStore } from "./key-store.js";
import {
  InMemoryKeyringDriver,
  KeychainKeyStore,
  type KeyringDriver,
} from "./keychain-store.js";
import { WalletStore } from "./store.js";

// ──────────────────────────────────────────────
// Test runner
// ──────────────────────────────────────────────

let passed = 0;
let failed = 0;
let skipped = 0;
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
  contains: string
): Promise<void> {
  let threw = false;
  try {
    await fn();
  } catch (e) {
    threw = true;
    const msg = (e as Error).message;
    if (!msg.includes(contains)) {
      throw new Error(`expected throw containing "${contains}", got: ${msg}`);
    }
  }
  if (!threw) {
    throw new Error(
      `expected throw containing "${contains}", but no throw occurred`
    );
  }
}

// ──────────────────────────────────────────────
// Test fixtures
// ──────────────────────────────────────────────

const TEST_MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const EXPECTED_COSMOS_ADDR = "cosmos19rl4cm2hmr8afy4kldpxz3fka4jguq0auqdal4";

console.log("\n━━━ keychain-store smoke (Ffern C-3 Phase 2) ━━━\n");

async function run() {
  const sandbox = resolve(
    tmpdir(),
    `junoclaw-keychain-store-${Date.now()}`
  );
  await fs.mkdir(sandbox, { recursive: true });

  // ──────────────────────────────────────────
  // InMemoryKeyringDriver semantics
  // ──────────────────────────────────────────

  await test("InMemoryKeyringDriver: get returns null for missing entry", async () => {
    const d = new InMemoryKeyringDriver();
    const got = await d.get("svc", "missing");
    if (got !== null) throw new Error(`expected null, got ${got}`);
  });

  await test("InMemoryKeyringDriver: set/get round-trips", async () => {
    const d = new InMemoryKeyringDriver();
    await d.set("svc", "a", "secret-a");
    const got = await d.get("svc", "a");
    if (got !== "secret-a") throw new Error(`got ${got}`);
  });

  await test("InMemoryKeyringDriver: delete is idempotent", async () => {
    const d = new InMemoryKeyringDriver();
    await d.delete("svc", "missing"); // no throw
    await d.set("svc", "a", "secret-a");
    await d.delete("svc", "a");
    const got = await d.get("svc", "a");
    if (got !== null) throw new Error(`got ${got}`);
  });

  await test("InMemoryKeyringDriver: service+account compose distinctly", async () => {
    const d = new InMemoryKeyringDriver();
    await d.set("svc1", "alice", "x");
    await d.set("svc2", "alice", "y");
    if ((await d.get("svc1", "alice")) !== "x") throw new Error("svc1");
    if ((await d.get("svc2", "alice")) !== "y") throw new Error("svc2");
  });

  // ──────────────────────────────────────────
  // KeychainKeyStore semantics
  // ──────────────────────────────────────────

  await test("KeychainKeyStore: getKey generates fresh 32-byte DEK on first call", async () => {
    const driver = new InMemoryKeyringDriver();
    const ks = new KeychainKeyStore({ driver, service: "test" });
    const key = await ks.getKey("alice");
    if (key.length !== 32) throw new Error(`length ${key.length}`);
    if (driver.size() !== 1) throw new Error(`expected 1 entry, got ${driver.size()}`);
  });

  await test("KeychainKeyStore: getKey is idempotent (returns same key on repeat)", async () => {
    const driver = new InMemoryKeyringDriver();
    const ks = new KeychainKeyStore({ driver, service: "test" });
    const k1 = await ks.getKey("alice");
    const k2 = await ks.getKey("alice");
    if (!k1.equals(k2)) throw new Error("DEKs differ across calls");
    if (driver.size() !== 1) throw new Error(`expected 1 entry, got ${driver.size()}`);
  });

  await test("KeychainKeyStore: distinct walletIds get distinct DEKs", async () => {
    const driver = new InMemoryKeyringDriver();
    const ks = new KeychainKeyStore({ driver, service: "test" });
    const k1 = await ks.getKey("alice");
    const k2 = await ks.getKey("bob");
    if (k1.equals(k2)) throw new Error("DEKs collided");
  });

  await test("KeychainKeyStore: removeKey clears the entry", async () => {
    const driver = new InMemoryKeyringDriver();
    const ks = new KeychainKeyStore({ driver, service: "test" });
    await ks.getKey("alice");
    if (driver.size() !== 1) throw new Error("setup");
    await ks.removeKey("alice");
    if (driver.size() !== 0) throw new Error(`expected 0 entries, got ${driver.size()}`);
  });

  await test("KeychainKeyStore: rejects corrupted entry (wrong byte length)", async () => {
    const driver = new InMemoryKeyringDriver();
    // Plant a wrong-length value as if some other software corrupted it.
    await driver.set("test", "alice", Buffer.alloc(16).toString("base64"));
    const ks = new KeychainKeyStore({ driver, service: "test" });
    await expectThrow(() => ks.getKey("alice"), "wrong length");
  });

  await test("KeychainKeyStore: distinct services compose independently", async () => {
    const driver = new InMemoryKeyringDriver();
    const ks1 = new KeychainKeyStore({ driver, service: "svc-A" });
    const ks2 = new KeychainKeyStore({ driver, service: "svc-B" });
    const k1 = await ks1.getKey("alice");
    const k2 = await ks2.getKey("alice");
    if (k1.equals(k2)) throw new Error("DEKs collided across services");
    if (driver.size() !== 2) throw new Error(`expected 2 entries, got ${driver.size()}`);
  });

  // ──────────────────────────────────────────
  // WalletStore + keychain end-to-end (mock driver)
  // ──────────────────────────────────────────

  function makeKeychainStore(rootDir: string, driver: KeyringDriver): WalletStore {
    const keychain = new KeychainKeyStore({ driver, service: "junoclaw-test" });
    return new WalletStore(rootDir, keychain);
  }

  await test("WalletStore + keychain: add / verifyAddress / remove round-trip", async () => {
    const root = join(sandbox, "kc-roundtrip");
    const driver = new InMemoryKeyringDriver();
    const store = makeKeychainStore(root, driver);

    const entry = await store.add("alice", TEST_MNEMONIC, {
      bech32Prefix: "cosmos",
    });
    if (entry.address !== EXPECTED_COSMOS_ADDR) throw new Error("addr");
    if (entry.backendName !== "keychain") {
      throw new Error(`backend was ${entry.backendName}`);
    }
    if (driver.size() !== 1) {
      throw new Error(`keychain entries: ${driver.size()}`);
    }

    const addr = await store.verifyAddress("alice");
    if (addr !== EXPECTED_COSMOS_ADDR) throw new Error("verify addr mismatch");

    await store.remove("alice");
    if (driver.size() !== 0) {
      throw new Error(`expected keychain cleared, ${driver.size()} entries left`);
    }
    try {
      await fs.access(join(root, "alice.enc"));
      throw new Error("file still exists after remove");
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
    }
  });

  await test("WalletStore + keychain: file records backend='keychain'", async () => {
    const root = join(sandbox, "kc-backend-field");
    const driver = new InMemoryKeyringDriver();
    const store = makeKeychainStore(root, driver);
    await store.add("alice", TEST_MNEMONIC, { bech32Prefix: "cosmos" });

    const raw = await fs.readFile(join(root, "alice.enc"), "utf-8");
    const file = JSON.parse(raw);
    if (file.backend !== "keychain") {
      throw new Error(`backend field was ${file.backend}`);
    }
  });

  await test("WalletStore + keychain: tampered ciphertext rejected", async () => {
    const root = join(sandbox, "kc-tamper");
    const driver = new InMemoryKeyringDriver();
    const store = makeKeychainStore(root, driver);
    await store.add("victim", TEST_MNEMONIC, { bech32Prefix: "cosmos" });

    const path = join(root, "victim.enc");
    const file = JSON.parse(await fs.readFile(path, "utf-8"));
    const ct: string = file.ciphertext_b64;
    const mid = Math.floor(ct.length / 2);
    const flipped = ct[mid] === "A" ? "B" : "A";
    file.ciphertext_b64 = ct.slice(0, mid) + flipped + ct.slice(mid + 1);
    await fs.writeFile(path, JSON.stringify(file, null, 2), {
      encoding: "utf-8",
      mode: 0o600,
    });

    await expectThrow(() => store.verifyAddress("victim"), "decryption failed");
  });

  await test("WalletStore + keychain: removed keychain entry → decrypt fails", async () => {
    const root = join(sandbox, "kc-revoked");
    const driver = new InMemoryKeyringDriver();
    const store = makeKeychainStore(root, driver);
    await store.add("alice", TEST_MNEMONIC, { bech32Prefix: "cosmos" });

    // Simulate someone revoking the keychain entry without removing the file.
    await driver.delete("junoclaw-test", "alice");

    // Now verifyAddress should fail: the keychain returns null,
    // KeychainKeyStore generates a *fresh* DEK, which won't decrypt
    // the file written under the original DEK.
    await expectThrow(() => store.verifyAddress("alice"), "decryption failed");
  });

  // ──────────────────────────────────────────
  // Multi-backend WalletStore: per-wallet dispatch
  // ──────────────────────────────────────────

  await test("WalletStore multi-backend: per-wallet dispatch by recorded backend", async () => {
    const root = join(sandbox, "multi-backend");
    const driver = new InMemoryKeyringDriver();
    const passphraseKs = new PassphraseKeyStore(root, async () => "test-pass");
    const keychainKs = new KeychainKeyStore({ driver, service: "junoclaw-test" });

    const store = new WalletStore(
      root,
      new Map<string, KeyStore>([
        ["passphrase", passphraseKs],
        ["keychain", keychainKs],
      ]),
      "passphrase"
    );

    // Two wallets, different backends.
    const a = await store.add("alice", TEST_MNEMONIC, {
      bech32Prefix: "cosmos",
      backend: "passphrase",
    });
    const b = await store.add("bob", TEST_MNEMONIC, {
      bech32Prefix: "juno",
      backend: "keychain",
    });
    if (a.backendName !== "passphrase") throw new Error(`alice: ${a.backendName}`);
    if (b.backendName !== "keychain") throw new Error(`bob: ${b.backendName}`);

    const list = await store.list();
    const aEntry = list.find((e) => e.id === "alice");
    const bEntry = list.find((e) => e.id === "bob");
    if (aEntry?.backendName !== "passphrase") throw new Error("list: alice");
    if (bEntry?.backendName !== "keychain") throw new Error("list: bob");

    // Each verifies via its own backend.
    if ((await store.verifyAddress("alice")) !== a.address) {
      throw new Error("alice verify");
    }
    if ((await store.verifyAddress("bob")) !== b.address) {
      throw new Error("bob verify");
    }

    // Keychain has exactly one entry (bob's), not two.
    if (driver.size() !== 1) {
      throw new Error(`keychain entries: ${driver.size()}`);
    }
  });

  await test("WalletStore: rejects unknown backend at add", async () => {
    const root = join(sandbox, "unknown-backend");
    const driver = new InMemoryKeyringDriver();
    const store = makeKeychainStore(root, driver);
    await expectThrow(
      () =>
        store.add("alice", TEST_MNEMONIC, {
          bech32Prefix: "cosmos",
          backend: "nonexistent",
        }),
      "not available"
    );
  });

  await test("WalletStore: backend map cannot be empty", async () => {
    let threw = false;
    try {
      new WalletStore(sandbox, new Map());
    } catch (e) {
      threw = true;
      if (!(e as Error).message.includes("cannot be empty")) {
        throw new Error(`bad error: ${(e as Error).message}`);
      }
    }
    if (!threw) throw new Error("expected throw");
  });

  await test("WalletStore: defaultBackend must be in map", async () => {
    const driver = new InMemoryKeyringDriver();
    const ks = new KeychainKeyStore({ driver, service: "x" });
    let threw = false;
    try {
      new WalletStore(sandbox, new Map([["keychain", ks]]), "passphrase");
    } catch (e) {
      threw = true;
      if (!(e as Error).message.includes("not in the backend map")) {
        throw new Error(`bad error: ${(e as Error).message}`);
      }
    }
    if (!threw) throw new Error("expected throw");
  });

  // ──────────────────────────────────────────
  // Backward compat: Phase 1 file (no backend field) reads as passphrase
  // ──────────────────────────────────────────

  await test("WalletStore: Phase 1 file (no backend field) reads as 'passphrase'", async () => {
    const root = join(sandbox, "phase1-compat");
    await fs.mkdir(root, { recursive: true });

    const passphrase = "phase1-compat-test-passphrase";

    // Write a Phase-1-style wallet file (no `backend` field).
    const passphraseKs = new PassphraseKeyStore(root, async () => passphrase);
    const phase1Store = new WalletStore(root, passphraseKs);
    const entry = await phase1Store.add("legacy", TEST_MNEMONIC, {
      bech32Prefix: "cosmos",
    });

    // Strip the `backend` field as a Phase 1 file would have looked.
    const path = join(root, "legacy.enc");
    const file = JSON.parse(await fs.readFile(path, "utf-8"));
    delete file.backend;
    await fs.writeFile(path, JSON.stringify(file, null, 2), {
      encoding: "utf-8",
      mode: 0o600,
    });

    // Re-open as a multi-backend store and verify it routes through passphrase.
    const driver = new InMemoryKeyringDriver();
    const multi = new WalletStore(
      root,
      new Map<string, KeyStore>([
        ["passphrase", new PassphraseKeyStore(root, async () => passphrase)],
        ["keychain", new KeychainKeyStore({ driver, service: "x" })],
      ]),
      "keychain"
    );

    const list = await multi.list();
    const legacy = list.find((e) => e.id === "legacy");
    if (legacy?.backendName !== "passphrase") {
      throw new Error(`legacy backend: ${legacy?.backendName}`);
    }
    if ((await multi.verifyAddress("legacy")) !== entry.address) {
      throw new Error("verify mismatch");
    }
  });

  // ──────────────────────────────────────────
  // Real native driver round-trip (DPAPI on Windows)
  // ──────────────────────────────────────────

  let nativeAvailable = false;
  let nativeError: string | null = null;
  try {
    // Probe via the same code path the production code uses.
    const ks = await import("./keychain-store.js");
    const probe = new ks.KeychainKeyStore({
      service: "junoclaw-keychain-test-probe",
    });
    const drv = await probe.driverName();
    nativeAvailable = drv !== "in-memory";
  } catch (e) {
    nativeError = (e as Error).message;
  }

  if (nativeAvailable) {
    await test("native keyring: real DPAPI / Keychain / libsecret round-trip", async () => {
      // Use a unique service name so we don't collide with anything else.
      const service = `junoclaw-test-${Date.now()}`;
      const ks = new KeychainKeyStore({ service });

      const k1 = await ks.getKey("smoke-1");
      if (k1.length !== 32) throw new Error(`bad length: ${k1.length}`);

      const k2 = await ks.getKey("smoke-1");
      if (!k1.equals(k2)) throw new Error("native get-then-get not idempotent");

      await ks.removeKey("smoke-1");

      // After remove, getKey should generate a fresh key (different bytes).
      const k3 = await ks.getKey("smoke-1");
      if (k3.equals(k1)) {
        // Almost-zero probability: 32 random bytes colliding twice.
        throw new Error("post-remove key matches pre-remove (collision?)");
      }

      // Cleanup.
      await ks.removeKey("smoke-1");
    });

    await test("native keyring: full WalletStore add/verify/remove with real backend", async () => {
      const root = join(sandbox, "native-roundtrip");
      const service = `junoclaw-test-walletstore-${Date.now()}`;
      const ks = new KeychainKeyStore({ service });
      const store = new WalletStore(root, ks);

      const wid = `native-smoke-${Date.now()}`;
      try {
        const entry = await store.add(wid, TEST_MNEMONIC, {
          bech32Prefix: "cosmos",
        });
        if (entry.address !== EXPECTED_COSMOS_ADDR) {
          throw new Error("addr mismatch");
        }
        const verified = await store.verifyAddress(wid);
        if (verified !== EXPECTED_COSMOS_ADDR) {
          throw new Error("verify mismatch");
        }
      } finally {
        // Always clean up the keychain entry, even on failure.
        try {
          await store.remove(wid);
        } catch {
          /* file may already be gone */
        }
      }
    });
  } else {
    console.log(
      `  SKIP  native keyring round-trip (driver unavailable${
        nativeError ? `: ${nativeError}` : ""
      })`
    );
    skipped++;
  }

  // ──────────────────────────────────────────
  // Cleanup
  // ──────────────────────────────────────────

  try {
    await fs.rm(sandbox, { recursive: true, force: true });
  } catch (e) {
    console.warn(`(cleanup warning: ${(e as Error).message})`);
  }

  console.log(
    `\n${passed} passed, ${failed} failed${skipped ? `, ${skipped} skipped` : ""}\n`
  );

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
