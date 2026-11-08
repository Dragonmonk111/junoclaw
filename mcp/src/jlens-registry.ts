#!/usr/bin/env tsx
/**
 * J-Lens probe-registry driver — junoclaw-g1.
 *
 * Registers a probe bank on chain so a snapshot's bank_hash resolves to a
 * governed record instead of a self-declared file. Banks land as Proposed;
 * activation is an admin (DAO) act and, with require_calibration=true,
 * needs a published calibration report hash.
 *
 * Usage:
 *   cd mcp && npx tsx src/jlens-registry.ts \
 *     --bank ../tools/brainmaxx/j-lens/probe_bank_onchain.json \
 *     --model-revision <hf commit sha> --weights-hash <content pin> \
 *     --uri <public url of the bank file> \
 *     [--calibration-hash <hex>] \
 *     (--wasm <probe_registry.wasm> | --code-id N | --contract <addr>)
 */

import { createHash } from "crypto";
import { readFileSync } from "fs";
import { resolve } from "path";

import { executeContract, instantiateContract, uploadWasm } from "./tools/tx-builder.js";
import { getDefaultWalletStore } from "./wallet/store.js";

const CHAIN_ID = "junoclaw-g1";
const WALLET_ID = "jlens-ops";

function arg(flag: string): string | undefined {
  const i = process.argv.indexOf(flag);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

function need(flag: string): string {
  const v = arg(flag);
  if (!v) throw new Error(`${flag} required`);
  return v;
}

async function main() {
  const bankPath = resolve(need("--bank"));
  const raw = readFileSync(bankPath);
  // same definition as probe_runtime.load_bank: sha256 over the raw file bytes
  const bankHash = createHash("sha256").update(raw).digest("hex");
  const bank = JSON.parse(raw.toString("utf-8")) as {
    probe_model: string;
    layer: number;
    concepts: Record<string, { threshold: number }>;
  };
  const concepts = Object.keys(bank.concepts).sort();

  const wallet = (await getDefaultWalletStore().list()).find((e) => e.id === WALLET_ID);
  if (!wallet) throw new Error(`wallet ${WALLET_ID} not enrolled — run jlens-attest.ts first`);
  console.log(`ops wallet: ${wallet.address}`);

  let contract = arg("--contract");
  if (!contract) {
    let codeId = arg("--code-id") ? Number(arg("--code-id")) : undefined;
    if (!codeId) {
      const up = await uploadWasm(CHAIN_ID, WALLET_ID, resolve(need("--wasm")), "jlens probe-registry store");
      codeId = up.codeId;
      console.log(`stored code_id=${codeId} tx=${up.txHash}`);
    }
    const inst = await instantiateContract(
      CHAIN_ID,
      WALLET_ID,
      codeId,
      { admin: wallet.address, require_calibration: true },
      "jlens-probe-registry",
      undefined,
      undefined,
      "jlens probe-registry instantiate"
    );
    contract = inst.contractAddress;
    console.log(`instantiated ${contract} tx=${inst.txHash}`);
  }

  const reg = await executeContract(
    CHAIN_ID,
    WALLET_ID,
    contract,
    {
      register_probe_bank: {
        model_id: bank.probe_model,
        model_revision: need("--model-revision"),
        weights_hash: need("--weights-hash"),
        layers: [bank.layer],
        concepts,
        method: "contrastive-dom-v0.1",
        bank_hash: bankHash,
        uri: need("--uri"),
        calibration_hash: arg("--calibration-hash") ?? "",
        thresholds: concepts.map((c) => ({
          concept: c,
          threshold_bp: Math.round(bank.concepts[c].threshold * 10_000),
        })),
      },
    },
    undefined,
    `jlens register bank ${bankHash.slice(0, 12)}`
  );

  console.log(`\n✅ bank registered (status: proposed) tx=${reg.txHash}`);
  console.log(`   contract  ${contract}`);
  console.log(`   model     ${bank.probe_model} layer ${bank.layer}`);
  console.log(`   bank_hash ${bankHash}`);
  console.log(`   concepts  ${concepts.length}`);
}

main().catch((err: unknown) => {
  console.error(`❌ ${err instanceof Error ? err.message : err}`);
  process.exit(1);
});
