/**
 * Smoke test for ssrf-guard module — Ffern H-3 regression coverage.
 *
 * Run:
 *   npm run ssrf-guard-test         (from wavs/bridge/)
 *   tsx src/utils/ssrf-guard-test.ts
 *
 * Exits 0 on success, 1 on any failure. Uses injected DNS and fetch
 * seams to test the guard logic in isolation — no real network I/O.
 */

import { safeFetch, isPrivateIp, _internal } from "./ssrf-guard.js";

const { isPrivateIPv4, isPrivateIPv6 } = _internal;

let passed = 0;
let failed = 0;
const failures: string[] = [];

function test(name: string, fn: () => void | Promise<void>): Promise<void> {
  return Promise.resolve(fn()).then(
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

async function expectThrow(
  fn: () => Promise<unknown>,
  contains: string
): Promise<void> {
  let threw = false;
  try {
    await fn();
  } catch (e) {
    threw = true;
    const msg = (e as Error).message;
    if (!msg.includes(contains)) {
      throw new Error(
        `expected throw containing "${contains}", got: ${msg}`
      );
    }
  }
  if (!threw) {
    throw new Error(
      `expected throw containing "${contains}", but no throw occurred`
    );
  }
}

// ──────────────────────────────────────────────
// Test seams: mock DNS + mock fetch
// ──────────────────────────────────────────────

function mockDns(
  map: Record<string, string[]>
): (hostname: string) => Promise<Array<{ address: string; family: number }>> {
  return async (hostname: string) => {
    const addresses = map[hostname];
    if (!addresses) {
      throw new Error(`ENOTFOUND ${hostname}`);
    }
    return addresses.map((address) => ({
      address,
      family: address.includes(":") ? 6 : 4,
    }));
  };
}

function mockFetch(
  responses: Record<string, { status?: number; body?: string | Uint8Array; delayMs?: number }>
): typeof fetch {
  return (async (
    input: RequestInfo | URL,
    init?: RequestInit
  ): Promise<Response> => {
    const url = typeof input === "string" ? input : input.toString();
    const entry = responses[url];
    if (!entry) {
      throw new Error(`mockFetch: no response configured for ${url}`);
    }

    // Respect AbortController if present
    if (init?.signal?.aborted) {
      const e = new Error("aborted");
      e.name = "AbortError";
      throw e;
    }

    if (entry.delayMs) {
      await new Promise((resolve, reject) => {
        const t = setTimeout(resolve, entry.delayMs);
        init?.signal?.addEventListener("abort", () => {
          clearTimeout(t);
          const e = new Error("aborted");
          e.name = "AbortError";
          reject(e);
        });
      });
    }

    const body = entry.body ?? "";
    // Wrap the body in a Blob so TS's Response(BodyInit) overload
    // resolves cleanly under strict mode for both string and bytes.
    const blob =
      typeof body === "string"
        ? new Blob([body])
        : new Blob([body as BlobPart]);

    return new Response(blob, {
      status: entry.status ?? 200,
      statusText: "OK",
    });
  }) as typeof fetch;
}

// ──────────────────────────────────────────────
// Fixtures
// ──────────────────────────────────────────────

const publicDns = mockDns({
  "example.com": ["93.184.216.34"], // real example.com IP, safely public
  "api.drand.sh": ["151.101.1.229"], // drand's public CDN
});

const privateDns = mockDns({
  "attacker.evil": ["127.0.0.1"],
  "metadata.evil": ["169.254.169.254"],
  "rfc1918.evil": ["10.0.0.5"],
  "rfc1918b.evil": ["172.16.0.1"],
  "rfc1918c.evil": ["192.168.1.1"],
  "cgnat.evil": ["100.64.0.1"],
  "ipv6loop.evil": ["::1"],
  "ipv6ula.evil": ["fd00::1"],
  "ipv6mapped.evil": ["::ffff:127.0.0.1"],
  "mixed.evil": ["93.184.216.34", "10.0.0.1"], // one public, one private → reject
});

console.log("\n━━━ ssrf-guard smoke (Ffern H-3 regression) ━━━\n");

async function run() {
  // ──────────────────────────────────────────
  // Pure IP-classification tests (no fetch)
  // ──────────────────────────────────────────

  await test("isPrivateIPv4: loopback 127.0.0.1", () => {
    if (!isPrivateIPv4("127.0.0.1")) throw new Error("127.0.0.1 must be private");
  });
  await test("isPrivateIPv4: RFC 1918 10.0.0.5", () => {
    if (!isPrivateIPv4("10.0.0.5")) throw new Error("10.0.0.5 must be private");
  });
  await test("isPrivateIPv4: RFC 1918 172.16.0.1", () => {
    if (!isPrivateIPv4("172.16.0.1")) throw new Error("must be private");
  });
  await test("isPrivateIPv4: RFC 1918 192.168.1.1", () => {
    if (!isPrivateIPv4("192.168.1.1")) throw new Error("must be private");
  });
  await test("isPrivateIPv4: cloud metadata 169.254.169.254", () => {
    if (!isPrivateIPv4("169.254.169.254"))
      throw new Error("metadata IP must be private");
  });
  await test("isPrivateIPv4: CGNAT 100.64.0.1", () => {
    if (!isPrivateIPv4("100.64.0.1")) throw new Error("CGNAT must be private");
  });
  await test("isPrivateIPv4: 172.15.0.1 is NOT private (outside /12)", () => {
    if (isPrivateIPv4("172.15.0.1"))
      throw new Error("172.15.x.x is outside RFC 1918");
  });
  await test("isPrivateIPv4: 172.32.0.1 is NOT private (outside /12)", () => {
    if (isPrivateIPv4("172.32.0.1"))
      throw new Error("172.32.x.x is outside RFC 1918");
  });
  await test("isPrivateIPv4: public 8.8.8.8 is NOT private", () => {
    if (isPrivateIPv4("8.8.8.8")) throw new Error("8.8.8.8 must be public");
  });
  await test("isPrivateIPv4: public 93.184.216.34 (example.com) is NOT private", () => {
    if (isPrivateIPv4("93.184.216.34")) throw new Error("must be public");
  });
  await test("isPrivateIPv4: multicast 224.0.0.1 is private", () => {
    if (!isPrivateIPv4("224.0.0.1")) throw new Error("multicast must be rejected");
  });
  await test("isPrivateIPv4: broadcast 255.255.255.255 is private", () => {
    if (!isPrivateIPv4("255.255.255.255"))
      throw new Error("broadcast must be rejected");
  });
  await test("isPrivateIPv4: malformed '1.2.3' is rejected conservatively", () => {
    if (!isPrivateIPv4("1.2.3")) throw new Error("malformed must be rejected");
  });

  await test("isPrivateIPv6: loopback ::1", () => {
    if (!isPrivateIPv6("::1")) throw new Error("::1 must be private");
  });
  await test("isPrivateIPv6: unspecified ::", () => {
    if (!isPrivateIPv6("::")) throw new Error(":: must be private");
  });
  await test("isPrivateIPv6: fc00::/7 ULA fd12:3456::1", () => {
    if (!isPrivateIPv6("fd12:3456::1")) throw new Error("fd12 must be private");
  });
  await test("isPrivateIPv6: fe80::/10 link-local", () => {
    if (!isPrivateIPv6("fe80::1")) throw new Error("link-local must be private");
  });
  await test("isPrivateIPv6: ff00::/8 multicast ff02::1", () => {
    if (!isPrivateIPv6("ff02::1")) throw new Error("multicast must be private");
  });
  await test("isPrivateIPv6: IPv4-mapped ::ffff:127.0.0.1", () => {
    if (!isPrivateIPv6("::ffff:127.0.0.1"))
      throw new Error("IPv4-mapped loopback must be private");
  });
  await test("isPrivateIPv6: IPv4-mapped ::ffff:169.254.169.254", () => {
    if (!isPrivateIPv6("::ffff:169.254.169.254"))
      throw new Error("IPv4-mapped metadata must be private");
  });
  await test("isPrivateIPv6: public 2606:4700:4700::1111 (Cloudflare) is NOT private", () => {
    if (isPrivateIPv6("2606:4700:4700::1111"))
      throw new Error("Cloudflare DNS must be public");
  });
  await test("isPrivateIp: garbage string rejected conservatively", () => {
    if (!isPrivateIp("not an ip")) throw new Error("garbage must be rejected");
  });

  // ──────────────────────────────────────────
  // safeFetch: input/scheme/port defenses
  // ──────────────────────────────────────────

  await test("safeFetch: rejects empty URL", () =>
    expectThrow(() => safeFetch(""), "empty"));

  await test("safeFetch: rejects unparseable URL", () =>
    expectThrow(() => safeFetch("not a url"), "unparseable"));

  await test("safeFetch: rejects file:// scheme", () =>
    expectThrow(
      () =>
        safeFetch("file:///etc/passwd", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({}),
        }),
      "scheme"
    ));

  await test("safeFetch: rejects ftp:// scheme", () =>
    expectThrow(
      () =>
        safeFetch("ftp://example.com/", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({}),
        }),
      "scheme"
    ));

  await test("safeFetch: rejects gopher:// scheme", () =>
    expectThrow(
      () =>
        safeFetch("gopher://example.com/", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({}),
        }),
      "scheme"
    ));

  await test("safeFetch: rejects port 22 (SSH)", () =>
    expectThrow(
      () =>
        safeFetch("http://example.com:22/", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({}),
        }),
      "port 22"
    ));

  await test("safeFetch: rejects port 6379 (Redis)", () =>
    expectThrow(
      () =>
        safeFetch("http://example.com:6379/", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({}),
        }),
      "port 6379"
    ));

  await test("safeFetch: rejects port 26657 (Cosmos RPC)", () =>
    expectThrow(
      () =>
        safeFetch("http://example.com:26657/status", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({}),
        }),
      "port 26657"
    ));

  // ──────────────────────────────────────────
  // safeFetch: private-IP defenses (full SSRF payloads)
  // ──────────────────────────────────────────

  await test("safeFetch: blocks loopback (attacker.evil → 127.0.0.1)", () =>
    expectThrow(
      () =>
        safeFetch("http://attacker.evil/", {
          dnsLookup: privateDns,
          fetchImpl: mockFetch({}),
        }),
      "private"
    ));

  await test("safeFetch: blocks AWS/GCP/Azure metadata (metadata.evil → 169.254.169.254)", () =>
    expectThrow(
      () =>
        safeFetch("http://metadata.evil/latest/meta-data/", {
          dnsLookup: privateDns,
          fetchImpl: mockFetch({}),
        }),
      "169.254.169.254"
    ));

  await test("safeFetch: blocks RFC 1918 10/8 (rfc1918.evil → 10.0.0.5)", () =>
    expectThrow(
      () =>
        safeFetch("http://rfc1918.evil/", {
          dnsLookup: privateDns,
          fetchImpl: mockFetch({}),
        }),
      "10.0.0.5"
    ));

  await test("safeFetch: blocks RFC 1918 172.16/12 (rfc1918b.evil → 172.16.0.1)", () =>
    expectThrow(
      () =>
        safeFetch("http://rfc1918b.evil/", {
          dnsLookup: privateDns,
          fetchImpl: mockFetch({}),
        }),
      "172.16.0.1"
    ));

  await test("safeFetch: blocks RFC 1918 192.168/16 (rfc1918c.evil → 192.168.1.1)", () =>
    expectThrow(
      () =>
        safeFetch("http://rfc1918c.evil/", {
          dnsLookup: privateDns,
          fetchImpl: mockFetch({}),
        }),
      "192.168.1.1"
    ));

  await test("safeFetch: blocks CGNAT (cgnat.evil → 100.64.0.1)", () =>
    expectThrow(
      () =>
        safeFetch("http://cgnat.evil/", {
          dnsLookup: privateDns,
          fetchImpl: mockFetch({}),
        }),
      "100.64.0.1"
    ));

  await test("safeFetch: blocks literal IP URL http://127.0.0.1/", () =>
    expectThrow(
      () =>
        safeFetch("http://127.0.0.1/", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({}),
        }),
      "127.0.0.1"
    ));

  await test("safeFetch: blocks literal IP URL http://169.254.169.254/", () =>
    expectThrow(
      () =>
        safeFetch("http://169.254.169.254/latest/meta-data/", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({}),
        }),
      "169.254.169.254"
    ));

  await test("safeFetch: blocks IPv6 loopback http://[::1]/", () =>
    expectThrow(
      () =>
        safeFetch("http://[::1]/", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({}),
        }),
      "private"
    ));

  await test("safeFetch: blocks IPv6 ULA http://[fd00::1]/", () =>
    expectThrow(
      () =>
        safeFetch("http://[fd00::1]/", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({}),
        }),
      "private"
    ));

  await test("safeFetch: blocks IPv4-mapped IPv6 (ipv6mapped.evil → ::ffff:127.0.0.1)", () =>
    expectThrow(
      () =>
        safeFetch("http://ipv6mapped.evil/", {
          dnsLookup: privateDns,
          fetchImpl: mockFetch({}),
        }),
      "private"
    ));

  await test("safeFetch: blocks when ANY resolved IP is private (mixed pub+priv)", () =>
    expectThrow(
      () =>
        safeFetch("http://mixed.evil/", {
          dnsLookup: privateDns,
          fetchImpl: mockFetch({}),
        }),
      "10.0.0.1"
    ));

  // ──────────────────────────────────────────
  // safeFetch: happy path with mocked fetch
  // ──────────────────────────────────────────

  await test("safeFetch: happy path returns response text", async () => {
    const result = await safeFetch("https://example.com/data.json", {
      dnsLookup: publicDns,
      fetchImpl: mockFetch({
        "https://example.com/data.json": { body: '{"ok":true}' },
      }),
    });
    if (result.status !== 200) {
      throw new Error(`expected status 200, got ${result.status}`);
    }
    if (result.text !== '{"ok":true}') {
      throw new Error(`expected body '{"ok":true}', got ${JSON.stringify(result.text)}`);
    }
    if (result.bytesRead !== 11) {
      throw new Error(`expected bytesRead 11, got ${result.bytesRead}`);
    }
  });

  // ──────────────────────────────────────────
  // safeFetch: body size cap
  // ──────────────────────────────────────────

  await test("safeFetch: aborts when body exceeds maxBytes", () =>
    expectThrow(
      () =>
        safeFetch("https://example.com/huge.bin", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({
            "https://example.com/huge.bin": {
              body: new Uint8Array(200), // 200 bytes
            },
          }),
          maxBytes: 64,
        }),
      "exceeded"
    ));

  // ──────────────────────────────────────────
  // safeFetch: timeout
  // ──────────────────────────────────────────

  await test("safeFetch: aborts when fetch exceeds timeoutMs", () =>
    expectThrow(
      () =>
        safeFetch("https://example.com/slow", {
          dnsLookup: publicDns,
          fetchImpl: mockFetch({
            "https://example.com/slow": {
              body: "too late",
              delayMs: 5_000,
            },
          }),
          timeoutMs: 50,
        }),
      "timed out"
    ));

  // ──────────────────────────────────────────
  // Summary
  // ──────────────────────────────────────────

  console.log(`\n${passed} passed, ${failed} failed\n`);

  if (failed > 0) {
    console.log("Failed tests:");
    for (const name of failures) console.log(`  - ${name}`);
    process.exit(1);
  }

  process.exit(0);
}

run().catch((e) => {
  console.error("test runner crashed:", e);
  process.exit(1);
});
