#!/usr/bin/env tsx
/**
 * J-Lens on-chain attestation driver — junoclaw-g1.
 *
 * Anchors a thought batch on the merkle-verifier contract using the ops
 * wallet (WAVS_OPERATOR_MNEMONIC via WalletStore — key material never
 * crosses this script's boundary).
 *
 * Flow:
 *   1. enrol ops wallet (idempotent) as "jlens-ops"
 *   2. uploadWasm(merkle_verifier.wasm)            -> codeId   (skippable via --code-id)
 *   3. instantiateContract(codeId, {})             -> address  (skippable via --contract)
 *   4. executeContract(anchor_root, packet msg)    -> tx hash
 *   5. executeContract(verify_proof, packet msg)   -> tx hash (on-chain inclusion check)
 *
 * Usage:
 *   cd mcp && npx tsx src/jlens-attest.ts \
 *     --packet ../tools/brainmaxx/jlens-serve/packet.json \
 *     --wasm ../contracts/target/wasm32-unknown-unknown/release/merkle_verifier.wasm
 *
 * Env: JUNOCLAW_WALLET_PASSPHRASE or keychain backend (same as signing-smoke);
 *      ../wavs/.env must contain WAVS_OPERATOR_MNEMONIC;
 *      JUNOCLAW_GRPC_ENDPOINT to override the default fleet gRPC seat.
 */

import { readFileSync } from "fs";
import { resolve } from "path";

import {
  executeContract,
  instantiateContract,
  uploadWasm,
} from "./tools/tx-builder.js";
import { getDefaultWalletStore } from "./wallet/store.js";

const CHAIN_ID = "junoclaw-g1";
const WALLET_ID = "jlens-ops";

function arg(flag: string): string | undefined {
  const i = process.argv.indexOf(flag);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

function loadMnemonicFromEnvFile(): string {
  const envPath = resolve(
    process.cwd(),
    process.env.WAVS_ENV_PATH ?? "../wavs/.env"
  );
  const raw = readFileSync(envPath, "utf-8");
  const line = raw
    .split(/\r?\n/)
    .find((l) => l.trim().startsWith("WAVS_OPERATOR_MNEMONIC="));
  if (!line) throw new Error(`WAVS_OPERATOR_MNEMONIC not found in ${envPath}`);
  const value = line.slice("WAVS_OPERATOR_MNEMONIC=".length).trim();
  if (!value || value === "PASTE_YOUR_MNEMONIC_HERE")
    throw new Error("WAVS_OPERATOR_MNEMONIC empty/placeholder");
  return value;
}

async function ensureWallet(): Promise<string> {
  const store = getDefaultWalletStore();
  const found = (await store.list()).find((e) => e.id === WALLET_ID);
  if (found) return found.address;
  const entry = await store.add(WALLET_ID, loadMnemonicFromEnvFile(), {
    bech32Prefix: "juno",
  });
  return entry.address;
}

async function main() {
  const packetPath = arg("--packet");
  if (!packetPath) throw new Error("--packet <path> required");
  const packet = JSON.parse(readFileSync(resolve(packetPath), "utf-8"));

  const address = await ensureWallet();
  console.log(`ops wallet: ${address}`);

  let codeId = arg("--code-id") ? Number(arg("--code-id")) : undefined;
  if (!codeId) {
    const wasmPath = arg("--wasm");
    if (!wasmPath) throw new Error("--wasm or --code-id required");
    const up = await uploadWasm(CHAIN_ID, WALLET_ID, resolve(wasmPath), "jlens merkle-verifier store");
    codeId = up.codeId;
    console.log(`stored code_id=${codeId} tx=${up.txHash}`);
  }

  let contract = arg("--contract");
  if (!contract) {
    const inst = await instantiateContract(
      CHAIN_ID,
      WALLET_ID,
      codeId,
      { admin: address },
      "jlens-merkle-verifier",
      undefined,
      undefined,
      "jlens merkle-verifier instantiate"
    );
    contract = inst.contractAddress;
    console.log(`instantiated ${contract} tx=${inst.txHash}`);
  }
  console.log(`contract: ${contract}`);

  const anchor = await executeContract(
    CHAIN_ID,
    WALLET_ID,
    contract,
    packet.anchor_msg,
    undefined,
    `jlens thought batch ${packet.anchor_msg.anchor_root.batch_height}`
  );
  console.log(`anchored root ${packet.merkle_root.slice(0, 24)}… tx=${anchor.txHash}`);

  const entries: PacketEntry[] =
    packet.schema === "jlens.attest.v2"
      ? packet.entries
      : [{ leaf_index: 0, leaf_hash: packet.leaf_hash, verify_msg: packet.verify_msg, thought_record: packet.thought_record }];

  for (const e of entries) {
    const verify = await executeContract(
      CHAIN_ID,
      WALLET_ID,
      contract,
      e.verify_msg,
      undefined,
      `jlens proof check leaf ${e.leaf_index}`
    );
    console.log(`leaf[${e.leaf_index}] proof verified on-chain tx=${verify.txHash}`);
  }

  console.log("\n✅ J-Lens attestation anchored on junoclaw-g1");
  console.log(`   contract      ${contract}`);
  console.log(`   merkle_root   ${packet.merkle_root}`);
  for (const e of entries)
    console.log(`   leaf[${e.leaf_index}]       ${e.leaf_hash}  snapshot ${e.thought_record.snapshot_hash}`);
}

interface PacketEntry {
  leaf_index: number;
  leaf_hash: string;
  verify_msg: Record<string, unknown>;
  thought_record: { snapshot_hash: string };
}

main().catch((err: unknown) => {
  console.error(`❌ ${err instanceof Error ? err.message : err}`);
  process.exit(1);
});
