#!/usr/bin/env node
import { execFileSync } from "child_process";
import { mkdtempSync, writeFileSync, unlinkSync, rmSync } from "fs";
import { join, dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { tmpdir } from "os";

const __dirname = dirname(fileURLToPath(import.meta.url));
const WSL_DISTRO = process.env.WSL_DISTRO || "Ubuntu-24.04";
const AKASH_NODE = "https://akash-rpc.polkachu.com:443";
const AKASH_CHAIN_ID = "akashnet-2";
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
  console.error("Usage: node update-deployment.mjs --dseq <dseq> --provider <provider-addr>");
  process.exit(1);
}

let mnemonic = process.env.AKASH_MNEMONIC;
if (!mnemonic || mnemonic.length < 20) {
  try {
    const { getDefaultWalletStore } = await import("../../mcp/dist/wallet/store.js");
    const ws = getDefaultWalletStore();
    mnemonic = await ws.exportMnemonicForExternalSigner("akash-jlens");
  } catch (e) {
    console.error("Set AKASH_MNEMONIC env var first (or ensure wallet store has 'akash-jlens')");
    process.exit(1);
  }
}

const keyringDir = mkdtempSync(join(tmpdir(), "buzz-update-"));
const keyringHome = toWslPath(keyringDir);
const keyName = "buzz-deployer";

function execAkash(akashArgs, execOpts = {}) {
  const wslArgs = akashArgs.map(a => {
    if (typeof a === "string" && /^[A-Za-z]:[\\\/]/.test(a)) {
      return toWslPath(a);
    }
    return a;
  });
  return execFileSync("wsl.exe", ["-d", WSL_DISTRO, "--", "akash", ...wslArgs], {
    encoding: "utf-8",
    timeout: execOpts.timeout || 120000,
    input: execOpts.input,
    maxBuffer: 10 * 1024 * 1024,
  }).trim();
}

function execProviderServices(psArgs, execOpts = {}) {
  return execFileSync("wsl.exe", ["-d", WSL_DISTRO, "--", "provider-services", ...psArgs], {
    encoding: "utf-8",
    timeout: execOpts.timeout || 120000,
    input: execOpts.input,
    maxBuffer: 10 * 1024 * 1024,
  }).trim();
}

try {
  // 1. Create temp keyring
  console.log("[1/4] Creating temporary keyring...");
  execAkash(["keys", "add", keyName, "--recover", "--keyring-backend", "test", "--home", keyringHome], {
    input: mnemonic + "\n",
    timeout: 30000,
  });
  const addrRaw = execAkash(["keys", "show", keyName, "--keyring-backend", "test", "--home", keyringHome, "--output", "json"], { timeout: 15000 });
  const address = JSON.parse(addrRaw).address;
  console.log(`  Wallet: ${address}`);

  // 2. Update deployment on-chain
  console.log(`\n[2/4] Updating deployment ${opts.dseq} on-chain...`);
  const sdlWslPath = toWslPath(SDL_PATH);
  const updateResult = execAkash([
    "tx", "deployment", "update", sdlWslPath,
    "--dseq", opts.dseq,
    "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID,
    "--from", keyName, "--keyring-backend", "test", "--home", keyringHome,
    "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "80000uakt",
    "--output", "json", "-y",
  ], { timeout: 120000 });
  const updateTx = JSON.parse(updateResult);
  console.log(`  Update TX: ${updateTx.txhash} (code: ${updateTx.code})`);
  if (updateTx.code !== 0) {
    console.error(`  ERROR: ${updateTx.raw_log}`);
    process.exit(1);
  }

  console.log("  Waiting 15s for confirmation...");
  await new Promise(r => setTimeout(r, 15000));

  // 3. Send manifest to provider
  console.log(`\n[3/4] Sending updated manifest to provider ${opts.provider}...`);
  try {
    const manifestResult = execProviderServices([
      "send-manifest", sdlWslPath,
      "--dseq", opts.dseq,
      "--provider", opts.provider,
      "--node", AKASH_NODE,
      "--from", keyName, "--keyring-backend", "test", "--home", keyringHome,
    ], { timeout: 120000 });
    console.log("  Manifest sent.");
  } catch (e) {
    console.log(`  Manifest send error: ${e.message}`);
    if (e.stdout) console.log(e.stdout);
  }

  // 4. Done
  console.log(`\n[4/4] Deployment ${opts.dseq} updated.`);
  console.log("  The relay container will restart with new env vars.");
  console.log("  Wait ~60s for it to come back up, then verify.");

} catch (e) {
  console.error("Error:", e.message);
  if (e.stdout) console.log("stdout:", e.stdout);
  if (e.stderr) console.error("stderr:", e.stderr);
} finally {
  try { rmSync(keyringDir, { recursive: true, force: true }); } catch {}
  console.log("Keyring scrubbed");
}
