#!/usr/bin/env tsx
/**
 * WAVS Bridge Daemon
 *
 * Polls the WAVS aggregator for completed verification results and relays
 * them to the agent-company contract on Juno testnet (uni-7).
 *
 * Handles result types:
 * - data_verify       → SubmitAttestation (from WavsPush proposals)
 * - drand_randomness  → SubmitRandomness (from SortitionRequest proposals)
 * - outcome_verify    → SubmitAttestation (from OutcomeCreate proposals)
 * - swap_verify       → SubmitAttestation (DEX swap verification)
 * - pool_health_check → SubmitAttestation (DEX pool monitoring)
 * - governance_watch  → SubmitAttestation (Chain Intelligence: governance surveillance)
 * - migration_watch   → SubmitAttestation (Chain Intelligence: migration watchdog)
 * - whale_alert       → SubmitAttestation (Chain Intelligence: large trade detection)
 * - ibc_health_check  → SubmitAttestation (Chain Intelligence: IBC monitoring)
 *
 * Usage:
 *   npm run bridge
 */

import { config, validateConfig } from "./config.js";
import { submitAttestation, submitRandomness } from "./client.js";
import {
  AdminRpcHandle,
  defaultEgressController,
  startAdminRpcServer,
} from "./admin/rpc-server.js";

interface WavsResult {
  workflow_id: string;
  task_type: string;
  proposal_id?: number;
  job_id?: string;
  data_hash: string;
  attestation_hash: string;
  randomness_hex?: string;
  output?: Record<string, unknown>;
  processed?: boolean;
}

// Track which results we've already submitted
const processedSet = new Set<string>();

// Chain Intelligence alert log (kept in-memory for dashboard queries)
const chainIntelLog: Array<{
  timestamp: number;
  task_type: string;
  risk_level?: string;
  summary: string;
  attestation_hash: string;
}> = [];

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

function logChainIntel(result: WavsResult, summary: string): void {
  const riskLevel = (result.output?.risk_level as string) || (result.output?.health as string) || "info";
  chainIntelLog.push({
    timestamp: Date.now(),
    task_type: result.task_type,
    risk_level: riskLevel,
    summary,
    attestation_hash: result.attestation_hash,
  });
  // Keep last 1000 entries
  if (chainIntelLog.length > 1000) chainIntelLog.shift();

  const prefix = riskLevel === "high" || riskLevel === "critical"
    ? "🚨" : riskLevel === "medium" || riskLevel === "warning"
    ? "⚠️" : "✅";
  console.log(`[chain-intel] ${prefix} ${result.task_type} | ${riskLevel} | ${summary}`);
}

