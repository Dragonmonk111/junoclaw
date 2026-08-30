import { execFileSync } from "child_process";
import { mkdtempSync, rmSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";

async function main() {
  const dseqArg = process.argv.find(a => a.startsWith("--dseq="))?.split("=")[1]
    || process.argv[process.argv.indexOf("--dseq") + 1]
    || process.argv.find(a => /^\d+$/.test(a));
  const dseq = dseqArg || "27951583";

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

  const dir = mkdtempSync(join(tmpdir(), "akash-keyring-"));
  const wslDir = dir.replace(/^([A-Za-z]):[\\/]/, (m, d) => "/mnt/" + d.toLowerCase() + "/").replace(/\\/g, "/");
  const keyName = "buzz-deployer";

  try {
    execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "keys", "add", keyName, "--recover", "--keyring-backend", "test", "--home", wslDir], { input: mnemonic + "\n", encoding: "utf-8", timeout: 30000 });
    const result = execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "tx", "deployment", "close", "--dseq", dseq, "--node", "https://akash-rpc.polkachu.com:443", "--chain-id", "akashnet-2", "--from", keyName, "--keyring-backend", "test", "--home", wslDir, "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "80000uakt", "--output", "json", "-y"], { encoding: "utf-8", timeout: 60000 });
    console.log(`Closed deployment ${dseq}:`, JSON.parse(result).txhash);
  } finally {
    try { rmSync(dir, { recursive: true, force: true }); } catch {}
  }
}

main().catch(e => { console.error(e.message); process.exit(1); });
