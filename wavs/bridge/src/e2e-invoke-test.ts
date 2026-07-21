/**
 * End-to-end invoke flow test on uni-7 testnet.
 *
 * Flow:
 *   1. Load operator wallet from .env, check balance on uni-7
 *   2. Generate a sealed signer key via wasmtime → get address + sealed blob
 *   3. Fund the sealed signer address from operator wallet (send ujunox)
 *   4. Start the invoke server with the sealed blob
 *   5. Call POST /invoke/sealed-signer with a test Moultbook post message
 *   6. Broadcast the returned tx_bytes on uni-7
 *   7. Verify the transaction landed on-chain
 *
 * Safety:
 *   - Uses a fresh sealed signer key each run (wasi:random non-deterministic keygen)
 *   - Signing is deterministic (proven by determinism-test.js)
 *   - Posts a harmless test commitment to the testnet Moultbook contract
 *   - All funds are testnet ujunox (no real value)
 *
 * Usage:
 *   npx tsx src/e2e-invoke-test.ts
 *
 * Env vars (loaded from wavs/bridge/.env or wavs/.env):
 *   WAVS_OPERATOR_MNEMONIC — operator wallet with ujunox on uni-7
 *   JUNO_RPC — RPC endpoint (default: https://juno.rpc.t.stavr.tech)
 *   MOULTBOOK_ADDR — moultbook contract on uni-7 (default: testnet deploy)
 */

import dotenv from "dotenv";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { spawnSync } from "child_process";
import { createServer, IncomingMessage, ServerResponse } from "http";
import { timingSafeEqual } from "crypto";
import { StargateClient, GasPrice } from "@cosmjs/stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { SigningStargateClient } from "@cosmjs/stargate";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Load .env files — try bridge .env first, then wavs/.env
dotenv.config({ path: resolve(__dirname, ".env") });
dotenv.config({ path: resolve(__dirname, "../.env") });
dotenv.config({ path: resolve(__dirname, "../../.env") });

// ── Configuration ──
const RPC_ENDPOINTS = [
  process.env.JUNO_RPC || "https://juno-testnet-rpc.polkachu.com:443",
  "https://juno.rpc.t.stavr.tech",
  "https://rpc.uni.junonetwork.io:443",
];

async function connectStargate(): Promise<StargateClient> {
  let lastErr: Error | null = null;
  for (const rpc of RPC_ENDPOINTS) {
    try {
      console.log(`  Trying RPC: ${rpc}...`);
      const client = await StargateClient.connect(rpc);
      console.log(`  Connected to ${rpc}`);
      return client;
    } catch (e) {
      lastErr = e as Error;
      console.log(`  Failed: ${(e as Error).message}`);
    }
  }
  throw new Error(`All RPC endpoints failed. Last: ${lastErr?.message}`);
}
const CHAIN_ID = "uni-7";
const DENOM = "ujunox";
const GAS_PRICE = `0.1${DENOM}`;
const MNEMONIC = process.env.WAVS_OPERATOR_MNEMONIC || process.env.JUNO_MNEMONIC || "";
const MOULTBOOK_ADDR = process.env.MOULTBOOK_ADDR || "juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4";

const WASMTIME = process.env.WAVS_INVOKE_WASMTIME || "C:\\Users\\Taj\\.cargo\\bin\\wasmtime.exe";
const WASM_PATH = resolve(__dirname, "../../sealed-signer/target/wasm32-wasip2/release/junoclaw_sealed_signer.wasm");
const PASSPHRASE = "e2e-test-passphrase";
const INVOKE_TOKEN = "e2e-test-token-at-least-32-bytes-long-aaaa-bbbb";
const FUND_AMOUNT = "1000000"; // 1 ujunox (1,000,000 ujunox = 1 JUNO)

// ── Helpers ──

function runWasmtime(invoke: string, env: Record<string, string> = {}): string {
  const args = ["run", ...Object.entries(env).flatMap(([k, v]) => ["--env", `${k}=${v}`]), "--invoke", invoke, WASM_PATH];
  const result = spawnSync(WASMTIME, args, { encoding: "utf8", maxBuffer: 10 * 1024 * 1024 });
  if (result.status !== 0) {
    throw new Error(`wasmtime failed (exit ${result.status}): ${result.stderr}`);
  }
  return result.stdout.trim();
}

