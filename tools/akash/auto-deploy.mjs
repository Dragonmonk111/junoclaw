#!/usr/bin/env node
/**
 * auto-deploy.mjs — Budget-capped autonomous Akash deployment signer.
 *
 * Implements the "Option 2d" budget-capped autonomous signing approach
 * for the J-Lens Kimi K3 pilot (A039/A040).
 *
 * Features:
 *   - Decrypts wallet from encrypted WalletStore (no plaintext key on disk)
 *   - Creates Akash deployment from an SDL file
 *   - Selects cheapest bid within a spend cap
 *   - Auto-closes deployment after a timeout (no runaway billing)
 *   - Posts every signed transaction to Moultbook for audit trail
 *   - Scrubs all temporary key material after signing
 *
 * Usage:
 *   node auto-deploy.mjs \
 *     --sdl deploy.yml \
 *     --wallet-id akash-jlens \
 *     --max-spend-uakt 5000000 \
 *     --timeout-minutes 120 \
 *     [--akash-bin /usr/local/bin/akash] \
 *     [--akash-node https://akash-rpc.polkachu.com:443] \
 *     [--moultbook-addr juno1r59ulw66alrv7s65egfk03zqs28yz04ajnl95r877e85mx8h7qnq8ze2w5] \
 *     [--juno-wallet-id builder] \
 *     [--dry-run]
 */

import { execFileSync, execSync } from "child_process";
import { readFileSync, writeFileSync, unlinkSync, existsSync, mkdtempSync, rmSync } from "fs";
import { join, dirname, resolve } from "path";
import { fileURLToPath, pathToFileURL } from "url";
import { createHash } from "crypto";
import { tmpdir } from "os";

const __dirname = dirname(fileURLToPath(import.meta.url));
const MCP_DIST = join(__dirname, "..", "..", "mcp", "dist");

// Windows ESM dynamic import() requires file:// URLs, not raw paths.
function distImport(...segments) {
  return import(pathToFileURL(join(MCP_DIST, ...segments)).href);
}

// ──────────────────────────────────────────────
// Windows/WSL bridge for akash CLI
// ──────────────────────────────────────────────

const isWindows = process.platform === "win32";
const WSL_DISTRO = process.env.WSL_DISTRO || "Ubuntu-24.04";

/** Translate a Windows path (C:\foo\bar) to a WSL path (/mnt/c/foo/bar). */
function toWslPath(p) {
  if (!isWindows) return p;
  const match = p.match(/^([A-Za-z]):[\\\/](.*)/);
  if (!match) return p;
  return `/mnt/${match[1].toLowerCase()}/${match[2].replace(/\\/g, "/")}`;
}

/**
 * Execute akash CLI — on Windows via WSL, on Linux/macOS directly.
 * Translates file paths in args from Windows to WSL format.
 */
function execAkash(opts, args, execOpts = {}) {
  if (isWindows) {
    const wslArgs = args.map(a => {
      // Translate paths that look like Windows absolute paths
      if (typeof a === "string" && /^[A-Za-z]:[\\\/]/.test(a)) {
        return toWslPath(a);
      }
      return a;
    });
    return execFileSync("wsl.exe", ["-d", WSL_DISTRO, "--", "akash", ...wslArgs], {
      ...execOpts,
      encoding: execOpts.encoding || "utf-8",
      timeout: execOpts.timeout || 120000,
      maxBuffer: execOpts.maxBuffer || 10 * 1024 * 1024,
    }).trim();
  }
  return execFileSync(opts.akashBin, args, execOpts).trim();
}

/** Log the akash command for audit/debugging. */
function logAkashCmd(args) {
  if (isWindows) {
    console.log(`[akash] wsl ${WSL_DISTRO} -- akash ${args.join(" ")}`);
  } else {
    console.log(`[akash] akash ${args.join(" ")}`);
  }
}

// ──────────────────────────────────────────────
// Argument parsing
// ──────────────────────────────────────────────

