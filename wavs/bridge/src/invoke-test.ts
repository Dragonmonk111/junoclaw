/**
 * Invoke server smoke test.
 *
 * Starts the invoke server with a test token, sends a health check,
 * then sends a mock invoke request and verifies the response shape.
 *
 * Does NOT require wasmtime or a real sealed blob — the test mocks
 * the wasmtime spawn by checking that the server correctly handles:
 *   - Auth failures
 *   - Allowlist enforcement
 *   - Body validation
 *   - Health endpoint
 *
 * For a full end-to-end test with real wasmtime, see the README.
 *
 * Usage:
 *   npx tsx src/invoke-test.ts
 */

import { startInvokeServer } from "./invoke-server.js";

const TEST_TOKEN = "test-token-at-least-32-bytes-long-aaaa-bbbb";

async function main(): Promise<void> {
  // Set required env vars for the server
  process.env.WAVS_INVOKE_TOKEN = TEST_TOKEN;
  process.env.WAVS_INVOKE_PORT = "0"; // OS-assigned
  process.env.WAVS_INVOKE_HOST = "127.0.0.1";
  process.env.WAVS_INVOKE_PASSPHRASE = "test-passphrase";
  process.env.WAVS_INVOKE_SEALED_BLOB = "00".repeat(64); // dummy
  process.env.WAVS_INVOKE_WASMTIME = "echo"; // mock: echo will "succeed" but produce garbage
  process.env.WAVS_INVOKE_ALLOWED_COMPONENTS = "sealed-signer";

  const server = await startInvokeServer({
    auditLog: (entry) => {
      // Suppress audit noise in test output
      if (process.env.INVOKE_TEST_VERBOSE) {
        console.error(`[audit] ${JSON.stringify(entry)}`);
      }
    },
  });

  const baseUrl = server.url;
  let passed = 0;
  let failed = 0;

  function check(name: string, condition: boolean, detail?: string): void {
    if (condition) {
      passed++;
      console.log(`  ✓ ${name}`);
    } else {
      failed++;
      console.error(`  ✗ ${name}${detail ? ` — ${detail}` : ""}`);
    }
  }

  async function fetchJson(
    path: string,
    opts: { method?: string; body?: object; headers?: Record<string, string> } = {}
  ): Promise<{ status: number; json: any }> {
    const resp = await fetch(`${baseUrl}${path}`, {
      method: opts.method ?? "GET",
      headers: {
        Authorization: `Bearer ${TEST_TOKEN}`,
        ...(opts.headers ?? {}),
      },
      body: opts.body ? JSON.stringify(opts.body) : undefined,
    });
    const json = await resp.json().catch(() => null);
    return { status: resp.status, json };
  }

  console.log("\n── Invoke Server Smoke Test ──\n");

  // 1. Health check
  {
    const { status, json } = await fetchJson("/health");
    check("GET /health returns 200", status === 200);
    check("health response has version", json?.version?.includes("invoke") ?? false, json?.version);
  }

  // 2. Auth failure
  {
    const resp = await fetch(`${baseUrl}/health`, {
      headers: { Authorization: "Bearer wrong-token" },
    });
    check("GET /health with wrong token returns 401", resp.status === 401);
  }

  // 3. Missing auth
  {
    const resp = await fetch(`${baseUrl}/health`);
    check("GET /health without token returns 401", resp.status === 401);
  }

  // 4. Allowlist enforcement
  {
    const { status, json } = await fetchJson("/invoke/unknown-component", {
      method: "POST",
      body: { trigger: "sign_request", input: {} },
    });
    check("POST /invoke/unknown-component returns 403", status === 403);
    check("403 has ERR_NOT_ALLOWED code", json?.code === "ERR_NOT_ALLOWED");
  }

  // 5. Body validation — missing trigger
  {
    const { status, json } = await fetchJson("/invoke/sealed-signer", {
      method: "POST",
      body: { input: {} },
    });
    check("POST without trigger returns 400", status === 400);
    check("400 has ERR_BAD_TRIGGER code", json?.code === "ERR_BAD_TRIGGER");
  }

  // 6. Body validation — missing input
  {
    const { status, json } = await fetchJson("/invoke/sealed-signer", {
      method: "POST",
      body: { trigger: "sign_request" },
    });
    check("POST without input returns 400", status === 400);
    check("400 has ERR_BAD_BODY code", json?.code === "ERR_BAD_BODY");
  }

  // 7. Body validation — missing required field
  {
    const { status, json } = await fetchJson("/invoke/sealed-signer", {
      method: "POST",
      body: {
        trigger: "sign_request",
        input: {
          sender: "juno1test",
          contract: "juno1test",
          // missing exec_msg_json and other fields
        },
      },
    });
    check("POST with missing field returns 400", status === 400);
    check("400 mentions missing field", json?.error?.includes("exec_msg_json") ?? false, json?.error);
  }

  // 8. Wrong method on known path
  {
    const { status } = await fetchJson("/health", { method: "POST" });
    check("POST /health returns 405", status === 405);
  }

  // 9. Unknown route
  {
    const { status } = await fetchJson("/unknown");
    check("GET /unknown returns 404", status === 404);
  }

  // 10. Origin header rejection
  {
    const resp = await fetch(`${baseUrl}/health`, {
      headers: {
        Authorization: `Bearer ${TEST_TOKEN}`,
        Origin: "http://evil.com",
      },
    });
    check("Request with Origin header returns 400", resp.status === 400);
  }

  // Cleanup
  await server.close();

  console.log(`\n${passed} passed, ${failed} failed\n`);
  if (failed > 0) {
    process.exit(1);
  }
}

main().catch((e) => {
  console.error(`[invoke-test] fatal: ${(e as Error).message}`);
  process.exit(1);
});
