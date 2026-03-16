#!/usr/bin/env tsx
/**
 * Execute an already-passed proposal by ID.
 * Usage: npx tsx src/execute-proposal.ts <proposal_id>
 */

import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config, validateConfig } from "./config.js";

async function main() {
  const proposalId = Number(process.argv[2] || "2");
  validateConfig();

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();
  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );

  const contract = config.agentCompanyContract;
  const height = await client.getHeight();
  console.log(`[exec] Block: ${height}, Proposal: ${proposalId}, Contract: ${contract}`);

  const result = await client.execute(
    account.address,
    contract,
    { execute_proposal: { proposal_id: proposalId } },
    "auto",
    "CP-4: Execute OutcomeCreate proposal"
  );

  console.log(`[exec] TX: ${result.transactionHash}`);
  console.log(`[exec] Gas: ${result.gasUsed}`);

  // Show all wasm events
  for (const ev of result.events || []) {
    if (ev.type.includes("wasm")) {
      console.log(`\n[exec] Event: ${ev.type}`);
      for (const attr of ev.attributes) {
        console.log(`  ${attr.key} = ${attr.value}`);
      }
    }
  }

  // Verify final state
  const proposal = await client.queryContractSmart(contract, {
    get_proposal: { proposal_id: proposalId },
  });
  console.log(`\n[exec] Final status: ${proposal.status}`);
  console.log(`[exec] Executed: ${proposal.executed}`);
}

main().catch((err) => {
  console.error("[exec] Failed:", err.message || err);
  process.exit(1);
});
