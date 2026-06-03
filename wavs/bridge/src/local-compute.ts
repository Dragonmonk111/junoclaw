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
import { safeFetch } from "./utils/ssrf-guard.js";

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
 * Structured, template-driven resolution rule. Mirrors the Rust
 * `ResolutionCriteria` struct in wavs/src/lib.rs.
 */
interface ResolutionCriteria {
  template: string;
  source: string;
  path?: string;
  comparator?: string;
  value?: unknown;
}

/**
 * Navigate a dotted JSON path (e.g. `data.result.price`). Array indices are
 * supported as numeric path segments. Mirrors Rust `json_path_get`.
 */
function jsonPathGet(root: unknown, path: string): unknown {
  let cur: unknown = root;
  for (const seg of path.split(".").filter((s) => s.length > 0)) {
    if (Array.isArray(cur)) {
      const idx = Number(seg);
      if (!Number.isInteger(idx)) return undefined;
      cur = cur[idx];
    } else if (cur !== null && typeof cur === "object") {
      cur = (cur as Record<string, unknown>)[seg];
    } else {
      return undefined;
    }
    if (cur === undefined) return undefined;
  }
  return cur;
}

/** Coerce a JSON value to a number (accepts numbers or numeric strings). */
function asNumber(v: unknown): number | undefined {
  if (typeof v === "number") return v;
  if (typeof v === "string") {
    const n = Number(v.trim());
    return Number.isNaN(n) ? undefined : n;
  }
  return undefined;
}

/**
 * Pure evaluation of a parsed criteria against fetched JSON. Mirrors Rust
 * `evaluate_resolution`. Returns the resolved boolean outcome plus evidence.
 */
export function evaluateResolution(
  criteria: ResolutionCriteria,
  fetched: unknown
): { outcome: boolean; evidence: Record<string, unknown> } {
  switch (criteria.template) {
    case "numeric_threshold": {
      if (!criteria.path) throw new Error("numeric_threshold requires 'path'");
      if (!criteria.comparator)
        throw new Error("numeric_threshold requires 'comparator'");
      const threshold = asNumber(criteria.value);
      if (threshold === undefined)
        throw new Error("numeric_threshold requires numeric 'value'");

      const observed = asNumber(jsonPathGet(fetched, criteria.path));
      if (observed === undefined)
        throw new Error(`no numeric value at path '${criteria.path}'`);

      let outcome: boolean;
      switch (criteria.comparator) {
        case "gt":
          outcome = observed > threshold;
          break;
        case "gte":
          outcome = observed >= threshold;
          break;
        case "lt":
          outcome = observed < threshold;
          break;
        case "lte":
          outcome = observed <= threshold;
          break;
        case "eq":
          outcome = Math.abs(observed - threshold) < Number.EPSILON;
          break;
        default:
          throw new Error(`unknown comparator '${criteria.comparator}'`);
      }
      return {
        outcome,
        evidence: {
          template: "numeric_threshold",
          path: criteria.path,
          comparator: criteria.comparator,
          threshold,
          observed,
        },
      };
    }
    case "string_match": {
      if (!criteria.path) throw new Error("string_match requires 'path'");
      if (typeof criteria.value !== "string")
        throw new Error("string_match requires string 'value'");
      const observed = jsonPathGet(fetched, criteria.path);
      if (typeof observed !== "string")
        throw new Error(`no string value at path '${criteria.path}'`);
      return {
        outcome: observed === criteria.value,
        evidence: {
          template: "string_match",
          path: criteria.path,
          expected: criteria.value,
          observed,
        },
      };
    }
    case "boolean_field": {
      if (!criteria.path) throw new Error("boolean_field requires 'path'");
      const observed = jsonPathGet(fetched, criteria.path);
      if (typeof observed !== "boolean")
        throw new Error(`no boolean value at path '${criteria.path}'`);
      return {
        outcome: observed,
        evidence: { template: "boolean_field", path: criteria.path, observed },
      };
    }
    default:
      throw new Error(`unknown resolution template '${criteria.template}'`);
  }
}

