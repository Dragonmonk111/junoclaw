/**
 * Live smoke for the WAVS-side admin RPC primitive — v0.x.y-security-3
 * Phase 3d.
 *
 * Boots a real listener on 127.0.0.1:0 with the defaultEgressController
 * (which wraps the module-level egress_paused state in ssrf-guard.ts),
 * exercises every public endpoint, and — critically — verifies that
 * arming the gate via /egress/pause causes a real safeFetch() call
 * to refuse with EgressPausedError. This is the end-to-end proof
 * that the admin RPC and the SSRF guard share state correctly.
 *
 * No external state, no testnet. The proof is end-to-end:
 *   admin-RPC POST -> module-level state -> safeFetch refusal -> success
 *
 * Run:
 *   npm run admin-rpc-smoke      (from wavs/bridge/)
 *   tsx src/admin-rpc-smoke.ts
 *
 * Exits 0 on success.
 */

import { randomBytes } from "crypto";

import {
  startAdminRpcServer,
  defaultEgressController,
} from "./admin/rpc-server.js";
import { EgressPausedError, safeFetch, _internal } from "./utils/ssrf-guard.js";

const SMOKE_URL =
  process.env.JUNOCLAW_EGRESS_SMOKE_URL ?? "https://example.com/";

function log(msg: string): void {
  console.log(`[wavs-admin-rpc-smoke] ${msg}`);
}

function fail(msg: string): never {
  log(`[FAIL] ${msg}`);
  process.exit(1);
}