function parseArgs() {
  const args = process.argv.slice(2);
  const opts = {};
  for (let i = 0; i < args.length; i++) {
    if (args[i].startsWith("--")) {
      const key = args[i].slice(2).replace(/-/g, "_");
      const val = args[i + 1] && !args[i + 1].startsWith("--") ? args[++i] : true;
      opts[key] = val;
    }
  }

  const required = ["sdl", "wallet_id", "max_spend_uakt", "timeout_minutes"];
  for (const r of required) {
    if (!(r in opts)) {
      console.error(`Missing required argument: --${r.replace(/_/g, "-")}`);
      printUsage();
      process.exit(1);
    }
  }

  return {
    sdlPath: opts.sdl,
    walletId: opts.wallet_id,
    maxSpendUakt: parseInt(opts.max_spend_uakt, 10),
    timeoutMinutes: parseInt(opts.timeout_minutes, 10),
    akashBin: opts.akash_bin || "akash",
    akashNode: opts.akash_node || "https://akash-rpc.polkachu.com:443",
    akashChainId: opts.akash_chain_id || "akashnet-2",
    moultbookAddr: opts.moultbook_addr || null,
    junoWalletId: opts.juno_wallet_id || null,
    dryRun: opts.dry_run === true,
    keyringDir: opts.keyring_dir || null,
    deposit: opts.deposit || "5000000uact",
    bidsOnly: opts.bids_only === true,
  };
}

function printUsage() {
  console.error(`
Usage: node auto-deploy.mjs [options]

Required:
  --sdl <path>                  Path to Akash SDL file
  --wallet-id <id>              WalletStore ID for Akash wallet
  --max-spend-uakt <n>          Maximum total spend in uakt (e.g. 5000000 = 5 AKT)
  --timeout-minutes <n>         Auto-close after this many minutes

Optional:
  --akash-bin <path>            Path to akash CLI binary (default: "akash")
  --akash-node <url>            Akash RPC endpoint (default: polkachu)
  --akash-chain-id <id>         Akash chain ID (default: akashnet-2)
  --moultbook-addr <addr>       Moultbook contract address for audit posting
  --juno-wallet-id <id>         WalletStore ID for Juno wallet (for Moultbook txs)
  --dry-run                     Skip actual broadcast, just print what would happen
  --keyring-dir <path>          Temp keyring directory (default: auto-generated)
`);
}

// ──────────────────────────────────────────────
// Wallet decryption (uses WalletStore)
// ──────────────────────────────────────────────

async function decryptMnemonic(walletId) {
  const { getDefaultWalletStore } = await distImport("wallet", "store.js");
  const ws = getDefaultWalletStore();
  return ws.exportMnemonicForExternalSigner(walletId);
}

// ──────────────────────────────────────────────
// Temporary keyring management
// ──────────────────────────────────────────────

function createTempKeyring(mnemonic, opts) {
  // On Windows, the keyring must be in a WSL-accessible path.
  // We create a temp dir on the Windows side (so Node can write/scrub it),
  // but pass the WSL-translated path to akash.
  const keyringDir = opts.keyringDir || mkdtempSync(join(tmpdir(), "akash-keyring-"));
  const keyringHome = isWindows ? toWslPath(keyringDir) : keyringDir;

  // Write mnemonic to a temp file for `akash keys add --recover`
  const mnemonicFile = join(keyringDir, "mnemonic.txt");
  writeFileSync(mnemonicFile, mnemonic, { mode: 0o600 });

  try {
    // Import key into temporary keyring
    const keyName = "junoclaw-autodeploy";
    execAkash(opts, [
      "keys", "add", keyName,
      "--recover",
      "--keyring-backend", "test",
      "--home", keyringHome,
    ], {
      input: mnemonic + "\n",
      encoding: "utf-8",
      timeout: 30000,
    });

    // Get the address
    const address = execAkash(opts, [
      "keys", "show", keyName,
      "--keyring-backend", "test",
      "--home", keyringHome,
      "--output", "json",
    ], { encoding: "utf-8", timeout: 15000 });

    const addrJson = JSON.parse(address);
    return { keyringDir, keyringHome, keyName, address: addrJson.address };
  } finally {
    // Scrub mnemonic file immediately
    try { unlinkSync(mnemonicFile); } catch {}
  }
}

function cleanupKeyring(keyringDir) {
  if (keyringDir && keyringDir.startsWith(tmpdir())) {
    try {
      rmSync(keyringDir, { recursive: true, force: true });
      console.log(`[cleanup] Removed temp keyring: ${keyringDir}`);
    } catch (e) {
      console.warn(`[cleanup] Failed to remove ${keyringDir}: ${e.message}`);
    }
  }
}

// ──────────────────────────────────────────────
// Akash CLI wrapper
// ──────────────────────────────────────────────

