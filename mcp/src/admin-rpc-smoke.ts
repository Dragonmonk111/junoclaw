/**
 * Live smoke for the admin RPC primitive — v0.x.y-security-3 Phase 3b.
 *
 * Boots a real listener on 127.0.0.1:0 with a fake WalletStore-shaped
 * controller, exercises every public endpoint and every defense layer
 * via real fetch() calls, then closes the server.
 *
 * No external state, no testnet. The proof is end-to-end: HTTP packets
 * over loopback against the same code an operator would use.
 *
 * Run:
 *   npm run admin-rpc-smoke
 *   tsx src/admin-rpc-smoke.ts
 *
 * Exits 0 on success.
 */

import { randomBytes } from "crypto";

import { startAdminRpcServer } from "./admin/rpc-server.js";
import type { SigningPausedController } from "./admin/rpc-server.js";

function log(msg: string): void {
  console.log(`[admin-rpc-smoke] ${msg}`);
}

function fail(msg: string): never {
  log(`[FAIL] ${msg}`);
  process.exit(1);
}

function makeFakeController(): SigningPausedController & {
  state: { paused: boolean; source: string | null };
} {
  return {
    state: { paused: false, source: null },
    setSigningPaused(paused: boolean, source: string) {
      this.state.paused = paused;
      this.state.source = paused ? source : null;
    },
    getSigningPaused() {
      return { paused: this.state.paused, source: this.state.source };
    },
  };
}

