/**
 * Unit tests for the egress_paused runtime kill-switch shipped in
 * v0.x.y-security-3 Phase 3a. Mirrors the signing-pause-test.ts
 * contract from v0.x.y-security-2 (MCP side) on the WAVS-bridge
 * side.
 *
 * Run:
 *   npm run egress-pause-test
 *   tsx src/utils/egress-pause-test.ts
 *
 * Exits 0 on success, 1 on any failure. Uses injected DNS and
 * fetch seams so no real network I/O occurs.
 */

import {
  EgressPausedError,
  getEgressPaused,
  getEgressPausedSource,
  parseEgressPausedEnv,
  safeFetch,
  setEgressPaused,
  _internal,
} from "./ssrf-guard.js";

let passed = 0;
let failed = 0;
const failures: string[] = [];

function test(name: string, fn: () => void | Promise<void>): Promise<void> {
  return Promise.resolve()
    .then(() => {
      // Reset egress_paused state between cases so each test starts
      // from a known-disarmed baseline. Module-local state otherwise
      // leaks across cases.
      _internal._resetEgressPaused();
    })
    .then(fn)
    .then(
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

function assertThrows(fn: () => unknown, contains: string, label: string): void {
  let threw = false;
  try {
    fn();
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

async function assertAsyncThrows(
  fn: () => Promise<unknown>,
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

// ──────────────────────────────────────────────
// parseEgressPausedEnv — env var parsing (fail-closed)
// ──────────────────────────────────────────────

async function runTests(): Promise<void> {
  console.log("\negress-paused tests — v0.x.y-security-3 Phase 3a\n");

  // --- parseEgressPausedEnv ---

  await test("parseEgressPausedEnv: undefined -> false", () => {
    assertEqual(parseEgressPausedEnv(undefined), false, "undefined");
  });

  await test("parseEgressPausedEnv: empty string -> false", () => {
    assertEqual(parseEgressPausedEnv(""), false, "empty");
    assertEqual(parseEgressPausedEnv("   "), false, "whitespace-only");
  });

  await test("parseEgressPausedEnv: '0' / 'false' / 'no' / 'off' -> false", () => {
    assertEqual(parseEgressPausedEnv("0"), false, "0");
    assertEqual(parseEgressPausedEnv("false"), false, "false");
    assertEqual(parseEgressPausedEnv("FALSE"), false, "FALSE (case-insensitive)");
    assertEqual(parseEgressPausedEnv("no"), false, "no");
    assertEqual(parseEgressPausedEnv("OFF"), false, "OFF (case-insensitive)");
    assertEqual(parseEgressPausedEnv("  false  "), false, "trimmed 'false'");
  });

  await test("parseEgressPausedEnv: '1' / 'true' -> true", () => {
    assertEqual(parseEgressPausedEnv("1"), true, "1");
    assertEqual(parseEgressPausedEnv("true"), true, "true");
    assertEqual(parseEgressPausedEnv("TRUE"), true, "TRUE (case-insensitive)");
    assertEqual(parseEgressPausedEnv("yes"), true, "yes (not in disarm list)");
  });

  await test("parseEgressPausedEnv: fail-closed on typos", () => {
    // Anything non-empty and not in the explicit disarm list arms.
    // Typos of "false" like "flase", "flse", "fales" all arm the gate.
    assertEqual(parseEgressPausedEnv("flase"), true, "flase -> arms");
    assertEqual(parseEgressPausedEnv("fales"), true, "fales -> arms");
    assertEqual(parseEgressPausedEnv("fls"), true, "fls -> arms");
    assertEqual(parseEgressPausedEnv("n0"), true, "n0 -> arms");
    assertEqual(parseEgressPausedEnv("0x"), true, "0x -> arms (not exactly '0')");
    assertEqual(parseEgressPausedEnv("disable"), true, "disable -> arms");
  });

  // --- set/get ---

  await test("setEgressPaused / getEgressPaused round-trip", () => {
    assertEqual(getEgressPaused(), false, "initial state disarmed");
    assertEqual(getEgressPausedSource(), undefined, "initial source undefined");

    setEgressPaused(true, "test:arm");
    assertEqual(getEgressPaused(), true, "after arm, paused=true");
    assertEqual(getEgressPausedSource(), "test:arm", "source recorded");

    setEgressPaused(false, "test:disarm");
    assertEqual(getEgressPaused(), false, "after disarm, paused=false");
    assertEqual(getEgressPausedSource(), "test:disarm", "source updated");
  });

  await test("setEgressPaused rejects non-boolean paused", () => {
    assertThrows(
      // @ts-expect-error — intentional runtime-type violation
      () => setEgressPaused("yes", "test"),
      "paused must be a boolean",
      "non-bool"
    );
    assertThrows(
      // @ts-expect-error
      () => setEgressPaused(1, "test"),
      "paused must be a boolean",
      "numeric"
    );
    assertThrows(
      // @ts-expect-error
      () => setEgressPaused(null, "test"),
      "paused must be a boolean",
      "null"
    );
  });

  await test("setEgressPaused rejects empty/non-string source", () => {
    assertThrows(
      () => setEgressPaused(true, ""),
      "source must be a non-empty string",
      "empty"
    );
    assertThrows(
      // @ts-expect-error
      () => setEgressPaused(true, undefined),
      "source must be a non-empty string",
      "undefined"
    );
    assertThrows(
      // @ts-expect-error
      () => setEgressPaused(true, 42),
      "source must be a non-empty string",
      "numeric"
    );
  });

  // --- EgressPausedError shape ---

  await test("EgressPausedError has .url and instanceof works", () => {
    const err = new EgressPausedError("http://example.com/x");
    if (!(err instanceof EgressPausedError)) {
      throw new Error("instanceof EgressPausedError failed");
    }
    if (!(err instanceof Error)) {
      throw new Error("instanceof Error failed");
    }
    assertEqual(err.url, "http://example.com/x", "err.url");
    assertEqual(err.name, "EgressPausedError", "err.name");
    if (!err.message.includes("egress_paused")) {
      throw new Error(
        `err.message should mention egress_paused, got: ${err.message}`
      );
    }
    if (!err.message.includes("http://example.com/x")) {
      throw new Error(
        `err.message should include the URL, got: ${err.message}`
      );
    }
  });

  // --- safeFetch gate-first behavior ---

  await test("safeFetch refuses with EgressPausedError when armed", async () => {
    setEgressPaused(true, "test:gate");

    // Provide a DNS / fetch that would explode if called — if the
    // gate-first check runs correctly, neither seam should be invoked.
    const explodingDns = async (): Promise<never> => {
      throw new Error(
        "BUG: DNS lookup called despite egress_paused armed (gate is not first)"
      );
    };
    const explodingFetch = (async () => {
      throw new Error(
        "BUG: fetch called despite egress_paused armed (gate is not first)"
      );
    }) as unknown as typeof fetch;

    let caught: Error | undefined;
    try {
      await safeFetch("http://example.com/x", {
        dnsLookup: explodingDns,
        fetchImpl: explodingFetch,
      });
    } catch (e) {
      caught = e as Error;
    }
    if (!caught) {
      throw new Error("safeFetch should have thrown");
    }
    if (!(caught instanceof EgressPausedError)) {
      throw new Error(
        `expected EgressPausedError, got ${caught.constructor.name}: ${caught.message}`
      );
    }
    assertEqual(caught.url, "http://example.com/x", "err.url matches");
  });

  await test("safeFetch refuses even for malformed / empty URLs when armed", async () => {
    setEgressPaused(true, "test:gate");
    // Gate fires before URL parsing, so the empty-URL branch is unreachable.
    await assertAsyncThrows(
      () => safeFetch(""),
      "egress_paused",
      "empty-url when armed"
    );
    await assertAsyncThrows(
      () => safeFetch("not a url"),
      "egress_paused",
      "garbage-url when armed"
    );
  });

  await test("safeFetch proceeds normally when disarmed", async () => {
    // Default: disarmed (reset by test harness). Construct a fully
    // valid fetch path through the guard with mocked DNS + fetch
    // and assert the body round-trips.
    const dns = async (hostname: string) => {
      if (hostname !== "example.com") {
        throw new Error(`unexpected hostname: ${hostname}`);
      }
      return [{ address: "93.184.216.34", family: 4 }];
    };
    const fetchImpl = (async () => {
      return new Response("hello world", {
        status: 200,
        statusText: "OK",
      });
    }) as unknown as typeof fetch;

    const result = await safeFetch("http://example.com/", {
      dnsLookup: dns,
      fetchImpl,
    });
    assertEqual(result.status, 200, "status");
    assertEqual(result.text, "hello world", "body");
    assertEqual(result.bytesRead, "hello world".length, "bytesRead");
  });

  await test("setEgressPaused(true) then (false) re-enables fetching", async () => {
    // Arm, confirm refusal, disarm, confirm success in the same test.
    setEgressPaused(true, "test:arm");
    await assertAsyncThrows(
      () => safeFetch("http://example.com/"),
      "egress_paused",
      "armed refuses"
    );

    setEgressPaused(false, "test:disarm");
    const dns = async () => [{ address: "93.184.216.34", family: 4 }];
    const fetchImpl = (async () =>
      new Response("back", { status: 200 })) as unknown as typeof fetch;
    const result = await safeFetch("http://example.com/", {
      dnsLookup: dns,
      fetchImpl,
    });
    assertEqual(result.status, 200, "disarmed succeeds");
  });

  await test("gate-first: no DNS or fetch side effects when armed", async () => {
    // Explicit counter-based check — increment should stay at 0.
    setEgressPaused(true, "test:sideeffects");
    let dnsCalls = 0;
    let fetchCalls = 0;
    const dns = async (_hostname: string) => {
      dnsCalls++;
      return [{ address: "1.2.3.4", family: 4 }];
    };
    const fetchImpl = (async () => {
      fetchCalls++;
      return new Response("x", { status: 200 });
    }) as unknown as typeof fetch;

    try {
      await safeFetch("http://anything.example/", {
        dnsLookup: dns,
        fetchImpl,
      });
    } catch (e) {
      if (!(e instanceof EgressPausedError)) throw e;
    }
    assertEqual(dnsCalls, 0, "no DNS lookups");
    assertEqual(fetchCalls, 0, "no fetch calls");
  });

  await test("gate state is isolated from the SSRF defense layer", async () => {
    // Disarmed + a URL that would fail SSRF (169.254.169.254) should
    // produce the SSRF error, not an egress_paused error. This proves
    // the gate doesn't interfere with the underlying defenses.
    setEgressPaused(false, "test:ssrf-interaction");
    const dns = async () => [{ address: "169.254.169.254", family: 4 }];
    const fetchImpl = (async () =>
      new Response("nope", { status: 200 })) as unknown as typeof fetch;

    await assertAsyncThrows(
      () =>
        safeFetch("http://metadata.local/", {
          dnsLookup: dns,
          fetchImpl,
        }),
      "private/restricted IP",
      "SSRF still fires when disarmed"
    );
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
  console.error(`test harness crashed: ${e.message}`);
  process.exit(2);
});