function akashCmd(opts, keyring, args) {
  const home = keyring.keyringHome || keyring.keyringDir;
  const fullArgs = [
    ...args,
    "--node", opts.akashNode,
    "--chain-id", opts.akashChainId,
    "--from", keyring.keyName,
    "--keyring-backend", "test",
    "--home", home,
    "--gas", "auto",
    "--gas-adjustment", "1.5",
    "--fees", "80000uakt",
    "--output", "json",
    "-y",
  ];
  logAkashCmd(fullArgs);
  if (opts.dryRun) {
    return JSON.stringify({ dryRun: true, args: fullArgs });
  }
  return execAkash(opts, fullArgs, {
    encoding: "utf-8",
    timeout: 120000,
    maxBuffer: 10 * 1024 * 1024,
  });
}

function createCertificate(opts, keyring) {
  // v0.24.0: two-step cert flow — generate locally, then publish to chain
  const home = keyring.keyringHome || keyring.keyringDir;
  const baseArgs = [
    "--node", opts.akashNode,
    "--chain-id", opts.akashChainId,
    "--from", keyring.keyName,
    "--keyring-backend", "test",
    "--home", home,
    "--gas", "auto",
    "--gas-adjustment", "1.5",
    "--fees", "80000uakt",
    "--output", "json",
    "-y",
  ];

  // Step 1: Generate client cert locally
  logAkashCmd(["tx", "cert", "generate", "client", ...baseArgs]);
  execAkash(opts, ["tx", "cert", "generate", "client", ...baseArgs], {
    encoding: "utf-8",
    timeout: 30000,
  });
  console.log("[cert] Client certificate generated locally");

  // Step 2: Publish to chain
  logAkashCmd(["tx", "cert", "publish", "client", ...baseArgs]);
  const result = execAkash(opts, ["tx", "cert", "publish", "client", ...baseArgs], {
    encoding: "utf-8",
    timeout: 120000,
    maxBuffer: 10 * 1024 * 1024,
  });
  console.log("[cert] Client certificate published to chain");
  return JSON.parse(result);
}

function createDeployment(opts, keyring) {
  const sdlPath = isWindows ? toWslPath(resolve(opts.sdlPath)) : opts.sdlPath;
  const result = akashCmd(opts, keyring, [
    "tx", "deployment", "create", sdlPath,
    "--deposit", opts.deposit,
  ]);
  console.log("[deploy] Deployment created");
  return JSON.parse(result);
}

function queryBids(opts, keyring, dseq) {
  if (opts.dryRun) {
    return JSON.stringify({ bids: [
      { bid: { id: { dseq, gseq: 1, oseq: 1, provider: "akash1fake" }, state: "open", price: { denom: "uact", amount: "100" } } },
    ]});
  }
  return execAkash(opts, [
    "query", "market", "bid", "list",
    "--owner", keyring.address,
    "--dseq", dseq,
    "--state", "open",
    "--node", opts.akashNode,
    "--output", "json",
  ], { encoding: "utf-8", timeout: 30000 });
}

function selectBid(bids, maxSpendUakt, timeoutMinutes, preferredProvider) {
  // Parse bids and find cheapest within cap
  // max total cost = bid_price_per_block * blocks_in_session
  // blocks_in_session = timeout_minutes * 60 / 6 (Akash block time ~6s)
  const blocksInSession = Math.ceil(timeoutMinutes * 60 / 6);
  const maxPerBlock = Math.floor(maxSpendUakt / blocksInSession);

  // v2.x: bid.id (not bid.bid_id), price.amount is decimal string
  const getBidId = b => b.bid.id || b.bid.bid_id;
  const getPrice = b => parseFloat(b.bid.price.amount);

  // Filter to open bids only, within cap
  const validBids = bids.filter(b => {
    const state = b.bid.state || b.state || "open";
    const price = getPrice(b);
    return state === "open" && price > 0 && price <= maxPerBlock;
  }).sort((a, b) => {
    // Prefer known-working provider if specified
    if (preferredProvider) {
      const aPref = (getBidId(a).provider === preferredProvider) ? 0 : 1;
      const bPref = (getBidId(b).provider === preferredProvider) ? 0 : 1;
      if (aPref !== bPref) return aPref - bPref;
    }
    return getPrice(a) - getPrice(b);
  });

  if (validBids.length === 0) {
    // Show all bids for debugging
    const allPrices = bids.map(b => `${getPrice(b)} (${b.bid.state || b.state || "?"})`).join(", ");
    throw new Error(
      `No open bids within spend cap. Max per-block: ${maxPerBlock} uact ` +
      `(${maxSpendUakt} uact / ${blocksInSession} blocks). ` +
      `Bids received: ${bids.length}. Prices: ${allPrices}`
    );
  }

  return validBids[0];
}

