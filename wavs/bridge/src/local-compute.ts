/**
 * Local compute module — replicates the WASI component's attestation hash logic exactly.
 *
 * The SHA-256 computations here are byte-identical to those in wavs/src/lib.rs.
 * This allows the bridge to run in "local operator" mode, producing the same
 * attestation hashes that the WASI component would produce inside a TEE.
 *
 * When running in a real WAVS TEE, the operator wraps these hashes with
 * hardware attestation. In local mode, the hashes are software-computed
 * but cryptographically identical.
 */

import { createHash } from "crypto";

// Must match: wavs/src/lib.rs → compute_attestation_hash()
const COMPONENT_ID = "junoclaw-wavs-v0.1.0";

export interface AttestationResult {
  taskType: string;
  dataHash: string;
  attestationHash: string;
  output: Record<string, unknown>;
  timestamp: number;
}

/**
 * Compute attestation hash — mirrors the Rust WASI component exactly.
 *
 * Rust equivalent:
 *   let mut hasher = Sha256::new();
 *   hasher.update(b"junoclaw-wavs-v0.1.0");
 *   hasher.update(task_type.as_bytes());
 *   hasher.update(data_hash.as_bytes());
 *   hex::encode(hasher.finalize())
 */
function computeAttestationHash(taskType: string, dataHash: string): string {
  const hasher = createHash("sha256");
  hasher.update(Buffer.from(COMPONENT_ID, "utf-8"));
  hasher.update(Buffer.from(taskType, "utf-8"));
  hasher.update(Buffer.from(dataHash, "utf-8"));
  return hasher.digest("hex");
}

/**
 * Process an OutcomeVerify task — mirrors wavs/src/lib.rs → process_outcome_verify()
 *
 * Rust equivalent:
 *   hasher.update(question.as_bytes());
 *   hasher.update(resolution_criteria.as_bytes());
 *   hasher.update(market_id.to_le_bytes());  // u64 little-endian
 */
export function computeOutcomeVerify(
  marketId: number,
  question: string,
  resolutionCriteria: string
): AttestationResult {
  const hasher = createHash("sha256");
  hasher.update(Buffer.from(question, "utf-8"));
  hasher.update(Buffer.from(resolutionCriteria, "utf-8"));

  // u64 little-endian — matches Rust's market_id.to_le_bytes()
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(BigInt(marketId));
  hasher.update(buf);

  const dataHash = hasher.digest("hex");
  const attestationHash = computeAttestationHash("outcome_verify", dataHash);

  return {
    taskType: "outcome_verify",
    dataHash,
    attestationHash,
    output: {
      market_id: marketId,
      question,
      resolution_criteria: resolutionCriteria,
      compute_mode: "local_operator",
    },
    timestamp: Math.floor(Date.now() / 1000),
  };
}

/**
 * Process a DataVerify task — mirrors wavs/src/lib.rs → process_data_verify()
 *
 * Fetches URLs, hashes responses, produces attestation.
 */
export async function computeDataVerify(
  taskId: number,
  dataSources: string[],
  verificationCriteria: string
): Promise<AttestationResult> {
  const hasher = createHash("sha256");
  const sourceDetails: Record<string, unknown>[] = [];

  for (let i = 0; i < dataSources.length; i++) {
    const url = dataSources[i];
    try {
      const resp = await fetch(url);
      const text = await resp.text();
      hasher.update(Buffer.from(text, "utf-8"));

      const srcHash = createHash("sha256")
        .update(Buffer.from(text, "utf-8"))
        .digest("hex");
      sourceDetails.push({ url, bytes: text.length, hash: srcHash });
    } catch (err: any) {
      sourceDetails.push({ url, error: err.message });
    }
  }

  const dataHash = hasher.digest("hex");
  const attestationHash = computeAttestationHash("data_verify", dataHash);

  return {
    taskType: "data_verify",
    dataHash,
    attestationHash,
    output: {
      task_id: taskId,
      sources_fetched: dataSources.length,
      source_details: sourceDetails,
      compute_mode: "local_operator",
    },
    timestamp: Math.floor(Date.now() / 1000),
  };
}

/**
 * Process a DrandRandomness task — mirrors wavs/src/lib.rs → process_drand_randomness()
 */
export async function computeDrandRandomness(
  jobId: string,
  drandRound?: number
): Promise<AttestationResult> {
  const DRAND_URL =
    "https://api.drand.sh/52db9ba70e0cc0f6eaf7803dd07447a1f5477735fd3f661792ba94600c84e971";

  const url = drandRound
    ? `${DRAND_URL}/public/${drandRound}`
    : `${DRAND_URL}/public/latest`;

  const resp = await fetch(url);
  const beacon = (await resp.json()) as {
    round: number;
    randomness: string;
  };

  const dataHash = beacon.randomness;
  const attestationHash = computeAttestationHash("drand", dataHash);

  return {
    taskType: "drand_randomness",
    dataHash,
    attestationHash,
    output: {
      job_id: jobId,
      drand_round: beacon.round,
      randomness_hex: beacon.randomness,
      compute_mode: "local_operator",
    },
    timestamp: Math.floor(Date.now() / 1000),
  };
}
