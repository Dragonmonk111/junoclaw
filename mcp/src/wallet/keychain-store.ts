/**
 * keychain-store — OS keychain backend for the wallet registry.
 *
 * Per-wallet 32-byte data-encryption keys live in the OS credential
 * manager:
 *   Windows:  Credential Manager (DPAPI-protected)
 *   macOS:    Keychain Services
 *   Linux:    libsecret / Secret Service
 *
 * On-disk format is unchanged from the passphrase backend: the same
 * AES-256-GCM envelope, only the DEK source differs. The wallet file
 * records `backend: "keychain"` so `WalletStore` knows which `KeyStore`
 * to consult on decrypt.
 *
 * The native binding (`@napi-rs/keyring`) is loaded lazily via
 * dynamic `import()`, so a process that never registers a keychain
 * backend never pulls in the native library. The package is declared
 * as an `optionalDependencies` entry so platforms without a prebuilt
 * binary (and operators who don't want the keychain backend at all)
 * can run `npm install --no-optional` and skip it.
 *
 * Threat model: a keychain-stored DEK is protected by the OS user
 * session. On Windows, DPAPI binds the secret to the user's logon
 * credentials; another user on the same machine cannot read it.
 * On macOS, Keychain Services prompts the user to authorise other
 * apps reading the entry. This is a meaningful step up from the
 * passphrase backend (where the operator must protect a long-lived
 * passphrase manually).
 */

import { randomBytes } from "crypto";

import { KEY_LEN } from "./crypto.js";
import type { KeyStore } from "./key-store.js";

// ──────────────────────────────────────────────
// Driver abstraction (real + mock)
// ──────────────────────────────────────────────

/**
 * Thin abstraction over a keyring backend. Used so the tests can
 * substitute an in-memory implementation without loading the native
 * library, and so the production code can swap libraries (e.g.
 * @napi-rs/keyring → @zowe/secrets) without a rewrite.
 */
export interface KeyringDriver {
  readonly name: string;
  get(service: string, account: string): Promise<string | null>;
  set(service: string, account: string, password: string): Promise<void>;
  delete(service: string, account: string): Promise<void>;
}

/**
 * In-memory keyring driver for tests. The "keychain" is just a Map
 * keyed by `${service}::${account}`. Behaviour matches the real
 * driver semantics: `get` returns null for missing entries, `set`
 * overwrites, `delete` is idempotent (no-op if missing).
 */
export class InMemoryKeyringDriver implements KeyringDriver {
  readonly name = "in-memory";
  private store = new Map<string, string>();

  private k(service: string, account: string): string {
    return `${service}::${account}`;
  }

  async get(service: string, account: string): Promise<string | null> {
    return this.store.get(this.k(service, account)) ?? null;
  }
  async set(service: string, account: string, password: string): Promise<void> {
    this.store.set(this.k(service, account), password);
  }
  async delete(service: string, account: string): Promise<void> {
    this.store.delete(this.k(service, account));
  }

  /** Inspection helper for tests. */
  size(): number {
    return this.store.size;
  }
}

/**
 * Adapter for `@napi-rs/keyring`. Loaded lazily via
 * `loadNativeKeyringDriver()` so the native module is only required
 * when actually used.
 */
