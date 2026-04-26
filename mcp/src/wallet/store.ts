/**
 * WalletStore — encrypted wallet registry for the MCP server.
 *
 * Addresses Ffern Institute audit finding C-3 (April 2026): every
 * write tool used to take `mnemonic` as a parameter, exposing it to
 * the LLM's tool-call JSON, the MCP transport, conversation logs,
 * and any downstream telemetry. The fix replaces `mnemonic` with an
 * opaque `wallet_id` handle. The mnemonic is enrolled once via the
 * `cosmos-mcp wallet add` CLI, encrypted at rest under a `KeyStore`,
 * and only decrypted inside `signFor()` for the lifetime of a single
 * signing client construction.
 *
 * On-disk layout:
 *   ~/.junoclaw/wallets/                     ($JUNOCLAW_WALLET_ROOT)
 *     .keystore.json                         (KeyStore metadata)
 *     <id>.enc                               (per-wallet encrypted file)
 *
 * The encrypted file is JSON with the AES-GCM envelope; the IV is
 * fresh per wallet, so two wallets with the same mnemonic have
 * distinct ciphertexts. Tampering with the file (or using the wrong
 * passphrase) raises a clear "decryption failed" error.
 *
 * Phase 1 ships the `PassphraseKeyStore`. Phase 2 will plug in
 * `KeychainKeyStore` (DPAPI / Keychain / libsecret) without
 * changing the WalletStore interface or the on-disk format.
 */

import { promises as fs } from "fs";
import { homedir } from "os";
import { join, resolve } from "path";

import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";

import { aesGcmDecrypt, aesGcmEncrypt } from "./crypto.js";
import {
  defaultPassphraseSource,
  PassphraseKeyStore,
  type KeyStore,
} from "./key-store.js";
import { KeychainKeyStore } from "./keychain-store.js";
import type { ChainConfig } from "../resources/chains.js";
import type { SigningContext } from "../utils/cosmos-client.js";

// ──────────────────────────────────────────────
// Types
// ──────────────────────────────────────────────

export interface WalletEntry {
  id: string;
  address: string;
  bech32Prefix: string;
  hdPath: string;
  /** Which KeyStore backend protects this wallet's DEK. */
  backendName: string;
  createdAt: string;
}

interface WalletFile {
  version: 1;
  id: string;
  /**
   * Backend that protects this wallet's data-encryption key.
   * Phase 1 files written before April 2026 may omit this field;
   * those are read as `"passphrase"`.
   */
  backend?: string;
  cipher: { algo: "aes-256-gcm"; iv_b64: string };
  ciphertext_b64: string;
  metadata: {
    address: string;
    bech32Prefix: string;
    hdPath: string;
    createdAt: string;
  };
}

const DEFAULT_HD_PATH = "m/44'/118'/0'/0/0";
const WALLET_ID_RE = /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/;

// ────────────────────────────────
// SigningPausedError (v0.x.y-security-2)
// ────────────────────────────────

/**
 * Thrown by WalletStore.signFor() when the operator-armed
 * `signing_paused` kill-switch is set.
 *
 * Designed to be distinguishable from other errors via
 * `instanceof SigningPausedError` so a downstream task scheduler
 * can treat this as "operator halt, retry later" rather than a
 * hard failure. Emitting the walletId and chainId is intentional
 * for operator forensics; both are operator-chosen identifiers,
 * not secrets.
 */
export class SigningPausedError extends Error {
  constructor(
    public readonly walletId: string,
    public readonly chainId: string
  ) {
    super(
      `signing is paused (operator-armed kill-switch). ` +
        `Refused to sign for wallet "${walletId}" on chain "${chainId}". ` +
        `Unset JUNOCLAW_SIGNING_PAUSED and restart the MCP process to resume.`
    );
    this.name = "SigningPausedError";
  }
}

/**
 * Parse JUNOCLAW_SIGNING_PAUSED from env. Fail-closed on typos:
 * any non-empty, non-"0" value is treated as paused. Canonical
 * value is "1". Unset, empty, or "0" means not paused.
 */
function parseSigningPausedEnv(): boolean {
  const val = process.env.JUNOCLAW_SIGNING_PAUSED;
  if (val === undefined || val === "" || val === "0") return false;
  return true;
}

