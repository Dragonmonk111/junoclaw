#!/usr/bin/env node
import { execFileSync } from "child_process";
import { mkdtempSync, rmSync } from "fs";
import { join, dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { tmpdir } from "os";

const __dirname = dirname(fileURLToPath(import.meta.url));
const WSL_DISTRO = process.env.WSL_DISTRO || "Ubuntu-24.04";
const SDL_PATH = resolve(__dirname, "sdl-buzz-relay.yml");

function toWslPath(p) {
  const match = p.match(/^([A-Za-z]):[\\/](.*)/);
  if (!match) return p;
  return `/mnt/${match[1].toLowerCase()}/${match[2].replace(/\\/g, "/")}`;
}

const args = process.argv.slice(2);
const opts = {};
for (let i = 0; i < args.length; i++) {
  if (args[i].startsWith("--")) {
    opts[args[i].slice(2).replace(/-/g, "_")] = args[++i];
  }
}

if (!opts.dseq || !opts.provider) {
  console.error("Usage: node send-manifest.mjs --dseq <dseq> --provider <provider-addr>");
  process.exit(1);
}

let mnemonic = process.env.AKASH_MNEMONIC;
if (!mnemonic || mnemonic.length < 20) {
  try {
    const { getDefaultWalletStore } = await import("../../mcp/dist/wallet/store.js");
    const ws = getDefaultWalletStore();
    mnemonic = await ws.exportMnemonicForExternalSigner("akash-jlens");
  } catch (e) {
    console.error("Set AKASH_MNEMONIC env var first");
    process.exit(1);
  }
}

const keyringDir = mkdtempSync(join(tmpdir(), "buzz-manifest-"));
const keyringHome = toWslPath(keyringDir);
const keyName = "buzz-deployer";

try {
  execFileSync("wsl.exe", ["-d", WSL_DISTRO, "--", "akash", "keys", "add", keyName,
    "--recover", "--keyring-backend", "test", "--home", keyringHome],
    { encoding: "utf-8", input: mnemonic + "\n", timeout: 30000 });

  const sdlWslPath = toWslPath(SDL_PATH);
  console.log("Sending manifest to provider...");
  const result = execFileSync("wsl.exe", ["-d", WSL_DISTRO, "--", "provider-services",
    "send-manifest", sdlWslPath,
    "--dseq", opts.dseq,
    "--provider", opts.provider,
    "--node", "https://akash-rpc.polkachu.com:443",
    "--from", keyName, "--keyring-backend", "test", "--home", keyringHome],
    { encoding: "utf-8", timeout: 120000, maxBuffer: 10 * 1024 * 1024 }).trim();
  console.log(result);
  console.log("\nManifest sent. Container will restart with new env vars.");
} catch (e) {
  console.error("Error:", e.message);
  if (e.stdout) console.log(e.stdout);
  if (e.stderr) console.error(e.stderr);
} finally {
  try { rmSync(keyringDir, { recursive: true, force: true }); } catch {}
  console.log("Keyring scrubbed");
}
