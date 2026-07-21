/**
 * WAVS Invoke Server — off-chain component invocation prototype.
 *
 * Implements the POST /invoke/:componentId endpoint from the WAVS
 * off-chain invoke API spec (drafts/PLAN_WAVS_OFF_CHAIN_INVOKE_API.md).
 *
 * This is a bridge-side prototype: it spawns wasmtime to run the sealed
 * signer WASI component directly, rather than going through the WAVS
 * daemon. When the WAVS runtime adds native invoke support, this same
 * HTTP contract moves into the daemon and the wasmtime spawn is removed.
 *
 * Security:
 *   - Bearer token auth (reuses rpc-server.ts pattern)
 *   - Loopback-only binding
 *   - Rate limiting
 *   - Allowlist of permitted component IDs
 *   - Audit logging
 *   - Body size cap
 *
 * Endpoints:
 *   GET  /health              -> { ok, version }
 *   POST /invoke/:componentId  -> component output JSON
 *
 * Env vars:
 *   WAVS_INVOKE_TOKEN     — bearer token (≥32 bytes, required)
 *   WAVS_INVOKE_PORT      — bind port (default 0 = OS-assigned)
 *   WAVS_INVOKE_HOST      — bind host (default 127.0.0.1, must be loopback)
 *   WAVS_INVOKE_WASMTIME  — path to wasmtime binary
 *   WAVS_INVOKE_WASM      — path to sealed signer .wasm
 *   WAVS_INVOKE_PASSPHRASE — sealing passphrase for the sealed blob
 *   WAVS_INVOKE_SEALED_BLOB — hex-encoded sealed blob to use for signing
 *   WAVS_INVOKE_ALLOWED_COMPONENTS — comma-separated allowlist (default: "sealed-signer")
 */

import { createServer, IncomingMessage, ServerResponse, Server } from "http";
import { timingSafeEqual } from "crypto";
import { AddressInfo } from "net";
import { spawn, type ChildProcess } from "child_process";
import { resolve } from "path";

const VERSION = "0.1.0-invoke-prototype";
const MIN_TOKEN_BYTES = 32;
const MAX_BODY_BYTES = 64 * 1024;
const DEFAULT_RATE_LIMIT_MAX = 20;
const DEFAULT_RATE_LIMIT_WINDOW_MS = 60_000;
const DEFAULT_ALLOWED = "sealed-signer";

// ──────────────────────────────────────────────
// Config
// ──────────────────────────────────────────────

interface InvokeConfig {
  token: string;
  port: number;
  host: string;
  wasmtime: string;
  wasmPath: string;
  passphrase: string;
  sealedBlobHex: string;
  allowedComponents: Set<string>;
}

function loadConfig(): InvokeConfig {
  const token = process.env.WAVS_INVOKE_TOKEN ?? "";
  if (token.length < MIN_TOKEN_BYTES) {
    throw new Error(
      `WAVS_INVOKE_TOKEN must be at least ${MIN_TOKEN_BYTES} bytes (got ${token.length})`
    );
  }

  const portRaw = process.env.WAVS_INVOKE_PORT ?? "0";
  const port = parseInt(portRaw, 10);
  if (!Number.isFinite(port) || port < 0 || port > 65535) {
    throw new Error(`WAVS_INVOKE_PORT must be 0-65535, got "${portRaw}"`);
  }

  const host = process.env.WAVS_INVOKE_HOST ?? "127.0.0.1";
  if (host !== "127.0.0.1" && host !== "localhost") {
    throw new Error(`WAVS_INVOKE_HOST must be loopback, got "${host}"`);
  }

  const wasmtime = process.env.WAVS_INVOKE_WASMTIME ?? "wasmtime";
  const wasmPath = process.env.WAVS_INVOKE_WASM ?? resolve(
    import.meta.dirname,
    "..",
    "sealed-signer",
    "target",
    "wasm32-wasip2",
    "release",
    "junoclaw_sealed_signer.wasm"
  );
  const passphrase = process.env.WAVS_INVOKE_PASSPHRASE ?? "";
  const sealedBlobHex = process.env.WAVS_INVOKE_SEALED_BLOB ?? "";

  if (!passphrase) {
    throw new Error("WAVS_INVOKE_PASSPHRASE not set");
  }
  if (!sealedBlobHex) {
    throw new Error("WAVS_INVOKE_SEALED_BLOB not set (hex-encoded sealed blob)");
  }

  const allowedStr = process.env.WAVS_INVOKE_ALLOWED_COMPONENTS ?? DEFAULT_ALLOWED;
  const allowedComponents = new Set(allowedStr.split(",").map((s) => s.trim()).filter(Boolean));

  return { token, port, host, wasmtime, wasmPath, passphrase, sealedBlobHex, allowedComponents };
}