// ──────────────────────────────────────────────
// WalletStore
// ──────────────────────────────────────────────

export class WalletStore {
  private readonly backends: Map<string, KeyStore>;
  private readonly defaultBackend: string;

  // v0.x.y-security-2: signing_paused kill-switch state. Mutated
  // via setSigningPaused(); read in signFor(). The instance-field
  // placement is intentional so tests can construct independent
  // stores with independent pause state.
  private signingPaused: boolean = false;
  private pauseSource: string | null = null;

  /**
   * Construct a `WalletStore`. The first form takes a single
   * `KeyStore` and uses its `backendName` as the default backend;
   * the second form takes a map of {backendName → KeyStore} and an
   * explicit default. Phase 1 callers and tests use the first form
   * (passphrase only); Phase 2 uses the second to register both
   * `passphrase` and `keychain` simultaneously.
   *
   * The `signing_paused` kill-switch is always initialised OFF.
   * Callers that need to pause at construction time should call
   * `setSigningPaused(true, "<source>")` immediately after. The
   * `defaultStore()` factory does this automatically from the
   * `JUNOCLAW_SIGNING_PAUSED` env var.
   */
  constructor(
    private readonly walletsDir: string,
    keyStoreOrMap: KeyStore | Map<string, KeyStore>,
    defaultBackend?: string
  ) {
    if (keyStoreOrMap instanceof Map) {
      if (keyStoreOrMap.size === 0) {
        throw new Error("WalletStore: backend map cannot be empty");
      }
      this.backends = new Map(keyStoreOrMap);
      const dflt = defaultBackend ?? Array.from(keyStoreOrMap.keys())[0];
      if (!keyStoreOrMap.has(dflt)) {
        throw new Error(
          `WalletStore: defaultBackend "${dflt}" is not in the backend map`
        );
      }
      this.defaultBackend = dflt;
    } else {
      this.backends = new Map([[keyStoreOrMap.backendName, keyStoreOrMap]]);
      this.defaultBackend = keyStoreOrMap.backendName;
    }
  }

  /** Default root: `$JUNOCLAW_WALLET_ROOT` or `~/.junoclaw/wallets`. */
  static defaultRoot(): string {
    return process.env.JUNOCLAW_WALLET_ROOT
      ? resolve(process.env.JUNOCLAW_WALLET_ROOT)
      : resolve(homedir(), ".junoclaw", "wallets");
  }

  /**
   * Construct the production singleton. Registers both the passphrase
   * and keychain backends so the MCP server can decrypt whichever
   * backend a CLI-enrolled wallet is using. Both backends are
   * lazy-initialised, so callers that never touch a keychain wallet
   * never load the native keyring library, and callers that never
   * touch a passphrase wallet never read `JUNOCLAW_WALLET_PASSPHRASE`.
   *
   * Default-backend selection for newly-added wallets:
   *   1. `JUNOCLAW_WALLET_DEFAULT_BACKEND` env var if it names a
   *      registered backend;
   *   2. `keychain` if no passphrase env var is set;
   *   3. otherwise `passphrase`.
   */
  static defaultStore(): WalletStore {
    const root = WalletStore.defaultRoot();
    const backends = new Map<string, KeyStore>([
      ["passphrase", new PassphraseKeyStore(root, defaultPassphraseSource())],
      // KeychainKeyStore loads the native keyring lazily on first
      // getKey()/removeKey(); construction is always cheap and
      // does not require the optional dependency to be installed.
      ["keychain", new KeychainKeyStore()],
    ]);

    const envDefault = process.env.JUNOCLAW_WALLET_DEFAULT_BACKEND;
    let dflt: string;
    if (envDefault && backends.has(envDefault)) {
      dflt = envDefault;
    } else if (!process.env.JUNOCLAW_WALLET_PASSPHRASE) {
      dflt = "keychain";
    } else {
      dflt = "passphrase";
    }

    const store = new WalletStore(root, backends, dflt);

    // v0.x.y-security-2: apply startup-time kill-switch from env.
    // The setSigningPaused() call logs the state transition for
    // forensics; the extra error line below is an operator tip.
    if (parseSigningPausedEnv()) {
      store.setSigningPaused(true, "env:JUNOCLAW_SIGNING_PAUSED");
      console.error(
        "[junoclaw] signing_paused=true at startup; signFor() will refuse " +
          "with SigningPausedError until JUNOCLAW_SIGNING_PAUSED is unset " +
          "(or set to '0') and the process is restarted."
      );
    }

    return store;
  }

