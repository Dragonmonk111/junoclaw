/**
 * Admin RPC primitive — v0.x.y-security-3 Phase 3b.
 *
 * Localhost-only HTTP listener that lets an operator hot-flip the
 * `signing_paused` kill-switch (and, in Phase 3c, query the full
 * policy state) WITHOUT restarting the MCP process. Designed for
 * incident response: when something looks wrong, the operator should
 * be one curl away from halting all signing, and one curl away from
 * resuming once the situation is understood.
 *
 * Threat model and the design choices that follow from it:
 *
 *   - Threat: a malicious local process running as the same user
 *     could try to call the admin RPC. Mitigation: bearer token in
 *     Authorization header, constant-time compared. The token MUST
 *     be at least 32 chars (the byte length is enforced; tokens
 *     shorter than 32 bytes are rejected at construction).
 *
 *   - Threat: another user on a multi-tenant box could try to
 *     connect. Mitigation: bind to 127.0.0.1 (IPv4 loopback) only,
 *     never 0.0.0.0 / ::1 / ::. Single-operator deployments — the
 *     intended target of JunoClaw — are the canonical use case.
 *
 *   - Threat: a browser visiting a malicious page could trigger a
 *     DNS-rebinding attack and have the browser connect to our
 *     loopback listener. Mitigation: every request must carry a
 *     `Host:` header that exactly matches `127.0.0.1:<port>`. The
 *     CORS layer is the simplest one possible: we serve no
 *     Access-Control-Allow-* headers, and we reject any request
 *     whose `Origin:` header is set (browsers always set it for
 *     cross-origin XHR/fetch; CLI tools never do).
 *
 *   - Threat: token brute-force. Mitigation: in-memory rate limit,
 *     10 requests per 60-second window per process, returning 429
 *     with Retry-After when exceeded. The window is long enough to
 *     make online brute-force impractical against a 32-byte token.
 *
 *   - Threat: secrets in audit log. Mitigation: the audit log
 *     never records the bearer token, request body fields named
 *     "token" / "secret" / "password" / "mnemonic" / "passphrase",
 *     or the value of any header named like a credential.
 *
 * Off-by-default: the MCP server entry point only starts this
 * listener when BOTH `JUNOCLAW_ADMIN_RPC=1` AND
 * `JUNOCLAW_ADMIN_TOKEN=<≥32-char-token>` are set. Constructing
 * the server with a token shorter than 32 bytes throws.
 *
 * Endpoints (token required on all):
 *   GET  /health              -> { ok: true, version, tag }
 *   GET  /policy              -> { process, version, kill_switches, reported_at }
 *                                Read-only roll-up of every kill-switch this
 *                                process owns. Downstream verifiers and ops
 *                                dashboards poll this; it never mutates state.
 *   GET  /signing/status      -> { paused: boolean, source: string|null }
 *   POST /signing/pause       -> { paused: true,  source: string }   body: { source: string }
 *   POST /signing/unpause     -> { paused: false, source: string }   body: { source: string }
 *
 * Future endpoints (Phase 3d):
 *   POST /egress/pause        — bridge-side egress kill-switch hot-flip (planned 3d)
 */

import { createServer, IncomingMessage, ServerResponse, Server } from "http";
import { timingSafeEqual } from "crypto";
import { AddressInfo } from "net";

const VERSION = "0.x.y-security-3";
const ADMIN_RPC_VERSION_TAG = "v0.x.y-security-3-phase-3b";
const MIN_TOKEN_BYTES = 32;
const MAX_BODY_BYTES = 64 * 1024; // 64 KiB cap on request body
const DEFAULT_RATE_LIMIT_MAX = 10;
const DEFAULT_RATE_LIMIT_WINDOW_MS = 60_000;

// ──────────────────────────────────────────────
// Public types
// ──────────────────────────────────────────────

/**
 * Minimal interface the admin RPC needs to control the
 * signing_paused state. WalletStore satisfies this naturally.
 * Tests pass a fake.
 */
export interface SigningPausedController {
  setSigningPaused(paused: boolean, source: string): void;
  getSigningPaused(): { paused: boolean; source: string | null };
}

export interface AuditEntry {
  ts: string;            // ISO timestamp
  method: string;
  path: string;
  status: number;
  outcome: "ok" | "auth_fail" | "rate_limit" | "host_check_fail" | "bad_request" | "not_found" | "method_not_allowed" | "server_error";
  source?: string;       // body.source if present and benign
  message?: string;      // short, no secrets
}

