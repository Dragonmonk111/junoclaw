#!/usr/bin/env node
/**
 * deploy-flux-and-generate.mjs — End-to-end Flux deployment + image generation.
 *
 * Steps:
 *   1. Import wallet from mnemonic (AKASH_MNEMONIC env var)
 *   2. Create client certificate
 *   3. Create deployment from SDL
 *   4. Wait for bids, accept cheapest GPU provider
 *   5. Wait for container to be ready
 *   6. Send all 13 prompts to Flux API
 *   7. Save PNGs to article-images/
 *   8. Close deployment (stop billing)
 *
 * Usage:
 *   Set AKASH_MNEMONIC in your terminal, then:
 *   node tools/akash/deploy-flux-and-generate.mjs
 *
 * Or inline:
 *   $env:AKASH_MNEMONIC="your mnemonic words here"; node tools/akash/deploy-flux-and-generate.mjs
 */

import { execFileSync, execSync } from "child_process";
import { writeFileSync, readFileSync, unlinkSync, existsSync, mkdtempSync, rmSync, mkdirSync } from "fs";
import { join, dirname, resolve } from "path";
import { tmpdir } from "os";
import { fileURLToPath, pathToFileURL } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = resolve(__dirname, "..", "..");
const MCP_DIST = join(PROJECT_ROOT, "mcp", "dist");
function distImport(...segments) {
  return import(pathToFileURL(join(MCP_DIST, ...segments)).href);
}

const SDL_PATH = join(PROJECT_ROOT, "deploy", "flux-akash-sdl.yaml");
const IMAGES_DIR = join(PROJECT_ROOT, "article-images");
const GENERATE_SCRIPT = join(PROJECT_ROOT, "tools", "generate_images.py");

const AKASH_NODE = "https://akash-rpc.polkachu.com:443";
const AKASH_CHAIN_ID = "akashnet-2";
const WSL_DISTRO = process.env.WSL_DISTRO || "Ubuntu-24.04";
const isWindows = process.platform === "win32";

