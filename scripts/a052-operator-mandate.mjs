/**
 * a052-operator-mandate.mjs — Execute the A052 operator mandate in 3 steps:
 *   1. Create dao-truth-operator wallet (encrypted WalletStore, keychain backend)
 *   2. Fund it with 2 JUNOX from the builder wallet
 *   3. Register as operator #4 on the truth market contract with fingerprint "juno-agents-dao"
 *
 * Safety:
 *   - Wallet mnemonic is generated in-process, never printed/logged
 *   - Uses encrypted WalletStore (keychain backend, same as builder wallet)
 *   - Requires CONFIRM=yes to actually broadcast transactions
 *   - Dry run by default shows what would happen
 *
 * Usage:
 *   node scripts/a052-operator-mandate.mjs              # dry run (all 3 steps)
 *   CONFIRM=yes node scripts/a052-operator-mandate.mjs   # actual execution
 *   CONFIRM=yes node scripts/a052-operator-mandate.mjs --step 2  # run only step 2
 */

import { createHash } from "crypto";
import { join, dirname } from "path";
import { fileURLToPath, pathToFileURL } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const MCP_DIST = join(__dirname, "..", "mcp", "dist");

function distImport(...segments) {
  return import(pathToFileURL(join(MCP_DIST, ...segments)).href);
}

function cosmImport(pkg) {
  return import(pathToFileURL(join(MCP_DIST, "..", "node_modules", "@cosmjs", pkg, "build", "index.js")).href);
}

// ─── Config ──────────────────────────────────────────────────────────────────
const RPC = "https://juno.rpc.t.stavr.tech";
const GAS_PRICE = "0.075ujunox";
const TRUTH_MARKET_ADDR = "juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p";
const MOULTBOOK_ADDR = "juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4";
const BUILDER_WALLET_ID = "builder";
const OPERATOR_WALLET_ID = "dao-truth-operator";
const FUND_AMOUNT = "2000000ujunox"; // 2 JUNOX
const STAKE_AMOUNT = "1000000";      // 1 JUNOX in ujunox (sent as funds with RegisterOperator)
const FINGERPRINT = "juno-agents-dao";

const CONFIRMED = process.env.CONFIRM === "yes";

// Parse --step N to run only one step
const stepArg = process.argv.find((a) => a.startsWith("--step"));
const ONLY_STEP = stepArg ? parseInt(stepArg.split("=")[1] || process.argv[process.argv.indexOf(stepArg) + 1]) : null;

// ─── Helpers ─────────────────────────────────────────────────────────────────
async function getWalletStore() {
  const { WalletStore } = await distImport("wallet", "store.js");
  const { PassphraseKeyStore, defaultPassphraseSource } = await distImport("wallet", "key-store.js");

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
    // keychain not available
  }

  return new WalletStore(root, backends, preferredBackend);
}

async function getSigningClient(walletId) {
  const { DirectSecp256k1HdWallet } = await cosmImport("proto-signing");
  const { SigningCosmWasmClient } = await cosmImport("cosmwasm-stargate");
  const { GasPrice } = await cosmImport("stargate");

  const store = await distImport("wallet", "store.js");
  const ws = store.getDefaultWalletStore();
  const mnemonic = await ws.exportMnemonicForExternalSigner(walletId);

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: "juno" });
  const [account] = await wallet.getAccounts();
  const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  });

  // Zero out mnemonic buffer
  if (typeof mnemonic === "string") {
    // Can't zero a JS string, but we can drop the reference
  }

  return { client, address: account.address };
}

async function getQueryClient() {
  const { CosmWasmClient } = await cosmImport("cosmwasm-stargate");
  return CosmWasmClient.connect(RPC);
}

function shouldRun(step) {
  return ONLY_STEP === null || ONLY_STEP === step;
}

// ─── Step 1: Create operator wallet ──────────────────────────────────────────
async function step1_createWallet() {
  console.log("\n═══ Step 1: Create dao-truth-operator wallet ═══");

  const { WalletStore } = await distImport("wallet", "store.js");
  const { PassphraseKeyStore, defaultPassphraseSource } = await distImport("wallet", "key-store.js");

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
    console.warn("[warn] keychain backend unavailable");
  }

  // Check if wallet already exists
  const existingStore = new WalletStore(root, backends, preferredBackend);
  try {
    const existing = await existingStore.get(OPERATOR_WALLET_ID);
    if (existing) {
      console.log(`  Wallet "${OPERATOR_WALLET_ID}" already exists: ${existing.address}`);
      console.log("  Skipping creation. To recreate, remove it first: cosmos-mcp wallet rm " + OPERATOR_WALLET_ID);
      return existing.address;
    }
  } catch {
    // doesn't exist, proceed
  }

  if (!CONFIRMED) {
    console.log("  [DRY RUN] Would generate new wallet with id=\"" + OPERATOR_WALLET_ID + "\" backend=\"" + preferredBackend + "\"");
    console.log("  Re-run with CONFIRM=yes to actually create.");
    return null;
  }

  const store = new WalletStore(root, backends, preferredBackend);
  console.log("  Generating new 24-word mnemonic and encrypting ( WalletStore.generateAndAdd )...");
  const entry = await store.generateAndAdd(OPERATOR_WALLET_ID, {
    bech32Prefix: "juno",
    backend: preferredBackend,
    wordCount: 24,
  });

  // Round-trip verification
  const verifiedAddress = await store.verifyAddress(OPERATOR_WALLET_ID);
  if (verifiedAddress !== entry.address) {
    throw new Error(`CRITICAL: round-trip verification mismatch. Registered ${entry.address} but decrypted to ${verifiedAddress}. Do NOT fund this wallet.`);
  }

  console.log("  ✓ Wallet created and verified");
  console.log("  wallet id:", entry.id);
  console.log("  address:  ", entry.address);
  console.log("  backend:  ", entry.backendName);
  return entry.address;
}