async function loadNativeKeyringDriver(): Promise<KeyringDriver> {
  let mod: Record<string, unknown>;
  try {
    // dynamic import so unrelated MCP runs never touch the native lib.
    // The package is declared in `optionalDependencies` and may legitimately
    // be absent on some platforms / install profiles; the runtime catch
    // below surfaces a clear actionable error in that case.
    // @ts-ignore — optional dependency, intentionally unresolved at typecheck time
    mod = (await import("@napi-rs/keyring")) as Record<string, unknown>;
  } catch (e) {
    throw new Error(
      "keychain backend requires the optional dependency `@napi-rs/keyring`. " +
        "Install it with `npm install @napi-rs/keyring` from the mcp/ directory, " +
        `then retry. Underlying: ${(e as Error).message}`
    );
  }

  const EntryCls = (mod as { Entry?: unknown }).Entry as
    | (new (service: string, account: string) => {
        getPassword(): string | null;
        setPassword(password: string): void;
        deletePassword?: () => void;
        delete?: () => void;
      })
    | undefined;
  if (typeof EntryCls !== "function") {
    throw new Error(
      "keychain backend: `@napi-rs/keyring` did not export a usable `Entry` " +
        "constructor. Loaded module keys: " +
        Object.keys(mod).join(", ")
    );
  }

  return {
    name: "@napi-rs/keyring",
    async get(service, account) {
      const e = new EntryCls(service, account);
      try {
        return e.getPassword();
      } catch {
        // Most platforms throw "no entry found" rather than returning null.
        return null;
      }
    },
    async set(service, account, password) {
      const e = new EntryCls(service, account);
      e.setPassword(password);
    },
    async delete(service, account) {
      const e = new EntryCls(service, account);
      const del = e.deletePassword ?? e.delete;
      if (typeof del === "function") {
        try {
          del.call(e);
        } catch {
          // idempotent: deleting a non-existent entry must not throw
        }
      }
    },
  };
}

// ──────────────────────────────────────────────
// KeychainKeyStore
// ──────────────────────────────────────────────

const DEFAULT_KEYCHAIN_SERVICE = "junoclaw-cosmos-mcp";

export interface KeychainKeyStoreOptions {
  /** Service name in the OS credential manager. Defaults to "junoclaw-cosmos-mcp". */
  service?: string;
  /** Override the keyring driver (e.g. for tests). Defaults to the native driver. */
  driver?: KeyringDriver;
}

/**
 * KeyStore backed by the OS credential manager. Each wallet has its
 * own random 32-byte DEK; the DEK is generated on first call to
 * `getKey(id)` and stored in the keychain at
 * (`service`, `walletId`). Subsequent calls retrieve the same DEK.
 */
export class KeychainKeyStore implements KeyStore {
  readonly backendName = "keychain";
  private readonly service: string;
  private driver: KeyringDriver | null;
  private driverPromise: Promise<KeyringDriver> | null;

  constructor(opts: KeychainKeyStoreOptions = {}) {
    this.service = opts.service ?? DEFAULT_KEYCHAIN_SERVICE;
    this.driver = opts.driver ?? null;
    this.driverPromise = null;
  }

  private async getDriver(): Promise<KeyringDriver> {
    if (this.driver) return this.driver;
    if (!this.driverPromise) {
      this.driverPromise = loadNativeKeyringDriver().then((d) => {
        this.driver = d;
        return d;
      });
    }
    return this.driverPromise;
  }

  /** Inspection: which driver is in use. Useful for `wallet list` output and tests. */
  async driverName(): Promise<string> {
    const d = await this.getDriver();
    return d.name;
  }

  async getKey(walletId: string): Promise<Buffer> {
    const driver = await this.getDriver();

    const existing = await driver.get(this.service, walletId);
    if (existing !== null) {
      const key = Buffer.from(existing, "base64");
      if (key.length !== KEY_LEN) {
        throw new Error(
          `keychain entry for "${walletId}" has wrong length ` +
            `(${key.length} bytes, expected ${KEY_LEN}). ` +
            `Possibly corrupted; remove and re-enrol the wallet.`
        );
      }
      return key;
    }

    // First use: generate a fresh random DEK and persist it.
    const fresh = randomBytes(KEY_LEN);
    await driver.set(this.service, walletId, fresh.toString("base64"));
    return fresh;
  }

  async removeKey(walletId: string): Promise<void> {
    const driver = await this.getDriver();
    await driver.delete(this.service, walletId);
  }
}

// ──────────────────────────────────────────────
// Factory helper
// ──────────────────────────────────────────────

/**
 * Construct a `KeychainKeyStore` using the native driver. Throws a
 * descriptive error (caught by the CLI) if the native dependency is
 * missing or unusable. Pre-flights the driver load so the failure
 * surfaces at construction time rather than on the first `getKey`.
 */
export async function keychainKeyStore(
  opts: KeychainKeyStoreOptions = {}
): Promise<KeychainKeyStore> {
  const ks = new KeychainKeyStore(opts);
  // Pre-load the driver so we fail fast with a clear message if
  // @napi-rs/keyring is missing.
  await ks.driverName();
  return ks;
}
