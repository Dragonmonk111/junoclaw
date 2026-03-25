#!/usr/bin/env tsx
/**
 * Verify the wasmd admin of all JunoClaw contracts.
 *
 * This is a read-only query — no wallet or mnemonic needed.
 * Anyone can run this to confirm admin transfer status.
 *
 * Usage:
 *   npx tsx src/verify-admin.ts
 *   npx tsx src/verify-admin.ts <expected-admin-address>
 */

import { CosmWasmClient } from "@cosmjs/cosmwasm-stargate";

const RPC = process.env.JUNO_RPC || "https://juno-testnet-rpc.polkachu.com";
const CHAIN_ID = process.env.CHAIN_ID || "uni-7";

const CONTRACTS = [
  {
    name: "agent-company v3",
    address: "juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6",
  },
  {
    name: "junoswap factory",
    address: "juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh",
  },
  {
    name: "escrow",
    address: "juno1dh43lswg5ekv7q2p44s6hgays47k5mz67742vdwpd025p8q05kgs0azwrv",
  },
  {
    name: "agent-registry",
    address: "juno1qulyspwzjzsz7rq65v6ptzt278f9ta9uh0upxu6xa08gf4v5gzaqm676j7",
  },
  {
    name: "task-ledger",
    address: "juno1agw6f05wxx5rm8d3etq7cejcm5g8e224s00dvykylaja7jlx3ljq6f0u46",
  },
];

// Neo wallet (original deployer) — for reference
const NEO_ADDRESS = "juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m";

async function main() {
  const expectedAdmin = process.argv[2] || null;

  console.log(`\n[verify] Chain:    ${CHAIN_ID}`);
  console.log(`[verify] RPC:      ${RPC}`);
  if (expectedAdmin) {
    console.log(`[verify] Expected: ${expectedAdmin}`);
  }
  console.log(`[verify] Neo (old): ${NEO_ADDRESS}`);
  console.log();

  const client = await CosmWasmClient.connect(RPC);

  console.log(`${"Contract".padEnd(22)} ${"Admin".padEnd(45)} Status`);
  console.log(`${"─".repeat(22)} ${"─".repeat(45)} ${"─".repeat(15)}`);

  let allMatch = true;
  let allTransferred = true;

  for (const contract of CONTRACTS) {
    try {
      const info = await client.getContract(contract.address);
      const admin = info.admin || "(none)";
      const isNeo = admin === NEO_ADDRESS;
      const isExpected = expectedAdmin ? admin === expectedAdmin : !isNeo;

      let status: string;
      if (expectedAdmin) {
        status = isExpected ? "✅ CORRECT" : `❌ WRONG`;
      } else {
        status = isNeo ? "⏳ STILL NEO" : "✅ TRANSFERRED";
      }

      if (!isExpected) allMatch = false;
      if (isNeo) allTransferred = false;

      console.log(`${contract.name.padEnd(22)} ${admin.padEnd(45)} ${status}`);
    } catch (err: any) {
      console.log(`${contract.name.padEnd(22)} ${"ERROR".padEnd(45)} ❌ ${err.message}`);
      allMatch = false;
      allTransferred = false;
    }
  }

  console.log();
  if (expectedAdmin) {
    if (allMatch) {
      console.log(`✅ All contracts point to the expected admin.`);
    } else {
      console.log(`⚠️  Some contracts do not match the expected admin.`);
    }
  } else {
    if (allTransferred) {
      console.log(`✅ All contracts have been transferred away from Neo.`);
    } else {
      console.log(`⏳ Some contracts still have Neo as admin.`);
    }
  }
}

main().catch((err) => {
  console.error("\n[verify] Fatal:", err.message || err);
  process.exit(1);
});
