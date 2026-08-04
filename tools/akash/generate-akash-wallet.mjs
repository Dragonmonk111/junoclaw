#!/usr/bin/env node
/**
 * generate-akash-wallet.mjs — Generate a NEW dedicated Akash wallet and
 * register it directly into the encrypted WalletStore, in-process.
 *
 * Safety properties (this is the highest-safety wallet creation path
 * in the repo — stricter than `cosmos-mcp wallet add`, which is designed
 * to *import* an externally-generated mnemonic and therefore has to
 * accept it via stdin/file/env):
 *
 *   - The mnemonic is generated in-process via DirectSecp256k1HdWallet.
 *     It never exists as a file, env var, or CLI argument.
 *   - The mnemonic is NEVER printed, logged, or written anywhere. Only
 *     the derived bech32 address is printed to stdout.
 *   - Immediately after WalletStore.add() encrypts and persists the
 *     wallet, the in-memory mnemonic string/buffer references are
 *     dropped and the object is discarded. Node's GC will reclaim the
 *     string; there is no way to force-zero a JS string in place, which
 *     is why WalletStore's own decrypt path uses a Buffer it can .fill(0)
 *     — but this script has no decrypt path, only a generate-then-encrypt
 *     path, so the exposure window is a single process lifetime with no
 *     disk/network write of the plaintext at any point.
 *   - Default backend is `keychain` (Windows DPAPI / macOS Keychain /
 *     libsecret) when available, so there is no operator passphrase to
 *     manage or leak via shell history / .env files. Falls back to
 *     `passphrase` (scrypt+AES-256-GCM) only if keychain is unavailable —
 *     same precedence as `mcp/src/wallet/cli.ts::buildCliWalletStore`.
 *     Override explicitly with `--backend passphrase` or `--backend keychain`.
 *   - Post-write round-trip verification: after `store.add()` persists the
 *     encrypted file, this script immediately calls `store.verifyAddress()`
 *     (decrypt-derive-compare, no network) to confirm the on-disk file
 *     actually decrypts to the address just generated, before declaring
 *     success. Catches silent corruption/backend mismatch at creation time
 *     rather than at first-use.
 *   - Requires `--yes` to actually persist anything. Without it, the
 *     script generates nothing and only prints which backend/id it
 *     *would* use — prevents an accidental double-invocation from
 *     silently creating (or attempting to overwrite) a wallet.
 *
 * Usage:
 *   node tools/akash/generate-akash-wallet.mjs --id akash-jlens --yes
 *   node tools/akash/generate-akash-wallet.mjs --id akash-jlens --yes --backend passphrase
 *
 * Output: prints the wallet id, address, prefix, backend. Does NOT
 * print the mnemonic. If you need to recover funds without this
 * process's WalletStore, there is no other copy of the mnemonic —
 * this is intentional (autonomous-signing wallets are meant to hold a
 * bounded, disposable amount of funds; see tools/akash/README.md
 * "Security model").
 */

import { join, dirname } from "path";
import { fileURLToPath, pathToFileURL } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const MCP_DIST = join(__dirname, "..", "..", "mcp", "dist");

// Windows ESM dynamic import() requires file:// URLs, not raw paths.
function distImport(...segments) {
  return import(pathToFileURL(join(MCP_DIST, ...segments)).href);
}

function parseArgs() {
  const args = process.argv.slice(2);
  const opts = {};
  for (let i = 0; i < args.length; i++) {
    if (args[i].startsWith("--")) {
      const key = args[i].slice(2);
      const val = args[i + 1] && !args[i + 1].startsWith("--") ? args[++i] : true;
      opts[key] = val;
    }
  }
  return opts;
}

async function main() {
  const opts = parseArgs();
  const id = opts.id;
  if (!id) {
    console.error("Usage: node generate-akash-wallet.mjs --id <wallet-id> --yes [--backend passphrase|keychain]");
    process.exit(1);
  }
  const confirmed = opts.yes === true;

  const { WalletStore } = await distImport("wallet", "store.js");
  const { PassphraseKeyStore, defaultPassphraseSource } = await distImport(
    "wallet",
    "key-store.js"
  );

  // Build a WalletStore with keychain preferred, passphrase as fallback —
  // mirrors mcp/src/wallet/cli.ts::buildCliWalletStore.
  const root = WalletStore.defaultRoot();
  const backends = new Map();
  backends.set("passphrase", new PassphraseKeyStore(root, defaultPassphraseSource()));

  let preferredBackend = "passphrase";
  try {
    const { keychainKeyStore } = await distImport("wallet", "keychain-store.js");
    const ks = await keychainKeyStore();
    backends.set("keychain", ks);
    preferredBackend = "keychain";
  } catch {
    console.warn("[warn] keychain backend unavailable; falling back to passphrase backend.");
    console.warn("[warn] Ensure JUNOCLAW_WALLET_PASSPHRASE is set before running.");
  }

  const requestedBackend = typeof opts.backend === "string" ? opts.backend : preferredBackend;
  if (!backends.has(requestedBackend)) {
    console.error(`Backend "${requestedBackend}" not available. Available: ${[...backends.keys()].join(", ")}`);
    process.exit(1);
  }

  if (!confirmed) {
    console.log("[dry-run] No --yes flag supplied. Nothing will be generated or written.");
    console.log(`[dry-run] Would create wallet id="${id}" using backend="${requestedBackend}" (prefix=akash).`);
    console.log("[dry-run] Re-run with --yes to actually generate and persist the wallet.");
    return;
  }

  const store = new WalletStore(root, backends, requestedBackend);

  console.log(`[1/3] Generating new 24-word mnemonic and encrypting it in one step (WalletStore.generateAndAdd — mnemonic never leaves that function)...`);
  const entry = await store.generateAndAdd(id, {
    bech32Prefix: "akash",
    backend: requestedBackend,
    wordCount: 24,
  });

  console.log(`[2/3] Wallet registered. Verifying round-trip decrypt (no network)...`);

  // Round-trip verification: decrypt the just-written file and confirm
  // it derives the same address. Catches backend/DEK/ciphertext
  // corruption at creation time instead of at first real signing.
  const verifiedAddress = await store.verifyAddress(id);
  if (verifiedAddress !== entry.address) {
    throw new Error(
      `CRITICAL: round-trip verification mismatch. Registered address ${entry.address} ` +
        `but decrypted file yields ${verifiedAddress}. Do NOT fund this wallet. ` +
        `Remove it with: cosmos-mcp wallet rm ${id}`
    );
  }

  console.log(`[3/3] Round-trip verified: encrypted file decrypts to the same address.`);
  console.log("");
  console.log("=== Akash wallet created ===");
  console.log(`  wallet id: ${entry.id}`);
  console.log(`  address:   ${entry.address}`);
  console.log(`  prefix:    ${entry.bech32Prefix}`);
  console.log(`  backend:   ${entry.backendName}`);
  console.log("");
  console.log(`Send AKT to: ${entry.address}`);
  console.log("The mnemonic was generated inside WalletStore.generateAndAdd() and never");
  console.log("left that function — not printed, logged, or returned to this script.");
  console.log("It exists only inside the encrypted WalletStore file, decryptable");
  console.log("only via this machine's DPAPI/Keychain (or passphrase, if that");
  console.log("backend was used).");
}

main().catch((e) => {
  console.error(`Fatal: ${e.message}`);
  process.exit(1);
});
