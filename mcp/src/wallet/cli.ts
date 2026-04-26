/**
 * cosmos-mcp wallet — CLI for the wallet registry.
 *
 * Subcommands:
 *   wallet add <id> [--prefix <bech32>] [--hd-path <path>]
 *                   [--mnemonic <m> | --mnemonic-stdin
 *                    | --mnemonic-env <VAR>
 *                    | --mnemonic-file <path>]
 *   wallet list
 *   wallet rm <id>
 *
 * The mnemonic is intentionally hard to pass on the command line —
 * `--mnemonic <m>` puts it in shell history and `ps -ef` output, and
 * is therefore discouraged. The default for `wallet add` (no source
 * flag) is `--mnemonic-stdin`, which reads a single line from stdin
 * with no echo if a TTY is attached.
 *
 * The passphrase that protects the encrypted store is supplied via
 * `JUNOCLAW_WALLET_PASSPHRASE` (or `_PASSPHRASE_FILE`). The CLI
 * never asks for it twice.
 */

import { promises as fs } from "fs";
import { createInterface } from "readline";

import { getChain } from "../resources/chains.js";
import {
  defaultPassphraseSource,
  PassphraseKeyStore,
  type KeyStore,
} from "./key-store.js";
import { keychainKeyStore } from "./keychain-store.js";
import { WalletStore, type WalletEntry } from "./store.js";

interface ParsedArgs {
  positional: string[];
  flags: Map<string, string | true>;
}

function parseArgs(args: readonly string[]): ParsedArgs {
  const positional: string[] = [];
  const flags = new Map<string, string | true>();
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a.startsWith("--")) {
      const eq = a.indexOf("=");
      if (eq !== -1) {
        flags.set(a.slice(2, eq), a.slice(eq + 1));
      } else if (i + 1 < args.length && !args[i + 1].startsWith("--")) {
        flags.set(a.slice(2), args[++i]);
      } else {
        flags.set(a.slice(2), true);
      }
    } else {
      positional.push(a);
    }
  }
  return { positional, flags };
}

function printUsage(): void {
  console.error(
    [
      "cosmos-mcp wallet — encrypted wallet registry (Ffern C-3)",
      "",
      "Usage:",
      "  cosmos-mcp wallet add <id> [--backend passphrase|keychain]",
      "                              [--prefix <bech32>] [--hd-path <path>]",
      "                              [--mnemonic-stdin | --mnemonic-env <VAR>",
      "                               | --mnemonic-file <path> | --mnemonic <m>]",
      "                              [--chain <chainId>]",
      "  cosmos-mcp wallet list",
      "  cosmos-mcp wallet rm <id>",
      "",
      "Backends:",
      "  passphrase  (default if JUNOCLAW_WALLET_PASSPHRASE is set, or as a fallback)",
      "              scrypt + AES-256-GCM under an operator passphrase. Portable.",
      "  keychain    OS credential manager (DPAPI / Keychain / libsecret) holds a",
      "              per-wallet random DEK. No passphrase to manage. Requires the",
      "              optional dependency `@napi-rs/keyring` (installed by default;",
      "              re-add with `npm install @napi-rs/keyring` if --no-optional was used).",
      "",
      "Environment:",
      "  JUNOCLAW_WALLET_ROOT             override storage dir (default: ~/.junoclaw/wallets)",
      "  JUNOCLAW_WALLET_PASSPHRASE       passphrase for the passphrase backend",
      "  JUNOCLAW_WALLET_PASSPHRASE_FILE  read passphrase from a file",
      "  JUNOCLAW_WALLET_DEFAULT_BACKEND  default backend if --backend isn't passed",
      "                                   ('passphrase' or 'keychain')",
      "",
      "Mnemonic source priority for `wallet add`:",
      "  --mnemonic <m>            INSECURE (shell history); use only for tests",
      "  --mnemonic-env <VAR>      read from env var (e.g. existing WAVS_OPERATOR_MNEMONIC)",
      "  --mnemonic-file <path>    read from a file (no trailing newline)",
      "  --mnemonic-stdin          read one line from stdin (default)",
      "",
      "  --chain <chainId>         shortcut for --prefix (looks up bech32Prefix)",
    ].join("\n")
  );
}

