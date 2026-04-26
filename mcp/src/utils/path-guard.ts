/**
 * path-guard — validate filesystem paths handed to write tools.
 *
 * Addresses Ffern Institute audit finding C-4 (April 2026): the
 * `upload_wasm` MCP tool accepted unbounded local file paths with
 *   - no allow-root,
 *   - no symlink check (so an attacker who could plant a symlink at
 *     a path the operator was about to upload could exfiltrate any
 *     readable file via the on-chain MsgStoreCode bytes),
 *   - no size cap, and
 *   - no magic-byte verification.
 *
 * This module centralises the four checks so every file-reading
 * write tool gets the same defenses, and so the logic is unit-
 * testable in isolation.
 *
 * Defaults:
 *   - WASM_ROOT defaults to `process.env.JUNOCLAW_WASM_ROOT` if set,
 *     else `~/.junoclaw/wasm`.
 *   - Max size: 8 MiB (the practical ceiling for an optimised
 *     CosmWasm contract; on-chain consensus rejects larger anyway).
 *   - Magic bytes: `\0asm` (0x00 0x61 0x73 0x6d), the wasm v1 magic.
 *
 * The defaults can be overridden via the `options` argument or via
 * environment variables. See the README and `SECURITY.md` for the
 * deployment-time configuration guidance.
 */

import { lstatSync, readFileSync, realpathSync } from "fs";
import { homedir } from "os";
import { isAbsolute, relative, resolve } from "path";

const DEFAULT_WASM_ROOT = resolve(homedir(), ".junoclaw", "wasm");
const DEFAULT_MAX_WASM_BYTES = 8 * 1024 * 1024; // 8 MiB
const WASM_MAGIC = Buffer.from([0x00, 0x61, 0x73, 0x6d]); // "\0asm"

export interface ValidatedWasm {
  absolutePath: string;
  bytes: Buffer;
}

export interface ValidateWasmOptions {
  /**
   * Directory the wasm file must reside under. Defaults to
   * `process.env.JUNOCLAW_WASM_ROOT` if set, otherwise
   * `~/.junoclaw/wasm`.
   */
  wasmRoot?: string;
  /** Maximum allowed file size in bytes. Defaults to 8 MiB. */
  maxBytes?: number;
  /**
   * Test seam — override the `fs` functions used. Production code
   * should leave this undefined.
   */
  fs?: {
    lstatSync: typeof lstatSync;
    readFileSync: typeof readFileSync;
    realpathSync: typeof realpathSync;
  };
}

/**
 * Validate `rawPath` and return its canonical absolute path plus
 * its bytes. Throws an `Error` whose message names the rejection
 * cause when any defense fires.
 *
 * The defenses, in order:
 *   1. Empty / non-string input → reject.
 *   2. WASM_ROOT must exist and be readable.
 *   3. The file at `rawPath` must not itself be a symlink.
 *   4. The file's `realpath` (which resolves any symlinked parent
 *      directories) must lie under WASM_ROOT.
 *   5. Pre-read size check against `maxBytes`.
 *   6. Read.
 *   7. Post-read size check (catches TOCTOU growth).
 *   8. Magic-byte check (`\0asm`).
 */