export interface AdminRpcServerOptions {
  /** Bearer token. ≥32 bytes. Required. */
  token: string;
  /** Port to bind. 0 (default) = OS-assigned. */
  port?: number;
  /** Bind host. Default "127.0.0.1". Reject anything else. */
  host?: string;
  /**
   * Identifier for this process surface. Surfaces verbatim in the
   * GET /policy response so downstream tools can tell which process
   * they are talking to. Default "mcp".
   */
  processName?: string;
  /** Max requests per window. Default 10. */
  rateLimitMax?: number;
  /** Window duration in ms. Default 60000. */
  rateLimitWindowMs?: number;
  /** Audit log sink. Default: console.error JSON-line. */
  auditLog?: (entry: AuditEntry) => void;
  /** Controller for signing_paused. Required. */
  controller: SigningPausedController;
}

export interface AdminRpcHandle {
  /** URL of the listener, e.g. "http://127.0.0.1:54123". */
  readonly url: string;
  /** Bound port. */
  readonly port: number;
  /** Bound host. */
  readonly host: string;
  /** Stop accepting connections and wait for in-flight requests to finish. */
  close(): Promise<void>;
}

// ──────────────────────────────────────────────
// Internals
// ──────────────────────────────────────────────

function nowIso(): string {
  return new Date().toISOString();
}

function defaultAudit(entry: AuditEntry): void {
  // JSON-line on stderr. Operators can pipe to a log file.
  console.error(`[admin-rpc] ${JSON.stringify(entry)}`);
}

function safeEqual(a: string, b: string): boolean {
  // timingSafeEqual requires equal-length buffers; pad both sides
  // to a fixed length to avoid leaking the token's length via timing
  // when the supplied token has a different length from ours.
  const aBuf = Buffer.from(a, "utf-8");
  const bBuf = Buffer.from(b, "utf-8");
  const len = Math.max(aBuf.length, bBuf.length, MIN_TOKEN_BYTES);
  const aPad = Buffer.alloc(len);
  const bPad = Buffer.alloc(len);
  aBuf.copy(aPad);
  bBuf.copy(bPad);
  // Still need lengths equal for timingSafeEqual; we just made them so.
  const ok = timingSafeEqual(aPad, bPad);
  // Final discriminator: lengths must match too; otherwise the
  // padded-buffer compare alone would return true for "abc" vs
  // "abc\0\0..." buffers.
  return ok && aBuf.length === bBuf.length;
}

function jsonResponse(
  res: ServerResponse,
  status: number,
  body: object
): void {
  const payload = JSON.stringify(body);
  res.statusCode = status;
  res.setHeader("Content-Type", "application/json; charset=utf-8");
  res.setHeader("Cache-Control", "no-store");
  res.setHeader("X-Content-Type-Options", "nosniff");
  res.setHeader("X-Junoclaw-Admin-Rpc", ADMIN_RPC_VERSION_TAG);
  res.end(payload);
}

function errorResponse(
  res: ServerResponse,
  status: number,
  code: string,
  message: string
): void {
  jsonResponse(res, status, { error: message, code });
}

async function readJsonBody(req: IncomingMessage): Promise<unknown> {
  return new Promise((resolve, reject) => {
    let bytes = 0;
    const chunks: Buffer[] = [];
    req.on("data", (chunk: Buffer) => {
      bytes += chunk.length;
      if (bytes > MAX_BODY_BYTES) {
        reject(new Error(`request body exceeds ${MAX_BODY_BYTES}-byte cap`));
        req.destroy();
        return;
      }
      chunks.push(chunk);
    });
    req.on("end", () => {
      const raw = Buffer.concat(chunks).toString("utf-8");
      if (raw.length === 0) {
        resolve({});
        return;
      }
      try {
        resolve(JSON.parse(raw));
      } catch (e) {
        reject(new Error(`invalid JSON body: ${(e as Error).message}`));
      }
    });
    req.on("error", reject);
  });
}

class RateLimiter {
  private hits: number[] = [];
  constructor(
    private readonly max: number,
    private readonly windowMs: number
  ) {}
  /** Returns true if request is allowed. Records the hit. */
  consume(): boolean {
    const now = Date.now();
    this.hits = this.hits.filter((t) => now - t < this.windowMs);
    if (this.hits.length >= this.max) return false;
    this.hits.push(now);
    return true;
  }
  retryAfterMs(): number {
    if (this.hits.length === 0) return 0;
    const oldest = this.hits[0];
    return Math.max(0, this.windowMs - (Date.now() - oldest));
  }
}