async function processResult(result: WavsResult): Promise<void> {
  const key = `${result.workflow_id}:${result.proposal_id || result.job_id || result.data_hash.slice(0, 16)}`;
  if (processedSet.has(key)) return;

  try {
    switch (result.task_type) {
      // ── Original workflows ──
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

      // ── DEX verification workflows ──
      case "swap_verify": {
        const out = result.output || {};
        const pair = (out.pair as string) || "unknown";
        const whale = out.whale_flag ? ` | WHALE(${out.whale_tier})` : "";
        const manip = out.manipulation_flag ? " | MANIPULATION" : "";
        logChainIntel(result,
          `Swap ${pair} | impact=${out.price_impact_pct}%${whale}${manip}`
        );
        // Submit attestation (proposal_id = 0 for auto-triggered swaps)
        await submitAttestation(
          result.proposal_id || 0,
          result.task_type,
          result.data_hash,
          result.attestation_hash
        );
        break;
      }
      case "pool_health_check": {
        const out = result.output || {};
        logChainIntel(result,
          `Pool ${out.pair} | health=${out.health} | ratio=${out.balance_ratio}`
        );
        await submitAttestation(
          result.proposal_id || 0,
          result.task_type,
          result.data_hash,
          result.attestation_hash
        );
        break;
      }

      // ── Chain Intelligence Module (7-10) ──
      case "governance_watch": {
        const out = result.output || {};
        const flags = (out.risk_flags as string[]) || [];
        logChainIntel(result,
          `Proposal #${out.proposal_id} ${out.action_type} by ${out.actor} | ` +
          `risk=${out.risk_level}(${out.risk_score}) | flags=[${flags.join(",")}]`
        );
        await submitAttestation(
          result.proposal_id || Number(out.proposal_id) || 0,
          result.task_type,
          result.data_hash,
          result.attestation_hash
        );
        break;
      }
      case "migration_watch": {
        const out = result.output || {};
        const authStr = out.authorized ? "AUTHORIZED" : "UNAUTHORIZED";
        logChainIntel(result,
          `Migration ${out.contract_addr} → code_id=${out.new_code_id} | ` +
          `${authStr} | risk=${out.risk_level}(${out.risk_score})`
        );
        await submitAttestation(
          result.proposal_id || Number(out.proposal_id) || 0,
          result.task_type,
          result.data_hash,
          result.attestation_hash
        );
        break;
      }
      case "whale_alert": {
        const out = result.output || {};
        logChainIntel(result,
          `WHALE ${out.whale_tier} on ${out.pair} | ${out.offer_amount} ${out.offer_asset} | ` +
          `size=${out.trade_size_pct}% of reserve | sandwich_risk=${out.sandwich_risk}`
        );
        await submitAttestation(
          result.proposal_id || 0,
          result.task_type,
          result.data_hash,
          result.attestation_hash
        );
        break;
      }
      case "ibc_health_check": {
        const out = result.output || {};
        const flags = (out.risk_flags as string[]) || [];
        logChainIntel(result,
          `IBC ${out.channel_id} (${out.port_id}) | health=${out.health} | ` +
          `loss=${out.loss_rate_pct}% | flags=[${flags.join(",")}]`
        );
        await submitAttestation(
          result.proposal_id || 0,
          result.task_type,
          result.data_hash,
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
  console.log(`[bridge] Chain Intelligence Module: ACTIVE (governance, migration, whale, ibc)`);
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

// ══════════════════════════════════════════════════
//  ADMIN RPC startup helper (v0.x.y-security-3 Phase 3d)
// ══════════════════════════════════════════════════

/**
 * Conditionally start the WAVS-side admin RPC if both
 * JUNOCLAW_ADMIN_RPC=1 and JUNOCLAW_ADMIN_TOKEN are set. Mirrors the
 * MCP-side helper (mcp/src/index.ts) so both processes follow the
 * same env-var contract.
 *
 * Off-by-default. If JUNOCLAW_ADMIN_RPC=1 is set but the token is
 * missing or too short, this function throws and the bridge aborts
 * startup -- fail-loud rather than start a half-configured listener.
 *
 * Optional knobs:
 *   JUNOCLAW_ADMIN_RPC_PORT  -- bind port (default 0 = OS-assigned)
 *   JUNOCLAW_ADMIN_RPC_HOST  -- bind host (default 127.0.0.1)
 */
async function maybeStartAdminRpc(): Promise<AdminRpcHandle | null> {
  const enabled = process.env.JUNOCLAW_ADMIN_RPC === "1";
  if (!enabled) return null;

  const token = process.env.JUNOCLAW_ADMIN_TOKEN ?? "";
  if (token.length === 0) {
    throw new Error(
      "JUNOCLAW_ADMIN_RPC=1 set but JUNOCLAW_ADMIN_TOKEN is empty. " +
        "Set a >=32-byte bearer token (e.g. `openssl rand -hex 32`) or unset " +
        "JUNOCLAW_ADMIN_RPC to disable the admin listener."
    );
  }

  const portRaw = process.env.JUNOCLAW_ADMIN_RPC_PORT;
  const port = portRaw ? Number.parseInt(portRaw, 10) : 0;
  if (portRaw && (!Number.isFinite(port) || port < 0 || port > 65535)) {
    throw new Error(
      `JUNOCLAW_ADMIN_RPC_PORT must be a number 0-65535, got "${portRaw}"`
    );
  }
  const host = process.env.JUNOCLAW_ADMIN_RPC_HOST ?? "127.0.0.1";

  const handle = await startAdminRpcServer({
    token,
    port,
    host,
    processName: "wavs-bridge",
    controller: defaultEgressController,
  });
  console.error(
    `[admin-rpc] listening on ${handle.url} (token from JUNOCLAW_ADMIN_TOKEN, ` +
      `${token.length}-byte; ${process.env.JUNOCLAW_ADMIN_RPC_HOST ? "custom host" : "loopback only"})`
  );
  return handle;
}

// Main
validateConfig();

// v0.x.y-security-3 Phase 3d: optional admin RPC for hot-flipping
// the egress_paused kill-switch in this same process. The listener
// is started before runLoop() so a SIGINT during normal operation
// closes it cleanly before the daemon exits. Top-level await is
// supported here (target ES2022, module ESNext).
let adminRpcHandle: AdminRpcHandle | null;
try {
  adminRpcHandle = await maybeStartAdminRpc();
} catch (e) {
  console.error(`[bridge] admin RPC startup failed: ${(e as Error).message}`);
  process.exit(1);
}

if (adminRpcHandle !== null) {
  const handle: AdminRpcHandle = adminRpcHandle;
  const shutdown = async (signal: string): Promise<void> => {
    console.error(`[admin-rpc] received ${signal}, closing listener`);
    try {
      await handle.close();
      console.error("[admin-rpc] closed");
    } catch (e) {
      console.error(`[admin-rpc] close error: ${(e as Error).message}`);
    }
    process.exit(0);
  };
  process.on("SIGINT", () => void shutdown("SIGINT"));
  process.on("SIGTERM", () => void shutdown("SIGTERM"));
}

runLoop().catch((err) => {
  console.error("[bridge] Fatal:", err.message || err);
  process.exit(1);
});
