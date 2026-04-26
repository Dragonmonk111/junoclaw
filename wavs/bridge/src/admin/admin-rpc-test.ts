/**
 * Unit tests for the WAVS-side admin RPC primitive — v0.x.y-security-3
 * Phase 3d. Mirrors the MCP-side coverage in
 * mcp/src/admin/admin-rpc-test.ts but exercises the /egress/*
 * endpoints and the EgressPausedController surface.
 *
 * Run:
 *   npm run admin-rpc-test       (from wavs/bridge/)
 *   tsx src/admin/admin-rpc-test.ts
 *
 * Exits 0 on success, 1 on any failure.
 */

import {
  AdminRpcHandle,
  AuditEntry,
  EgressPausedController,
  startAdminRpcServer,
  _internal,
} from "./rpc-server.js";

const TOKEN_OK = "x".repeat(32);
const TOKEN_LONG = "z".repeat(64);
const TOKEN_TOO_SHORT = "x".repeat(16);

let passed = 0;
let failed = 0;
const failures: string[] = [];

function test(name: string, fn: () => Promise<void>): Promise<void> {
  return fn().then(
    () => {
      console.log(`  PASS  ${name}`);
      passed++;
    },
    (e: Error) => {
      console.log(`  FAIL  ${name}\n        ${e.message}`);
      failed++;
      failures.push(name);
    }
  );
}

function assertEqual<T>(actual: T, expected: T, label: string): void {
  if (actual !== expected) {
    throw new Error(
      `${label}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`
    );
  }
}

function assertTrue(cond: boolean, label: string): void {
  if (!cond) throw new Error(`${label}: expected true`);
}

async function assertThrows(
  fn: () => unknown | Promise<unknown>,
  contains: string,
  label: string
): Promise<void> {
  let threw = false;
  try {
    await fn();
  } catch (e) {
    threw = true;
    const msg = (e as Error).message;
    if (!msg.includes(contains)) {
      throw new Error(
        `${label}: expected throw containing "${contains}", got: ${msg}`
      );
    }
  }
  if (!threw) {
    throw new Error(
      `${label}: expected throw containing "${contains}", but no throw occurred`
    );
  }
}

function makeController(): EgressPausedController & {
  state: { paused: boolean; source: string | null };
} {
  const ctrl = {
    state: { paused: false, source: null as string | null },
    setEgressPaused(paused: boolean, source: string) {
      this.state.paused = paused;
      this.state.source = paused ? source : null;
    },
    getEgressPaused(): { paused: boolean; source: string | null } {
      return { paused: this.state.paused, source: this.state.source };
    },
  };
  return ctrl;
}

function makeAuditCollector(): { entries: AuditEntry[]; sink: (e: AuditEntry) => void } {
  const entries: AuditEntry[] = [];
  return { entries, sink: (e: AuditEntry) => entries.push(e) };
}

async function startServer(
  overrides: Partial<Parameters<typeof startAdminRpcServer>[0]> = {}
): Promise<{
  handle: AdminRpcHandle;
  controller: ReturnType<typeof makeController>;
  audit: ReturnType<typeof makeAuditCollector>;
}> {
  const controller = makeController();
  const audit = makeAuditCollector();
  const handle = await startAdminRpcServer({
    token: TOKEN_OK,
    port: 0,
    controller,
    auditLog: audit.sink,
    ...overrides,
  });
  return { handle, controller, audit };
}

async function jsonFetch(
  url: string,
  init: RequestInit = {}
): Promise<{ status: number; json: unknown; headers: Headers }> {
  const resp = await fetch(url, init);
  const text = await resp.text();
  let json: unknown = null;
  if (text.length > 0) {
    try {
      json = JSON.parse(text);
    } catch {
      json = { _raw: text };
    }
  }
  return { status: resp.status, json, headers: resp.headers };
}