function createLease(opts, keyring, dseq, gseq, provider) {
  const result = akashCmd(opts, keyring, [
    "tx", "market", "lease", "create",
    "--dseq", dseq,
    "--gseq", String(gseq),
    "--oseq", "1",
    "--provider", provider,
  ]);
  console.log(`[lease] Lease created: dseq=${dseq}, provider=${provider}`);
  return JSON.parse(result);
}

function sendManifest(opts, keyring, dseq, provider) {
  const sdlPath = isWindows ? toWslPath(resolve(opts.sdlPath)) : opts.sdlPath;
  const home = keyring.keyringHome || keyring.keyringDir;
  const args = [
    "send-manifest", sdlPath,
    "--dseq", dseq,
    "--provider", provider,
    "--node", opts.akashNode,
    "--from", keyring.keyName,
    "--keyring-backend", "test",
    "--home", home,
    "--output", "json",
  ];
  console.log(`[manifest] Sending manifest to provider ${provider}...`);
  if (isWindows) {
    console.log(`[manifest] wsl ${WSL_DISTRO} -- provider-services ${args.join(" ")}`);
  } else {
    console.log(`[manifest] provider-services ${args.join(" ")}`);
  }
  if (opts.dryRun) {
    console.log("[manifest] (dry-run) skipped");
    return { dryRun: true };
  }
  const bin = isWindows ? "wsl.exe" : "provider-services";
  const binArgs = isWindows ? ["-d", WSL_DISTRO, "--", "provider-services", ...args] : args;
  const result = execFileSync(bin, binArgs, {
    encoding: "utf-8",
    timeout: 60000,
    maxBuffer: 10 * 1024 * 1024,
  }).trim();
  console.log(`[manifest] Manifest sent successfully`);
  return result;
}

function getLeaseStatus(opts, keyring, dseq, provider) {
  if (opts.dryRun) return { services: {} };
  try {
    const result = execAkash(opts, [
      "query", "market", "lease", "get",
      "--owner", keyring.address,
      "--dseq", dseq,
      "--gseq", "1",
      "--oseq", "1",
      "--provider", provider,
      "--node", opts.akashNode,
      "--output", "json",
    ], { encoding: "utf-8", timeout: 30000 });
    return JSON.parse(result);
  } catch (e) {
    console.warn(`[lease] Failed to query lease status: ${e.message}`);
    return null;
  }
}

function getServiceEndpoint(opts, keyring, dseq, provider) {
  if (opts.dryRun) return "http://localhost:8000";
  try {
    const home = keyring.keyringHome || keyring.keyringDir;
    const args = [
      "lease-status",
      "--dseq", dseq,
      "--provider", provider,
      "--node", opts.akashNode,
      "--from", keyring.keyName,
      "--keyring-backend", "test",
      "--home", home,
    ];
    console.log(`[endpoint] Querying service endpoint...`);
    if (isWindows) {
      console.log(`[endpoint] wsl ${WSL_DISTRO} -- provider-services ${args.join(" ")}`);
    }
    const bin = isWindows ? "wsl.exe" : "provider-services";
    const binArgs = isWindows ? ["-d", WSL_DISTRO, "--", "provider-services", ...args] : args;
    const result = execFileSync(bin, binArgs, {
      encoding: "utf-8",
      timeout: 30000,
      maxBuffer: 10 * 1024 * 1024,
    }).trim();
    const parsed = JSON.parse(result);
    // Find the forwarded port for our service
    const serviceName = Object.keys(parsed.services || {})[0];
    if (serviceName && parsed.services[serviceName]?.uris?.length) {
      const uri = parsed.services[serviceName].uris[0];
      console.log(`[endpoint] Service URI: ${uri}`);
      return uri;
    }
    console.log(`[endpoint] No service URI found in lease status`);
    return null;
  } catch (e) {
    console.warn(`[endpoint] Failed to query service endpoint: ${e.message}`);
    return null;
  }
}