// ─── Prompts (same as generate_images.py) ───
const PROMPTS = [
  { id: "scaling_01_cover", prompt: "A lone humanoid robot standing at the edge of a vast desert at dawn, its chest cavity open revealing a glowing geometric proof — a small crystalline 128-byte shard emitting blue light, behind the robot a trail of footprints each containing a tiny glowing Merkle tree root, the desert stretches to a horizon where a massive blockchain consensus tower rises like a mirage, in the style of a 2D hand-drawn manga cross-section meets ukiyo-e woodblock print, warm sepia and sand tones with electric blue accents for the proof shard, wide cinematic composition, wabi-sabi imperfection in every line" },
  { id: "scaling_01b_unitree", prompt: "A massive Shanghai stock exchange building made of brass and glass, its digital ticker showing a humanoid robot company logo and a 600% upward green arrow, tiny figures of investors in suits looking up at the ticker, a humanoid robot standing on a pedestal in front of the building holding a small glowing 128-byte proof shard in one hand, the scene is split between the chaotic market floor below and the calm blue light of the proof shard above, in the style of Japanese financial newspaper illustration meets cyberpunk manga, sepia and black ink with green and electric blue accents, dramatic perspective" },
  { id: "scaling_02_circuits", prompt: "An exploded isometric diagram of five nested crystalline circuits arranged in ascending order like a Japanese pagoda, each floor glowing with a different color — bottom floor amber with sensor waveform patterns, second floor green with zone boundary lines, third floor deep blue with validator key icons, fourth floor violet with interlocking hash chains connecting all floors, a tiny 128-byte shard floats above the top like a star, in the style of architectural blueprint meets manga technical illustration, ink outlines with watercolor fills, each floor labeled with tiny kanji-like symbols representing its function, dark background with the circuits glowing" },
  { id: "scaling_03_benchmark", prompt: "A traditional Japanese stopwatch made of brass and crystal, its face divided into five concentric rings each measuring a different duration, the outer ring shows 80ms in amber, the next 119ms in green, then 51ms in blue, then 68ms in violet, the center shows 187ms in white with a small 128-byte crystal shard at the exact center, the stopwatch sits on a wooden workbench surrounded by scattered circuit diagrams and empty tea cups, in the style of 19th century scientific instrument illustration meets manga panel design, warm sepia with the crystal shard glowing electric blue" },
  { id: "scaling_04_ages", prompt: "A sweeping panoramic landscape divided into five horizontal bands representing five ages of robotics hardware, bottom band shows a single small robot in a workshop with hand tools, second band shows a fleet of robots in a warehouse with basic GPUs visible, third band shows a futuristic factory with GPU racks alongside robots, fourth band shows a vast city with thousands of robots and data centers glowing, top band shows an abstract cosmic-scale network of robots covering a planet surface with ASIC chips embedded in the ground, each band transitions smoothly into the next like a geological stratum, in the style of Hokusai's Great Wave but vertical showing time ascending, sepia at bottom transitioning to electric blue at top, hand-drawn manga linework throughout" },
  { id: "scaling_05_proof_size", prompt: "A tiny crystalline shard no larger than a rice grain held between chopsticks above a traditional Japanese tea ceremony table, the shard emits a soft blue glow containing mathematical symbols and elliptic curve points, on the table below are scattered large scrolls of sensor data and rosbag files representing the full robot telemetry, a magnifying glass reveals that the shard contains exactly three G1 points and three Fq scalars, in the style of Japanese still life illustration meets technical diagram, warm cream and sand colors with the shard glowing electric blue, extreme close-up composition with shallow depth of field" },
  { id: "scaling_06_trust_spectrum", prompt: "A horizontal spectrum diagram drawn in ink on rice paper, left side labeled TEE in warm amber showing a hardware lockbox with a glowing seal, right side labeled PURE CRYPTO in cool blue showing mathematical equations floating freely, five stepping stones cross the spectrum from left to right each labeled with a different approach, at the far right a small figure reaches the shore of pure cryptography, in the style of Japanese ink painting meets mathematical diagram, minimal color with amber on left transitioning to blue on right, contemplative mood" },
  { id: "scaling_07_epilogue", prompt: "A quiet scene at dusk: a single robot sitting motionless in a zen garden, its chest panel closed but a faint blue glow visible through the seams showing the proof shard inside, before the robot a trail of Merkle tree roots fades into the raked sand like footprints being washed by rain, in the distance a consensus tower glows softly on the horizon, cherry blossoms drift through the scene carrying tiny 128-byte shards, the robot is still but not off, in the style of minimalist Japanese ink painting with maximum negative space, black ink on cream paper with only electric blue for the proof glow and pale pink for blossoms, wabi-sabi imperfection, the feeling of quiet trust" },
  { id: "melange_01_cover", prompt: "A vast hexagonal tower rising from a desert floor at golden hour, six distinct horizontal bands of light glowing within its crystalline structure — bottom band warm amber with tiny glowing contract glyphs, second band green with interconnected node meshes, third band deep blue with waveform patterns, fourth band violet with elliptic curve symbols, fifth band indigo with lattice cryptography patterns, top band bright electric blue with a humanoid robot silhouette operating machinery, at the very peak a small 128-byte crystalline shard emits a beam of light into the sky, in the style of Japanese woodblock print meets architectural cross-section diagram, warm sepia base transitioning to cool blues at the top, hand-drawn linework with watercolor fills, wide cinematic composition" },
  { id: "melange_02_contracts", prompt: "Four glowing stone tablets arranged in a semicircle inside a traditional Japanese shrine, each tablet inscribed with different glowing symbols — the first showing a registry of names with light connections branching outward, the second showing a crystal lens refracting proof-light into rainbow bands, the third showing a soulbound thread weaving through a key, the fourth showing coral-like memory structures recording pulses, small mechanical sprites tend to each tablet, the shrine is made of dark wood with paper lanterns providing warm amber light, in the style of ukiyo-e woodblock print meets technical illustration, sepia and amber tones with electric blue accents for the glowing symbols, contemplative and sacred mood" },
  { id: "melange_03_ledger", prompt: "A long horizontal scroll made of aged rice paper unrolled across a wooden workbench, ten distinct contract seals stamped along its length in two rows, the top four seals glow with steady amber light indicating mainnet deployment, the bottom six seals glow with softer green light indicating testnet or ready status, each seal contains a tiny pictogram representing its function, an ink brush moves across the scroll writing gas measurements next to each seal, in the style of Japanese calligraphy illustration meets infographic, warm cream and amber tones with green and blue accents for the seals" },
  { id: "melange_04_delivery", prompt: "An exploded isometric diagram of a warehouse robot deployment, at the center a humanoid robot on a factory floor surrounded by sensor halos, to the left a Python bridge represented as a traditional Japanese wooden bridge with data streams flowing across it like water, to the right a Rust prover daemon represented as a small forge hammering crystalline proof shards, below them a Docker container represented as a lacquered bento box containing the full stack neatly organized, above the robot a fleet dashboard represented as a paper lantern showing green and red status indicators, in the background a chain of mountains representing the Juno blockchain with blocks stacked like stone pagodas, in the style of technical illustration meets Japanese landscape painting, warm industrial tones with electric blue for proof shards and green for status indicators, clean isometric perspective" },
  { id: "melange_05_built", prompt: "A traditional Japanese workshop at dawn with every tool hung neatly on the wall in its proper place, the walls are covered with completed work — five framed circuit diagrams glowing with blue light, a shelf holding four mainnet contract tablets lit with amber, a forge with proof shards cooling on the anvil, a wooden bridge model with data streams, a bento box with the full deployment stack, a paper lantern fleet dashboard glowing green, a key cabinet with organized robot keys, a compliance scroll with ISO stamps, a brass cost calculator, and a tiny crystalline soak-test hourglass still running with sand flowing, through the window the Juno blockchain mountains are visible with blocks stacking in real time, in the style of Japanese workshop illustration meets technical diagram, warm amber and wood tones with electric blue accents for the technology elements, everything in its place, nothing unfinished, the feeling of quiet competence" },
];

