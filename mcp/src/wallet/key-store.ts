/**
 * key-store — abstraction over "where the data encryption key lives".
 *
 * Phase 1 ships only `PassphraseKeyStore`: a single 32-byte master key
 * derived from an operator-supplied passphrase via scrypt (N=2^17, r=8,
 * p=1 — OWASP 2024 baseline). The same master key is used to encrypt
 * every wallet on disk; freshness comes from the per-wallet random IV
 * baked into the AES-GCM envelope.
 *
 * Phase 2 will add `KeychainKeyStore` (DPAPI / macOS Keychain /
 * libsecret) behind the same interface. Wallet files written by Phase 1
 * remain readable as long as the operator still has the passphrase.
 *
 * Threat-model note: scrypt + AES-GCM is the same primitive your
 * password manager and SSH `id_ed25519` use. The protection is only as
 * strong as the passphrase. If the operator ships
 * `JUNOCLAW_WALLET_PASSPHRASE=hunter2` in a world-readable `.env`,
 * encryption-at-rest becomes ceremonial. Phase 2's keychain backend
 * removes the operator-managed passphrase from the picture entirely
 * on Windows (DPAPI).
 */

import { promises as fs } from "fs";
import { join } from "path";
import { scryptSync, randomBytes } from "crypto";

import { KEY_LEN } from "./crypto.js";

export interface KeyStore {
  /**
   * Get the 32-byte data-encryption key for `walletId`. For
   * `PassphraseKeyStore` the same master key is returned regardless
   * of `walletId`. For `KeychainKeyStore` (Phase 2) each wallet has
   * its own keychain entry.
   */
  getKey(walletId: string): Promise<Buffer>;

  /**
   * Forget the key for `walletId`. No-op for `PassphraseKeyStore`
   * (the master key serves all wallets). Removes the keychain entry
   * for `KeychainKeyStore`.
   */
  removeKey(walletId: string): Promise<void>;

  /** Display name for `wallet list`. */
  readonly backendName: string;
}

// ──────────────────────────────────────────────
// PassphraseKeyStore — scrypt-derived master key
// ──────────────────────────────────────────────

const KEYSTORE_FILENAME = ".keystore.json";

// scrypt cost parameters — OWASP 2024 baseline.
// 128 * r * N bytes of RAM = 128 * 8 * 131_072 = 128 MiB.
const SCRYPT_N = 131_072; // 2^17
const SCRYPT_R = 8;
const SCRYPT_P = 1;
const SCRYPT_MAXMEM = 256 * 1024 * 1024; // 256 MiB
const SALT_LEN = 32;
const MIN_PASSPHRASE_LEN = 8;

interface PassphraseKeystoreFile {
  version: 1;
  backend: "passphrase";
  kdf: {
    algo: "scrypt";
    N: number;
    r: number;
    p: number;
    salt_b64: string;
  };
}

export class PassphraseKeyStore implements KeyStore {
  readonly backendName = "passphrase";
  private masterKey: Buffer | null = null;
  private initPromise: Promise<void> | null = null;

  /**
   * @param walletsDir directory where `.keystore.json` lives
   * @param passphraseSource async function returning the operator passphrase
   */
  constructor(
    private readonly walletsDir: string,
    private readonly passphraseSource: () => Promise<string>
  ) {}

  /**
   * Initialise the keystore: read or create `.keystore.json`, derive
   * the master key. Idempotent and safe to call concurrently — the
   * promise is memoised.
   */
  init(): Promise<void> {
    if (!this.initPromise) {
      this.initPromise = this.doInit();
    }
    return this.initPromise;
  }

  private async doInit(): Promise<void> {
    const path = join(this.walletsDir, KEYSTORE_FILENAME);

    let file: PassphraseKeystoreFile;
    try {
      const raw = await fs.readFile(path, "utf-8");
      const parsed = JSON.parse(raw);
      if (parsed.version !== 1 || parsed.backend !== "passphrase") {
        throw new Error(
          `unsupported keystore at ${path}: version=${parsed.version} backend=${parsed.backend}`
        );
      }
      file = parsed as PassphraseKeystoreFile;
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
      // First-time init.
      const salt = randomBytes(SALT_LEN);
      file = {
        version: 1,
        backend: "passphrase",
        kdf: {
          algo: "scrypt",
          N: SCRYPT_N,
          r: SCRYPT_R,
          p: SCRYPT_P,
          salt_b64: salt.toString("base64"),
        },
      };
      await fs.mkdir(this.walletsDir, { recursive: true });
      await fs.writeFile(path, JSON.stringify(file, null, 2), {
        encoding: "utf-8",
        mode: 0o600,
      });
    }

    const passphrase = await this.passphraseSource();
    if (typeof passphrase !== "string" || passphrase.length < MIN_PASSPHRASE_LEN) {
      throw new Error(
        `wallet passphrase must be at least ${MIN_PASSPHRASE_LEN} characters`
      );
    }

    const salt = Buffer.from(file.kdf.salt_b64, "base64");
    this.masterKey = scryptSync(passphrase, salt, KEY_LEN, {
      N: file.kdf.N,
      r: file.kdf.r,
      p: file.kdf.p,
      maxmem: SCRYPT_MAXMEM,
    });
  }

  async getKey(_walletId: string): Promise<Buffer> {
    await this.init();
    return this.masterKey!;
  }

  async removeKey(_walletId: string): Promise<void> {
    // No-op: the master key serves all wallets. Removing a single
    // wallet's encrypted file is enough to forget it.
  }
}

// ──────────────────────────────────────────────
// Default passphrase source (env var / file)
// ──────────────────────────────────────────────

/**
 * The default passphrase source: prefer `JUNOCLAW_WALLET_PASSPHRASE`,
 * fall back to the file at `JUNOCLAW_WALLET_PASSPHRASE_FILE`. Both
 * trim trailing whitespace. Throws a clear error if neither is set.
 *
 * For the MCP server (which runs as a non-interactive subprocess of
 * Claude Desktop / Cursor / etc.), this is the only sane default. The
 * CLI subcommand `cosmos-mcp wallet add` uses the same source so the
 * passphrase is never typed twice.
 */
export function defaultPassphraseSource(): () => Promise<string> {
  return async () => {
    if (process.env.JUNOCLAW_WALLET_PASSPHRASE) {
      return process.env.JUNOCLAW_WALLET_PASSPHRASE;
    }
    const fileEnv = process.env.JUNOCLAW_WALLET_PASSPHRASE_FILE;
    if (fileEnv) {
      try {
        const raw = await fs.readFile(fileEnv, "utf-8");
        return raw.replace(/\r?\n$/, "").trim();
      } catch (e) {
        throw new Error(
          `JUNOCLAW_WALLET_PASSPHRASE_FILE=${fileEnv}: cannot read ` +
            `(${(e as Error).message})`
        );
      }
    }
    throw new Error(
      "wallet passphrase not provided: set JUNOCLAW_WALLET_PASSPHRASE " +
        "or JUNOCLAW_WALLET_PASSPHRASE_FILE before starting the MCP server " +
        "or running `cosmos-mcp wallet ...` commands."
    );
  };
}
