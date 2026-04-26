/**
 * On-process smoke test for egress_paused — v0.x.y-security-3 Phase 3a.
 *
 * Mirrors the two-phase shape of signing-pause-smoke.ts but with an
 * HTTP fetch to a public well-known endpoint instead of an on-chain
 * broadcast. No testnet gas or wallet needed; the proof is that
 * safeFetch() returns a real response in the same process where it
 * refused with EgressPausedError seconds earlier.
 *
 * Run:
 *   npm run egress-pause-smoke
 *   tsx src/egress-pause-smoke.ts
 *
 * Exits 0 on success, non-zero on any failure.
 *
 * The target URL is https://example.com/ (IANA reserved, minimal
 * response, no auth, zero rate-limit concern). To smoke against a
 * different endpoint, set JUNOCLAW_EGRESS_SMOKE_URL.
 */

import {
  EgressPausedError,
  getEgressPaused,
  safeFetch,
  setEgressPaused,
} from "./utils/ssrf-guard.js";

const DEFAULT_URL = "https://example.com/";

function log(msg: string): void {
  console.log(`[egress-pause-smoke] ${msg}`);
}

async function main(): Promise<void> {
  const url = process.env.JUNOCLAW_EGRESS_SMOKE_URL ?? DEFAULT_URL;
  log(`target URL: ${url}`);
  log("");
  log("This smoke test verifies egress_paused in the same process");
  log("by arming the gate, confirming a real safeFetch refuses, then");
  log("disarming and confirming the same URL is fetched successfully.");
  log("");

  // ──────────────────────────────────────────────
  // Phase A: armed -> expect EgressPausedError
  // ──────────────────────────────────────────────

  log("═══ Phase A: arming egress_paused and attempting fetch ═══");
  setEgressPaused(true, "smoke:phase-a");
  log(`  getEgressPaused() = ${getEgressPaused()}`);

  let phaseACaught: Error | undefined;
  try {
    await safeFetch(url);
  } catch (e) {
    phaseACaught = e as Error;
  }

  if (!phaseACaught) {
    log("  [FAIL] Phase A: safeFetch returned without throwing");
    log("         Expected EgressPausedError; got nothing. Gate is not firing.");
    process.exit(1);
  }
  if (!(phaseACaught instanceof EgressPausedError)) {
    log(`  [FAIL] Phase A: wrong error type: ${phaseACaught.constructor.name}`);
    log(`         message: ${phaseACaught.message}`);
    process.exit(1);
  }
  log(`  [OK] Phase A refused as expected:`);
  log(`       ${phaseACaught.message}`);
  log(`       (url property: ${phaseACaught.url})`);
  log("");

  // ──────────────────────────────────────────────
  // Phase B: disarmed -> expect successful fetch
  // ──────────────────────────────────────────────

  log("═══ Phase B: disarming egress_paused and re-attempting fetch ═══");
  setEgressPaused(false, "smoke:phase-b");
  log(`  getEgressPaused() = ${getEgressPaused()}`);

  let result;
  try {
    result = await safeFetch(url);
  } catch (e) {
    const err = e as Error;
    log(`  [FAIL] Phase B: fetch threw despite disarmed state:`);
    log(`         ${err.constructor.name}: ${err.message}`);
    log(
      "         Check network connectivity; ensure JUNOCLAW_EGRESS_SMOKE_URL is reachable"
    );
    log("         and does not resolve to a private IP.");
    process.exit(1);
  }

  if (!result || typeof result.status !== "number") {
    log("  [FAIL] Phase B: safeFetch returned malformed result");
    process.exit(1);
  }

  log(`  [OK] Phase B fetched successfully:`);
  log(`       status: ${result.status} ${result.statusText}`);
  log(`       bytes:  ${result.bytesRead}`);
  log(`       url:    ${result.url}`);
  log("");

  // ──────────────────────────────────────────────
  // Overall result
  // ──────────────────────────────────────────────

  log("═══ SMOKE PASSED ═══");
  log("");
  log("Verified in a single process:");
  log(
    "  1. egress_paused=armed   -> safeFetch throws EgressPausedError, no fetch"
  );
  log("  2. egress_paused=disarmed -> same URL fetches and returns bytes");
  log("");
  log("Mean-time-to-halt in this release is module-restart (or a");
  log("setEgressPaused(true, ...) call from surrounding code). Hot-flip");
  log("via admin RPC is scheduled for v0.x.y-security-3 Phase 3d.");
  process.exit(0);
}

main().catch((e: Error) => {
  console.error(`[egress-pause-smoke] harness crashed: ${e.message}`);
  process.exit(2);
});