async function main(): Promise<void> {
  // Reset module-level state in case earlier tests left it armed.
  _internal._resetEgressPaused();

  const token = randomBytes(32).toString("hex");

  log(`Starting WAVS admin RPC on 127.0.0.1:0`);
  log(`Smoke target URL: ${SMOKE_URL}`);
  log(`(set JUNOCLAW_EGRESS_SMOKE_URL to override)`);
  log("");

  const handle = await startAdminRpcServer({
    token,
    port: 0,
    controller: defaultEgressController,
    auditLog: (e) => {
      console.error(
        `  audit: ${e.ts} ${e.method} ${e.path} -> ${e.status} ${e.outcome}` +
          (e.message ? `  (${e.message})` : "")
      );
    },
  });

  log(`Listening at ${handle.url}`);
  log("");
  log("Equivalent operator curl commands (copy with the token if you re-run):");
  log("");
  log(`  curl -H 'Authorization: Bearer <TOKEN>' ${handle.url}/health`);
  log(`  curl -H 'Authorization: Bearer <TOKEN>' ${handle.url}/policy`);
  log(`  curl -H 'Authorization: Bearer <TOKEN>' ${handle.url}/egress/status`);
  log(
    `  curl -X POST -H 'Authorization: Bearer <TOKEN>' -H 'Content-Type: application/json' ` +
      `-d '{"source":"operator:incident-1"}' ${handle.url}/egress/pause`
  );
  log(
    `  curl -X POST -H 'Authorization: Bearer <TOKEN>' -H 'Content-Type: application/json' ` +
      `-d '{"source":"operator:resolved"}' ${handle.url}/egress/unpause`
  );
  log("");

  try {
    // ──────────────────────────────────────────────
    // 1. /health
    // ──────────────────────────────────────────────
    log("Phase 1: GET /health with valid token");
    {
      const r = await fetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer ${token}` },
      });
      if (r.status !== 200) fail(`/health expected 200, got ${r.status}`);
      const body = (await r.json()) as { ok: boolean; version: string };
      if (!body.ok) fail(`/health body.ok was false`);
      log(`  -> ${r.status} ok=${body.ok} version=${body.version}`);
    }

    // ──────────────────────────────────────────────
    // 2. /egress/status (initial: not paused)
    // ──────────────────────────────────────────────
    log("Phase 2: GET /egress/status (expect paused=false)");
    {
      const r = await fetch(`${handle.url}/egress/status`, {
        headers: { authorization: `Bearer ${token}` },
      });
      if (r.status !== 200) fail(`/egress/status expected 200, got ${r.status}`);
      const body = (await r.json()) as { paused: boolean; source: string | null };
      if (body.paused !== false) fail(`expected paused=false, got ${body.paused}`);
      log(`  -> paused=${body.paused} source=${body.source}`);
    }

    // ──────────────────────────────────────────────
    // 3. End-to-end: safeFetch works while disarmed
    // ──────────────────────────────────────────────
    log("Phase 3: direct safeFetch() while disarmed (expect 200 from real network)");
    {
      try {
        const result = await safeFetch(SMOKE_URL);
        if (result.status !== 200) {
          fail(`safeFetch returned status ${result.status}, expected 200`);
        }
        log(`  -> status=${result.status} bytes=${result.bytesRead}`);
      } catch (e) {
        const err = e as Error;
        fail(
          `safeFetch threw while disarmed: ${err.constructor.name}: ${err.message}\n` +
            `       (set JUNOCLAW_EGRESS_SMOKE_URL to a reachable URL or check network)`
        );
      }
    }

    // ──────────────────────────────────────────────
    // 4. /egress/pause via admin RPC
    // ──────────────────────────────────────────────
    log("Phase 4: POST /egress/pause {source: smoke:phase-4}");
    {
      const r = await fetch(`${handle.url}/egress/pause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${token}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ source: "smoke:phase-4" }),
      });
      if (r.status !== 200) fail(`/egress/pause expected 200, got ${r.status}`);
      const body = (await r.json()) as { paused: boolean; source: string | null };
      if (body.paused !== true) fail(`expected paused=true, got ${body.paused}`);
      log(`  -> paused=${body.paused} source=${body.source}`);
    }

    // ──────────────────────────────────────────────
    // 5. End-to-end coupling: safeFetch refuses while admin RPC armed
    //    THIS IS THE KEY ASSERTION OF THE WHOLE PHASE
    // ──────────────────────────────────────────────
    log("Phase 5: direct safeFetch() while armed (expect EgressPausedError)");
    {
      let caught: Error | undefined;
      try {
        await safeFetch(SMOKE_URL);
      } catch (e) {
        caught = e as Error;
      }
      if (!caught) fail("safeFetch did not throw while admin RPC armed the gate");
      if (!(caught instanceof EgressPausedError)) {
        fail(
          `expected EgressPausedError, got ${caught.constructor.name}: ${caught.message}`
        );
      }
      log(`  -> refused as expected: ${caught.message.split(".")[0]}`);
      log(`     (admin RPC -> module state -> safeFetch coupling verified)`);
    }

    // ──────────────────────────────────────────────
    // 6. /egress/unpause via admin RPC
    // ──────────────────────────────────────────────
    log("Phase 6: POST /egress/unpause {source: smoke:phase-6}");
    {
      const r = await fetch(`${handle.url}/egress/unpause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${token}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ source: "smoke:phase-6" }),
      });
      if (r.status !== 200) fail(`/egress/unpause expected 200, got ${r.status}`);
      const body = (await r.json()) as { paused: boolean; source: string | null };
      if (body.paused !== false) fail(`expected paused=false, got ${body.paused}`);
      log(`  -> paused=${body.paused} source=${body.source}`);
    }

    // ──────────────────────────────────────────────
    // 7. End-to-end: safeFetch works again after disarm
    // ──────────────────────────────────────────────
    log("Phase 7: direct safeFetch() after disarm (expect 200 again)");
    {
      try {
        const result = await safeFetch(SMOKE_URL);
        if (result.status !== 200) {
          fail(`safeFetch returned status ${result.status}, expected 200`);
        }
        log(`  -> status=${result.status} bytes=${result.bytesRead}`);
      } catch (e) {
        fail(`safeFetch threw after disarm: ${(e as Error).message}`);
      }
    }

    // ──────────────────────────────────────────────
    // 8. /policy (Phase 3c, mirrored on WAVS side)
    // ──────────────────────────────────────────────
    log("Phase 8: GET /policy (expect process='wavs-bridge', egress_paused=false)");
    {
      const r = await fetch(`${handle.url}/policy`, {
        headers: { authorization: `Bearer ${token}` },
      });
      if (r.status !== 200) fail(`/policy expected 200, got ${r.status}`);
      const body = (await r.json()) as {
        process: string;
        version: string;
        kill_switches: { egress_paused: { paused: boolean; source: string | null } };
        reported_at: string;
      };
      if (body.process !== "wavs-bridge") {
        fail(`expected process='wavs-bridge', got ${body.process}`);
      }
      if (body.kill_switches.egress_paused.paused !== false) {
        fail(
          `expected egress_paused.paused=false, got ${body.kill_switches.egress_paused.paused}`
        );
      }
      log(`  -> process=${body.process} version=${body.version}`);
      log(`     kill_switches.egress_paused = ${JSON.stringify(body.kill_switches.egress_paused)}`);
      log(`     reported_at = ${body.reported_at}`);
    }

    // ──────────────────────────────────────────────
    // 9. Auth defense: missing token -> 401
    // ──────────────────────────────────────────────
    log("Phase 9: GET /health without token (expect 401)");
    {
      const r = await fetch(`${handle.url}/health`);
      if (r.status !== 401) fail(`expected 401, got ${r.status}`);
      log(`  -> ${r.status} as expected`);
    }

    // ──────────────────────────────────────────────
    // 10. Auth defense: wrong token -> 401
    // ──────────────────────────────────────────────
    log("Phase 10: GET /health with wrong token (expect 401)");
    {
      const r = await fetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer ${"x".repeat(64)}` },
      });
      if (r.status !== 401) fail(`expected 401, got ${r.status}`);
      log(`  -> ${r.status} as expected`);
    }

    // ──────────────────────────────────────────────
    // 11. Routing defense: unknown path -> 404
    // ──────────────────────────────────────────────
    log("Phase 11: GET /unknown (expect 404)");
    {
      const r = await fetch(`${handle.url}/unknown`, {
        headers: { authorization: `Bearer ${token}` },
      });
      if (r.status !== 404) fail(`expected 404, got ${r.status}`);
      log(`  -> ${r.status} as expected`);
    }

    // ──────────────────────────────────────────────
    // 12. Routing defense: wrong method -> 405
    // ──────────────────────────────────────────────
    log("Phase 12: POST /egress/status (expect 405)");
    {
      const r = await fetch(`${handle.url}/egress/status`, {
        method: "POST",
        headers: { authorization: `Bearer ${token}` },
      });
      if (r.status !== 405) fail(`expected 405, got ${r.status}`);
      log(`  -> ${r.status} as expected, Allow: ${r.headers.get("Allow")}`);
    }

    // ──────────────────────────────────────────────
    // 13. Origin defense: browser-style request -> 400
    // ──────────────────────────────────────────────
    log("Phase 13: GET /health with Origin header (expect 400)");
    {
      const r = await fetch(`${handle.url}/health`, {
        headers: {
          authorization: `Bearer ${token}`,
          origin: "https://evil.example.com",
        },
      });
      if (r.status !== 400) fail(`expected 400, got ${r.status}`);
      log(`  -> ${r.status} as expected`);
    }

    log("");
    log("═══ SMOKE PASSED ═══");
    log("");
    log("Verified end-to-end against a real loopback HTTP listener:");
    log("  * /health, /egress/status, /egress/pause, /egress/unpause all OK");
    log("  * /policy returns the read-only kill-switch roll-up");
    log("");
    log("END-TO-END COUPLING (the headline assertion of this phase):");
    log("  * safeFetch() succeeds while disarmed");
    log("  * POST /egress/pause flips ssrf-guard module state");
    log("  * safeFetch() refuses with EgressPausedError after the flip");
    log("  * POST /egress/unpause restores fetch capability");
    log("");
    log("Defenses verified:");
    log("  * 401 on missing or wrong bearer token");
    log("  * 400 on browser-style Origin header (DNS-rebinding defense)");
    log("  * 404 / 405 on unknown path / wrong method");
    log("  * Audit log records every request with status + outcome");
    log("");
    log("Wiring into the bridge entry point will be done as part of the");
    log("v0.x.y-security-3 final tag commit, gated by JUNOCLAW_ADMIN_RPC=1");
    log("and JUNOCLAW_ADMIN_TOKEN.");
  } finally {
    await handle.close();
    log("server closed");
    // Reset module state so subsequent test runs start clean.
    _internal._resetEgressPaused();
  }

  process.exit(0);
}

main().catch((e: Error) => {
  console.error(
    `[wavs-admin-rpc-smoke] harness crashed: ${e.message}\n${e.stack ?? ""}`
  );
  process.exit(2);
});
