/**
 * Admin RPC primitive — v0.x.y-security-3 Phase 3d (WAVS-bridge side).
 *
 * Mirror of the MCP-side admin RPC shipped in Phase 3b/3c (see
 * mcp/src/admin/rpc-server.ts). Same threat model, same defense
 * layers, same wire format. The only material differences:
 *
 *   - Controls `egress_paused` (the kill-switch defined in
 *     wavs/bridge/src/utils/ssrf-guard.ts) instead of `signing_paused`.
 *   - Exposes /egress/* endpoints in place of /signing/*.
 *   - Default processName is "wavs-bridge".
 *
 * This is an intentional, minimal copy rather than a shared module.
 * The MCP server and the WAVS bridge are separate npm packages
 * intended to evolve independently; cross-package imports are
 * fragile and out of scope for v0.x.y-security-3. The duplicated
 * surface is small (~370 LoC) and the test surface is owned by
 * the consumer side.
 *
 * Threat model and the design choices that follow from it:
 *
 *   - Threat: a malicious local process running as the same user
 *     Mitigation: bearer token in Authorization header, constant-time
 *     compared. Token MUST be ≥32 bytes (enforced at construction).
 *
 *   - Threat: another user on a multi-tenant box
 *     Mitigation: bind to 127.0.0.1 only. Constructor rejects
 *     0.0.0.0 / ::1 / ::.
 *
 *   - Threat: DNS-rebinding via a malicious browser page
 *     Mitigation: Host header check (must be 127.0.0.1:<port> or
 *     localhost:<port>) AND Origin header rejection.
 *
 *   - Threat: token brute-force
 *     Mitigation: in-memory rate limit (default 10 req / 60 s),
 *     fires BEFORE auth check.
 *
 *   - Threat: secrets in audit log
 *     Mitigation: token never appears in any audit-entry field.
 *
 * Endpoints (token required on all):
 *   GET  /health             -> { ok: true, version, tag }
 *   GET  /policy             -> { process, version, kill_switches, reported_at }
 *   GET  /egress/status      -> { paused: boolean, source: string|null }
 *   POST /egress/pause       body: { source: string } -> { paused: true,  source }
 *   POST /egress/unpause     body: { source: string } -> { paused: false, source }
 */

import { createServer, IncomingMessage, ServerResponse, Server } from "http";
import { timingSafeEqual } from "crypto";
import { AddressInfo } from "net";

import {
  getEgressPaused as ssrfGetPaused,
  getEgressPausedSource as ssrfGetSource,
  setEgressPaused as ssrfSet,
} from "../utils/ssrf-guard.js";

const VERSION = "0.x.y-security-3";
const ADMIN_RPC_VERSION_TAG = "v0.x.y-security-3-phase-3d";
const MIN_TOKEN_BYTES = 32;
const MAX_BODY_BYTES = 64 * 1024; // 64 KiB cap on request body
const DEFAULT_RATE_LIMIT_MAX = 10;
const DEFAULT_RATE_LIMIT_WINDOW_MS = 60_000;

// ──────────────────────────────────────────────
// Public types
// ──────────────────────────────────────────────

/**
 * Minimal interface the WAVS-bridge admin RPC needs to control the
 * egress_paused state. The default implementation
 * (`defaultEgressController`) wraps the module-level setters and
 * getters from ssrf-guard.ts. Tests pass a fake.
 */
export interface EgressPausedController {
  setEgressPaused(paused: boolean, source: string): void;
  getEgressPaused(): { paused: boolean; source: string | null };
}

/**
 * Controller wired against the real module-level egress_paused
 * state in ssrf-guard.ts. This is what the bridge entry point
 * passes when it starts the listener.
 */
export const defaultEgressController: EgressPausedController = {
  setEgressPaused(paused: boolean, source: string): void {
    ssrfSet(paused, source);
  },
  getEgressPaused(): { paused: boolean; source: string | null } {
    return {
      paused: ssrfGetPaused(),
      source: ssrfGetSource() ?? null,
    };
  },
};

export interface AuditEntry {
  ts: string;
  method: string;
  path: string;
  status: number;
  outcome:
    | "ok"
    | "auth_fail"
    | "rate_limit"
    | "host_check_fail"
    | "bad_request"
    | "not_found"
    | "method_not_allowed"
    | "server_error";
  source?: string;
  message?: string;
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
   * GET /policy response. Default "wavs-bridge".
   */
  processName?: string;
  /** Max requests per window. Default 10. */
  rateLimitMax?: number;
  /** Window duration in ms. Default 60000. */
  rateLimitWindowMs?: number;
  /** Audit log sink. Default: console.error JSON-line. */
  auditLog?: (entry: AuditEntry) => void;
  /** Controller for egress_paused. Required. */
  controller: EgressPausedController;
}