function parseRecord(out: string): Record<string, any> {
  const m = out.match(/^ok\(\{(.+)\}\)$/s);
  if (!m) throw new Error(`unexpected wasmtime output: ${out}`);
  const record: Record<string, any> = {};
  const fieldRe = /(\w[\w-]*):\s*("(?:[^"\\]|\\.)*"|\[[^\]]*\])/g;
  let match;
  while ((match = fieldRe.exec(m[1])) !== null) {
    const key = match[1];
    const raw = match[2];
    if (raw.startsWith('"')) {
      record[key] = JSON.parse(raw);
    } else if (raw.startsWith('[')) {
      record[key] = raw === '[]' ? [] : raw.slice(1, -1).split(',').map(s => parseInt(s.trim(), 10));
    }
  }
  return record;
}

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}

// ── Inline invoke server (no separate process needed) ──

function startInvokeServer(sealedBlobHex: string, port: number): Promise<{ url: string; close: () => void }> {
  return new Promise((resolveP, rejectP) => {
    const server = createServer(async (req: IncomingMessage, res: ServerResponse) => {
      try {
        // Health
        if (req.method === "GET" && req.url === "/health") {
          res.writeHead(200, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ status: "ok", version: "0.1.0-e2e" }));
          return;
        }

        // Auth check
        const authHeader = req.headers.authorization;
        if (!authHeader || !authHeader.startsWith("Bearer ")) {
          res.writeHead(401, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: "missing or invalid Authorization header" }));
          return;
        }
        const token = Buffer.from(authHeader.slice(7));
        const expected = Buffer.from(INVOKE_TOKEN);
        if (token.length !== expected.length || !timingSafeEqual(token, expected)) {
          res.writeHead(401, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: "invalid token" }));
          return;
        }

        // POST /invoke/sealed-signer
        if (req.method === "POST" && req.url === "/invoke/sealed-signer") {
          const chunks: Buffer[] = [];
          for await (const chunk of req) chunks.push(chunk as Buffer);
          const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
          const input = body.input;

          const sealedBlobBytes = Buffer.from(sealedBlobHex, "hex");
          const blobList = Array.from(sealedBlobBytes).join(",");
          const fundsAmount = BigInt(input.funds_amount || "0").toString();
          const feeAmount = BigInt(input.fee_amount || "0").toString();

          const invokeExpr = `sign-cosmos-execute-tx([${blobList}], {sender: "${input.sender}", contract: "${input.contract}", exec-msg-json: ${JSON.stringify(input.exec_msg_json)}, funds-denom: "${input.funds_denom}", funds-amount: ${fundsAmount}, gas-limit: ${input.gas_limit}, fee-denom: "${input.fee_denom}", fee-amount: ${feeAmount}, memo: ${JSON.stringify(input.memo)}, chain-id: "${input.chain_id}", account-number: ${input.account_number}, sequence: ${input.sequence}})`;

          const wasmOut = runWasmtime(invokeExpr, { WAVS_ENV_SIGNER_PASSPHRASE: PASSPHRASE });
          const parsed = parseRecord(wasmOut);
          const txBytes = Buffer.from(parsed["tx-bytes"]).toString("base64");

          res.writeHead(200, { "Content-Type": "application/json" });
          res.end(JSON.stringify({
            output: {
              address: parsed.address,
              pubkey: parsed.pubkey,
              tx_bytes: txBytes,
              sign_doc_sha256_hex: parsed["sign-doc-sha256-hex"],
            },
            attestation: { attestation_hash: parsed["sign-doc-sha256-hex"] },
          }));
          return;
        }

        res.writeHead(404, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "not found" }));
      } catch (e) {
        res.writeHead(500, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: (e as Error).message }));
      }
    });

    server.on("error", rejectP);
    server.listen(port, "127.0.0.1", () => {
      const addr = server.address();
      const actualPort = typeof addr === "object" && addr ? addr.port : port;
      resolveP({
        url: `http://127.0.0.1:${actualPort}`,
        close: () => server.close(),
      });
    });
  });
}

// ── Main E2E flow ──