export function validateWasmPath(
  rawPath: string,
  options: ValidateWasmOptions = {}
): ValidatedWasm {
  const fs = options.fs ?? { lstatSync, readFileSync, realpathSync };
  const wasmRootRaw =
    options.wasmRoot ?? process.env.JUNOCLAW_WASM_ROOT ?? DEFAULT_WASM_ROOT;
  const maxBytes = options.maxBytes ?? DEFAULT_MAX_WASM_BYTES;

  // 1. Sanity check on input.
  if (typeof rawPath !== "string" || rawPath.length === 0) {
    throw new Error("wasm_path rejected: empty or non-string path");
  }

  // 2. Canonicalise WASM_ROOT.
  let wasmRootReal: string;
  try {
    wasmRootReal = fs.realpathSync(resolve(wasmRootRaw));
  } catch (e) {
    throw new Error(
      `wasm_path rejected: WASM_ROOT (${wasmRootRaw}) does not exist or is unreadable. ` +
        `Create it (mkdir -p) or set JUNOCLAW_WASM_ROOT to an existing directory. ` +
        `Underlying error: ${(e as Error).message}`
    );
  }

  // 3. Resolve the user-supplied path to absolute. Relative paths
  //    are resolved against the process CWD. We then `lstat` the
  //    literal path: if it is itself a symlink, reject. (Following
  //    a symlink at the leaf would let an attacker exfiltrate any
  //    file the operator process can read via the on-chain wasm.)
  const abs = isAbsolute(rawPath)
    ? resolve(rawPath)
    : resolve(process.cwd(), rawPath);

  let lstatResult: ReturnType<typeof lstatSync>;
  try {
    lstatResult = fs.lstatSync(abs);
  } catch (e) {
    throw new Error(
      `wasm_path rejected: cannot stat ${abs}: ${(e as Error).message}`
    );
  }
  if (lstatResult.isSymbolicLink()) {
    throw new Error(
      `wasm_path rejected: ${abs} is a symbolic link. ` +
        `Symlinks are not followed for security reasons (Ffern C-4).`
    );
  }
  if (!lstatResult.isFile()) {
    throw new Error(
      `wasm_path rejected: ${abs} is not a regular file ` +
        `(mode 0o${lstatResult.mode.toString(8)}).`
    );
  }

  // 4. realpath catches symlinks higher up the directory chain.
  //    If any parent of `abs` is a symlink, `realpath` resolves it,
  //    and the resulting absReal may be outside WASM_ROOT.
  let absReal: string;
  try {
    absReal = fs.realpathSync(abs);
  } catch (e) {
    throw new Error(
      `wasm_path rejected: cannot canonicalise ${abs}: ${(e as Error).message}`
    );
  }
  const rel = relative(wasmRootReal, absReal);
  if (rel === "" || rel.startsWith("..") || isAbsolute(rel)) {
    throw new Error(
      `wasm_path rejected: ${abs} (real: ${absReal}) is outside WASM_ROOT (${wasmRootReal}). ` +
        `Move the wasm into the allow-root or set JUNOCLAW_WASM_ROOT.`
    );
  }

  // 5. Pre-read size check.
  if (lstatResult.size > maxBytes) {
    throw new Error(
      `wasm_path rejected: ${abs} is ${lstatResult.size} bytes, ` +
        `exceeds the ${maxBytes}-byte cap (${(maxBytes / 1024 / 1024).toFixed(1)} MiB).`
    );
  }
  if (lstatResult.size < WASM_MAGIC.length) {
    throw new Error(
      `wasm_path rejected: ${abs} is too small (${lstatResult.size} bytes) ` +
        `to be a wasm module.`
    );
  }

  // 6. Read.
  let bytes: Buffer;
  try {
    bytes = fs.readFileSync(abs);
  } catch (e) {
    throw new Error(
      `wasm_path rejected: read failed for ${abs}: ${(e as Error).message}`
    );
  }

  // 7. Post-read size check (TOCTOU-lite: file may have grown).
  if (bytes.length > maxBytes) {
    throw new Error(
      `wasm_path rejected: file grew during read ` +
        `(now ${bytes.length} bytes, cap ${maxBytes}).`
    );
  }

  // 8. Magic bytes: \0asm.
  if (
    bytes.length < WASM_MAGIC.length ||
    !bytes.subarray(0, WASM_MAGIC.length).equals(WASM_MAGIC)
  ) {
    const got = bytes
      .subarray(0, Math.min(WASM_MAGIC.length, bytes.length))
      .toString("hex");
    throw new Error(
      `wasm_path rejected: ${abs} does not start with the wasm magic bytes ` +
        `(\\0asm = 0061736d). Got: 0x${got}`
    );
  }

  return { absolutePath: abs, bytes };
}