// ──────────────────────────────
// CLI store builder
// ──────────────────────────────

/**
 * Build a `WalletStore` for CLI use. Registers as many backends as
 * the environment supports:
 *   - `passphrase` is always available; lazy-init defers actually
 *     reading the passphrase until a wallet operation needs it.
 *   - `keychain` is registered only if `@napi-rs/keyring` loads;
 *     otherwise the slot is left empty (operations targeting an
 *     existing keychain wallet will surface a clear error).
 *
 * The default backend is chosen as:
 *   1. `JUNOCLAW_WALLET_DEFAULT_BACKEND` if set and registered;
 *   2. `keychain` if available;
 *   3. otherwise `passphrase`.
 *
 * The passphrase backend stays the safe default for `add` because
 * keychain prebuilts may not exist on every operator's platform.
 */
async function buildCliWalletStore(): Promise<WalletStore> {
  const root = WalletStore.defaultRoot();
  const backends = new Map<string, KeyStore>();

  // Always register passphrase — lazy-init defers reading the env var
  // until an operation that actually needs the passphrase fires.
  backends.set(
    "passphrase",
    new PassphraseKeyStore(root, defaultPassphraseSource())
  );

  // Try keychain. Failure is non-fatal: the CLI can still serve
  // passphrase-backed wallets and emit a clear error if a keychain
  // operation is attempted.
  try {
    const ks = await keychainKeyStore();
    backends.set("keychain", ks);
  } catch {
    // @napi-rs/keyring not installed or platform unsupported; fine.
  }

  // Pick the default.
  const envDefault = process.env.JUNOCLAW_WALLET_DEFAULT_BACKEND;
  let dflt: string;
  if (envDefault && backends.has(envDefault)) {
    dflt = envDefault;
  } else if (backends.has("keychain") && !process.env.JUNOCLAW_WALLET_PASSPHRASE) {
    // Operator hasn't supplied a passphrase: prefer keychain if available.
    dflt = "keychain";
  } else {
    dflt = "passphrase";
  }

  return new WalletStore(root, backends, dflt);
}

// ──────────────────────────────────────────────
// Mnemonic source resolution
// ──────────────────────────────────────────────

async function readStdinLine(): Promise<string> {
  const rl = createInterface({ input: process.stdin, terminal: false });
  for await (const line of rl) {
    rl.close();
    return line;
  }
  return "";
}

async function resolveMnemonic(flags: Map<string, string | true>): Promise<string> {
  const m = flags.get("mnemonic");
  if (typeof m === "string") return m.trim();

  const envVar = flags.get("mnemonic-env");
  if (typeof envVar === "string") {
    const v = process.env[envVar];
    if (!v) {
      throw new Error(`--mnemonic-env ${envVar}: env var is empty or unset`);
    }
    return v.trim();
  }

  const filePath = flags.get("mnemonic-file");
  if (typeof filePath === "string") {
    try {
      const raw = await fs.readFile(filePath, "utf-8");
      return raw.replace(/\r?\n$/, "").trim();
    } catch (e) {
      throw new Error(
        `--mnemonic-file ${filePath}: cannot read (${(e as Error).message})`
      );
    }
  }

  // Default: stdin (matches --mnemonic-stdin flag).
  if (process.stdin.isTTY) {
    process.stderr.write("Enter mnemonic (one line, will be encrypted): ");
  }
  const line = await readStdinLine();
  if (!line) {
    throw new Error("no mnemonic provided on stdin");
  }
  return line.trim();
}

// ──────────────────────────────────────────────
// Output helpers
// ──────────────────────────────────────────────

function formatEntryLine(e: WalletEntry): string {
  return `  ${e.id.padEnd(24)}  ${e.address.padEnd(46)}  prefix=${e.bech32Prefix}  backend=${e.backendName}  created=${e.createdAt}`;
}

