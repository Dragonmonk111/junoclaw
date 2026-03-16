#!/usr/bin/env tsx
/**
 * WAVS Bridge Daemon
 *
 * Polls the WAVS aggregator for completed verification results and relays
 * them to the agent-company contract on Juno testnet (uni-7).
 *
 * Handles three result types:
 * - data_verify    → SubmitAttestation (from WavsPush proposals)
 * - drand_randomness → SubmitRandomness (from SortitionRequest proposals)
 * - outcome_verify → SubmitAttestation (from OutcomeCreate proposals)
 *
 * Usage:
 *   npm run bridge
 */

import { config, validateConfig } from "./config.js";
import { submitAttestation, submitRandomness } from "./client.js";

interface WavsResult {
  workflow_id: string;
  task_type: string;
  proposal_id?: number;
  job_id?: string;
  data_hash: string;
  attestation_hash: string;
  randomness_hex?: string;
  processed?: boolean;
}

// Track which results we've already submitted
const processedSet = new Set<string>();

async function pollAggregator(): Promise<WavsResult[]> {
  try {
    const resp = await fetch(
      `${config.wavsAggregatorUrl}/api/v1/results?service=junoclaw-verifier&pending=true`
    );
    if (!resp.ok) {
      console.warn(`[bridge] Aggregator returned ${resp.status}`);
      return [];
    }
    return (await resp.json()) as WavsResult[];
  } catch (err: any) {
    // Aggregator not available — common during development
    if (err.code === "ECONNREFUSED") {
      return [];
    }
    console.warn(`[bridge] Poll error: ${err.message}`);
    return [];
  }
}

async function processResult(result: WavsResult): Promise<void> {
  const key = `${result.workflow_id}:${result.proposal_id || result.job_id}`;
  if (processedSet.has(key)) return;

  try {
    switch (result.task_type) {
      case "data_verify":
      case "outcome_verify": {
        if (!result.proposal_id) {
          console.warn(`[bridge] Missing proposal_id for ${result.task_type}`);
          return;
        }
        await submitAttestation(
          result.proposal_id,
          result.task_type,
          result.data_hash,
          result.attestation_hash
        );
        break;
      }
      case "drand_randomness": {
        if (!result.job_id || !result.randomness_hex) {
          console.warn(`[bridge] Missing job_id/randomness for drand result`);
          return;
        }
        await submitRandomness(
          result.job_id,
          result.randomness_hex,
          result.attestation_hash
        );
        break;
      }
      default:
        console.warn(`[bridge] Unknown task_type: ${result.task_type}`);
        return;
    }
    processedSet.add(key);
  } catch (err: any) {
    console.error(`[bridge] Failed to submit ${key}: ${err.message}`);
  }
}

async function runLoop(): Promise<void> {
  console.log("[bridge] Starting WAVS bridge daemon...");
  console.log(`[bridge] RPC:        ${config.rpcEndpoint}`);
  console.log(`[bridge] Chain:      ${config.chainId}`);
  console.log(`[bridge] Contract:   ${config.agentCompanyContract}`);
  console.log(`[bridge] Aggregator: ${config.wavsAggregatorUrl}`);
  console.log(`[bridge] Poll interval: ${config.pollIntervalMs}ms`);
  console.log("");

  while (true) {
    const results = await pollAggregator();
    if (results.length > 0) {
      console.log(`[bridge] Found ${results.length} pending result(s)`);
    }
    for (const result of results) {
      await processResult(result);
    }
    await new Promise((resolve) => setTimeout(resolve, config.pollIntervalMs));
  }
}

// Main
validateConfig();
runLoop().catch((err) => {
  console.error("[bridge] Fatal:", err.message || err);
  process.exit(1);
});
