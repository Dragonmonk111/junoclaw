#!/usr/bin/env tsx
/**
 * Local Operator — autonomous WAVS bridge running locally.
 *
 * This is the "fill the infra gap" mode: instead of waiting for a WAVS TEE
 * operator, this process watches for trigger events on-chain, computes
 * attestation hashes locally (byte-identical to the WASI component), and
 * submits them to the contract automatically.
 *
 * Pipeline:
 *   1. Poll contract for executed proposals without attestations
 *   2. Compute attestation hashes (same SHA-256 logic as wavs/src/lib.rs)
 *   3. Submit attestation to contract on-chain
 *   4. Log result and continue watching
 *
 * Usage:
 *   npx tsx src/local-operator.ts
 *   npx tsx src/local-operator.ts --once     # single pass, then exit
 */

import { CosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { config, validateConfig } from "./config.js";
import { findUnattestedProposals, TriggerEvent } from "./event-watcher.js";
import {
  computeOutcomeVerify,
  computeDataVerify,
  computeDrandRandomness,
  AttestationResult,
} from "./local-compute.js";
import { submitAttestation, submitRandomness } from "./client.js";

// Track submitted proposals to avoid re-processing
const submitted = new Set<number>();

async function processEvent(event: TriggerEvent): Promise<void> {
  if (submitted.has(event.proposalId)) return;

  console.log(
    `\n[operator] Processing ${event.eventType} — proposal ${event.proposalId}`
  );

  let result: AttestationResult;

  switch (event.eventType) {
    case "wasm-outcome_create": {
      const marketId = parseInt(event.attributes.market_id || "0", 10);
      const question = event.attributes.question || "";
      const criteria = event.attributes.resolution_criteria || "";

      console.log(`[operator]   question: "${question}"`);
      console.log(`[operator]   criteria: "${criteria}"`);

      result = computeOutcomeVerify(marketId, question, criteria);
      break;
    }

    case "wasm-wavs_push": {
      const taskId = parseInt(event.attributes.task_id || "0", 10);
      const sources = (event.attributes.data_sources || "")
        .split(",")
        .filter(Boolean);
      const criteria = event.attributes.task_description || "";

      console.log(`[operator]   task_id: ${taskId}, sources: ${sources.length}`);

      result = await computeDataVerify(taskId, sources, criteria);
      break;
    }

    case "wasm-sortition_request": {
      const jobId = event.attributes.job_id || `sort-${event.proposalId}`;
      const round = event.attributes.drand_round
        ? parseInt(event.attributes.drand_round, 10)
        : undefined;

      console.log(`[operator]   job_id: ${jobId}`);

      result = await computeDrandRandomness(jobId, round);
      break;
    }

    default:
      console.warn(`[operator] Unknown event type: ${event.eventType}`);
      return;
  }

  console.log(`[operator]   data_hash:        ${result.dataHash}`);
  console.log(`[operator]   attestation_hash:  ${result.attestationHash}`);

  // Submit to contract
  try {
    if (result.taskType === "drand_randomness") {
      const txHash = await submitRandomness(
        (result.output as any).job_id,
        (result.output as any).randomness_hex,
        result.attestationHash
      );
      console.log(`[operator]   ✅ Submitted randomness — TX: ${txHash}`);
    } else {
      const txHash = await submitAttestation(
        event.proposalId,
        result.taskType,
        result.dataHash,
        result.attestationHash
      );
      console.log(`[operator]   ✅ Submitted attestation — TX: ${txHash}`);
    }
    submitted.add(event.proposalId);
  } catch (err: any) {
    if (err.message?.includes("Duplicate attestation")) {
      console.log(`[operator]   ⏭️  Already attested (duplicate), skipping`);
      submitted.add(event.proposalId);
    } else {
      console.error(`[operator]   ❌ Submit failed: ${err.message}`);
    }
  }
}

async function runOnce(client: CosmWasmClient): Promise<number> {
  const events = await findUnattestedProposals(client);

  if (events.length === 0) {
    return 0;
  }

  console.log(`[operator] Found ${events.length} unattested proposal(s)`);

  for (const event of events) {
    await processEvent(event);
  }

  return events.length;
}

async function main(): Promise<void> {
  validateConfig();

  const oneShot = process.argv.includes("--once");

  console.log("╔═══════════════════════════════════════════════════╗");
  console.log("║        JunoClaw Local WAVS Operator              ║");
  console.log("╠═══════════════════════════════════════════════════╣");
  console.log(`║  Contract: ${config.agentCompanyContract.slice(0, 20)}...`);
  console.log(`║  RPC:      ${config.rpcEndpoint}`);
  console.log(`║  Chain:    ${config.chainId}`);
  console.log(`║  Mode:     ${oneShot ? "single pass" : `polling every ${config.pollIntervalMs}ms`}`);
  console.log("║  Compute:  local (SHA-256 identical to WASI component)");
  console.log("╚═══════════════════════════════════════════════════╝");
  console.log("");

  const client = await CosmWasmClient.connect(config.rpcEndpoint);
  const height = await client.getHeight();
  console.log(`[operator] Connected at block ${height}\n`);

  if (oneShot) {
    const count = await runOnce(client);
    console.log(
      count === 0
        ? "\n[operator] No unattested proposals found."
        : `\n[operator] Processed ${count} proposal(s).`
    );
    return;
  }

  // Continuous polling loop
  console.log("[operator] Watching for trigger events...\n");
  while (true) {
    try {
      await runOnce(client);
    } catch (err: any) {
      console.error(`[operator] Loop error: ${err.message}`);
    }
    await new Promise((resolve) => setTimeout(resolve, config.pollIntervalMs));
  }
}

main().catch((err) => {
  console.error("[operator] Fatal:", err.message || err);
  process.exit(1);
});