// ──────────────────────────────────────────────
// Subcommand dispatch
// ──────────────────────────────────────────────

async function runAdd(args: ParsedArgs, store: WalletStore): Promise<void> {
  const id = args.positional[0];
  if (!id) {
    printUsage();
    throw new Error("wallet add: missing <id>");
  }

  // Determine bech32Prefix: explicit --prefix, or via --chain lookup, or default cosmos.
  let bech32Prefix: string | undefined;
  const prefixFlag = args.flags.get("prefix");
  if (typeof prefixFlag === "string") {
    bech32Prefix = prefixFlag;
  } else {
    const chainFlag = args.flags.get("chain");
    if (typeof chainFlag === "string") {
      const chain = getChain(chainFlag);
      if (!chain) {
        throw new Error(`--chain ${chainFlag}: unknown chain id`);
      }
      bech32Prefix = chain.bech32Prefix;
    }
  }

  const hdPathFlag = args.flags.get("hd-path");
  const hdPath = typeof hdPathFlag === "string" ? hdPathFlag : undefined;

  const backendFlag = args.flags.get("backend");
  const backend =
    typeof backendFlag === "string" ? backendFlag : undefined;
  if (
    backend !== undefined &&
    backend !== "passphrase" &&
    backend !== "keychain"
  ) {
    throw new Error(
      `--backend must be 'passphrase' or 'keychain', got '${backend}'`
    );
  }
  if (backend === "keychain" && !store.listBackends().includes("keychain")) {
    throw new Error(
      "--backend keychain requested but @napi-rs/keyring is not installed. " +
        "Run: npm install @napi-rs/keyring  (from the mcp/ directory) and retry."
    );
  }

  const mnemonic = await resolveMnemonic(args.flags);
  if (!mnemonic) {
    throw new Error("wallet add: empty mnemonic");
  }

  const entry = await store.add(id, mnemonic, { bech32Prefix, hdPath, backend });

  console.log(`✓ wallet "${entry.id}" added`);
  console.log(`  address: ${entry.address}`);
  console.log(`  prefix:  ${entry.bech32Prefix}`);
  console.log(`  hd-path: ${entry.hdPath}`);
  console.log(`  backend: ${entry.backendName}`);
}

async function runList(store: WalletStore): Promise<void> {
  const entries = await store.list();
  if (entries.length === 0) {
    console.log("(no wallets registered)");
    console.log(`Available backends: ${store.listBackends().join(", ")}`);
    console.log("Add one with: cosmos-mcp wallet add <id> --chain <chainId>");
    return;
  }
  console.log(
    `${entries.length} wallet(s) (backends loaded: ${store.listBackends().join(", ")}):`
  );
  for (const e of entries) {
    console.log(formatEntryLine(e));
  }
}

async function runRemove(args: ParsedArgs, store: WalletStore): Promise<void> {
  const id = args.positional[0];
  if (!id) {
    printUsage();
    throw new Error("wallet rm: missing <id>");
  }
  await store.remove(id);
  console.log(`✓ wallet "${id}" removed`);
}

/**
 * Entry point for the `cosmos-mcp wallet ...` subcommand. Called
 * from `index.ts` when `process.argv[2] === "wallet"`. Throws on
 * any error; the caller should print and exit non-zero.
 */
export async function runWalletCli(args: readonly string[]): Promise<void> {
  if (args.length === 0 || args[0] === "--help" || args[0] === "-h") {
    printUsage();
    return;
  }

  const sub = args[0];
  const parsed = parseArgs(args.slice(1));
  const store = await buildCliWalletStore();

  switch (sub) {
    case "add":
      await runAdd(parsed, store);
      return;
    case "list":
    case "ls":
      await runList(store);
      return;
    case "rm":
    case "remove":
      await runRemove(parsed, store);
      return;
    default:
      printUsage();
      throw new Error(`unknown subcommand: ${sub}`);
  }
}
