/**
 * ssrf-guard — validate outbound HTTP URLs to prevent SSRF attacks.
 *
 * Addresses Ffern Institute audit finding H-3 (April 2026): the
 * `computeDataVerify` function in `local-compute.ts` called
 * `fetch(url)` on agent-provided URLs with zero validation. A
 * compromised agent (or a prompt-injected one) could exfiltrate:
 *   - cloud metadata endpoints (`169.254.169.254`) — AWS IAM creds,
 *     GCP service-account tokens, Azure managed-identity tokens
 *   - RFC 1918 private networks (`10/8`, `172.16/12`, `192.168/16`)
 *   - localhost services (`http://127.0.0.1:6379/flushall`, admin
 *     RPCs on `26657`, Elasticsearch on `9200`, etc.)
 *   - non-HTTP schemes — `file:///etc/passwd`, `gopher://`
 *
 * This guard implements layered defenses:
 *   1. Scheme allowlist (default: `http`, `https`).
 *   2. Port allowlist (default: `80`, `443`).
 *   3. DNS pre-resolution with private-IP block (IPv4 + IPv6,
 *      including IPv4-mapped IPv6 addresses).
 *   4. 5-second request timeout via AbortController.
 *   5. 1 MiB response-body cap via streaming + abort.
 *
 * Every default is overridable via options (for tests) or via
 * `JUNOCLAW_SSRF_*` environment variables (for deployment).
 *
 * Known limitations:
 *   - TOCTOU DNS-rebinding: the OS resolver may return different
 *     addresses between our pre-check and the fetch's own lookup.
 *     Complete TOCTOU-free defense requires pinning the connection
 *     to a validated IP (via `undici.Agent`). For the current
 *     threat model (local operator running user-approved tasks),
 *     DNS rebinding requires attacker-controlled DNS — a high bar.
 *     See `SECURITY.md` for the deployment-hardening guidance.
 *   - IPv6 private-range detection covers the common ranges (`::1`,
 *     `fc00::/7`, `fe80::/10`, `ff00::/8`, IPv4-mapped) but not
 *     every RFC 6890 special-use block. Extend as needed.
 */

import { promises as dnsPromises } from "dns";
import { isIPv4, isIPv6 } from "net";

// ──────────────────────────────────────────────
// Defaults and env-var parsing
// ──────────────────────────────────────────────

const DEFAULT_ALLOWED_SCHEMES: readonly string[] = ["http:", "https:"];
const DEFAULT_ALLOWED_PORTS: ReadonlySet<number> = new Set([80, 443]);
const DEFAULT_TIMEOUT_MS = 5_000;
const DEFAULT_MAX_BYTES = 1 * 1024 * 1024; // 1 MiB

