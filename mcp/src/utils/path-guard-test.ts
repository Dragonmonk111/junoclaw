/**
 * Smoke test for path-guard module — Ffern C-4 regression coverage.
 *
 * Run:
 *   npm run path-guard-test       (from mcp/)
 *   tsx mcp/src/utils/path-guard-test.ts
 *
 * Exits 0 on success, 1 on any failure. Prints a per-test pass/fail
 * line plus a summary.
 *
 * The test creates a sandboxed WASM_ROOT under the OS tmpdir, plants
 * a valid wasm and a battery of trap files, and exercises every
 * defense in `validateWasmPath`. It cleans up after itself.
 */

import {
  existsSync,
  mkdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "fs";
import { tmpdir } from "os";
import { join, resolve } from "path";
import { validateWasmPath } from "./path-guard.js";

let passed = 0;
let failed = 0;
const failures: string[] = [];

function test(name: string, fn: () => void) {
  try {
    fn();
    console.log(`  PASS  ${name}`);
    passed++;
  } catch (e) {
    console.log(`  FAIL  ${name}\n        ${(e as Error).message}`);
    failed++;
    failures.push(name);
  }
}

function expectThrow(fn: () => void, contains: string) {
  let threw = false;
  try {
    fn();
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

// ─── Test fixture ──────────────────────────────────────────────

const sandbox = resolve(tmpdir(), `junoclaw-path-guard-${Date.now()}`);
const wasmRoot = join(sandbox, "wasm-root");
const outsideDir = join(sandbox, "outside");
const validWasmPath = join(wasmRoot, "valid.wasm");
const tooSmallPath = join(wasmRoot, "tiny.wasm");
const wrongMagicPath = join(wasmRoot, "not-wasm.bin");
const symlinkPath = join(wasmRoot, "evil-link.wasm");
const outsidePath = join(outsideDir, "outside.wasm");
const oversizedPath = join(wasmRoot, "huge.wasm");

mkdirSync(wasmRoot, { recursive: true });
mkdirSync(outsideDir, { recursive: true });

// Valid wasm: magic + version (4 bytes) + a tiny payload.
const wasmMagic = Buffer.from([0x00, 0x61, 0x73, 0x6d]);
const wasmVersion = Buffer.from([0x01, 0x00, 0x00, 0x00]);
const validBytes = Buffer.concat([wasmMagic, wasmVersion, Buffer.alloc(64, 0)]);

writeFileSync(validWasmPath, validBytes);
writeFileSync(tooSmallPath, Buffer.from([0x00, 0x61])); // 2 bytes
writeFileSync(wrongMagicPath, Buffer.from("not a wasm file at all"));
writeFileSync(outsidePath, validBytes);

// 9 MiB file with a valid magic header — exercises the size cap.
const huge = Buffer.alloc(9 * 1024 * 1024);
huge.set(wasmMagic, 0);
huge.set(wasmVersion, 4);
writeFileSync(oversizedPath, huge);

let symlinkSupported = true;
try {
  symlinkSync(outsidePath, symlinkPath);
} catch (e) {
  // On Windows without admin / Developer Mode, symlink creation may fail.
  symlinkSupported = false;
  console.log(
    `  SKIP  symlink creation not supported in this environment: ${
      (e as Error).message
    }`
  );
}

const opts = { wasmRoot };

console.log("\n━━━ path-guard smoke (Ffern C-4 regression) ━━━\n");

// ─── Happy path ─────────────────────────────────────────────────

test("happy path: valid wasm under wasm_root returns matching bytes", () => {
  const r = validateWasmPath(validWasmPath, opts);
  if (!r.bytes.equals(validBytes)) {
    throw new Error(
      `bytes mismatch: got ${r.bytes.length} bytes, expected ${validBytes.length}`
    );
  }
  if (resolve(r.absolutePath) !== resolve(validWasmPath)) {
    throw new Error(
      `absolutePath mismatch: ${r.absolutePath} vs ${validWasmPath}`
    );
  }
});

// ─── Defense: input sanity ─────────────────────────────────────

test("rejects empty path", () => {
  expectThrow(() => validateWasmPath("", opts), "empty");
});

test("rejects non-existent path", () => {
  expectThrow(
    () => validateWasmPath(join(wasmRoot, "does-not-exist.wasm"), opts),
    "cannot stat"
  );
});

// ─── Defense: WASM_ROOT must exist ─────────────────────────────

test("rejects when WASM_ROOT does not exist", () => {
  expectThrow(
    () =>
      validateWasmPath(validWasmPath, {
        wasmRoot: join(sandbox, "nonexistent-root"),
      }),
    "WASM_ROOT"
  );
});

// ─── Defense: allow-root ───────────────────────────────────────

test("rejects path outside WASM_ROOT", () => {
  expectThrow(() => validateWasmPath(outsidePath, opts), "outside WASM_ROOT");
});

test("rejects path with traversal segment (..)", () => {
  expectThrow(
    () =>
      validateWasmPath(
        join(wasmRoot, "..", "outside", "outside.wasm"),
        opts
      ),
    "outside WASM_ROOT"
  );
});

// ─── Defense: symlink reject ───────────────────────────────────

if (symlinkSupported) {
  test("rejects symlink even when target is a valid wasm", () => {
    expectThrow(
      () => validateWasmPath(symlinkPath, opts),
      "symbolic link"
    );
  });
}

// ─── Defense: size cap ─────────────────────────────────────────

test("rejects file too small to hold magic bytes", () => {
  expectThrow(() => validateWasmPath(tooSmallPath, opts), "too small");
});

test("rejects oversized file (9 MiB > default 8 MiB cap)", () => {
  expectThrow(() => validateWasmPath(oversizedPath, opts), "exceeds the");
});

test("respects custom maxBytes via options", () => {
  expectThrow(
    () => validateWasmPath(validWasmPath, { ...opts, maxBytes: 32 }),
    "exceeds the"
  );
});

// ─── Defense: magic bytes ──────────────────────────────────────

test("rejects file with wrong magic bytes", () => {
  expectThrow(
    () => validateWasmPath(wrongMagicPath, opts),
    "magic bytes"
  );
});

// ─── Cleanup ───────────────────────────────────────────────────

try {
  rmSync(sandbox, { recursive: true, force: true });
} catch (e) {
  console.warn(`(cleanup warning: ${(e as Error).message})`);
}

console.log(`\n${passed} passed, ${failed} failed${
  symlinkSupported ? "" : " (symlink test skipped)"
}\n`);

if (failed > 0) {
  console.log("Failed tests:");
  for (const name of failures) console.log(`  - ${name}`);
  process.exit(1);
}

// Sanity check: at least 8 tests should have run on any platform
// (the symlink test is skipped on Windows without dev-mode).
const minExpected = symlinkSupported ? 11 : 10;
if (passed < minExpected) {
  console.log(
    `WARNING: only ${passed} tests ran, expected at least ${minExpected}.`
  );
  process.exit(1);
}

if (existsSync(symlinkPath)) {
  console.log("WARNING: symlink fixture not cleaned up properly");
}

process.exit(0);