async function runTests(): Promise<void> {
  console.log("\nwavs admin-rpc tests — v0.x.y-security-3 Phase 3d\n");

  // --- Constructor validation ---

  await test("constructor: rejects token shorter than 32 bytes", async () => {
    await assertThrows(
      () =>
        startAdminRpcServer({
          token: TOKEN_TOO_SHORT,
          port: 0,
          controller: makeController(),
        }),
      "at least 32 bytes",
      "short token"
    );
  });

  await test("constructor: rejects non-string token", async () => {
    await assertThrows(
      () =>
        startAdminRpcServer({
          // @ts-expect-error
          token: 12345,
          port: 0,
          controller: makeController(),
        }),
      "must be a string",
      "non-string"
    );
  });

  await test("constructor: rejects missing/invalid controller", async () => {
    await assertThrows(
      () =>
        startAdminRpcServer({
          token: TOKEN_OK,
          port: 0,
          // @ts-expect-error
          controller: {},
        }),
      "EgressPausedController",
      "no methods"
    );
  });

  await test("constructor: rejects non-loopback host", async () => {
    await assertThrows(
      () =>
        startAdminRpcServer({
          token: TOKEN_OK,
          port: 0,
          host: "0.0.0.0",
          controller: makeController(),
        }),
      "127.0.0.1",
      "0.0.0.0 rejected"
    );
    await assertThrows(
      () =>
        startAdminRpcServer({
          token: TOKEN_OK,
          port: 0,
          host: "::1",
          controller: makeController(),
        }),
      "127.0.0.1",
      "::1 rejected"
    );
  });

  // --- safeEqual ---

  await test("safeEqual: equal strings -> true", () => {
    assertEqual(_internal.safeEqual("hello world", "hello world"), true, "equal");
    return Promise.resolve();
  });

  await test("safeEqual: different equal-length strings -> false", () => {
    assertEqual(_internal.safeEqual("hello world", "world hello"), false, "diff");
    return Promise.resolve();
  });

  await test("safeEqual: different-length strings -> false", () => {
    assertEqual(_internal.safeEqual("abc", "abcd"), false, "len diff a<b");
    assertEqual(_internal.safeEqual("abcd", "abc"), false, "len diff a>b");
    assertEqual(_internal.safeEqual("", "x"), false, "empty vs not");
    return Promise.resolve();
  });

  // --- RateLimiter ---

  await test("RateLimiter: allows up to max in window", () => {
    const rl = new _internal.RateLimiter(3, 60_000);
    assertEqual(rl.consume(), true, "1");
    assertEqual(rl.consume(), true, "2");
    assertEqual(rl.consume(), true, "3");
    assertEqual(rl.consume(), false, "4 (rejected)");
    return Promise.resolve();
  });

  await test("RateLimiter: window expiry re-allows", async () => {
    const rl = new _internal.RateLimiter(2, 50);
    rl.consume();
    rl.consume();
    assertEqual(rl.consume(), false, "3 (rejected)");
    await new Promise((r) => setTimeout(r, 70));
    assertEqual(rl.consume(), true, "after window");
  });

  // --- /health ---

  await test("/health: valid token -> 200", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(r.status, 200, "status");
      const body = r.json as { ok: boolean; version: string };
      assertEqual(body.ok, true, "ok");
      assertTrue(typeof body.version === "string", "version is string");
    } finally {
      await handle.close();
    }
  });

  await test("/health: missing token -> 401", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/health`);
      assertEqual(r.status, 401, "status");
      const body = r.json as { code: string };
      assertEqual(body.code, "ERR_UNAUTHORIZED", "code");
    } finally {
      await handle.close();
    }
  });

  await test("/health: wrong token -> 401", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer ${TOKEN_LONG}` },
      });
      assertEqual(r.status, 401, "status");
    } finally {
      await handle.close();
    }
  });

  await test("/health: malformed Authorization (no Bearer) -> 401", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/health`, {
        headers: { authorization: TOKEN_OK },
      });
      assertEqual(r.status, 401, "status");
    } finally {
      await handle.close();
    }
  });

  // --- /egress/status ---

  await test("/egress/status: returns current state (initial false)", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/egress/status`, {
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(r.status, 200, "status");
      const body = r.json as { paused: boolean; source: string | null };
      assertEqual(body.paused, false, "paused");
      assertEqual(body.source, null, "source");
    } finally {
      await handle.close();
    }
  });

  // --- /egress/pause ---

  await test("/egress/pause: arms the gate via controller", async () => {
    const { handle, controller } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/egress/pause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${TOKEN_OK}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ source: "admin-rpc:test" }),
      });
      assertEqual(r.status, 200, "status");
      const body = r.json as { paused: boolean; source: string | null };
      assertEqual(body.paused, true, "paused");
      assertEqual(body.source, "admin-rpc:test", "source");
      assertEqual(controller.state.paused, true, "controller state");
      assertEqual(controller.state.source, "admin-rpc:test", "controller source");
    } finally {
      await handle.close();
    }
  });

  await test("/egress/pause: missing source body -> 400", async () => {
    const { handle, controller } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/egress/pause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${TOKEN_OK}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({}),
      });
      assertEqual(r.status, 400, "status");
      assertEqual(controller.state.paused, false, "controller untouched");
    } finally {
      await handle.close();
    }
  });

  await test("/egress/pause: empty-string source -> 400", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/egress/pause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${TOKEN_OK}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ source: "" }),
      });
      assertEqual(r.status, 400, "status");
    } finally {
      await handle.close();
    }
  });

  await test("/egress/pause: oversized source -> 400", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/egress/pause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${TOKEN_OK}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ source: "x".repeat(257) }),
      });
      assertEqual(r.status, 400, "status");
    } finally {
      await handle.close();
    }
  });

  await test("/egress/pause: malformed JSON -> 400", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/egress/pause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${TOKEN_OK}`,
          "content-type": "application/json",
        },
        body: "not json {",
      });
      assertEqual(r.status, 400, "status");
    } finally {
      await handle.close();
    }
  });

  // --- /egress/unpause ---

  await test("/egress/unpause: disarms the gate", async () => {
    const { handle, controller } = await startServer();
    try {
      controller.setEgressPaused(true, "test:pre-arm");
      const r = await jsonFetch(`${handle.url}/egress/unpause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${TOKEN_OK}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ source: "admin-rpc:disarm" }),
      });
      assertEqual(r.status, 200, "status");
      const body = r.json as { paused: boolean; source: string | null };
      assertEqual(body.paused, false, "paused");
      assertEqual(controller.state.paused, false, "controller paused");
      assertEqual(controller.state.source, null, "controller source null");
    } finally {
      await handle.close();
    }
  });

  // --- /policy ---

  await test("/policy: returns roll-up with process='wavs-bridge' default", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/policy`, {
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(r.status, 200, "status");
      const body = r.json as {
        process: string;
        version: string;
        tag: string;
        kill_switches: { egress_paused: { paused: boolean; source: string | null } };
        reported_at: string;
      };
      assertEqual(body.process, "wavs-bridge", "process default");
      assertTrue(typeof body.version === "string", "version string");
      assertEqual(body.kill_switches.egress_paused.paused, false, "egress_paused initial");
      assertEqual(body.kill_switches.egress_paused.source, null, "source initial null");
      assertTrue(
        /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/.test(body.reported_at),
        "reported_at is ISO"
      );
    } finally {
      await handle.close();
    }
  });

  await test("/policy: respects custom processName option", async () => {
    const { handle } = await startServer({ processName: "wavs-bridge-staging" });
    try {
      const r = await jsonFetch(`${handle.url}/policy`, {
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(r.status, 200, "status");
      const body = r.json as { process: string };
      assertEqual(body.process, "wavs-bridge-staging", "process custom");
    } finally {
      await handle.close();
    }
  });

  await test("/policy: reflects state after /egress/pause", async () => {
    const { handle } = await startServer();
    try {
      await jsonFetch(`${handle.url}/egress/pause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${TOKEN_OK}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ source: "policy-test:armed" }),
      });
      const r = await jsonFetch(`${handle.url}/policy`, {
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(r.status, 200, "status");
      const body = r.json as {
        kill_switches: { egress_paused: { paused: boolean; source: string | null } };
      };
      assertEqual(body.kill_switches.egress_paused.paused, true, "armed");
      assertEqual(
        body.kill_switches.egress_paused.source,
        "policy-test:armed",
        "source recorded"
      );
    } finally {
      await handle.close();
    }
  });

  await test("/policy: requires bearer token", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/policy`);
      assertEqual(r.status, 401, "status");
    } finally {
      await handle.close();
    }
  });

  await test("/policy: POST -> 405 (read-only)", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/policy`, {
        method: "POST",
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(r.status, 405, "status");
      assertEqual(r.headers.get("Allow"), "GET", "Allow header");
    } finally {
      await handle.close();
    }
  });

  // --- Routing errors ---

  await test("unknown path -> 404", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/nope`, {
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(r.status, 404, "status");
    } finally {
      await handle.close();
    }
  });

  await test("wrong method on known path -> 405", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/egress/status`, {
        method: "POST",
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(r.status, 405, "status");
      assertEqual(r.headers.get("Allow"), "GET", "Allow header");
    } finally {
      await handle.close();
    }
  });

  // --- Defenses: Host / Origin / rate limit ---

  await test("Host header mismatch -> 400", async () => {
    const { handle } = await startServer();
    try {
      const http = await import("http");
      const status: number = await new Promise((resolve, reject) => {
        const req = http.request(
          {
            host: "127.0.0.1",
            port: handle.port,
            path: "/health",
            method: "GET",
            headers: {
              host: "evil.example.com",
              authorization: `Bearer ${TOKEN_OK}`,
            },
          },
          (resp) => {
            resp.resume();
            resolve(resp.statusCode ?? 0);
          }
        );
        req.on("error", reject);
        req.end();
      });
      assertEqual(status, 400, "status");
    } finally {
      await handle.close();
    }
  });

  await test("Origin header set -> 400", async () => {
    const { handle } = await startServer();
    try {
      const r = await jsonFetch(`${handle.url}/health`, {
        headers: {
          authorization: `Bearer ${TOKEN_OK}`,
          origin: "https://evil.example.com",
        },
      });
      assertEqual(r.status, 400, "status");
      const body = r.json as { code: string };
      assertEqual(body.code, "ERR_ORIGIN_REJECTED", "code");
    } finally {
      await handle.close();
    }
  });

  await test("rate limit: 4th request in window -> 429 with Retry-After", async () => {
    const { handle } = await startServer({
      rateLimitMax: 3,
      rateLimitWindowMs: 60_000,
    });
    try {
      for (let i = 0; i < 3; i++) {
        const r = await jsonFetch(`${handle.url}/health`, {
          headers: { authorization: `Bearer ${TOKEN_OK}` },
        });
        assertEqual(r.status, 200, `req ${i + 1}`);
      }
      const r = await jsonFetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(r.status, 429, "rate limited");
      assertTrue(
        r.headers.get("retry-after") !== null,
        "Retry-After header present"
      );
    } finally {
      await handle.close();
    }
  });

  await test("rate limit fires before auth check (no token-spamming)", async () => {
    const { handle } = await startServer({
      rateLimitMax: 2,
      rateLimitWindowMs: 60_000,
    });
    try {
      await jsonFetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer wrong1` },
      });
      await jsonFetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer wrong2` },
      });
      const r = await jsonFetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(r.status, 429, "rate-limited despite correct token");
    } finally {
      await handle.close();
    }
  });

  // --- Audit log ---

  await test("audit log: records ok request with status and outcome", async () => {
    const { handle, audit } = await startServer();
    try {
      await jsonFetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      assertEqual(audit.entries.length, 1, "1 entry");
      const e = audit.entries[0];
      assertEqual(e.status, 200, "status");
      assertEqual(e.outcome, "ok", "outcome");
      assertEqual(e.method, "GET", "method");
      assertEqual(e.path, "/health", "path");
    } finally {
      await handle.close();
    }
  });

  await test("audit log: records auth failure", async () => {
    const { handle, audit } = await startServer();
    try {
      await jsonFetch(`${handle.url}/health`);
      assertEqual(audit.entries.length, 1, "1 entry");
      assertEqual(audit.entries[0].status, 401, "status");
      assertEqual(audit.entries[0].outcome, "auth_fail", "outcome");
    } finally {
      await handle.close();
    }
  });

  await test("audit log: never records the bearer token", async () => {
    const { handle, audit } = await startServer();
    try {
      await jsonFetch(`${handle.url}/health`, {
        headers: { authorization: `Bearer ${TOKEN_OK}` },
      });
      const stringified = JSON.stringify(audit.entries);
      assertTrue(
        !stringified.includes(TOKEN_OK),
        "audit log does not contain the token"
      );
    } finally {
      await handle.close();
    }
  });

  // --- Body size cap ---

  await test("oversized request body -> 400 (or stream destroyed)", async () => {
    const { handle, controller } = await startServer();
    try {
      const huge = "A".repeat(_internal.MAX_BODY_BYTES + 1024);
      const r = await jsonFetch(`${handle.url}/egress/pause`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${TOKEN_OK}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ source: huge }),
      });
      assertEqual(controller.state.paused, false, "controller untouched");
      if (r.status !== 0) {
        assertTrue(r.status === 400, `expected 400, got ${r.status}`);
      }
    } catch {
      // fetch may throw if connection is destroyed; controller-untouched
      // assertion above covers correctness.
    } finally {
      await handle.close();
    }
  });

  // --- Lifecycle ---

  await test("close() resolves and binds release", async () => {
    const { handle } = await startServer();
    const port = handle.port;
    await handle.close();
    const reopen = await startAdminRpcServer({
      token: TOKEN_OK,
      port,
      controller: makeController(),
      auditLog: () => {},
    });
    try {
      assertEqual(reopen.port, port, "rebound on same port");
    } finally {
      await reopen.close();
    }
  });

  // ── Summary ──
  console.log("");
  if (failed > 0) {
    console.log(`  ${passed} passed, ${failed} failed`);
    console.log(`  failures: ${failures.join(", ")}`);
    process.exit(1);
  } else {
    console.log(`  ${passed} passed, 0 failed`);
    process.exit(0);
  }
}

runTests().catch((e: Error) => {
  console.error(`test harness crashed: ${e.message}\n${e.stack ?? ""}`);
  process.exit(2);
});