function closeDeployment(opts, keyring, dseq) {
  const result = akashCmd(opts, keyring, [
    "tx", "deployment", "close",
    "--dseq", dseq,
  ]);
  console.log(`[close] Deployment closed: dseq=${dseq}`);
  return JSON.parse(result);
}

function getDeploymentStatus(opts, keyring, dseq) {
  if (opts.dryRun) return "active";
  try {
    const result = execAkash(opts, [
      "query", "deployment", "get",
      "--owner", keyring.address,
      "--dseq", dseq,
      "--node", opts.akashNode,
      "--output", "json",
    ], { encoding: "utf-8", timeout: 30000 });
    const parsed = JSON.parse(result);
    // v2.x nests state under deployment, v0.x has it at top level
    return parsed.state || parsed.deployment?.state || "unknown";
  } catch {
    return "closed";
  }
}

// ──────────────────────────────────────────────
// Moultbook audit posting
// ──────────────────────────────────────────────

async function postAuditToMoultbook(opts, event, txHash, extra) {
  if (!opts.moultbookAddr || !opts.junoWalletId || opts.dryRun) {
    console.log(`[audit] (skipped) ${event}: tx=${txHash}`);
    return;
  }

  const auditPost = await import(pathToFileURL(join(__dirname, "audit-post.mjs")).href);
  const content = JSON.stringify({
    event,
    tx_hash: txHash,
    timestamp: new Date().toISOString(),
    wallet: "junoclaw-agent",
    ...extra,
  });

  const commitment = createHash("sha256").update(content).digest();
  await auditPost.postToMoultbook({
    moultbookAddr: opts.moultbookAddr,
    junoWalletId: opts.junoWalletId,
    commitment,
    contentType: "application/json+jlens-akash-audit",
    refs: [txHash],
  });
  console.log(`[audit] Posted to Moultbook: ${event} (tx=${txHash})`);
}

// ──────────────────────────────────────────────
// Main deployment lifecycle
// ──────────────────────────────────────────────