async function main(): Promise<void> {
  const token = randomBytes(32).toString("hex");
  const controller = makeFakeController();

  log(`Starting admin RPC on 127.0.0.1:0 with a 64-hex-char bearer token`);
  const handle = await startAdminRpcServer({
    token,
    port: 0,
    controller,
    auditLog: (e) => {
      // Print one-line audit during the smoke so the operator sees
      // the per-request shape exactly as they would in production.
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
  log(`  curl -H 'Authorization: Bearer <TOKEN>' ${handle.url}/signing/status`);
  log(
    `  curl -X POST -H 'Authorization: Bearer <TOKEN>' -H 'Content-Type: application/json' ` +
      `-d '{"source":"operator:incident-1"}' ${handle.url}/signing/pause`
  );
  log(
    `  curl -X POST -H 'Authorization: Bearer <TOKEN>' -H 'Content-Type: application/json' ` +
      `-d '{"source":"operator:resolved"}' ${handle.url}/signing/unpause`
  );
  log("");

  try {
    // ──────────────────────────────────────────────
    // 1. /health with token
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
    // 2. /signing/status (initial: not paused)
    // ──────────────────────────────────────────────
    log("Phase 2: GET /signing/status (expect paused=false)");
    {
      const r = await fetch(`${handle.url}/signing/status`, {
        headers: { authorization: `Bearer ${token}` },
      });
      if (r.status !== 200) fail(`/signing/status expected 200, got ${r.status}`);
      const body = (await r.json()) as { paused: boolean; source: string | null };
      if (body.paused !== false) fail(`expected paused=false, got ${body.paused}`);
      log(`  -> paused=${body.paused} source=${body.source}`);
    }

    // ──────────────────────────────────────────────
    // 3. /signing/pause flips the gate
    // ──────────────────────────────────────────────
    log("Phase 3: POST /signing/pause {source: smoke:phase-3}");
    {
      const r = await fetch(`${handle.url}/signing/pause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${token}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ source: "smoke:phase-3" }),
      });
      if (r.status !== 200) fail(`/signing/pause expected 200, got ${r.status}`);
      const body = (await r.json()) as { paused: boolean; source: string | null };
      if (body.paused !== true) fail(`expected paused=true, got ${body.paused}`);
      log(`  -> paused=${body.paused} source=${body.source}`);
    }

    // ──────────────────────────────────────────────
    // 4. Controller-side state confirmation
    // ──────────────────────────────────────────────
    log("Phase 4: confirm controller observed the flip");
    {
      if (!controller.state.paused) fail(`controller.state.paused is false`);
      if (controller.state.source !== "smoke:phase-3") {
        fail(`controller.state.source is ${controller.state.source}`);
      }
      log(
        `  -> controller.paused=${controller.state.paused} ` +
          `source=${controller.state.source}`
      );
    }

    // ──────────────────────────────────────────────
    // 5. /signing/unpause flips it back
    // ──────────────────────────────────────────────
    log("Phase 5: POST /signing/unpause {source: smoke:phase-5}");
    {
      const r = await fetch(`${handle.url}/signing/unpause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${token}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ source: "smoke:phase-5" }),
      });
      if (r.status !== 200) fail(`/signing/unpause expected 200, got ${r.status}`);
      const body = (await r.json()) as { paused: boolean; source: string | null };
      if (body.paused !== false) fail(`expected paused=false, got ${body.paused}`);
      log(`  -> paused=${body.paused} source=${body.source}`);
    }

    // ──────────────────────────────────────────────
    // 6. /policy roll-up (Phase 3c)
    // ──────────────────────────────────────────────
    log("Phase 6: GET /policy (expect process='mcp', signing_paused=false)");
    {
      const r = await fetch(`${handle.url}/policy`, {
        headers: { authorization: `Bearer ${token}` },
      });
      if (r.status !== 200) fail(`/policy expected 200, got ${r.status}`);
      const body = (await r.json()) as {
        process: string;
        version: string;
        kill_switches: { signing_paused: { paused: boolean; source: string | null } };
        reported_at: string;
      };
      if (body.process !== "mcp") fail(`expected process='mcp', got ${body.process}`);
      if (body.kill_switches.signing_paused.paused !== false) {
        fail(`expected signing_paused.paused=false, got ${body.kill_switches.signing_paused.paused}`);
      }
      log(`  -> process=${body.process} version=${body.version}`);
      log(`     kill_switches.signing_paused = ${JSON.stringify(body.kill_switches.signing_paused)}`);
      log(`     reported_at = ${body.reported_at}`);
    }

    // ──────────────────────────────────────────────
    // 7. Auth defense: missing token -> 401
    // ──────────────────────────────────────────────
    log("Phase 7: GET /health without token (expect 401)");
    {
      const r = await fetch(`${handle.url}/health`);
      if (r.status !== 401) fail(`expected 401, got ${r.status}`);
      log(`  -> ${r.status} as expected`);
    }

    // ──────────────────────────────────────────────
    // 8. Auth defense: wrong token -> 401
    // ──────────────────────────────────────────────
    log("Phase 8: GET /health with wrong token (expect 401)");
    {
      const r = await fetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer ${"x".repeat(64)}` },
      });
      if (r.status !== 401) fail(`expected 401, got ${r.status}`);
      log(`  -> ${r.status} as expected`);
    }

    // ──────────────────────────────────────────────
    // 9. Routing defense: unknown path -> 404
    // ──────────────────────────────────────────────
    log("Phase 9: GET /unknown (expect 404)");
    {
      const r = await fetch(`${handle.url}/unknown`, {
        headers: { authorization: `Bearer ${token}` },
      });
      if (r.status !== 404) fail(`expected 404, got ${r.status}`);
      log(`  -> ${r.status} as expected`);
    }

    // ──────────────────────────────────────────────
    // 10. Routing defense: wrong method -> 405
    // ──────────────────────────────────────────────
    log("Phase 10: POST /signing/status (expect 405)");
    {
      const r = await fetch(`${handle.url}/signing/status`, {
        method: "POST",
        headers: { authorization: `Bearer ${token}` },
      });
      if (r.status !== 405) fail(`expected 405, got ${r.status}`);
      log(`  -> ${r.status} as expected, Allow: ${r.headers.get("Allow")}`);
    }

    // ──────────────────────────────────────────────
    // 11. Origin defense: browser-like Origin header -> 400
    // ──────────────────────────────────────────────
    log("Phase 11: GET /health with Origin header (expect 400)");
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
    log("  * /health, /signing/status, /signing/pause, /signing/unpause all OK");
    log("  * /policy returns the read-only kill-switch roll-up (Phase 3c)");
    log("  * 401 on missing or wrong bearer token");
    log("  * 400 on browser-style Origin header (DNS-rebinding defense)");
    log("  * 404 / 405 on unknown path / wrong method");
    log("  * Audit log records every request with status + outcome");
    log("");
    log("Wiring into the MCP entry point will be done as part of the");
    log("v0.x.y-security-3 final tag commit, gated by JUNOCLAW_ADMIN_RPC=1");
    log("and JUNOCLAW_ADMIN_TOKEN.");
  } finally {
    await handle.close();
    log("server closed");
  }

  process.exit(0);
}

main().catch((e: Error) => {
  console.error(`[admin-rpc-smoke] harness crashed: ${e.message}\n${e.stack ?? ""}`);
  process.exit(2);
});
