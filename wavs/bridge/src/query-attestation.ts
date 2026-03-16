#!/usr/bin/env tsx
/**
 * Query attestation(s) from the agent-company contract.
 * Usage:
 *   npx tsx src/query-attestation.ts <proposal_id>   — single attestation
 *   npx tsx src/query-attestation.ts --list           — list all attestations
 */

import { CosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { config, validateConfig } from "./config.js";

async function main() {
  validateConfig();
  const client = await CosmWasmClient.connect(config.rpcEndpoint);
  const contract = config.agentCompanyContract;

  const arg = process.argv[2];

  if (arg === "--list") {
    console.log(`[query] Listing all attestations on ${contract}...\n`);
    const result = await client.queryContractSmart(contract, {
      list_attestations: { start_after: null, limit: 50 },
    });
    const attestations = Array.isArray(result) ? result : result.attestations || [];
    if (attestations.length === 0) {
      console.log("[query] No attestations found.");
      return;
    }
    for (const att of attestations) {
      printAttestation(att);
    }
    console.log(`\nTotal: ${attestations.length} attestation(s)`);
  } else {
    const proposalId = Number(arg || "2");
    console.log(`[query] Getting attestation for proposal ${proposalId}...\n`);
    const att = await client.queryContractSmart(contract, {
      get_attestation: { proposal_id: proposalId },
    });
    printAttestation(att);
  }
}

function printAttestation(att: any) {
  console.log(`╔══════════════════════════════════════════════════════════╗`);
  console.log(`║  Proposal ID:       ${att.proposal_id}`);
  console.log(`║  Task Type:         ${att.task_type}`);
  console.log(`║  Data Hash:         ${att.data_hash}`);
  console.log(`║  Attestation Hash:  ${att.attestation_hash}`);
  console.log(`║  Submitted Block:   ${att.submitted_at_block}`);
  console.log(`║  Submitter:         ${att.submitter}`);
  console.log(`╚══════════════════════════════════════════════════════════╝`);
}

main().catch((err) => {
  console.error("[query] Failed:", err.message || err);
  process.exit(1);
});