// ─── WSL path translation ───
function toWslPath(p) {
  if (!isWindows) return p;
  const match = p.match(/^([A-Za-z]):[\\\/](.*)/);
  if (!match) return p;
  return `/mnt/${match[1].toLowerCase()}/${match[2].replace(/\\/g, "/")}`;
}

function execAkash(args, opts = {}) {
  if (isWindows) {
    const wslArgs = args.map(a => {
      if (typeof a === "string" && /^[A-Za-z]:[\\\/]/.test(a)) return toWslPath(a);
      return a;
    });
    return execFileSync("wsl.exe", ["-d", WSL_DISTRO, "--", "akash", ...wslArgs], {
      encoding: "utf-8",
      timeout: opts.timeout || 120000,
      maxBuffer: 10 * 1024 * 1024,
      input: opts.input,
    }).trim();
  }
  return execFileSync("akash", args, { encoding: "utf-8", timeout: opts.timeout || 120000, maxBuffer: 10 * 1024 * 1024, input: opts.input }).trim();
}

function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

// ─── Main ───
async function main() {
  // Decrypt mnemonic from encrypted WalletStore (same as auto-deploy.mjs)
  console.log("[0/8] Decrypting wallet from WalletStore...");
  const { getDefaultWalletStore } = await distImport("wallet", "store.js");
  const ws = getDefaultWalletStore();
  let mnemonic;
  try {
    mnemonic = await ws.exportMnemonicForExternalSigner("akash-jlens");
  } catch (e) {
    console.error("ERROR: Could not export mnemonic from WalletStore for 'akash-jlens'.");
    console.error("       " + e.message);
    console.error("");
    console.error("Fallback: Set AKASH_MNEMONIC env var manually:");
    console.error("  $env:AKASH_MNEMONIC='word1 word2 ...'; node tools/akash/deploy-flux-and-generate.mjs");
    // Try env var fallback
    mnemonic = process.env.AKASH_MNEMONIC;
    if (!mnemonic || mnemonic.length < 20) {
      process.exit(1);
    }
  }
  console.log("  Wallet decrypted (24-word mnemonic).");

  console.log("=== Flux on Akash: Deploy + Generate Images ===\n");

  // Step 1: Import wallet into temp keyring
  console.log("[1/8] Importing wallet into temp keyring...");
  const keyringDir = mkdtempSync(join(tmpdir(), "akash-flux-"));
  const keyringHome = isWindows ? toWslPath(keyringDir) : keyringDir;
  const keyName = "flux-deployer";
  const mnemonicFile = join(keyringDir, "mnemonic.txt");
  writeFileSync(mnemonicFile, mnemonic + "\n", { mode: 0o600 });

  try {
    execAkash(["keys", "add", keyName, "--recover", "--keyring-backend", "test", "--home", keyringHome], {
      input: mnemonic + "\n",
      timeout: 30000,
    });
    const addrRaw = execAkash(["keys", "show", keyName, "--keyring-backend", "test", "--home", keyringHome, "--output", "json"], { timeout: 15000 });
    const addr = JSON.parse(addrRaw).address;
    console.log(`  Wallet: ${addr}`);

    // Check balance
    const balRaw = execAkash(["query", "bank", "balances", addr, "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID, "--output", "json"], { timeout: 30000 });
    const bal = JSON.parse(balRaw);
    const uakt = bal.balances?.find(b => b.denom === "uakt")?.amount || "0";
    console.log(`  Balance: ${parseInt(uakt) / 1000000} AKT (${uakt} uakt)`);

    if (parseInt(uakt) < 5000000) {
      console.error(`  ERROR: Need at least 5 AKT for deposit. You have ${parseInt(uakt) / 1000000} AKT.`);
      process.exit(1);
    }

    // Step 2: Create client certificate
    console.log("\n[2/8] Creating client certificate...");
    const certArgs = ["tx", "cert", "generate", "client", "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID,
      "--from", keyName, "--keyring-backend", "test", "--home", keyringHome,
      "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "80000uakt", "--output", "json", "-y"];
    try { execAkash(certArgs, { timeout: 30000 }); } catch (e) { console.log("  (cert may already exist, continuing)"); }

    const pubArgs = ["tx", "cert", "publish", "client", "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID,
      "--from", keyName, "--keyring-backend", "test", "--home", keyringHome,
      "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "80000uakt", "--output", "json", "-y"];
    try { execAkash(pubArgs, { timeout: 120000 }); } catch (e) { console.log("  (cert publish may already be done, continuing)"); }
    console.log("  Certificate ready.");

    // Step 3: Create deployment
    console.log("\n[3/8] Creating deployment from SDL...");
    const sdlWslPath = toWslPath(SDL_PATH);
    const deployArgs = ["tx", "deployment", "create", sdlWslPath,
      "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID,
      "--from", keyName, "--keyring-backend", "test", "--home", keyringHome,
      "--deposit", "5000000uact",
      "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "80000uakt",
      "--output", "json", "-y"];
    const deployResult = execAkash(deployArgs, { timeout: 120000 });
    console.log("  Deployment created.");

    // Extract dseq from events
    let dseq = null;
    try {
      const parsed = JSON.parse(deployResult);
      const event = parsed?.events?.find(e => e.type === "akash.v1.EventDeploymentCreated");
      dseq = event?.attributes?.find(a => a.key === "dseq")?.value;
    } catch {}
    if (!dseq) {
      // Try to get it from the tx events query
      try {
        const txHash = JSON.parse(deployResult)?.txhash;
        if (txHash) {
          const txResult = execAkash(["query", "tx", txHash, "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID, "--output", "json"], { timeout: 30000 });
          const txParsed = JSON.parse(txResult);
          const evt = txParsed?.events?.find(e => e.type === "akash.v1.EventDeploymentCreated");
          dseq = evt?.attributes?.find(a => a.key === "dseq")?.value;
        }
      } catch {}
    }
    if (!dseq) {
      // Fallback: query deployments for this owner
      const depList = execAkash(["query", "deployment", "list", "--owner", addr, "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID, "--output", "json"], { timeout: 30000 });
      const deps = JSON.parse(depList);
      const active = deps.deployments?.find(d => d.deployment?.state === "active");
      dseq = active?.deployment?.id?.dseq;
    }
    if (!dseq) {
      console.error("  ERROR: Could not determine deployment sequence (dseq).");
      console.error("  Raw result:", deployResult.substring(0, 500));
      process.exit(1);
    }
    console.log(`  DSEQ: ${dseq}`);

    // Step 4: Wait for bids and accept cheapest
    console.log("\n[4/8] Waiting for provider bids (30s)...");
    await sleep(30000);

    const bidsRaw = execAkash(["query", "bid", "list", "--owner", addr, "--dseq", dseq,
      "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID, "--output", "json"], { timeout: 30000 });
    const bids = JSON.parse(bidsRaw);
    const openBids = bids.bids?.filter(b => b.bid?.state === "open") || [];

    if (openBids.length === 0) {
      console.error("  No open bids found. Waiting 30 more seconds...");
      await sleep(30000);
      const bidsRaw2 = execAkash(["query", "bid", "list", "--owner", addr, "--dseq", dseq,
        "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID, "--output", "json"], { timeout: 30000 });
      const bids2 = JSON.parse(bidsRaw2);
      openBids.push(...(bids2.bids?.filter(b => b.bid?.state === "open") || []));
    }

    if (openBids.length === 0) {
      console.error("  ERROR: No bids received. GPU providers may not be available.");
      console.error("  Closing deployment to refund deposit...");
      execAkash(["tx", "deployment", "close", "--dseq", dseq,
        "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID,
        "--from", keyName, "--keyring-backend", "test", "--home", keyringHome,
        "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "80000uakt", "--output", "json", "-y"], { timeout: 60000 });
      process.exit(1);
    }

    // Sort by price (cheapest first)
    openBids.sort((a, b) => {
      const pa = parseFloat(a.bid?.price?.amount || "999999999");
      const pb = parseFloat(b.bid?.price?.amount || "999999999");
      return pa - pb;
    });

    console.log(`  Found ${openBids.length} bids:`);
    for (const b of openBids) {
      console.log(`    ${b.bid?.id?.provider}: ${b.bid?.price?.amount} ${b.bid?.price?.denom}`);
    }

    const chosenBid = openBids[0];
    const provider = chosenBid.bid?.id?.provider;
    console.log(`\n  Accepting cheapest bid: ${provider} at ${chosenBid.bid?.price?.amount} ${chosenBid.bid?.price?.denom}`);

    execAkash(["tx", "market", "lease", "create", "--dseq", dseq, "--gseq", "1",
      "--provider", provider,
      "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID,
      "--from", keyName, "--keyring-backend", "test", "--home", keyringHome,
      "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "80000uakt", "--output", "json", "-y"], { timeout: 120000 });
    console.log("  Lease accepted!");

    // Step 5: Wait for container to be ready
    console.log("\n[5/8] Waiting for Flux container to start (checking every 30s, up to 10 min)...");
    let providerUri = null;
    for (let attempt = 1; attempt <= 20; attempt++) {
      await sleep(30000);
      console.log(`  Attempt ${attempt}/20: Checking lease status...`);
      try {
        const statusRaw = execAkash(["query", "lease", "status", "--dseq", dseq, "--gseq", "1",
          "--provider", provider,
          "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID, "--output", "json"], { timeout: 30000 });
        const status = JSON.parse(statusRaw);
        // Look for forwarded ports / URIs
        const services = status?.services || [];
        for (const svc of services) {
          if (svc.uris?.length > 0) {
            providerUri = svc.uris[0];
            break;
          }
        }
        if (!providerUri && status?.forwarded_ports) {
          // Try to construct URI from forwarded ports
        }
        if (providerUri) {
          console.log(`  Found URI: ${providerUri}`);
          break;
        }
        // Also try: the lease status may have a different structure
        const matched = JSON.stringify(status).match(/https?:\/\/[^"\\]+/);
        if (matched) {
          providerUri = matched[0];
          console.log(`  Found URI: ${providerUri}`);
          break;
        }
        console.log(`  Services: ${services.length}, state: ${services[0]?.state || "unknown"}`);
      } catch (e) {
        console.log(`  Status query failed: ${e.message?.substring(0, 100)}`);
      }
    }

    if (!providerUri) {
      // Try to get the URI from lease list
      console.log("  Trying lease list for URIs...");
      try {
        const leaseList = execAkash(["query", "lease", "list", "--owner", addr,
          "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID, "--output", "json"], { timeout: 30000 });
        const matched = leaseList.match(/https?:\/\/[^"\\]+/);
        if (matched) providerUri = matched[0];
      } catch {}
    }

    if (!providerUri) {
      console.error("  ERROR: Could not find provider URI after 10 minutes.");
      console.error("  The container may still be downloading the model (Flux is ~12GB).");
      console.error("  You can check manually with:");
      console.error(`    akash query lease status --dseq ${dseq} --gseq 1 --provider ${provider} --node ${AKASH_NODE}`);
      console.error("");
      console.error("  Once you have the URI, run:");
      console.error(`    python tools/generate_images.py --flux-url http://<uri> --output article-images`);
      console.error("");
      console.error("  To close the deployment later:");
      console.error(`    akash tx deployment close --dseq ${dseq} --node ${AKASH_NODE} --chain-id ${AKASH_CHAIN_ID} --from <key> -y`);
      // Don't exit — let the user handle it
      process.exit(1);
    }

    // Ensure URI has scheme
    if (!providerUri.startsWith("http")) {
      providerUri = "http://" + providerUri;
    }

    // Step 6: Wait for Flux model to load (it needs to download ~12GB)
    console.log(`\n[6/8] Waiting for Flux model to load at ${providerUri}...`);
    console.log("  (Flux.1-schnell is ~12GB, this can take 5-15 min depending on provider)");
    let fluxReady = false;
    for (let attempt = 1; attempt <= 30; attempt++) {
      await sleep(30000);
      console.log(`  Health check ${attempt}/30...`);
      try {
        const http = await import("http");
        const ok = await new Promise((resolve) => {
          const url = new URL(providerUri);
          const req = http.default.request({
            hostname: url.hostname,
            port: url.port || 80,
            path: "/",
            method: "GET",
            timeout: 10000,
          }, (res) => {
            res.resume();
            resolve(res.statusCode < 500);
          });
          req.on("error", () => resolve(false));
          req.on("timeout", () => { req.destroy(); resolve(false); });
          req.end();
        });
        if (ok) {
          console.log("  Flux service is responding!");
          fluxReady = true;
          break;
        }
      } catch {
        // Not ready yet
      }
    }

    if (!fluxReady) {
      console.error("  WARNING: Flux service not responding after 15 min.");
      console.error("  It may still be loading. You can try generating images manually:");
      console.error(`    python tools/generate_images.py --flux-url ${providerUri} --output article-images`);
    }

    // Step 7: Generate all images
    console.log(`\n[7/8] Generating 13 images via Flux at ${providerUri}...`);
    mkdirSync(IMAGES_DIR, { recursive: true });

    const http = await import("http");
    let succeeded = 0, failed = 0;

    for (let i = 0; i < PROMPTS.length; i++) {
      const p = PROMPTS[i];
      const outputPath = join(IMAGES_DIR, `${p.id}.png`);
      console.log(`\n  [${i + 1}/${PROMPTS.length}] ${p.id}`);

      if (existsSync(outputPath)) {
        console.log("    Already exists, skipping.");
        succeeded++;
        continue;
      }

      try {
        const imageBuffer = await new Promise((resolve, reject) => {
          const url = new URL(providerUri);
          const body = JSON.stringify({ prompt: p.prompt });
          const req = http.default.request({
            hostname: url.hostname,
            port: url.port || 80,
            path: "/text-to-image",
            method: "POST",
            headers: { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body) },
            timeout: 120000,
          }, (res) => {
            const chunks = [];
            res.on("data", (c) => chunks.push(c));
            res.on("end", () => {
              const buf = Buffer.concat(chunks);
              const ct = res.headers["content-type"] || "";
              if (ct.includes("image")) {
                resolve(buf);
              } else if (ct.includes("json")) {
                try {
                  const j = JSON.parse(buf.toString());
                  if (j.image) {
                    resolve(Buffer.from(j.image, "base64"));
                  } else if (j.url) {
                    // Fetch the URL
                    const imgUrl = new URL(j.url, providerUri);
                    http.default.get(imgUrl, (imgRes) => {
                      const imgChunks = [];
                      imgRes.on("data", (c) => imgChunks.push(c));
                      imgRes.on("end", () => resolve(Buffer.concat(imgChunks)));
                    }).on("error", reject);
                  } else {
                    reject(new Error(`Unexpected JSON: ${Object.keys(j).join(", ")}`));
                  }
                } catch (e) { reject(e); }
              } else if (buf.length > 1000) {
                resolve(buf); // Assume it's an image
              } else {
                reject(new Error(`Unexpected response: ${ct}, ${buf.length} bytes`));
              }
            });
          });
          req.on("error", reject);
          req.on("timeout", () => { req.destroy(); reject(new Error("Request timed out")); });
          req.write(body);
          req.end();
        });

        writeFileSync(outputPath, imageBuffer);
        console.log(`    ✓ Saved: ${p.id}.png (${imageBuffer.length} bytes)`);
        succeeded++;
      } catch (e) {
        console.log(`    ✗ Failed: ${e.message?.substring(0, 200)}`);
        // Save prompt for retry
        writeFileSync(join(IMAGES_DIR, `${p.id}.txt`), p.prompt);
        failed++;
      }
    }

    console.log(`\n  Results: ${succeeded} succeeded, ${failed} failed`);

    // Step 8: Close deployment
    console.log("\n[8/8] Closing deployment to stop billing...");
    try {
      execAkash(["tx", "deployment", "close", "--dseq", dseq,
        "--node", AKASH_NODE, "--chain-id", AKASH_CHAIN_ID,
        "--from", keyName, "--keyring-backend", "test", "--home", keyringHome,
        "--gas", "auto", "--gas-adjustment", "1.5", "--fees", "80000uakt", "--output", "json", "-y"], { timeout: 60000 });
      console.log("  Deployment closed. Remaining AKT refunded to wallet.");
    } catch (e) {
      console.error("  WARNING: Failed to close deployment. Close manually:");
      console.error(`    akash tx deployment close --dseq ${dseq} --node ${AKASH_NODE} --chain-id ${AKASH_CHAIN_ID} --from <key> -y`);
    }

    console.log(`\n=== Done! ===`);
    console.log(`Images saved to: ${IMAGES_DIR}`);
    if (failed > 0) {
      console.log(`\nFailed prompts saved as .txt files. Retry with:`);
      console.log(`  python tools/generate_images.py --flux-url ${providerUri} --output ${IMAGES_DIR}`);
    }
  } finally {
    // Cleanup: scrub mnemonic file and keyring
    try { unlinkSync(mnemonicFile); } catch {}
    try { rmSync(keyringDir, { recursive: true, force: true }); } catch {}
    console.log("\n[cleanup] Temp keyring and mnemonic scrubbed.");
  }
}

main().catch(e => {
  console.error("Fatal error:", e);
  process.exit(1);
});