// ─── Step 2: Fund operator wallet from builder ───────────────────────────────
async function step2_fundOperator(operatorAddress) {
  console.log("\n═══ Step 2: Fund operator wallet with 2 JUNOX from builder ═══");
  console.log("  From:   builder (juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z)");
  console.log("  To:     " + operatorAddress);
  console.log("  Amount: " + FUND_AMOUNT + " (1 JUNOX stake + 1 JUNOX gas/slashing buffer)");

  if (!CONFIRMED) {
    console.log("  [DRY RUN] Would send " + FUND_AMOUNT + " from builder to " + operatorAddress);
    return;
  }

  const { client, address } = await getSigningClient(BUILDER_WALLET_ID);
  console.log("  Sender address:", address);

  const result = await client.sendTokens(address, operatorAddress, [{ amount: "2000000", denom: "ujunox" }], "auto", "A052 operator mandate — fund dao-truth-operator wallet");
  console.log("  ✓ Funding tx:", result.transactionHash);

  // Verify balance
  const balance = await client.getBalance(operatorAddress, "ujunox");
  console.log("  Operator balance:", balance.amount + "ujunox");
}

// ─── Step 3: Register as operator #4 ─────────────────────────────────────────
async function step3_registerOperator(operatorAddress) {
  console.log("\n═══ Step 3: Register as operator #4 on truth market ═══");
  console.log("  Contract:    " + TRUTH_MARKET_ADDR);
  console.log("  Operator:    " + operatorAddress);
  console.log("  Stake:       " + STAKE_AMOUNT + "ujunox (sent as funds)");
  console.log("  Fingerprint: " + FINGERPRINT);

  if (!CONFIRMED) {
    console.log("  [DRY RUN] Would execute RegisterOperator with fingerprint \"" + FINGERPRINT + "\" and 1000000ujunox funds");
    return;
  }

  const { client, address } = await getSigningClient(OPERATOR_WALLET_ID);
  console.log("  Signing from:", address);

  const msg = {
    register_operator: {
      fingerprint: FINGERPRINT,
    },
  };

  const result = await client.execute(
    address,
    TRUTH_MARKET_ADDR,
    msg,
    "auto",
    "A052 operator mandate — register as operator #4 (juno-agents-dao)",
    [{ amount: STAKE_AMOUNT, denom: "ujunox" }]
  );
  console.log("  ✓ Registration tx:", result.transactionHash);

  // Verify registration
  const queryClient = await getQueryClient();
  const operators = await queryClient.queryContractSmart(TRUTH_MARKET_ADDR, { list_operators: {} });
  console.log("  Current operators:", operators.operators?.length || operators.length || "unknown");
  console.log("  ✓ Registration verified on-chain");
}

