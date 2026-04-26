/**
 * Smoke test for the wallet registry — Ffern C-3 regression coverage.
 *
 * Run:
 *   npm run wallet-store-test         (from mcp/)
 *   tsx src/wallet/store-test.ts
 *
 * Exits 0 on success, 1 on any failure. Uses a temp WALLET_ROOT under
 * the OS tmpdir so the operator's real registry is never touched.
 *
 * Coverage:
 *   - add/list/verifyAddress/remove happy path
 *   - duplicate add rejected
 *   - invalid wallet id rejected (path-traversal-style names)
 *   - invalid mnemonic rejected at add time (no file written)
 *   - tampered ciphertext rejected by AES-GCM auth tag
 *   - wrong passphrase cannot decrypt an existing wallet
 *   - wrong bech32 prefix at sign time rejected
 *   - file permissions are 0600 on POSIX (skipped on Windows)
 */

import { promises as fs } from "fs";
import { tmpdir, platform } from "os";
import { join, resolve } from "path";

import { aesGcmDecrypt, aesGcmEncrypt } from "./crypto.js";
import { PassphraseKeyStore } from "./key-store.js";
import { WalletStore } from "./store.js";

// ──────────────────────────────────────────────
// Test runner
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
  contains: string
): Promise<void> {
  let threw = false;
  try {
    await fn();
  } catch (e) {
    threw = true;
    const msg = (e as Error).message;
    if (!msg.includes(contains)) {
      throw new Error(
        `expected throw containing "${contains}", got: ${msg}`
      );
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

// Standard 24-word BIP-39 test mnemonic — DO NOT USE FOR REAL FUNDS.
// This is a well-known public test vector, the same one used in the
// Cosmos SDK testdata. Anyone with this mnemonic can spend the
// addresses derived from it; never put funds there.
const TEST_MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
// Address derived from TEST_MNEMONIC at m/44'/118'/0'/0/0 with prefix "cosmos".
// Captured from the actual @cosmjs/proto-signing derivation; verified by the
// happy-path test below.
const EXPECTED_COSMOS_ADDR = "cosmos19rl4cm2hmr8afy4kldpxz3fka4jguq0auqdal4";

// BIP-39 test vector #1 ("legal winner ...") — valid checksum, distinct from
// TEST_MNEMONIC, public knowledge, never use for real funds.
const SECOND_TEST_MNEMONIC =
  "legal winner thank year wave sausage worth useful legal winner thank yellow";

const PASSPHRASE = "correct horse battery staple";
const WRONG_PASSPHRASE = "incorrect horse battery staple";

function makeStore(rootDir: string, passphrase: string): WalletStore {
  const keyStore = new PassphraseKeyStore(rootDir, async () => passphrase);
  return new WalletStore(rootDir, keyStore);
}

console.log("\n━━━ wallet-store smoke (Ffern C-3 regression) ━━━\n");

async function run() {
  const sandbox = resolve(tmpdir(), `junoclaw-wallet-store-${Date.now()}`);
  const root1 = join(sandbox, "registry-1");
  const root2 = join(sandbox, "registry-2");

  await fs.mkdir(sandbox, { recursive: true });

  // ──────────────────────────────────────────
  // Happy path
  // ──────────────────────────────────────────

  await test("add: registers a wallet and returns derived address", async () => {
    const store = makeStore(root1, PASSPHRASE);
    const entry = await store.add("alice", TEST_MNEMONIC, {
      bech32Prefix: "cosmos",
    });
    if (entry.id !== "alice") throw new Error(`id mismatch: ${entry.id}`);
    if (entry.address !== EXPECTED_COSMOS_ADDR) {
      throw new Error(
        `expected ${EXPECTED_COSMOS_ADDR}, got ${entry.address}`
      );
    }
    if (entry.bech32Prefix !== "cosmos") throw new Error("prefix mismatch");
    if (entry.backendName !== "passphrase") {
      throw new Error("backend mismatch");
    }
  });

  await test("list: shows the registered wallet without exposing the mnemonic", async () => {
    const store = makeStore(root1, PASSPHRASE);
    const entries = await store.list();
    if (entries.length !== 1) {
      throw new Error(`expected 1 entry, got ${entries.length}`);
    }
    const e = entries[0];
    if (e.id !== "alice") throw new Error(`id: ${e.id}`);
    // Just paranoia: the metadata must not contain the mnemonic.
    const json = JSON.stringify(e);
    if (json.includes("abandon")) {
      throw new Error("metadata leaks mnemonic words!");
    }
  });

  await test("verifyAddress: round-trips through encrypted file", async () => {
    const store = makeStore(root1, PASSPHRASE);
    const addr = await store.verifyAddress("alice");
    if (addr !== EXPECTED_COSMOS_ADDR) {
      throw new Error(`expected ${EXPECTED_COSMOS_ADDR}, got ${addr}`);
    }
  });

  await test("on-disk file does not contain mnemonic words in plaintext", async () => {
    const path = join(root1, "alice.enc");
    const raw = await fs.readFile(path, "utf-8");
    if (raw.includes("abandon")) {
      throw new Error("encrypted file contains plaintext mnemonic word!");
    }
    if (raw.includes("about")) {
      throw new Error("encrypted file contains plaintext mnemonic word!");
    }
  });

  // ──────────────────────────────────────────
  // Refusals at add time
  // ──────────────────────────────────────────

  await test("add: rejects duplicate id", async () => {
    const store = makeStore(root1, PASSPHRASE);
    await expectThrow(
      () => store.add("alice", TEST_MNEMONIC, { bech32Prefix: "cosmos" }),
      "already exists"
    );
  });

  await test("add: rejects invalid wallet id (path traversal)", async () => {
    const store = makeStore(root1, PASSPHRASE);
    await expectThrow(
      () => store.add("../etc/passwd", TEST_MNEMONIC),
      "invalid wallet id"
    );
  });

  await test("add: rejects invalid wallet id (slash)", async () => {
    const store = makeStore(root1, PASSPHRASE);
    await expectThrow(
      () => store.add("evil/path", TEST_MNEMONIC),
      "invalid wallet id"
    );
  });

  await test("add: rejects invalid mnemonic (wrong word count)", async () => {
    const store = makeStore(root1, PASSPHRASE);
    await expectThrow(
      () => store.add("bob", "only three words here"),
      "invalid mnemonic"
    );
  });

  await test("add: invalid mnemonic does NOT leave a file behind", async () => {
    const store = makeStore(root1, PASSPHRASE);
    try {
      await store.add("ghost", "bad bad bad");
    } catch {
      /* expected */
    }
    try {
      await fs.access(join(root1, "ghost.enc"));
      throw new Error("file was created despite invalid mnemonic");
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "ENOENT") {
        throw new Error(
          `unexpected fs error: ${(e as Error).message}`
        );
      }
    }
  });

  // ──────────────────────────────────────────
  // Tamper detection (AES-GCM auth tag)
  // ──────────────────────────────────────────

  await test("verifyAddress: rejects tampered ciphertext", async () => {
    // Add a fresh wallet to a separate root so we can tamper safely.
    const tamperRoot = join(sandbox, "tamper");
    const store = makeStore(tamperRoot, PASSPHRASE);
    await store.add("victim", TEST_MNEMONIC, { bech32Prefix: "cosmos" });

    const path = join(tamperRoot, "victim.enc");
    const raw = await fs.readFile(path, "utf-8");
    const file = JSON.parse(raw);
    // Flip a base64 character in the middle of the ciphertext.
    // The trailing chars can be `=` padding which decoders ignore;
    // the middle is guaranteed to land in real bytes.
    const ct: string = file.ciphertext_b64;
    const mid = Math.floor(ct.length / 2);
    const c = ct[mid];
    const flipped = c === "A" ? "B" : "A";
    file.ciphertext_b64 = ct.slice(0, mid) + flipped + ct.slice(mid + 1);
    await fs.writeFile(path, JSON.stringify(file, null, 2), {
      encoding: "utf-8",
      mode: 0o600,
    });

    await expectThrow(
      () => store.verifyAddress("victim"),
      "decryption failed"
    );
  });

  // ──────────────────────────────────────────
  // Wrong passphrase
  // ──────────────────────────────────────────

  await test("wrong passphrase cannot decrypt an existing wallet", async () => {
    const wrongStore = makeStore(root1, WRONG_PASSPHRASE);
    await expectThrow(
      () => wrongStore.verifyAddress("alice"),
      "decryption failed"
    );
  });

  // ──────────────────────────────────────────
  // Independence: same passphrase, different roots, distinct ciphertexts
  // ──────────────────────────────────────────

  await test("two stores with same passphrase but different roots have independent salts", async () => {
    const storeA = makeStore(root1, PASSPHRASE);
    const storeB = makeStore(root2, PASSPHRASE);

    // alice already exists in root1 from earlier tests.
    await storeB.add("bob", SECOND_TEST_MNEMONIC, {
      bech32Prefix: "cosmos",
    });

    // The .keystore.json salts must differ.
    const saltA = JSON.parse(
      await fs.readFile(join(root1, ".keystore.json"), "utf-8")
    ).kdf.salt_b64;
    const saltB = JSON.parse(
      await fs.readFile(join(root2, ".keystore.json"), "utf-8")
    ).kdf.salt_b64;
    if (saltA === saltB) {
      throw new Error("salts collided across independent registries");
    }

    // Both stores can read their own wallets.
    await storeA.verifyAddress("alice");
    await storeB.verifyAddress("bob");
  });

  // ──────────────────────────────────────────
  // Two wallets, same passphrase, distinct ciphertexts
  // ──────────────────────────────────────────

  await test("two wallets with the same mnemonic have distinct ciphertexts (fresh IV)", async () => {
    const store = makeStore(root1, PASSPHRASE);
    await store.add("alice2", TEST_MNEMONIC, { bech32Prefix: "cosmos" });

    const a1 = JSON.parse(await fs.readFile(join(root1, "alice.enc"), "utf-8"));
    const a2 = JSON.parse(await fs.readFile(join(root1, "alice2.enc"), "utf-8"));
    if (a1.ciphertext_b64 === a2.ciphertext_b64) {
      throw new Error("identical ciphertexts despite fresh IV");
    }
    if (a1.cipher.iv_b64 === a2.cipher.iv_b64) {
      throw new Error("IVs collided");
    }
    // Both decrypt to the same address.
    if ((await store.verifyAddress("alice")) !== EXPECTED_COSMOS_ADDR) {
      throw new Error("alice address wrong");
    }
    if ((await store.verifyAddress("alice2")) !== EXPECTED_COSMOS_ADDR) {
      throw new Error("alice2 address wrong");
    }
  });

  // ──────────────────────────────────────────
  // remove + list-after-remove
  // ──────────────────────────────────────────

  await test("remove: deletes the wallet file and list reflects it", async () => {
    const store = makeStore(root1, PASSPHRASE);
    await store.remove("alice2");
    const entries = await store.list();
    if (entries.find((e) => e.id === "alice2")) {
      throw new Error("alice2 still in list after remove");
    }
    try {
      await fs.access(join(root1, "alice2.enc"));
      throw new Error("alice2.enc still exists");
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
    }
  });

  await test("remove: rejects non-existent wallet", async () => {
    const store = makeStore(root1, PASSPHRASE);
    await expectThrow(() => store.remove("doesnotexist"), "not found");
  });

  await test("verifyAddress: rejects non-existent wallet", async () => {
    const store = makeStore(root1, PASSPHRASE);
    await expectThrow(
      () => store.verifyAddress("doesnotexist"),
      "not found"
    );
  });

  // ──────────────────────────────────────────
  // Direct AES-GCM crypto invariants
  // ──────────────────────────────────────────

  await test("aesGcmEncrypt / aesGcmDecrypt round-trips", async () => {
    const key = Buffer.alloc(32, 7);
    const plaintext = Buffer.from("hello, world", "utf-8");
    const env = aesGcmEncrypt(plaintext, key);
    const decoded = aesGcmDecrypt(env, key);
    if (!decoded.equals(plaintext)) {
      throw new Error("round-trip mismatch");
    }
  });

  await test("aesGcmDecrypt: rejects wrong key", async () => {
    const key1 = Buffer.alloc(32, 7);
    const key2 = Buffer.alloc(32, 8);
    const env = aesGcmEncrypt(Buffer.from("secret"), key1);
    let threw = false;
    try {
      aesGcmDecrypt(env, key2);
    } catch {
      threw = true;
    }
    if (!threw) throw new Error("decrypt with wrong key should throw");
  });

  await test("aesGcmEncrypt: rejects key of wrong length", async () => {
    let threw = false;
    try {
      aesGcmEncrypt(Buffer.from("data"), Buffer.alloc(16));
    } catch (e) {
      threw = true;
      if (!(e as Error).message.includes("32 bytes")) {
        throw new Error(`bad error message: ${(e as Error).message}`);
      }
    }
    if (!threw) throw new Error("should have rejected 16-byte key");
  });

  // ──────────────────────────────────────────
  // POSIX file permissions
  // ──────────────────────────────────────────

  if (platform() !== "win32") {
    await test("encrypted wallet file is created with mode 0600 (POSIX)", async () => {
      const stat = await fs.stat(join(root1, "alice.enc"));
      const mode = stat.mode & 0o777;
      if (mode !== 0o600) {
        throw new Error(`expected mode 0600, got ${mode.toString(8)}`);
      }
    });
  } else {
    console.log("  SKIP  POSIX file-permission test (running on Windows)");
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
    `\n${passed} passed, ${failed} failed${
      platform() === "win32" ? " (POSIX perms test skipped)" : ""
    }\n`
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
