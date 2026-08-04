import { execFileSync } from "child_process";
import { mkdtempSync, writeFileSync, unlinkSync, rmSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import { getDefaultWalletStore } from "../../mcp/dist/wallet/store.js";

async function main() {
  const dseq = process.argv[2] || "27951583";
  const ws = getDefaultWalletStore();
  const mnemonic = await ws.exportMnemonicForExternalSigner("akash-jlens");
  const dir = mkdtempSync(join(tmpdir(), "akash-keyring-"));
  const wslDir = dir.replace(/^([A-Za-z]):[\\/]/, (m, d) => "/mnt/" + d.toLowerCase() + "/").replace(/\\/g, "/");

  try {
    execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "keys", "add", "junoclaw-autodeploy", "--recover", "--keyring-backend", "test", "--home", wslDir], { input: mnemonic + "\n", encoding: "utf-8", timeout: 30000 });
    const result = execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "tx", "deployment", "close", "--dseq", dseq, "--node", "https://akash-rpc.polkachu.com:443", "--chain-id", "akashnet-2", "--from", "junoclaw-autodeploy", "--keyring-backend", "test", "--home", wslDir, "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "80000uakt", "--output", "json", "-y"], { encoding: "utf-8", timeout: 60000 });
    console.log(`Closed deployment ${dseq}:`, JSON.parse(result).txhash);
  } finally {
    try { rmSync(dir, { recursive: true, force: true }); } catch {}
  }
}

main().catch(e => { console.error(e.message); process.exit(1); });