// ──────────────────────────────────────────────
// Utilities (mirrors rpc-server.ts patterns)
// ──────────────────────────────────────────────

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

function jsonResponse(res: ServerResponse, status: number, body: object): void {
  const payload = JSON.stringify(body);
  res.statusCode = status;
  res.setHeader("Content-Type", "application/json; charset=utf-8");
  res.setHeader("Cache-Control", "no-store");
  res.setHeader("X-Content-Type-Options", "nosniff");
  res.setHeader("X-Wavs-Invoke", VERSION);
  res.end(payload);
}

function errorResponse(res: ServerResponse, status: number, code: string, message: string): void {
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

function nowIso(): string {
  return new Date().toISOString();
}

interface AuditEntry {
  ts: string;
  method: string;
  path: string;
  status: number;
  outcome: "ok" | "auth_fail" | "rate_limit" | "host_check_fail" | "bad_request" | "not_found" | "method_not_allowed" | "server_error" | "invoke_error";
  componentId?: string;
  message?: string;
}

function defaultAudit(entry: AuditEntry): void {
  console.error(`[wavs-invoke] ${JSON.stringify(entry)}`);
}

// ──────────────────────────────────────────────
// Wasmtime invocation
// ──────────────────────────────────────────────

interface InvokeResult {
  address: string;
  pubkey: string;
  tx_bytes: string;       // base64
  sign_doc_sha256_hex: string;
}

/**
 * Run the sealed signer component via wasmtime to sign a Cosmos execute tx.
 *
 * This mirrors the approach in run-component-test.js but calls
 * sign-cosmos-execute-tx instead of sign/generate-key.
 *
 * The wasmtime command is:
 *   wasmtime run --env WAVS_ENV_SIGNER_PASSPHRASE=... \
 *     --invoke 'sign-cosmos-execute-tx([sealed_blob_bytes], {req_fields})' \
 *     junoclaw_sealed_signer.wasm
 *
 * Returns parsed output from the component.
 */
function runSealedSignerSign(
  cfg: InvokeConfig,
  req: {
    sender: string;
    contract: string;
    exec_msg_json: string;
    funds_denom: string;
    funds_amount: string;
    gas_limit: number;
    fee_denom: string;
    fee_amount: string;
    memo: string;
    chain_id: string;
    account_number: number;
    sequence: number;
  }
): Promise<InvokeResult> {
  return new Promise((resolveP, rejectP) => {
    const sealedBlobBytes = Buffer.from(cfg.sealedBlobHex, "hex");
    const blobList = Array.from(sealedBlobBytes).join(",");

    // Build WAVE invoke expression.
    // WAVE field names are bare kebab-case identifiers (no quotes needed for hyphens).
    // String values use \" for escaped inner quotes (JSON.stringify handles this).
    const fundsAmount = BigInt(req.funds_amount || "0").toString();
    const feeAmount = BigInt(req.fee_amount || "0").toString();
    const invokeExpr = `sign-cosmos-execute-tx([${blobList}], {sender: "${req.sender}", contract: "${req.contract}", exec-msg-json: ${JSON.stringify(req.exec_msg_json)}, funds-denom: "${req.funds_denom}", funds-amount: ${fundsAmount}, gas-limit: ${req.gas_limit}, fee-denom: "${req.fee_denom}", fee-amount: ${feeAmount}, memo: ${JSON.stringify(req.memo)}, chain-id: "${req.chain_id}", account-number: ${req.account_number}, sequence: ${req.sequence}})`;

    const args = [
      "run",
      "--env", `WAVS_ENV_SIGNER_PASSPHRASE=${cfg.passphrase}`,
      "--invoke", invokeExpr,
      cfg.wasmPath,
    ];

    const proc: ChildProcess = spawn(cfg.wasmtime, args);

    let stdout = "";
    let stderr = "";

    proc.stdout?.on("data", (data: Buffer) => { stdout += data.toString("utf8"); });
    proc.stderr?.on("data", (data: Buffer) => { stderr += data.toString("utf8"); });

    proc.on("error", (err: Error) => {
      rejectP(new Error(`wasmtime spawn failed: ${err.message}`));
    });

    proc.on("close", (code: number) => {
      if (code !== 0) {
        rejectP(new Error(`wasmtime exited ${code}: ${stderr.trim()}`));
        return;
      }
      try {
        const result = parseWasmtimeOutput(stdout.trim());
        resolveP(result);
      } catch (e) {
        rejectP(new Error(`failed to parse wasmtime output: ${(e as Error).message}\nstdout: ${stdout}\nstderr: ${stderr}`));
      }
    });
  });
}

/**
 * Parse wasmtime output for sign-cosmos-execute-tx.
 *
 * Expected format (from WIT):
 *   ok({ address: "juno1...", pubkey: "hex...", tx-bytes: [byte, ...], "sign-doc-sha256-hex": "hex..." })
 */
function parseWasmtimeOutput(out: string): InvokeResult {
  // Match ok({...}) wrapper
  const m = out.match(/^ok\(\{(.+)\}\)$/s);
  if (!m) throw new Error(`unexpected wasmtime output format: ${out.slice(0, 200)}`);
  const inner = m[1];

  // Extract string fields
  const addressMatch = inner.match(/address:\s*"([^"]+)"/);
  const pubkeyMatch = inner.match(/pubkey:\s*"([^"]+)"/);
  const signDocMatch = inner.match(/"sign-doc-sha256-hex":\s*"([^"]+)"/);

  if (!addressMatch || !pubkeyMatch || !signDocMatch) {
    throw new Error(`missing required fields in wasmtime output: ${inner.slice(0, 200)}`);
  }

  // Extract tx-bytes as a list of numbers
  const txBytesMatch = inner.match(/"tx-bytes":\s*\[([^\]]*)\]/);
  if (!txBytesMatch) throw new Error(`missing tx-bytes in wasmtime output`);
  const txBytesList = txBytesMatch[1].trim();
  const txBytes = txBytesList
    ? Buffer.from(txBytesList.split(",").map((s) => parseInt(s.trim(), 10)))
    : Buffer.alloc(0);

  return {
    address: addressMatch[1],
    pubkey: pubkeyMatch[1],
    tx_bytes: txBytes.toString("base64"),
    sign_doc_sha256_hex: signDocMatch[1],
  };
}