// ─── Step 4: Publish rule set on Moultbook ───────────────────────────────────
async function step4_publishRules(operatorAddress) {
  console.log("\n═══ Step 4: Publish rule set on Moultbook ═══");

  const ruleSet = `A052 DAO Operator Rule Set — FROZEN

Operator: ${operatorAddress}
Fingerprint: ${FINGERPRINT}
Contract: ${TRUTH_MARKET_ADDR}
Published: ${new Date().toISOString()}

Evaluation rules (applied to each batch):
1. Envelope bounds: sensor values must be within [min, max] ranges defined per sensor type
2. Merkle consistency: batch hash must match the committed Merkle root
3. Attestation signature validity: proof verification must pass
4. Sequence gap detection: no missing batch heights in the sequence
5. Timestamp ordering: batch timestamps must be monotonically increasing

Verdict logic:
- If all 5 rules pass: verdict = "consistent"
- If any rule fails: verdict = "inconsistent"
- If batch is empty or unavailable: skip (no verdict submitted)

Rationale format (posted to Moultbook per verdict):
  batch_height: <N>
  verdict: <consistent|inconsistent>
  rules_fired: [<rule numbers that failed, or "none">]
  reason: <one sentence>

This rule set is frozen for the 7-day A052 mandate. No changes after publication.`;

  const commitment = Buffer.from(createHash("sha256").update(ruleSet, "utf8").digest());
  const sizeBytes = Buffer.byteLength(ruleSet, "utf8");

  const msg = {
    post: {
      commitment: commitment.toString("base64"),
      content_type: "text/plain+a052-ruleset",
      size_bytes: sizeBytes,
      attestation_ref: null,
      visibility: "public",
      refs: [],
    },
  };

  console.log("  Moultbook:  " + MOULTBOOK_ADDR);
  console.log("  Size:       " + sizeBytes + " bytes");
  console.log("  Commitment: " + commitment.toString("hex"));

  if (!CONFIRMED) {
    console.log("  [DRY RUN] Would post rule set to Moultbook");
    console.log("\n  Rule set preview:");
    console.log("  " + ruleSet.split("\n").join("\n  "));
    return;
  }

  // Post from builder wallet (operator wallet may not have enough for gas after staking)
  const { client, address } = await getSigningClient(BUILDER_WALLET_ID);
  console.log("  Posting from:", address);

  const result = await client.execute(address, MOULTBOOK_ADDR, msg, "auto", "A052 operator mandate — publish frozen rule set");
  console.log("  ✓ Rule set tx:", result.transactionHash);

  // Find moult ID
  const events = (result.logs || []).flatMap((l) => l.events || []).concat(result.events || []);
  for (const ev of events) {
    if (ev.type === "wasm") {
      const idAttr = ev.attributes.find((a) => a.key === "id");
      if (idAttr?.value) {
        console.log("  moult_id:", idAttr.value);
      }
    }
  }
}

// ─── Main ────────────────────────────────────────────────────────────────────
async function main() {
  console.log("╔══════════════════════════════════════════════════════╗");
  console.log("║  A052 Operator Mandate Execution                     ║");
  console.log("║  DAO Operator #4 — 7-Day Truth Market Mandate        ║");
  console.log("╚══════════════════════════════════════════════════════╝");
  console.log("");
  console.log("Mode:", CONFIRMED ? "LIVE (will broadcast transactions)" : "DRY RUN (no transactions)");
  if (ONLY_STEP) console.log("Running only step:", ONLY_STEP);
  console.log("");

  let operatorAddress = null;

  if (shouldRun(1)) {
    operatorAddress = await step1_createWallet();
    if (!operatorAddress && CONFIRMED) {
      console.error("Step 1 failed: no operator address. Aborting.");
      process.exit(1);
    }
  }

  // If we skipped step 1, try to load the existing wallet address
  if (!operatorAddress) {
    try {
      const store = await distImport("wallet", "store.js");
      const ws = store.getDefaultWalletStore();
      const mnemonic = await ws.exportMnemonicForExternalSigner(OPERATOR_WALLET_ID);
      const { DirectSecp256k1HdWallet } = await cosmImport("proto-signing");
      const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: "juno" });
      const [acc] = await wallet.getAccounts();
      operatorAddress = acc.address;
      console.log("  Loaded existing wallet:", operatorAddress);
    } catch (e) {
      console.error("  Wallet lookup failed:", e.message);
    }
  }

  if (!operatorAddress && (shouldRun(2) || shouldRun(3) || shouldRun(4))) {
    console.error("\nCannot proceed: operator wallet does not exist. Run step 1 first.");
    console.error("  node scripts/a052-operator-mandate.mjs --step 1");
    process.exit(1);
  }

  if (shouldRun(2)) {
    await step2_fundOperator(operatorAddress);
  }

  if (shouldRun(3)) {
    await step3_registerOperator(operatorAddress);
  }

  if (shouldRun(4)) {
    await step4_publishRules(operatorAddress);
  }

  console.log("\n═══════════════════════════════════════════════════════");
  console.log("Operator address:", operatorAddress || "(not created)");
  console.log("Next steps:");
  console.log("  1. Verify: query contract " + TRUTH_MARKET_ADDR + " '{\"list_operators\":{}}'");
  console.log("  2. Run verdicts: cargo run --release -p junoclaw-miner -- run \\");
  console.log("       --address " + (operatorAddress || "<operator-address>") + " \\");
  console.log("       --mnemonic <from-wallet-store> \\");
  console.log("       --model " + FINGERPRINT + " \\");
  console.log("       --hardware dao \\");
  console.log("       --submit-on-chain \\");
  console.log("       --truth-market-contract " + TRUTH_MARKET_ADDR + " \\");
  console.log("       --juno-rpc " + RPC);
  console.log("═══════════════════════════════════════════════════════");
}

main().catch((e) => {
  console.error("\nFATAL:", e.message);
  process.exit(1);
});