async function main(): Promise<void> {
  console.log("=== WAVS Invoke API End-to-End Test (uni-7) ===\n");

  if (!MNEMONIC) {
    throw new Error("WAVS_OPERATOR_MNEMONIC (or JUNO_MNEMONIC) not set in .env");
  }

  // ── Step 1: Load operator wallet, check balance ──
  console.log("Step 1: Load operator wallet and check balance...");
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: "juno" });
  const [operatorAccount] = await wallet.getAccounts();
  const operatorAddr = operatorAccount.address;
  console.log(`  Operator address: ${operatorAddr}`);

  const readOnlyClient = await connectStargate();
  const balance = await readOnlyClient.getBalance(operatorAddr, DENOM);
  console.log(`  Operator balance: ${balance.amount} ${DENOM}`);

  if (BigInt(balance.amount) < BigInt(FUND_AMOUNT)) {
    throw new Error(`Operator needs at least ${FUND_AMOUNT} ${DENOM}, has ${balance.amount}`);
  }
  console.log("  OK\n");

  // ── Step 2: Generate sealed signer key ──
  console.log("Step 2: Generate sealed signer key via wasmtime...");
  const genOut = runWasmtime("generate-key()", { WAVS_ENV_SIGNER_PASSPHRASE: PASSPHRASE });
  const keyInfo = parseRecord(genOut);
  const sealedSignerAddr = keyInfo.address;
  const sealedBlobHex = Buffer.from(keyInfo["sealed-blob"]).toString("hex");
  console.log(`  Sealed signer address: ${sealedSignerAddr}`);
  console.log(`  Sealed blob: ${sealedBlobHex.slice(0, 32)}... (${keyInfo["sealed-blob"].length} bytes)`);
  console.log("  OK\n");

  // ── Step 3: Fund the sealed signer address ──
  console.log(`Step 3: Fund sealed signer with ${FUND_AMOUNT} ${DENOM}...`);
  // Try each RPC for signing client
  let signingClient: SigningStargateClient | null = null;
  let lastSignErr: Error | null = null;
  for (const rpc of RPC_ENDPOINTS) {
    try {
      signingClient = await SigningStargateClient.connectWithSigner(rpc, wallet, {
        gasPrice: GasPrice.fromString(GAS_PRICE),
      });
      break;
    } catch (e) {
      lastSignErr = e as Error;
    }
  }
  if (!signingClient) throw new Error(`Signing client failed: ${lastSignErr?.message}`);
  const fundResult = await signingClient.sendTokens(
    operatorAddr,
    sealedSignerAddr,
    [{ amount: FUND_AMOUNT, denom: DENOM }],
    "auto",
    "e2e invoke test funding"
  );
  console.log(`  Fund tx: ${fundResult.transactionHash}`);
  console.log(`  Waiting for confirmation...`);

  // Wait for the funding tx to be included
  await sleep(6000);

  // Verify funding
  const sealedBalance = await readOnlyClient.getBalance(sealedSignerAddr, DENOM);
  console.log(`  Sealed signer balance: ${sealedBalance.amount} ${DENOM}`);
  if (BigInt(sealedBalance.amount) === 0n) {
    throw new Error("Funding failed — sealed signer balance is 0");
  }
  console.log("  OK\n");

  // ── Step 4: Get sealed signer account info ──
  console.log("Step 4: Get sealed signer account info...");
  const sealedAccount = await readOnlyClient.getAccount(sealedSignerAddr);
  if (!sealedAccount) {
    throw new Error(`Sealed signer account not found on chain: ${sealedSignerAddr}`);
  }
  const accountNumber = sealedAccount.accountNumber;
  const sequence = sealedAccount.sequence;
  console.log(`  Account number: ${accountNumber}`);
  console.log(`  Sequence: ${sequence}`);
  console.log("  OK\n");

  // ── Step 5: Start invoke server ──
  console.log("Step 5: Start invoke server...");
  const server = await startInvokeServer(sealedBlobHex, 0);
  console.log(`  Server: ${server.url}`);
  console.log("  OK\n");

  try {
    // ── Step 6: Build Moultbook post message and call invoke ──
    console.log("Step 6: Call POST /invoke/sealed-signer...");

    // Build a test Moultbook post
    const testPayload = JSON.stringify({
      agent: "e2e-invoke-test",
      text: "WAVS off-chain invoke API end-to-end test — sealed signer signing via wasmtime",
      timestamp: new Date().toISOString(),
    });
    const commitment = Buffer.from(
      await crypto.subtle.digest("SHA-256", Buffer.from(testPayload, "utf8"))
    ).toString("base64");

    const execMsg = {
      post: {
        commitment,
        content_type: "text/plain",
        size_bytes: Buffer.byteLength(testPayload, "utf8"),
        attestation_ref: null,
        visibility: "public",
        refs: [],
      },
    };

    const invokeBody = {
      trigger: "sign_request",
      input: {
        sender: sealedSignerAddr,
        contract: MOULTBOOK_ADDR,
        exec_msg_json: JSON.stringify(execMsg),
        funds_denom: "",
        funds_amount: "0",
        gas_limit: 300000,
        fee_denom: DENOM,
        fee_amount: "30000",
        memo: "e2e invoke test",
        chain_id: CHAIN_ID,
        account_number: accountNumber,
        sequence: sequence,
      },
    };

    console.log(`  POST ${server.url}/invoke/sealed-signer`);
    console.log(`  sender: ${sealedSignerAddr}`);
    console.log(`  contract: ${MOULTBOOK_ADDR}`);
    console.log(`  commitment: ${commitment.slice(0, 24)}...`);

    const invokeResp = await fetch(`${server.url}/invoke/sealed-signer`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${INVOKE_TOKEN}`,
      },
      body: JSON.stringify(invokeBody),
    });

    if (!invokeResp.ok) {
      const text = await invokeResp.text();
      throw new Error(`invoke failed (${invokeResp.status}): ${text}`);
    }

    const invokeResult = await invokeResp.json() as any;
    const txBytesBase64 = invokeResult.output?.tx_bytes;
    const signDocHash = invokeResult.output?.sign_doc_sha256_hex;
    const signedAddr = invokeResult.output?.address;

    console.log(`  Response address: ${signedAddr}`);
    console.log(`  sign_doc_sha256: ${signDocHash}`);
    console.log(`  tx_bytes: ${txBytesBase64?.slice(0, 48)}... (${Math.floor((txBytesBase64?.length || 0) * 3 / 4)} bytes)`);
    console.log("  OK\n");

    // ── Step 7: Broadcast the signed tx ──
    console.log("Step 7: Broadcast signed tx on uni-7...");
    const txBytes = Buffer.from(txBytesBase64, "base64");
    const broadcastResult = await readOnlyClient.broadcastTx(txBytes);
    console.log(`  Broadcast result code: ${broadcastResult.code}`);
    console.log(`  Transaction hash: ${broadcastResult.transactionHash}`);

    if (broadcastResult.code !== 0) {
      console.error(`  RAW LOG: ${broadcastResult.rawLog}`);
      throw new Error(`Broadcast failed with code ${broadcastResult.code}: ${broadcastResult.rawLog}`);
    }

    // Wait for broadcast to be fully confirmed
    await sleep(4000);

    // ── Step 8: Verify tx on-chain ──
    console.log("\nStep 8: Verify transaction on-chain...");
    const txResult = await readOnlyClient.getTx(broadcastResult.transactionHash);
    if (!txResult) {
      throw new Error(`Could not find tx ${broadcastResult.transactionHash} on-chain`);
    }
    console.log(`  Tx found: ${txResult.hash}`);
    console.log(`  Height: ${txResult.height}`);
    console.log(`  Code: ${txResult.code}`);
    console.log(`  Gas used: ${txResult.gasUsed} / ${txResult.gasWanted}`);

    if (txResult.code === 0) {
      console.log("\n=== E2E TEST PASSED ===");
      console.log(`  Sealed signer: ${sealedSignerAddr}`);
      console.log(`  Signed via: off-chain invoke (wasmtime)`);
      console.log(`  Broadcast tx: ${broadcastResult.transactionHash}`);
      console.log(`  sign_doc_sha256: ${signDocHash}`);
      console.log(`  Moultbook contract: ${MOULTBOOK_ADDR}`);
      console.log("\n  The sealed signer successfully signed and submitted a");
      console.log("  Moultbook post on uni-7 via the off-chain invoke API,");
      console.log("  bypassing the on-chain sign-request round-trip entirely.");
    } else {
      console.error(`\n=== E2E TEST FAILED ===`);
      console.error(`  Tx code: ${txResult.code}`);
      console.error(`  Raw log: ${txResult.rawLog}`);
      process.exit(1);
    }
  } finally {
    server.close();
    await readOnlyClient.disconnect();
    await signingClient.disconnect();
  }
}

main().catch((e) => {
  console.error(`\n[e2e-invoke-test] fatal: ${(e as Error).message}`);
  console.error((e as Error).stack);
  process.exit(1);
});