  /** List the registered backend names (e.g. ["passphrase", "keychain"]). */
  listBackends(): string[] {
    return Array.from(this.backends.keys());
  }

  /**
   * Arm or disarm the `signing_paused` kill-switch (v0.x.y-security-2).
   * When armed, `signFor()` refuses with `SigningPausedError`; `add`,
   * `list`, `remove`, and `verifyAddress` keep working so the operator
   * can still manage the registry during an incident.
   *
   * `source` is a free-text label describing who or what flipped the
   * switch (e.g. `"env:JUNOCLAW_SIGNING_PAUSED"`,
   * `"admin-rpc:127.0.0.1"` (planned in v0.x.y-security-3), `"test"`).
   * It is logged on every state change for operator forensics; do
   * NOT put secrets in it.
   */
  setSigningPaused(paused: boolean, source: string): void {
    const prev = this.signingPaused;
    this.signingPaused = paused;
    this.pauseSource = paused ? source : null;
    if (prev !== paused) {
      console.error(
        `[junoclaw] signing_paused: ${prev} -> ${paused} (source: ${source})`
      );
    }
  }

  /**
   * Read the current `signing_paused` state. For tests, metrics,
   * and the admin RPC planned in v0.x.y-security-3. `source` is
   * null when not paused.
   */
  getSigningPaused(): { paused: boolean; source: string | null } {
    return { paused: this.signingPaused, source: this.pauseSource };
  }

  private getBackend(name: string): KeyStore {
    const ks = this.backends.get(name);
    if (!ks) {
      throw new Error(
        `wallet backend "${name}" is not available in this process. ` +
          `Registered backends: ${this.listBackends().join(", ") || "(none)"}.`
      );
    }
    return ks;
  }

  private fileBackend(file: WalletFile): string {
    return file.backend ?? "passphrase";
  }

  // ──────────────────────────────
  // Internal helpers
  // ──────────────────────────────

  private walletPath(id: string): string {
    if (!WALLET_ID_RE.test(id)) {
      throw new Error(
        `invalid wallet id "${id}": must match ${WALLET_ID_RE.source}`
      );
    }
    return join(this.walletsDir, `${id}.enc`);
  }