/**
 * Process an OutcomeVerify task — mirrors wavs/src/lib.rs → process_outcome_verify()
 *
 * Structured (JSON) criteria are resolved against a fetched data source; the
 * data hash binds question, criteria, market_id, the resolved outcome byte,
 * and the raw source response — byte-identical to the Rust component.
 *
 * Legacy free-text criteria fall back to the original hash and report
 * `unresolved` (manual settlement).
 */
export async function computeOutcomeVerify(
  marketId: number,
  question: string,
  resolutionCriteria: string
): Promise<AttestationResult> {
  // u64 little-endian — matches Rust's market_id.to_le_bytes()
  const marketIdBuf = Buffer.alloc(8);
  marketIdBuf.writeBigUInt64LE(BigInt(marketId));

  let parsed: ResolutionCriteria | null = null;
  try {
    const obj = JSON.parse(resolutionCriteria);
    // Match Rust serde: requires at least the `template` + `source` fields.
    if (
      obj &&
      typeof obj === "object" &&
      typeof obj.template === "string" &&
      typeof obj.source === "string"
    ) {
      parsed = obj as ResolutionCriteria;
    }
  } catch {
    parsed = null;
  }

  // Legacy / free-text criteria: hash the criteria as before, report unresolved.
  if (!parsed) {
    const hasher = createHash("sha256");
    hasher.update(Buffer.from(question, "utf-8"));
    hasher.update(Buffer.from(resolutionCriteria, "utf-8"));
    hasher.update(marketIdBuf);
    const dataHash = hasher.digest("hex");
    return {
      taskType: "outcome_verify",
      dataHash,
      attestationHash: computeAttestationHash("outcome_verify", dataHash),
      output: {
        market_id: marketId,
        question,
        resolution_criteria: resolutionCriteria,
        resolved: false,
        status: "unresolved",
        reason: "criteria are not structured JSON; manual resolution required",
        compute_mode: "local_operator",
      },
      timestamp: Math.floor(Date.now() / 1000),
    };
  }

  // 1) Fetch the resolution data source (SSRF-guarded).
  const resp = await safeFetch(parsed.source);
  const raw = resp.text;

  // 2) Parse + evaluate the template.
  const fetched = JSON.parse(raw);
  const { outcome, evidence } = evaluateResolution(parsed, fetched);

  // 3) Bind evidence into a deterministic data hash — matches Rust ordering:
  //    question || resolution_criteria || market_id_le || [outcome] || raw.
  const hasher = createHash("sha256");
  hasher.update(Buffer.from(question, "utf-8"));
  hasher.update(Buffer.from(resolutionCriteria, "utf-8"));
  hasher.update(marketIdBuf);
  hasher.update(Buffer.from([outcome ? 1 : 0]));
  hasher.update(Buffer.from(raw, "utf-8"));
  const dataHash = hasher.digest("hex");

  return {
    taskType: "outcome_verify",
    dataHash,
    attestationHash: computeAttestationHash("outcome_verify", dataHash),
    output: {
      market_id: marketId,
      question,
      source: parsed.source,
      resolved: true,
      status: "resolved",
      outcome,
      evidence,
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
      // safeFetch enforces scheme/port allowlists, blocks private IPs
      // (cloud metadata, RFC 1918, loopback), applies a 5s timeout,
      // and caps the body at 1 MiB. See utils/ssrf-guard.ts.
      const resp = await safeFetch(url);
      const text = resp.text;
      hasher.update(Buffer.from(text, "utf-8"));

      const srcHash = createHash("sha256")
        .update(Buffer.from(text, "utf-8"))
        .digest("hex");
      sourceDetails.push({
        url,
        bytes: text.length,
        hash: srcHash,
        status: resp.status,
      });
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

  // Defense-in-depth: drand is a public API, but safeFetch ensures
  // DNS hijacks / misconfigured /etc/hosts can't route this request
  // to a private IP. Also enforces the 5s timeout and 1 MiB body cap.
  const resp = await safeFetch(url);
  const beacon = JSON.parse(resp.text) as {
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