export interface AdminRpcHandle {
  readonly url: string;
  readonly port: number;
  readonly host: string;
  close(): Promise<void>;
}

// ──────────────────────────────────────────────
// Internals
// ──────────────────────────────────────────────

function nowIso(): string {
  return new Date().toISOString();
}

function defaultAudit(entry: AuditEntry): void {
  console.error(`[wavs-admin-rpc] ${JSON.stringify(entry)}`);
}

function safeEqual(a: string, b: string): boolean {
  const aBuf = Buffer.from(a, "utf-8");
  const bBuf = Buffer.from(b, "utf-8");
  const len = Math.max(aBuf.length, bBuf.length, MIN_TOKEN_BYTES);
  const aPad = Buffer.alloc(len);
  const bPad = Buffer.alloc(len);
  aBuf.copy(aPad);
  bBuf.copy(bPad);
  const ok = timingSafeEqual(aPad, bPad);
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
    typeof opts.controller.setEgressPaused !== "function" ||
    typeof opts.controller.getEgressPaused !== "function"
  ) {
    throw new Error("admin-rpc: controller must implement EgressPausedController");
  }

  const host = opts.host ?? "127.0.0.1";
  const port = opts.port ?? 0;
  const processName = opts.processName ?? "wavs-bridge";
  const audit = opts.auditLog ?? defaultAudit;
  const rateLimit = new RateLimiter(
    opts.rateLimitMax ?? DEFAULT_RATE_LIMIT_MAX,
    opts.rateLimitWindowMs ?? DEFAULT_RATE_LIMIT_WINDOW_MS
  );

  if (host !== "127.0.0.1" && host !== "localhost") {
    throw new Error(
      `admin-rpc: host must be "127.0.0.1" or "localhost"; got "${host}"`
    );
  }
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
      // 1. Host header check.
      const hostHeader = req.headers.host ?? "";
      const expectedHostA = `${bindHost}:${(server.address() as AddressInfo).port}`;
      const expectedHostB = `localhost:${(server.address() as AddressInfo).port}`;
      if (hostHeader !== expectedHostA && hostHeader !== expectedHostB) {
        log(400, "host_check_fail", `unexpected Host: ${hostHeader}`);
        errorResponse(res, 400, "ERR_HOST_CHECK", "host header rejected");
        return;
      }

      // 2. Origin header rejection.
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

      // 3. Rate limit (before auth).
      if (!rateLimit.consume()) {
        const retryMs = rateLimit.retryAfterMs();
        res.setHeader("Retry-After", Math.ceil(retryMs / 1000).toString());
        log(429, "rate_limit", `retry in ${retryMs}ms`);
        errorResponse(res, 429, "ERR_RATE_LIMIT", "too many requests");
        return;
      }

      // 4. Bearer token check.
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

      if (method === "GET" && path === "/egress/status") {
        const state = opts.controller.getEgressPaused();
        log(200, "ok", `paused=${state.paused}`);
        jsonResponse(res, 200, state);
        return;
      }

      if (method === "GET" && path === "/policy") {
        const egress = opts.controller.getEgressPaused();
        const policy = {
          process: processName,
          version: VERSION,
          tag: ADMIN_RPC_VERSION_TAG,
          kill_switches: {
            egress_paused: {
              paused: egress.paused,
              source: egress.source,
            },
          },
          reported_at: nowIso(),
        };
        log(200, "ok", `policy reported (egress_paused=${egress.paused})`);
        jsonResponse(res, 200, policy);
        return;
      }

      if (method === "POST" && path === "/egress/pause") {
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
        opts.controller.setEgressPaused(true, source);
        const state = opts.controller.getEgressPaused();
        log(200, "ok", `paused=true source=${source}`, source);
        jsonResponse(res, 200, state);
        return;
      }

      if (method === "POST" && path === "/egress/unpause") {
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
        opts.controller.setEgressPaused(false, source);
        const state = opts.controller.getEgressPaused();
        log(200, "ok", `paused=false source=${source}`, source);
        jsonResponse(res, 200, state);
        return;
      }

      // Unknown route.
      const knownPaths = ["/health", "/policy", "/egress/status", "/egress/pause", "/egress/unpause"];
      if (knownPaths.includes(path)) {
        const isGetPath =
          path === "/health" || path === "/policy" || path === "/egress/status";
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
      errorResponse(res, 500, "ERR_INTERNAL", "internal server error");
    }
  });

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