async function main() {
  const opts = parseArgs();
  console.log("=== JunoClaw Autonomous Akash Deployer ===");
  console.log(`SDL: ${opts.sdlPath}`);
  console.log(`Wallet: ${opts.walletId}`);
  console.log(`Max spend: ${opts.maxSpendUakt} uakt (${opts.maxSpendUakt / 1_000_000} AKT)`);
  console.log(`Timeout: ${opts.timeoutMinutes} minutes`);
  console.log(`Dry run: ${opts.dryRun}`);
  console.log("");

  // 1. Decrypt mnemonic from WalletStore
  console.log("[1/7] Decrypting wallet from encrypted store...");
  let mnemonic;
  try {
    mnemonic = await decryptMnemonic(opts.walletId);
  } catch (e) {
    console.error(`Failed to decrypt wallet "${opts.walletId}": ${e.message}`);
    process.exit(1);
  }
  console.log("[1/7] Wallet decrypted.");

  // 2. Create temporary keyring
  console.log("[2/7] Creating temporary keyring...");
  let keyring;
  try {
    if (opts.dryRun) {
      keyring = {
        keyringDir: "/tmp/dry-run-keyring",
        keyName: "junoclaw-autodeploy",
        address: "akash1dryrunaddress0000000000000000000000000000",
      };
      console.log(`[2/7] (dry-run) Keyring simulated. Address: ${keyring.address}`);
    } else {
      keyring = createTempKeyring(mnemonic, opts);
      console.log(`[2/7] Keyring ready. Address: ${keyring.address}`);
    }
    mnemonic = ""; // Scrub mnemonic from memory
  } catch (e) {
    mnemonic = "";
    console.error(`Failed to create keyring: ${e.message}`);
    process.exit(1);
  }

  let dseq = null;
  let closeTimer = null;

  // Cleanup handler — always close deployment and scrub keyring
  async function cleanup() {
    console.log("\n[cleanup] Starting cleanup...");
    if (closeTimer) clearTimeout(closeTimer);

    if (dseq) {
      const status = getDeploymentStatus(opts, keyring, dseq);
      if (status === "active") {
        console.log(`[cleanup] Closing deployment dseq=${dseq}...`);
        try {
          const closeResult = closeDeployment(opts, keyring, dseq);
          const closeTxHash = closeResult?.txhash || closeResult?.code_hash || "unknown";
          await postAuditToMoultbook(opts, "deployment_closed", closeTxHash, { dseq });
        } catch (e) {
          console.error(`[cleanup] Failed to close deployment: ${e.message}`);
          console.error(`[cleanup] MANUAL INTERVENTION REQUIRED: close dseq=${dseq} on akash-2`);
        }
      } else {
        console.log(`[cleanup] Deployment already ${status}, skipping close.`);
      }
    }

    cleanupKeyring(keyring.keyringDir);
    console.log("[cleanup] Done.");
    process.exit(0);
  }

  process.on("SIGINT", cleanup);
  process.on("SIGTERM", cleanup);
  process.on("exit", () => cleanupKeyring(keyring.keyringDir));

  try {
    // 3. Create client certificate (required by Akash for deployments)
    console.log("[3/7] Creating client certificate...");
    if (!opts.dryRun) {
      createCertificate(opts, keyring);
      console.log("[3/7] Waiting 30s for certificate to activate on-chain...");
      await new Promise(r => setTimeout(r, 30000));
    } else {
      console.log("[3/7] (dry-run) Certificate simulated.");
    }

    // 4. Create deployment
    console.log("[4/7] Creating deployment...");
    const deployResult = createDeployment(opts, keyring);
    const txHash = deployResult?.txhash || "unknown";
    console.log(`[deploy] TX hash: ${txHash}`);

    if (!opts.dryRun) {
      // Wait a few seconds for the tx to be indexed
      await new Promise(r => setTimeout(r, 5000));

      // Query the tx by hash to extract dseq from events
      try {
        const txResult = execAkash(opts, [
          "query", "tx", "--type", "hash", txHash,
          "--node", opts.akashNode,
          "--output", "json",
        ], { encoding: "utf-8", timeout: 30000 });
        const txData = JSON.parse(txResult);
        // Search all events for dseq attribute
        const allEvents = txData?.logs?.flatMap(l => l.events || []) || txData?.events || [];
        const dseqAttr = allEvents
          .flatMap(e => e.attributes || [])
          .find(a => a.key === "dseq");
        if (dseqAttr) {
          dseq = dseqAttr.value;
          console.log(`[deploy] Extracted dseq=${dseq} from tx events`);
        }
      } catch (e) {
        console.log(`[deploy] Could not query tx by hash: ${e.message}`);
      }

      // Fallback: query deployment list, pick the most recent (highest dseq)
      if (!dseq) {
        const deployments = execAkash(opts, [
          "query", "deployment", "list",
          "--owner", keyring.address,
          "--node", opts.akashNode,
          "--output", "json",
          "--state", "active",
        ], { encoding: "utf-8", timeout: 30000 });
        const parsed = JSON.parse(deployments);
        if (parsed.deployments && parsed.deployments.length > 0) {
          // Sort by dseq descending to get the most recently created
          const sorted = parsed.deployments.sort((a, b) => {
            const aDseq = parseInt(a.deployment?.id?.dseq || a.deployment?.deployment_id?.dseq || "0", 10);
            const bDseq = parseInt(b.deployment?.id?.dseq || b.deployment?.deployment_id?.dseq || "0", 10);
            return bDseq - aDseq;
          });
          const dep = sorted[0].deployment;
          dseq = dep?.id?.dseq || dep?.deployment_id?.dseq;
          console.log(`[deploy] Extracted dseq=${dseq} from deployment list (most recent)`);
        }
      }
    }

    if (opts.dryRun) {
      dseq = dseq || "dry-run-dseq";
    }

    console.log(`[4/7] Deployment created: dseq=${dseq}`);
    const deployTxHash = deployResult?.txhash || "dry-run";
    await postAuditToMoultbook(opts, "deployment_created", deployTxHash, { dseq, sdl: opts.sdlPath });

    // 5. Wait for bids with retry loop (GPU providers may take several minutes)
    console.log("[5/7] Polling for bids (up to 5 min, every 30s)...");
    let bids = [];
    if (!opts.dryRun) {
      for (let attempt = 1; attempt <= 20; attempt++) {
        await new Promise(r => setTimeout(r, 30000));
        const bidsRaw = queryBids(opts, keyring, dseq);
        const bidsParsed = JSON.parse(bidsRaw);
        bids = bidsParsed.bids || bidsParsed;
        console.log(`[5/7] Attempt ${attempt}/20: ${bids.length} bid(s)`);
        if (bids.length > 0) break;
      }
    } else {
      bids = [{ bid: { id: { dseq, gseq: 1, oseq: 1, provider: "akash1fake" }, state: "open", price: { denom: "uact", amount: "100" } } }];
    }

    console.log(`[5/7] Received ${bids.length} bid(s) total`);

    if (opts.bidsOnly) {
      console.log("[bids-only] Liquidity probe complete. Bid summary:");
      for (const b of bids) {
        const id = b.bid?.id || b.bid?.bid_id || {};
        const price = b.bid?.price || {};
        console.log(`  provider=${id.provider} price=${price.amount}${price.denom}/block`);
      }
      if (bids.length === 0) console.log("  (no bids)");
      await cleanup();
      process.exit(0);
    }

    const selectedBid = selectBid(bids, opts.maxSpendUakt, opts.timeoutMinutes, "akash1sjwuwre4qprcaa34f6324yz7m8nn0awvc75gp5");
    // v2.x: bid.id (not bid.bid_id)
    const bidId = selectedBid.bid.id || selectedBid.bid.bid_id;
    const provider = bidId.provider;
    const gseq = bidId.gseq;
    const pricePerBlock = Math.round(parseFloat(selectedBid.bid.price.amount));
    const blocksInSession = Math.ceil(opts.timeoutMinutes * 60 / 6);
    const estimatedCost = pricePerBlock * blocksInSession;

    console.log(`[5/7] Selected bid: provider=${provider}, price=${pricePerBlock} uact/block`);
    console.log(`[5/7] Estimated session cost: ${estimatedCost} uact (${estimatedCost / 1_000_000} ACT)`);
    console.log(`[5/7] Spend cap: ${opts.maxSpendUakt} uact — ${estimatedCost <= opts.maxSpendUakt ? "WITHIN CAP" : "OVER CAP — ABORTING"}`);

    if (estimatedCost > opts.maxSpendUakt) {
      throw new Error(`Estimated cost ${estimatedCost} exceeds cap ${opts.maxSpendUakt}`);
    }

    // 6. Create lease
    console.log("[6/8] Creating lease...");
    const leaseResult = createLease(opts, keyring, dseq, gseq, provider);
    const leaseTxHash = leaseResult?.txhash || "dry-run";
    await postAuditToMoultbook(opts, "lease_created", leaseTxHash, {
      dseq, provider, gseq, price_per_block: pricePerBlock,
      estimated_cost_uakt: estimatedCost,
    });

    // 7. Send manifest to provider (REQUIRED or lease closes with manifest_timeout)
    console.log("[7/8] Sending manifest to provider...");
    sendManifest(opts, keyring, dseq, provider);

    // Wait a few seconds for the provider to start the workload
    console.log("[7/8] Waiting 15s for workload to start...");
    await new Promise(r => setTimeout(r, 15000));

    // Query service endpoint
    const serviceUri = getServiceEndpoint(opts, keyring, dseq, provider);
    if (serviceUri) {
      console.log(`[7/8] Service endpoint: ${serviceUri}`);
    }

    // 8. Set auto-close timer
    console.log(`[8/8] Deployment active. Auto-close in ${opts.timeoutMinutes} minutes.`);
    console.log(`[8/8] Provider: ${provider}`);
    console.log(`[8/8] To monitor: ${opts.akashBin} query lease status --dseq ${dseq} --provider ${provider} --node ${opts.akashNode}`);

    // Post deployment live audit
    await postAuditToMoultbook(opts, "deployment_active", leaseTxHash, {
      dseq, provider, timeout_minutes: opts.timeoutMinutes,
      max_spend_uakt: opts.maxSpendUakt,
    });

    // Set auto-close timer
    closeTimer = setTimeout(() => {
      console.log("\n[timer] Auto-close timer expired. Closing deployment...");
      cleanup();
    }, opts.timeoutMinutes * 60 * 1000);

    // Keep process alive
    console.log("\n[running] Press Ctrl+C to close deployment and exit.\n");
    setInterval(() => {
      const status = getDeploymentStatus(opts, keyring, dseq);
      const elapsed = Math.round(process.uptime() / 60);
      console.log(`[heartbeat] dseq=${dseq} status=${status} elapsed=${elapsed}min cap=${opts.maxSpendUakt}uakt`);
    }, 60000);

  } catch (e) {
    console.error(`\n[error] ${e.message}`);
    await cleanup();
    process.exit(1);
  }
}

main().catch(e => {
  console.error(`Fatal: ${e.message}`);
  process.exit(1);
});
