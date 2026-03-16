#!/usr/bin/env tsx
/**
 * CLI tool: Submit WAVS drand randomness for a pending sortition job.
 *
 * Usage:
 *   npx tsx src/submit-randomness.ts <job_id> <randomness_hex> <attestation_hash>
 *
 * Example:
 *   npx tsx src/submit-randomness.ts sortition_1_12345 aabbccdd...64chars... wavs_att_rand_001
 */

import { config, validateConfig } from "./config.js";
import { submitRandomness } from "./client.js";

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 3) {
    console.error(
      "Usage: submit-randomness <job_id> <randomness_hex> <attestation_hash>"
    );
    process.exit(1);
  }

  validateConfig();

  const [jobId, randomnessHex, attestationHash] = args;

  if (randomnessHex.length < 64) {
    console.error("randomness_hex must be at least 64 hex chars (32 bytes)");
    process.exit(1);
  }

  console.log(`Submitting randomness to ${config.agentCompanyContract}...`);
  console.log(`  job_id:      ${jobId}`);
  console.log(`  randomness:  ${randomnessHex.slice(0, 16)}...`);
  console.log(`  attestation: ${attestationHash}`);

  const txHash = await submitRandomness(jobId, randomnessHex, attestationHash);
  console.log(`\nSuccess! TX: ${txHash}`);
}

main().catch((err) => {
  console.error("Failed:", err.message || err);
  process.exit(1);
});