// ──────────────────────────────────────────────
// Server
// ──────────────────────────────────────────────

export async function startInvokeServer(opts?: {
  auditLog?: (entry: AuditEntry) => void;
}): Promise<{ url: string; port: number; close: () => Promise<void> }> {
  const cfg = loadConfig();
  const audit = opts?.auditLog ?? defaultAudit;
  const rateLimit = new RateLimiter(DEFAULT_RATE_LIMIT_MAX, DEFAULT_RATE_LIMIT_WINDOW_MS);
  const bindHost = "127.0.0.1";

  const server: Server = createServer(async (req, res) => {
    const method = req.method ?? "UNKNOWN";
    const url = req.url ?? "/";
    const path = url.split("?")[0];

    const log = (
      status: number,
      outcome: AuditEntry["outcome"],
      message?: string,
      componentId?: string
    ): void => {
      audit({ ts: nowIso(), method, path, status, outcome, componentId, message });
    };

    try {
      // 1. Host header check
      const hostHeader = req.headers.host ?? "";
      const port = (server.address() as AddressInfo).port;
      const expectedHostA = `${bindHost}:${port}`;
      const expectedHostB = `localhost:${port}`;
      if (hostHeader !== expectedHostA && hostHeader !== expectedHostB) {
        log(400, "host_check_fail", `unexpected Host: ${hostHeader}`);
        errorResponse(res, 400, "ERR_HOST_CHECK", "host header rejected");
        return;
      }

      // 2. Origin header rejection
      const origin = req.headers.origin;
      if (origin) {
        log(400, "host_check_fail", `Origin header set: ${origin}`);
        errorResponse(res, 400, "ERR_ORIGIN_REJECTED", "browser origin requests are not accepted");
        return;
      }

      // 3. Rate limit
      if (!rateLimit.consume()) {
        const retryMs = rateLimit.retryAfterMs();
        res.setHeader("Retry-After", Math.ceil(retryMs / 1000).toString());
        log(429, "rate_limit", `retry in ${retryMs}ms`);
        errorResponse(res, 429, "ERR_RATE_LIMIT", "too many requests");
        return;
      }

      // 4. Bearer token check
      const authz = req.headers.authorization ?? "";
      const expectedPrefix = "Bearer ";
      if (!authz.startsWith(expectedPrefix) || !safeEqual(authz.slice(expectedPrefix.length), cfg.token)) {
        log(401, "auth_fail", "bearer token missing or invalid");
        errorResponse(res, 401, "ERR_UNAUTHORIZED", "unauthorized");
        return;
      }

      // 5. Routes

      // GET /health
      if (method === "GET" && path === "/health") {
        log(200, "ok");
        jsonResponse(res, 200, { ok: true, version: VERSION });
        return;
      }

      // POST /invoke/:componentId
      if (method === "POST" && path.startsWith("/invoke/")) {
        const componentId = decodeURIComponent(path.slice("/invoke/".length));

        // Allowlist check
        if (!cfg.allowedComponents.has(componentId)) {
          log(403, "not_found", `component not in allowlist: ${componentId}`, componentId);
          errorResponse(res, 403, "ERR_NOT_ALLOWED", `component "${componentId}" is not in the invoke allowlist`);
          return;
        }

        // Parse body
        let body: unknown;
        try {
          body = await readJsonBody(req);
        } catch (e) {
          log(400, "bad_request", (e as Error).message, componentId);
          errorResponse(res, 400, "ERR_BAD_BODY", (e as Error).message);
          return;
        }

        const b = body as {
          trigger?: string;
          input?: Record<string, unknown>;
        };

        // Only sign_request trigger is supported in this prototype
        if (b.trigger !== "sign_request") {
          log(400, "bad_request", `unsupported trigger: ${b.trigger}`, componentId);
          errorResponse(res, 400, "ERR_BAD_TRIGGER", `trigger must be "sign_request" (got "${b.trigger}")`);
          return;
        }

        const input = b.input;
        if (!input || typeof input !== "object") {
          log(400, "bad_request", "missing input object", componentId);
          errorResponse(res, 400, "ERR_BAD_BODY", "request body must include \"input\" object");
          return;
        }

        // Validate required fields
        const required = ["sender", "contract", "exec_msg_json", "funds_denom", "funds_amount", "gas_limit", "fee_denom", "fee_amount", "memo", "chain_id", "account_number", "sequence"];
        for (const field of required) {
          if (!(field in input)) {
            log(400, "bad_request", `missing field: ${field}`, componentId);
            errorResponse(res, 400, "ERR_BAD_BODY", `input missing required field: ${field}`);
            return;
          }
        }

        // Invoke the sealed signer component via wasmtime
        try {
          const result = await runSealedSignerSign(cfg, {
            sender: String(input.sender),
            contract: String(input.contract),
            exec_msg_json: String(input.exec_msg_json),
            funds_denom: String(input.funds_denom),
            funds_amount: String(input.funds_amount),
            gas_limit: Number(input.gas_limit),
            fee_denom: String(input.fee_denom),
            fee_amount: String(input.fee_amount),
            memo: String(input.memo),
            chain_id: String(input.chain_id),
            account_number: Number(input.account_number),
            sequence: Number(input.sequence),
          });

          log(200, "ok", `signed tx for ${result.address}`, componentId);

          jsonResponse(res, 200, {
            component: componentId,
            output: {
              address: result.address,
              pubkey: result.pubkey,
              tx_bytes: result.tx_bytes,
              sign_doc_sha256_hex: result.sign_doc_sha256_hex,
            },
            attestation: {
              data_hash: result.sign_doc_sha256_hex,
              attestation_hash: result.sign_doc_sha256_hex, // same for sealed signer
              task_type: "store_signed_tx",
              timestamp: nowIso(),
            },
          });
          return;
        } catch (e) {
          const err = e as Error;
          log(500, "invoke_error", err.message, componentId);
          errorResponse(res, 500, "ERR_INVOKE_FAILED", err.message);
          return;
        }
      }

      // Unknown route
      if (path === "/health") {
        res.setHeader("Allow", "GET");
        log(405, "method_not_allowed", `${method} ${path}`);
        errorResponse(res, 405, "ERR_METHOD_NOT_ALLOWED", `${method} not allowed on ${path}`);
        return;
      }
      if (path.startsWith("/invoke/")) {
        res.setHeader("Allow", "POST");
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

  await new Promise<void>((resolveP, rejectP) => {
    server.once("error", rejectP);
    server.listen({ host: bindHost, port: cfg.port }, () => resolveP());
  });

  const addr = server.address();
  if (!addr || typeof addr === "string") {
    throw new Error("server.address() returned unexpected shape");
  }
  const boundPort = (addr as AddressInfo).port;
  const url = `http://${bindHost}:${boundPort}`;

  console.error(
    `[wavs-invoke] listening on ${url} ` +
    `(allowlist: ${[...cfg.allowedComponents].join(", ")})`
  );

  return {
    url,
    port: boundPort,
    close: () =>
      new Promise<void>((resolveP, rejectP) => {
        server.close((err) => (err ? rejectP(err) : resolveP()));
      }),
  };
}

// ──────────────────────────────────────────────
// Main (when run directly)
// ──────────────────────────────────────────────

if (import.meta.url === `file://${process.argv[1]}`) {
  startInvokeServer().catch((e) => {
    console.error(`[wavs-invoke] startup failed: ${(e as Error).message}`);
    process.exit(1);
  });
}
