import { execFileSync } from "child_process";
import { mkdtempSync, writeFileSync, unlinkSync, rmSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import { getDefaultWalletStore } from "../../mcp/dist/wallet/store.js";

async function main() {
  const ws = getDefaultWalletStore();
  const mnemonic = await ws.exportMnemonicForExternalSigner("akash-jlens");
  const dir = mkdtempSync(join(tmpdir(), "akash-keyring-"));
  const wslDir = dir.replace(/^([A-Za-z]):[\\/]/, (m, d) => "/mnt/" + d.toLowerCase() + "/").replace(/\\/g, "/");
  const mnemonicFile = join(dir, "mnemonic.txt");
  writeFileSync(mnemonicFile, mnemonic, { mode: 0o600 });

  try {
    execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "keys", "add", "junoclaw-autodeploy", "--recover", "--keyring-backend", "test", "--home", wslDir], { input: mnemonic + "\n", encoding: "utf-8", timeout: 30000 });

    // Cert
    execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "tx", "cert", "generate", "client", "--node", "https://akash-rpc.polkachu.com:443", "--chain-id", "akashnet-2", "--from", "junoclaw-autodeploy", "--keyring-backend", "test", "--home", wslDir, "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "10000uakt", "--output", "json", "-y"], { encoding: "utf-8", timeout: 30000 });
    execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "tx", "cert", "publish", "client", "--node", "https://akash-rpc.polkachu.com:443", "--chain-id", "akashnet-2", "--from", "junoclaw-autodeploy", "--keyring-backend", "test", "--home", wslDir, "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "10000uakt", "--output", "json", "-y"], { encoding: "utf-8", timeout: 60000 });
    await new Promise(r => setTimeout(r, 10000));

    // Deploy
    const deployResult = execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "tx", "deployment", "create", "/mnt/c/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/tools/akash/sdl-mixtral-8x7b.yml", "--deposit", "5000000uact", "--node", "https://akash-rpc.polkachu.com:443", "--chain-id", "akashnet-2", "--from", "junoclaw-autodeploy", "--keyring-backend", "test", "--home", wslDir, "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "10000uakt", "--output", "json", "-y"], { encoding: "utf-8", timeout: 120000, maxBuffer: 10 * 1024 * 1024 });
    const deployData = JSON.parse(deployResult);
    console.log("Deploy tx:", deployData.txhash);

    // Get dseq
    const depList = execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "query", "deployment", "list", "--owner", "akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt", "--node", "https://akash-rpc.polkachu.com:443", "--output", "json", "--state", "active"], { encoding: "utf-8", timeout: 30000 });
    const depData = JSON.parse(depList);
    const dseq = depData.deployments[0].deployment.id.dseq;
    console.log("dseq:", dseq);

    // Wait for bids
    console.log("Waiting 30s for bids...");
    await new Promise(r => setTimeout(r, 30000));

    // Query bids
    const bidResult = execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "query", "market", "bid", "list", "--owner", "akash1eehlc3mu8tdkhp7pc4whjkpkw329j9sq6l05dt", "--dseq", dseq, "--node", "https://akash-rpc.polkachu.com:443", "--output", "json"], { encoding: "utf-8", timeout: 30000 });
    const bidData = JSON.parse(bidResult);
    const bids = bidData.bids || [];
    console.log(`Bids: ${bids.length}`);
    if (bids.length > 0) {
      console.log("First bid structure:");
      console.log(JSON.stringify(bids[0], null, 2).slice(0, 3000));
    }

    // Close deployment
    execFileSync("wsl.exe", ["-d", "Ubuntu-24.04", "--", "akash", "tx", "deployment", "close", "--dseq", dseq, "--node", "https://akash-rpc.polkachu.com:443", "--chain-id", "akashnet-2", "--from", "junoclaw-autodeploy", "--keyring-backend", "test", "--home", wslDir, "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "10000uakt", "--output", "json", "-y"], { encoding: "utf-8", timeout: 60000 });
    console.log("Deployment closed.");
  } finally {
    try { unlinkSync(mnemonicFile); } catch {}
    try { rmSync(dir, { recursive: true, force: true }); } catch {}
  }
}

main().catch(e => { console.error(e.message); process.exit(1); });
