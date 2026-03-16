#!/usr/bin/env tsx
/**
 * CLI tool: Submit a WAVS attestation result to agent-company contract.
 *
 * Usage:
 *   npx tsx src/submit-attestation.ts <proposal_id> <task_type> <data_hash> <attestation_hash>
 *
 * Example:
 *   npx tsx src/submit-attestation.ts 1 data_verify abc123 wavs_att_001
 */

import { config, validateConfig } from "./config.js";
import { submitAttestation } from "./client.js";

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 4) {
    console.error(
      "Usage: submit-attestation <proposal_id> <task_type> <data_hash> <attestation_hash>"
    );
    process.exit(1);
  }

  validateConfig();

  const [proposalIdStr, taskType, dataHash, attestationHash] = args;
  const proposalId = parseInt(proposalIdStr, 10);

  if (isNaN(proposalId)) {
    console.error("proposal_id must be a number");
    process.exit(1);
  }

  console.log(`Submitting attestation to ${config.agentCompanyContract}...`);
  console.log(`  proposal_id: ${proposalId}`);
  console.log(`  task_type:   ${taskType}`);
  console.log(`  data_hash:   ${dataHash}`);
  console.log(`  attestation: ${attestationHash}`);

  const txHash = await submitAttestation(
    proposalId,
    taskType,
    dataHash,
    attestationHash
  );
  console.log(`\nSuccess! TX: ${txHash}`);
}

main().catch((err) => {
  console.error("Failed:", err.message || err);
  process.exit(1);
});