  private async readWalletFile(id: string): Promise<WalletFile> {
    const target = this.walletPath(id);
    let raw: string;
    try {
      raw = await fs.readFile(target, "utf-8");
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code === "ENOENT") {
        throw new Error(
          `wallet "${id}" not found. Register it with: cosmos-mcp wallet add ${id}`
        );
      }
      throw e;
    }
    let parsed: WalletFile;
    try {
      parsed = JSON.parse(raw) as WalletFile;
    } catch (e) {
      throw new Error(
        `wallet "${id}": file at ${target} is not valid JSON: ${(e as Error).message}`
      );
    }
    if (parsed.version !== 1) {
      throw new Error(
        `wallet "${id}": unsupported file version ${parsed.version}`
      );
    }
    return parsed;
  }

  // ──────────────────────────────
  // Public API
  // ──────────────────────────────

  /**
   * Register a new wallet by encrypting `mnemonic` on disk. The
   * mnemonic itself is validated by deriving a wallet from it; if
   * the mnemonic is malformed (wrong word count, bad checksum) this
   * throws before the encrypted file is written.
   *
   * The bech32 prefix is captured at add-time, so a wallet enrolled
   * for `juno` can only sign on chains with that prefix. Operators
   * who need a different prefix register a separate wallet.
   */
  async add(
    id: string,
    mnemonic: string,
    opts: {
      bech32Prefix?: string;
      hdPath?: string;
      /**
       * Which KeyStore backend protects this wallet's DEK. If
       * unspecified, the store's `defaultBackend` is used. Phase 1
       * stores have only the passphrase backend; Phase 2 stores
       * register both passphrase and keychain.
       */
      backend?: string;
    } = {}
  ): Promise<WalletEntry> {
    const bech32Prefix = opts.bech32Prefix ?? "cosmos";
    const hdPath = opts.hdPath ?? DEFAULT_HD_PATH;
    const backendName = opts.backend ?? this.defaultBackend;
    const keyStore = this.getBackend(backendName);

    const target = this.walletPath(id);

    // Refuse to overwrite.
    try {
      await fs.access(target);
      throw new Error(
        `wallet "${id}" already exists at ${target}; remove it first with: cosmos-mcp wallet rm ${id}`
      );
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
    }

    // Validate mnemonic by deriving the address.
    let address: string;
    try {
      const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
        prefix: bech32Prefix,
      });
      const accounts = await wallet.getAccounts();
      address = accounts[0].address;
    } catch (e) {
      throw new Error(`invalid mnemonic: ${(e as Error).message}`);
    }

    await fs.mkdir(this.walletsDir, { recursive: true });

    const dek = await keyStore.getKey(id);
    const envelope = aesGcmEncrypt(Buffer.from(mnemonic, "utf-8"), dek);

    const file: WalletFile = {
      version: 1,
      id,
      backend: backendName,
      cipher: { algo: "aes-256-gcm", iv_b64: envelope.iv_b64 },
      ciphertext_b64: envelope.ciphertext_b64,
      metadata: {
        address,
        bech32Prefix,
        hdPath,
        createdAt: new Date().toISOString(),
      },
    };

    await fs.writeFile(target, JSON.stringify(file, null, 2), {
      encoding: "utf-8",
      mode: 0o600,
    });

    return {
      id,
      address,
      bech32Prefix,
      hdPath,
      backendName,
      createdAt: file.metadata.createdAt,
    };
  }

  /** Enumerate all registered wallets (metadata only — never the mnemonic). */
  async list(): Promise<WalletEntry[]> {
    let names: string[];
    try {
      names = await fs.readdir(this.walletsDir);
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code === "ENOENT") return [];
      throw e;
    }

    const entries: WalletEntry[] = [];
    for (const name of names) {
      if (!name.endsWith(".enc")) continue;
      try {
        const raw = await fs.readFile(join(this.walletsDir, name), "utf-8");
        const file = JSON.parse(raw) as WalletFile;
        entries.push({
          id: file.id,
          address: file.metadata.address,
          bech32Prefix: file.metadata.bech32Prefix,
          hdPath: file.metadata.hdPath,
          backendName: this.fileBackend(file),
          createdAt: file.metadata.createdAt,
        });
      } catch {
        // Skip unreadable / malformed files; don't crash list.
      }
    }
    return entries.sort((a, b) => a.id.localeCompare(b.id));
  }

  /** Delete a wallet's encrypted file (and the matching keychain entry, if any). */
  async remove(id: string): Promise<void> {
    // Read the file first so we know which backend to clean up.
    let backendName: string | null = null;
    try {
      const file = await this.readWalletFile(id);
      backendName = this.fileBackend(file);
    } catch {
      // ignore — the unlink below will surface the canonical error
    }

    const target = this.walletPath(id);
    try {
      await fs.unlink(target);
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code === "ENOENT") {
        throw new Error(`wallet "${id}" not found`);
      }
      throw e;
    }

    if (backendName) {
      // Best-effort: if the backend isn't registered in this process
      // (e.g. keychain wallet on a process without the keychain backend
      // loaded), the file is still deleted but the keychain entry may
      // need manual cleanup. Surface a clear warning, not an error.
      const ks = this.backends.get(backendName);
      if (ks) {
        await ks.removeKey(id);
      } else {
        console.error(
          `warning: wallet "${id}" used backend "${backendName}" which is not loaded; ` +
            `the encrypted file is gone but any keychain entry must be cleaned up manually.`
        );
      }
    }
  }

  /**
   * Decrypt the wallet, derive its bech32 address, and return only
   * that address. The mnemonic is scrubbed from memory immediately.
   *
   * Useful for:
   *   - confirming the keystore can actually unlock the wallet
   *     (e.g. after a passphrase rotation),
   *   - unit/smoke tests that should not hit any RPC,
   *   - a future `cosmos-mcp wallet check <id>` subcommand.
   *
   * Throws "decryption failed" if the keystore can't unlock the file
   * (wrong passphrase, tampered ciphertext, wrong backend).
   */
  async verifyAddress(walletId: string): Promise<string> {
    const file = await this.readWalletFile(walletId);
    const keyStore = this.getBackend(this.fileBackend(file));
    const dek = await keyStore.getKey(walletId);

    let mnemonicBuf: Buffer;
    try {
      mnemonicBuf = aesGcmDecrypt(
        {
          iv_b64: file.cipher.iv_b64,
          ciphertext_b64: file.ciphertext_b64,
        },
        dek
      );
    } catch (e) {
      throw new Error(
        `wallet "${walletId}": decryption failed (wrong passphrase, tampered file, or wrong backend). ` +
          `Underlying: ${(e as Error).message}`
      );
    }

    let mnemonic = mnemonicBuf.toString("utf-8");
    try {
      const hdWallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
        prefix: file.metadata.bech32Prefix,
      });
      const accounts = await hdWallet.getAccounts();
      return accounts[0].address;
    } finally {
      mnemonicBuf.fill(0);
      mnemonic = "";
    }
  }

  /**
   * Decrypt the mnemonic for `walletId`, derive a signing wallet,
   * and connect a `SigningCosmWasmClient` on `chain`. The mnemonic
   * is held in process memory only for the duration of this call;
   * the buffer is zeroed in the `finally` block.
   *
   * The wallet's bech32 prefix must match the chain. A juno wallet
   * cannot sign for osmosis transactions — register a separate
   * wallet for each prefix.
   */
  async signFor(
    walletId: string,
    chain: ChainConfig
  ): Promise<SigningContext> {
    // v0.x.y-security-2: signing_paused kill-switch. Checked first,
    // before any file read or backend access, so a paused signer
    // (a) refuses for non-existent wallet IDs too — no enumeration
    //     signal via differentiated "paused" vs "not found" errors;
    // (b) pays no decryption or I/O cost on refused attempts;
    // (c) surfaces a clean `SigningPausedError` a downstream task
    //     scheduler can pattern-match on.
    if (this.signingPaused) {
      throw new SigningPausedError(walletId, chain.chainId);
    }

    const file = await this.readWalletFile(walletId);

    if (file.metadata.bech32Prefix !== chain.bech32Prefix) {
      throw new Error(
        `wallet "${walletId}" has bech32 prefix "${file.metadata.bech32Prefix}", ` +
          `but chain ${chain.chainId} expects "${chain.bech32Prefix}". ` +
          `Register a wallet for prefix "${chain.bech32Prefix}" first.`
      );
    }

    const keyStore = this.getBackend(this.fileBackend(file));
    const dek = await keyStore.getKey(walletId);

    let mnemonicBuf: Buffer;
    try {
      mnemonicBuf = aesGcmDecrypt(
        {
          iv_b64: file.cipher.iv_b64,
          ciphertext_b64: file.ciphertext_b64,
        },
        dek
      );
    } catch (e) {
      throw new Error(
        `wallet "${walletId}": decryption failed (wrong passphrase, tampered file, or wrong backend). ` +
          `Underlying: ${(e as Error).message}`
      );
    }

    let mnemonic = mnemonicBuf.toString("utf-8");
    try {
      const hdWallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
        prefix: chain.bech32Prefix,
      });
      const accounts = await hdWallet.getAccounts();
      const client = await SigningCosmWasmClient.connectWithSigner(
        chain.rpcEndpoint,
        hdWallet,
        { gasPrice: GasPrice.fromString(chain.gasPrice) }
      );
      return { client, address: accounts[0].address };
    } finally {
      // Best-effort scrub of the plaintext from memory. JS strings
      // are immutable so `mnemonic = ""` only drops our reference;
      // the buffer is the more useful target.
      mnemonicBuf.fill(0);
      mnemonic = "";
    }
  }
}

// ──────────────────────────────────────────────
// Module-level singleton
// ──────────────────────────────────────────────

let cachedStore: WalletStore | null = null;

/**
 * Get the process-wide default wallet store. Constructed lazily on
 * first use so a process that never touches signing tools (e.g. a
 * query-only MCP client) doesn't need to set
 * `JUNOCLAW_WALLET_PASSPHRASE`.
 */
export function getDefaultWalletStore(): WalletStore {
  if (!cachedStore) {
    cachedStore = WalletStore.defaultStore();
  }
  return cachedStore;
}

/** For tests: replace the singleton with an explicit store. */
export function _setDefaultWalletStoreForTests(store: WalletStore | null): void {
  cachedStore = store;
}