function parseEnvList(raw: string | undefined): string[] | undefined {
  if (!raw) return undefined;
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

function parseEnvPortSet(raw: string | undefined): Set<number> | undefined {
  const list = parseEnvList(raw);
  if (!list) return undefined;
  const ports = new Set<number>();
  for (const s of list) {
    const n = Number(s);
    if (!Number.isInteger(n) || n < 1 || n > 65535) {
      throw new Error(
        `JUNOCLAW_SSRF_ALLOWED_PORTS: "${s}" is not a valid port number`
      );
    }
    ports.add(n);
  }
  return ports;
}

function parseEnvNumber(raw: string | undefined): number | undefined {
  if (!raw) return undefined;
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? n : undefined;
}

// ──────────────────────────────────────────────
// Private-IP check — IPv4
// ──────────────────────────────────────────────

function isPrivateIPv4(ip: string): boolean {
  const parts = ip.split(".").map(Number);
  if (
    parts.length !== 4 ||
    parts.some((p) => !Number.isInteger(p) || p < 0 || p > 255)
  ) {
    return true; // malformed → reject conservatively
  }
  const [a, b, c] = parts;
  // 0.0.0.0/8 — "this network"
  if (a === 0) return true;
  // 10.0.0.0/8 — RFC 1918 private
  if (a === 10) return true;
  // 100.64.0.0/10 — CGNAT
  if (a === 100 && b >= 64 && b <= 127) return true;
  // 127.0.0.0/8 — loopback
  if (a === 127) return true;
  // 169.254.0.0/16 — link-local; includes 169.254.169.254 (cloud metadata)
  if (a === 169 && b === 254) return true;
  // 172.16.0.0/12 — RFC 1918 private
  if (a === 172 && b >= 16 && b <= 31) return true;
  // 192.0.0.0/24 — IETF protocol assignments
  if (a === 192 && b === 0 && c === 0) return true;
  // 192.0.2.0/24 — TEST-NET-1
  if (a === 192 && b === 0 && c === 2) return true;
  // 192.168.0.0/16 — RFC 1918 private
  if (a === 192 && b === 168) return true;
  // 198.18.0.0/15 — benchmark/testing
  if (a === 198 && (b === 18 || b === 19)) return true;
  // 198.51.100.0/24 — TEST-NET-2
  if (a === 198 && b === 51 && c === 100) return true;
  // 203.0.113.0/24 — TEST-NET-3
  if (a === 203 && b === 0 && c === 113) return true;
  // 224.0.0.0/4 — multicast
  if (a >= 224 && a <= 239) return true;
  // 240.0.0.0/4 — reserved (incl. 255.255.255.255 broadcast)
  if (a >= 240) return true;
  return false;
}

// ──────────────────────────────────────────────
// Private-IP check — IPv6
// ──────────────────────────────────────────────

function isPrivateIPv6(ip: string): boolean {
  const lower = ip.toLowerCase();
  // Loopback
  if (lower === "::1") return true;
  // Unspecified
  if (lower === "::") return true;

  // IPv4-mapped IPv6 in dotted form: ::ffff:X.Y.Z.W — unwrap and recheck
  const v4mapDotted = lower.match(/^::ffff:(\d+\.\d+\.\d+\.\d+)$/);
  if (v4mapDotted) return isPrivateIPv4(v4mapDotted[1]);

  // IPv4-mapped IPv6 in hex form: ::ffff:aabb:ccdd
  const v4mapHex = lower.match(/^::ffff:([0-9a-f]{1,4}):([0-9a-f]{1,4})$/);
  if (v4mapHex) {
    const high = parseInt(v4mapHex[1], 16);
    const low = parseInt(v4mapHex[2], 16);
    return isPrivateIPv4(
      `${(high >> 8) & 0xff}.${high & 0xff}.${(low >> 8) & 0xff}.${low & 0xff}`
    );
  }

  // fc00::/7 — unique local (0xfc.. or 0xfd.. as first byte of first segment).
  // First segment MUST be 4 hex digits (full form) for the high byte to be fc/fd.
  if (/^f[cd][0-9a-f]{2}:/.test(lower)) return true;

  // fe80::/10 — link-local (first byte 0xfe, next 2 bits = 10 → 0xfe8/9/a/b).
  if (/^fe[89ab][0-9a-f]:/.test(lower)) return true;

  // ff00::/8 — multicast (first byte 0xff).
  if (/^ff[0-9a-f]{2}:/.test(lower)) return true;

  return false;
}

export function isPrivateIp(ip: string): boolean {
  if (isIPv4(ip)) return isPrivateIPv4(ip);
  if (isIPv6(ip)) return isPrivateIPv6(ip);
  // Not a parseable IP — reject conservatively.
  return true;
}

// ──────────────────────────────────────────────
// Public API
// ──────────────────────────────────────────────

export interface SafeFetchOptions {
  /** Allowed URL schemes. Defaults to ["http:", "https:"]. */
  allowedSchemes?: readonly string[];
  /** Allowed destination ports. Defaults to {80, 443}. */
  allowedPorts?: ReadonlySet<number>;
  /** Per-request timeout in milliseconds. Defaults to 5000. */
  timeoutMs?: number;
  /** Max response body size in bytes. Defaults to 1 MiB. */
  maxBytes?: number;
  /**
   * Test seam — override the DNS lookup. Production code should
   * leave this undefined to use `dns.promises.lookup(..., { all: true })`.
   */
  dnsLookup?: (
    hostname: string
  ) => Promise<Array<{ address: string; family: number }>>;
  /** Test seam — override the fetch implementation. */
  fetchImpl?: typeof fetch;
}

export interface SafeFetchResult {
  status: number;
  statusText: string;
  headers: Headers;
  text: string;
  bytesRead: number;
  url: string;
}

/**
 * Fetch a URL with SSRF, timeout, and size-cap defenses.
 *
 * Throws an `Error` whose message names the rejection cause when any
 * defense fires (scheme / port / private-IP / timeout / cap). On
 * success, returns the response's text body (up to `maxBytes`) plus
 * metadata.
 */
export async function safeFetch(
  rawUrl: string,
  options: SafeFetchOptions = {}
): Promise<SafeFetchResult> {
  const allowedSchemes =
    options.allowedSchemes ??
    parseEnvList(process.env.JUNOCLAW_SSRF_ALLOWED_SCHEMES)?.map((s) =>
      s.endsWith(":") ? s : `${s}:`
    ) ??
    DEFAULT_ALLOWED_SCHEMES;
  const allowedPorts =
    options.allowedPorts ??
    parseEnvPortSet(process.env.JUNOCLAW_SSRF_ALLOWED_PORTS) ??
    DEFAULT_ALLOWED_PORTS;
  const timeoutMs =
    options.timeoutMs ??
    parseEnvNumber(process.env.JUNOCLAW_SSRF_TIMEOUT_MS) ??
    DEFAULT_TIMEOUT_MS;
  const maxBytes =
    options.maxBytes ??
    parseEnvNumber(process.env.JUNOCLAW_SSRF_MAX_BYTES) ??
    DEFAULT_MAX_BYTES;
  const dnsLookup =
    options.dnsLookup ??
    ((hostname: string) => dnsPromises.lookup(hostname, { all: true }));
  const fetchImpl = options.fetchImpl ?? fetch;

  // 1. Input sanity.
  if (typeof rawUrl !== "string" || rawUrl.length === 0) {
    throw new Error("ssrf-guard: empty or non-string URL");
  }

  // 2. Parse URL.
  let parsed: URL;
  try {
    parsed = new URL(rawUrl);
  } catch {
    throw new Error(`ssrf-guard: unparseable URL: ${rawUrl}`);
  }

  // 3. Scheme allowlist.
  if (!allowedSchemes.includes(parsed.protocol)) {
    throw new Error(
      `ssrf-guard: scheme "${parsed.protocol}" is not in the allowlist ` +
        `[${allowedSchemes.join(", ")}] for URL ${rawUrl}`
    );
  }

  // 4. Port allowlist. Implicit port = per-scheme default.
  const port = parsed.port
    ? Number(parsed.port)
    : parsed.protocol === "https:"
      ? 443
      : 80;
  if (!allowedPorts.has(port)) {
    throw new Error(
      `ssrf-guard: port ${port} is not in the allowlist ` +
        `[${[...allowedPorts].join(", ")}] for URL ${rawUrl}`
    );
  }

  // 5. Hostname → DNS → private-IP block.
  const rawHostname = parsed.hostname;
  if (!rawHostname) {
    throw new Error(`ssrf-guard: URL has no hostname: ${rawUrl}`);
  }

  // Node's URL preserves brackets around IPv6 hostnames (e.g.
  // `new URL("http://[::1]/").hostname === "[::1]"`). Strip them
  // before passing to isIPv6 / DNS lookup.
  const hostname =
    rawHostname.startsWith("[") && rawHostname.endsWith("]")
      ? rawHostname.slice(1, -1)
      : rawHostname;

  let addresses: string[];
  if (isIPv4(hostname) || isIPv6(hostname)) {
    // URL hostname is a literal IP — no DNS needed; check directly.
    addresses = [hostname];
  } else {
    let results: Array<{ address: string; family: number }>;
    try {
      results = await dnsLookup(hostname);
    } catch (e) {
      throw new Error(
        `ssrf-guard: DNS lookup failed for ${hostname}: ${(e as Error).message}`
      );
    }
    if (results.length === 0) {
      throw new Error(
        `ssrf-guard: DNS returned no addresses for ${hostname}`
      );
    }
    addresses = results.map((r) => r.address);
  }

  for (const addr of addresses) {
    if (isPrivateIp(addr)) {
      throw new Error(
        `ssrf-guard: ${hostname} resolves to private/restricted IP ${addr}`
      );
    }
  }

  // 6. Fetch with timeout.
  const controller = new AbortController();
  const timeoutHandle = setTimeout(() => controller.abort(), timeoutMs);

  let resp: Response;
  try {
    resp = await fetchImpl(rawUrl, { signal: controller.signal });
  } catch (e) {
    clearTimeout(timeoutHandle);
    const err = e as Error;
    if (err.name === "AbortError") {
      throw new Error(
        `ssrf-guard: fetch timed out after ${timeoutMs}ms for ${rawUrl}`
      );
    }
    throw new Error(`ssrf-guard: fetch failed for ${rawUrl}: ${err.message}`);
  }

  // 7. Stream body with size cap.
  let bytesRead = 0;
  let truncated = false;
  const chunks: Uint8Array[] = [];

  if (resp.body) {
    const reader = resp.body.getReader();
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        if (value) {
          bytesRead += value.byteLength;
          if (bytesRead > maxBytes) {
            truncated = true;
            controller.abort();
            try {
              await reader.cancel();
            } catch {
              /* already cancelled */
            }
            break;
          }
          chunks.push(value);
        }
      }
    } catch (e) {
      const err = e as Error;
      if (err.name !== "AbortError") {
        clearTimeout(timeoutHandle);
        throw new Error(
          `ssrf-guard: body read failed for ${rawUrl}: ${err.message}`
        );
      }
    }
  }

  clearTimeout(timeoutHandle);

  if (truncated) {
    throw new Error(
      `ssrf-guard: response body exceeded the ${maxBytes}-byte cap ` +
        `(${(maxBytes / 1024 / 1024).toFixed(1)} MiB) for ${rawUrl}`
    );
  }

  const text = Buffer.concat(chunks.map((c) => Buffer.from(c))).toString(
    "utf-8"
  );

  return {
    status: resp.status,
    statusText: resp.statusText,
    headers: resp.headers,
    text,
    bytesRead,
    url: rawUrl,
  };
}

// ──────────────────────────────────────────────
// Test-only internals
// ──────────────────────────────────────────────

/**
 * Internal helpers exposed for unit testing only. Do not use from
 * production code.
 */
export const _internal = {
  isPrivateIPv4,
  isPrivateIPv6,
  isPrivateIp,
};