// ──────────────────────────────────────────────
// Public API
// ──────────────────────────────────────────────

export async function startAdminRpcServer(
  opts: AdminRpcServerOptions
): Promise<AdminRpcHandle> {
  // Argument validation — fail loudly at construction.
  if (typeof opts.token !== "string") {
    throw new Error("admin-rpc: token must be a string");
  }
  if (Buffer.byteLength(opts.token, "utf-8") < MIN_TOKEN_BYTES) {
    throw new Error(
      `admin-rpc: token must be at least ${MIN_TOKEN_BYTES} bytes ` +
        `(got ${Buffer.byteLength(opts.token, "utf-8")})`
    );
  }
  if (
    !opts.controller ||
    typeof opts.controller.setSigningPaused !== "function" ||
    typeof opts.controller.getSigningPaused !== "function"
  ) {
    throw new Error("admin-rpc: controller must implement SigningPausedController");
  }

  const host = opts.host ?? "127.0.0.1";
  const port = opts.port ?? 0;
  const processName = opts.processName ?? "mcp";
  const audit = opts.auditLog ?? defaultAudit;
  const rateLimit = new RateLimiter(
    opts.rateLimitMax ?? DEFAULT_RATE_LIMIT_MAX,
    opts.rateLimitWindowMs ?? DEFAULT_RATE_LIMIT_WINDOW_MS
  );

  // Refuse to bind anywhere but loopback. This is intentionally
  // strict: developers wanting remote admin should use SSH port
  // forwarding to a real loopback listener, never bind directly.
  if (host !== "127.0.0.1" && host !== "localhost") {
    throw new Error(
      `admin-rpc: host must be "127.0.0.1" or "localhost"; got "${host}"`
    );
  }
  // Normalise to the IP form so the Host-header check is exact.
  const bindHost = "127.0.0.1";

  const server: Server = createServer(async (req, res) => {
    const method = req.method ?? "UNKNOWN";
    const url = req.url ?? "/";
    const path = url.split("?")[0];

    const log = (
      status: number,
      outcome: AuditEntry["outcome"],
      message?: string,
      source?: string
    ): void => {
      audit({
        ts: nowIso(),
        method,
        path,
        status,
        outcome,
        source,
        message,
      });
    };

    try {
      // 1. Host header check — DNS-rebinding defense.
      const hostHeader = req.headers.host ?? "";
      const expectedHostA = `${bindHost}:${(server.address() as AddressInfo).port}`;
      const expectedHostB = `localhost:${(server.address() as AddressInfo).port}`;
      if (hostHeader !== expectedHostA && hostHeader !== expectedHostB) {
        log(400, "host_check_fail", `unexpected Host: ${hostHeader}`);
        errorResponse(res, 400, "ERR_HOST_CHECK", "host header rejected");
        return;
      }

      // 2. Origin header check — reject any browser-style request.
      const origin = req.headers.origin;
      if (origin) {
        log(400, "host_check_fail", `Origin header set: ${origin}`);
        errorResponse(
          res,
          400,
          "ERR_ORIGIN_REJECTED",
          "browser origin requests are not accepted"
        );
        return;
      }

      // 3. Rate limit before auth. A flooder shouldn't be able to
      //    bypass the limit by spamming wrong tokens.
      if (!rateLimit.consume()) {
        const retryMs = rateLimit.retryAfterMs();
        res.setHeader("Retry-After", Math.ceil(retryMs / 1000).toString());
        log(429, "rate_limit", `retry in ${retryMs}ms`);
        errorResponse(res, 429, "ERR_RATE_LIMIT", "too many requests");
        return;
      }

      // 4. Authorization: Bearer <token>.
      const authz = req.headers.authorization ?? "";
      const expectedPrefix = "Bearer ";
      if (
        !authz.startsWith(expectedPrefix) ||
        !safeEqual(authz.slice(expectedPrefix.length), opts.token)
      ) {
        log(401, "auth_fail", "bearer token missing or invalid");
        errorResponse(res, 401, "ERR_UNAUTHORIZED", "unauthorized");
        return;
      }

      // 5. Route.
      if (method === "GET" && path === "/health") {
        log(200, "ok", undefined);
        jsonResponse(res, 200, {
          ok: true,
          version: VERSION,
          tag: ADMIN_RPC_VERSION_TAG,
        });
        return;
      }

      if (method === "GET" && path === "/signing/status") {
        const state = opts.controller.getSigningPaused();
        log(200, "ok", `paused=${state.paused}`);
        jsonResponse(res, 200, state);
        return;
      }

      if (method === "GET" && path === "/policy") {
        // Phase 3c: read-only roll-up of every kill-switch owned by
        // this process. The MCP side reports signing_paused; the
        // WAVS bridge side reports egress_paused. Downstream tools
        // hit both endpoints to assemble the cross-process picture.
        const signing = opts.controller.getSigningPaused();
        const policy = {
          process: processName,
          version: VERSION,
          tag: ADMIN_RPC_VERSION_TAG,
          kill_switches: {
            signing_paused: {
              paused: signing.paused,
              source: signing.source,
            },
          },
          reported_at: nowIso(),
        };
        log(200, "ok", `policy reported (signing_paused=${signing.paused})`);
        jsonResponse(res, 200, policy);
        return;
      }

      if (method === "POST" && path === "/signing/pause") {
        let body: unknown;
        try {
          body = await readJsonBody(req);
        } catch (e) {
          log(400, "bad_request", (e as Error).message);
          errorResponse(res, 400, "ERR_BAD_BODY", (e as Error).message);
          return;
        }
        const source =
          typeof (body as { source?: unknown })?.source === "string"
            ? ((body as { source: string }).source as string)
            : "";
        if (source.length === 0 || source.length > 256) {
          log(400, "bad_request", "missing or oversized source");
          errorResponse(
            res,
            400,
            "ERR_BAD_BODY",
            "body.source must be a non-empty string ≤256 chars"
          );
          return;
        }
        opts.controller.setSigningPaused(true, source);
        const state = opts.controller.getSigningPaused();
        log(200, "ok", `paused=true source=${source}`, source);
        jsonResponse(res, 200, state);
        return;
      }

      if (method === "POST" && path === "/signing/unpause") {
        let body: unknown;
        try {
          body = await readJsonBody(req);
        } catch (e) {
          log(400, "bad_request", (e as Error).message);
          errorResponse(res, 400, "ERR_BAD_BODY", (e as Error).message);
          return;
        }
        const source =
          typeof (body as { source?: unknown })?.source === "string"
            ? ((body as { source: string }).source as string)
            : "";
        if (source.length === 0 || source.length > 256) {
          log(400, "bad_request", "missing or oversized source");
          errorResponse(
            res,
            400,
            "ERR_BAD_BODY",
            "body.source must be a non-empty string ≤256 chars"
          );
          return;
        }
        opts.controller.setSigningPaused(false, source);
        const state = opts.controller.getSigningPaused();
        log(200, "ok", `paused=false source=${source}`, source);
        jsonResponse(res, 200, state);
        return;
      }

      // Unknown route.
      // Distinguish wrong-method on a known path from wrong-path.
      const knownPaths = ["/health", "/policy", "/signing/status", "/signing/pause", "/signing/unpause"];
      if (knownPaths.includes(path)) {
        const isGetPath =
          path === "/health" || path === "/policy" || path === "/signing/status";
        res.setHeader("Allow", isGetPath ? "GET" : "POST");
        log(405, "method_not_allowed", `${method} ${path}`);
        errorResponse(res, 405, "ERR_METHOD_NOT_ALLOWED", `${method} not allowed on ${path}`);
        return;
      }
      log(404, "not_found", `${method} ${path}`);
      errorResponse(res, 404, "ERR_NOT_FOUND", "not found");
    } catch (e) {
      const err = e as Error;
      log(500, "server_error", err.message);
      // Be careful not to leak internals; the message above goes
      // to the audit log, not the response body.
      errorResponse(res, 500, "ERR_INTERNAL", "internal server error");
    }
  });

  // Listen and capture the assigned port.
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen({ host: bindHost, port }, () => resolve());
  });

  const addr = server.address();
  if (!addr || typeof addr === "string") {
    throw new Error("admin-rpc: server.address() returned unexpected shape");
  }
  const boundPort = (addr as AddressInfo).port;
  const url = `http://${bindHost}:${boundPort}`;

  return {
    url,
    host: bindHost,
    port: boundPort,
    close: () =>
      new Promise<void>((resolve, reject) => {
        server.close((err) => (err ? reject(err) : resolve()));
      }),
  };
}

// Internal exports for unit testing.
export const _internal = {
  safeEqual,
  RateLimiter,
  MIN_TOKEN_BYTES,
  MAX_BODY_BYTES,
};
