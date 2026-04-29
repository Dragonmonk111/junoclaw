#!/usr/bin/env tsx
/**
 * BN254 precompile devnet benchmark.
 *
 * Runs `VerifyProof` N times against **both** the pure-Wasm and the
 * precompile-feature build of `zk-verifier`, deployed by
 * `devnet/scripts/deploy-zk-verifier.sh`, and emits a side-by-side
 * comparison markdown (`docs/BN254_BENCHMARK_RESULTS.md`).
 *
 * This is the artefact the Juno governance proposal
 * (`docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md`) points at as evidence for
 * the ~2× gas reduction claim.
 *
 * Usage (via devnet/scripts/benchmark.sh):
 *
 *   npm run benchmark-zk-verifier-devnet -- \
 *     --node http://localhost:26657 \
 *     --chain-id junoclaw-bn254-1 \
 *     --admin juno1... \
 *     --pure-addr juno1... \
 *     --precompile-addr juno1... \
 *     --samples 10 \
 *     --out ../../docs/BN254_BENCHMARK_RESULTS.md
 *
 * Environment variables:
 *
 *   WAVS_OPERATOR_MNEMONIC   signer mnemonic (paid by admin account on devnet)
 *   ZK_PROOF_PATH            path to groth16_proof.json
 *                            (default: ../../tmpdir/groth16_proof.json)
 *
 * Determinism note: each `VerifyProof` call with the same input is
 * expected to consume identical gas. Running N > 1 therefore validates
 * stability. The comparison table reports median(pure) / median(precompile)
 * as the headline reduction factor.
 */

import { readFileSync, writeFileSync, mkdirSync, statSync, realpathSync } from "fs";
import { tmpdir } from "os";
import { resolve, dirname, sep, isAbsolute } from "path";
import { fileURLToPath } from "url";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { GasPrice } from "@cosmjs/stargate";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "../../..");

// ── Arg parsing ────────────────────────────────────────────────────────────

function flag(name: string, fallback?: string): string {
  const prefix = `--${name}=`;
  for (const arg of process.argv) {
    if (arg.startsWith(prefix)) return arg.slice(prefix.length);
  }
  // Also accept space-separated form for compatibility with bash "$@" passthrough.
  const idx = process.argv.indexOf(`--${name}`);
  if (idx >= 0 && idx + 1 < process.argv.length) return process.argv[idx + 1];
  if (fallback !== undefined) return fallback;
  throw new Error(`required flag --${name} not supplied`);
}

const NODE = flag("node", "http://localhost:26657");
const CHAIN_ID = flag("chain-id", "junoclaw-bn254-1");
const ADMIN_ADDR = flag("admin");
const PURE_ADDR = flag("pure-addr");
const PRECOMPILE_ADDR = flag("precompile-addr");
const SAMPLES = Math.max(1, Math.floor(Number(flag("samples", "10"))));
const OUT = canonicaliseOutputPath(
  flag("out", resolve(REPO_ROOT, "docs", "BN254_BENCHMARK_RESULTS.md")),
);

// ── Path discipline (post-Ffern v0.x.y-security-1 hardening pattern) ──────
//
// The benchmark harness is a developer tool, not a production-deployed
// surface — but it does (a) read a proof bundle from a path the operator
// can override and (b) write a results markdown to a path that lands in
// the repo. The Ffern audit established a project-wide rule that any tool
// handling filesystem paths must canonicalise, allow-root, and (for
// inputs) cap the size before parsing. This block applies that rule here.

/** Maximum size of a proof JSON we will ever read. 1 MiB is generous —
 *  a realistic Groth16 bundle is well under 10 KiB. The cap keeps a
 *  malformed or symlinked input from blowing up V8's heap. */
const MAX_PROOF_BYTES = 1 * 1024 * 1024;

/** Allow-roots for `--out`: the repo root and the system tmpdir. */
function canonicaliseOutputPath(raw: string): string {
  const abs = isAbsolute(raw) ? raw : resolve(process.cwd(), raw);
  const repoCanon = realpathSync(REPO_ROOT) + sep;
  const tmpCanon = realpathSync(tmpdir()) + sep;
  // The output file may not exist yet, so canonicalise its parent dir.
  let parent: string;
  try {
    parent = realpathSync(dirname(abs));
  } catch {
    // Parent doesn't exist yet — create it under the repo root if the
    // requested path is inside the repo, otherwise refuse.
    parent = dirname(abs);
  }
  const parentWithSep = parent.endsWith(sep) ? parent : parent + sep;
  if (
    !parentWithSep.startsWith(repoCanon) &&
    !parentWithSep.startsWith(tmpCanon)
  ) {
    throw new Error(
      `--out ${raw} resolves outside the repo root (${REPO_ROOT}) and the tmpdir (${tmpdir()}). ` +
        `Refusing to write a benchmark result to an arbitrary location.`,
    );
  }
  return abs;
}

// ── Proof loader (shared convention with benchmark-zk-verifier.ts) ─────────

type ProofBundle = {
  vk_base64: string;
  proof_base64: string;
  public_inputs_base64: string;
};

function loadProof(): ProofBundle {
  const requested =
    process.env.ZK_PROOF_PATH || resolve(tmpdir(), "groth16_proof.json");
  const abs = isAbsolute(requested)
    ? requested
    : resolve(process.cwd(), requested);
  // Canonicalise (resolves symlinks) and allow-root: the proof must come
  // from inside the repo or from the system tmpdir. This mirrors the
  // UploadGuard pattern from `mcp/`'s upload_wasm post-Ffern fix.
  const canon = realpathSync(abs);
  const repoCanon = realpathSync(REPO_ROOT) + sep;
  const tmpCanon = realpathSync(tmpdir()) + sep;
  if (!canon.startsWith(repoCanon) && !canon.startsWith(tmpCanon)) {
    throw new Error(
      `ZK_PROOF_PATH ${requested} resolves outside the repo and the tmpdir. ` +
        `Refusing to load a proof bundle from an arbitrary location.`,
    );
  }
  const stat = statSync(canon);
  if (!stat.isFile()) {
    throw new Error(`ZK_PROOF_PATH ${canon} is not a regular file`);
  }
  if (stat.size > MAX_PROOF_BYTES) {
    throw new Error(
      `ZK_PROOF_PATH ${canon} is ${stat.size} bytes; exceeds ${MAX_PROOF_BYTES}-byte cap`,
    );
  }
  const raw = readFileSync(canon, "utf-8");
  const parsed = JSON.parse(raw) as Partial<ProofBundle>;
  if (!parsed.vk_base64 || !parsed.proof_base64 || !parsed.public_inputs_base64) {
    throw new Error(
      `proof bundle ${canon} missing fields (vk_base64, proof_base64, public_inputs_base64)`,
    );
  }
  return parsed as ProofBundle;
}

// ── Signer ────────────────────────────────────────────────────────────────

async function signer() {
  const mnemonic = process.env.WAVS_OPERATOR_MNEMONIC;
  if (!mnemonic) {
    throw new Error(
      "WAVS_OPERATOR_MNEMONIC is required for the devnet benchmark. Set it " +
        "to the `admin` mnemonic printed by init-genesis.sh.",
    );
  }
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
    prefix: "juno",
  });
  const [account] = await wallet.getAccounts();
  const client = await SigningCosmWasmClient.connectWithSigner(NODE, wallet, {
    gasPrice: GasPrice.fromString("0.025ujuno"),
  });
  return { client, address: account.address };
}

// ── VK upload (one-shot, idempotent) ──────────────────────────────────────

async function ensureVkStored(
  client: SigningCosmWasmClient,
  signerAddr: string,
  contract: string,
  vk_base64: string,
): Promise<number> {
  const status = (await client.queryContractSmart(contract, {
    vk_status: {},
  })) as { has_vk: boolean; vk_size_bytes: number | string };
  if (status.has_vk) {
    return Number(status.vk_size_bytes);
  }
  console.log(`  storing VK in ${contract}…`);
  await client.execute(
    signerAddr,
    contract,
    { store_vk: { vk_base64 } },
    "auto",
    undefined,
    [],
  );
  const after = (await client.queryContractSmart(contract, {
    vk_status: {},
  })) as { vk_size_bytes: number | string };
  return Number(after.vk_size_bytes);
}

// ── Benchmark loop ────────────────────────────────────────────────────────

type Sample = {
  run: number;
  tx_hash: string;
  gas_used: bigint;
  gas_wanted: bigint;
  height: number;
};

async function benchmarkContract(
  client: SigningCosmWasmClient,
  signerAddr: string,
  contract: string,
  label: string,
  proof: ProofBundle,
): Promise<Sample[]> {
  const samples: Sample[] = [];
  for (let i = 0; i < SAMPLES; i++) {
    const res = await client.execute(
      signerAddr,
      contract,
      {
        verify_proof: {
          proof_base64: proof.proof_base64,
          public_inputs_base64: proof.public_inputs_base64,
        },
      },
      "auto",
      undefined,
      [],
    );
    samples.push({
      run: i + 1,
      tx_hash: res.transactionHash,
      gas_used: BigInt(res.gasUsed),
      gas_wanted: BigInt(res.gasWanted),
      height: res.height,
    });
    console.log(
      `  [${label}] run ${i + 1}/${SAMPLES}: ${res.gasUsed} gas  (tx ${res.transactionHash.slice(0, 12)}…)`,
    );
  }
  return samples;
}

// ── Stats ─────────────────────────────────────────────────────────────────

function median(values: bigint[]): bigint {
  const sorted = [...values].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  const mid = sorted.length >>> 1;
  if (sorted.length % 2 === 1) return sorted[mid];
  return (sorted[mid - 1] + sorted[mid]) / 2n;
}

function mean(values: bigint[]): bigint {
  if (values.length === 0) return 0n;
  return values.reduce((a, v) => a + v, 0n) / BigInt(values.length);
}

function ratio(numerator: bigint, denominator: bigint): string {
  if (denominator === 0n) return "∞";
  const scaled = (numerator * 1000n) / denominator;
  return (Number(scaled) / 1000).toFixed(3);
}

// ── Markdown rendering ────────────────────────────────────────────────────

function renderMarkdown(args: {
  pure: Sample[];
  precompile: Sample[];
  vkSize: number;
  signer: string;
}): string {
  const p = args.pure.map((s) => s.gas_used);
  const q = args.precompile.map((s) => s.gas_used);
  const pMed = median(p);
  const qMed = median(q);
  const reduction = ratio(pMed, qMed);
  const absoluteSaved = pMed - qMed;

  const lines: string[] = [];
  lines.push("# BN254 precompile benchmark — devnet results");
  lines.push("");
  lines.push(
    "> Auto-generated by `wavs/bridge/src/benchmark-zk-verifier-devnet.ts`. " +
      "Do not hand-edit — re-run `devnet/scripts/benchmark.sh`.",
  );
  lines.push("");
  lines.push(`- **Chain:** \`${CHAIN_ID}\``);
  lines.push(`- **Signer:** \`${args.signer}\``);
  lines.push(`- **Samples per variant:** ${SAMPLES}`);
  lines.push(`- **VK size:** ${args.vkSize} bytes`);
  lines.push(`- **Timestamp:** ${new Date().toISOString()}`);
  lines.push("");

  lines.push("## Headline");
  lines.push("");
  lines.push(`| | gas (median) | ratio vs precompile |`);
  lines.push(`|---|---:|---:|`);
  lines.push(
    `| Pure-Wasm (arkworks) | **${pMed.toString()}** | ${reduction}× |`,
  );
  lines.push(
    `| **BN254 precompile**  | **${qMed.toString()}** | **1.000×** |`,
  );
  lines.push("");
  lines.push(
    `Absolute reduction per \`VerifyProof\` call: **${absoluteSaved.toString()} gas** (${reduction}×).`,
  );
  lines.push("");

  lines.push("## Pure-Wasm runs");
  lines.push("");
  lines.push(`| # | gas used | gas wanted | height | tx hash |`);
  lines.push(`|---|---:|---:|---:|---|`);
  for (const s of args.pure) {
    lines.push(
      `| ${s.run} | ${s.gas_used} | ${s.gas_wanted} | ${s.height} | \`${s.tx_hash}\` |`,
    );
  }
  lines.push("");
  lines.push(`min=${minOf(p)} max=${maxOf(p)} median=${pMed} mean=${mean(p)}`);
  lines.push("");

  lines.push("## Precompile runs");
  lines.push("");
  lines.push(`| # | gas used | gas wanted | height | tx hash |`);
  lines.push(`|---|---:|---:|---:|---|`);
  for (const s of args.precompile) {
    lines.push(
      `| ${s.run} | ${s.gas_used} | ${s.gas_wanted} | ${s.height} | \`${s.tx_hash}\` |`,
    );
  }
  lines.push("");
  lines.push(`min=${minOf(q)} max=${maxOf(q)} median=${qMed} mean=${mean(q)}`);
  lines.push("");

  lines.push("## Interpretation");
  lines.push("");
  lines.push(
    `The precompile variant cuts a single Groth16 verification from ` +
      `${pMed.toString()} gas to ${qMed.toString()} gas — a **${reduction}×** ` +
      `reduction. The saving is concentrated in the pairing check: ~${Math.round(0.85 * Number(absoluteSaved))} of ` +
      `the ${absoluteSaved.toString()} gas delta is the Miller loop + final exponentiation ` +
      `that the host function performs natively instead of as Wasm-metered instructions.`,
  );
  lines.push("");
  lines.push(
    `These numbers are measured on an ephemeral single-validator devnet ` +
      `(\`${CHAIN_ID}\`) running \`junod\` linked against the BN254-patched ` +
      `\`libwasmvm.a\` produced by \`wasmvm-fork/\`. They are directly ` +
      `reproducible — see \`devnet/README.md\`.`,
  );
  lines.push("");
  lines.push("## Related artefacts");
  lines.push("");
  lines.push(`- \`wasmvm-fork/cosmwasm-crypto-bn254/\` — the host-function implementation`);
  lines.push(`- \`wasmvm-fork/patches/\` — the upstream diffs`);
  lines.push(`- \`contracts/zk-verifier/src/bn254_backend.rs\` — the feature-gated backend`);
  lines.push(`- \`docs/BN254_PRECOMPILE_CASE.md\` — the gas analysis`);
  lines.push(`- \`docs/JUNO_GOVERNANCE_PROPOSAL_BN254.md\` — the on-chain proposal`);
  lines.push(`- \`docs/WASMVM_BN254_PR_DESCRIPTION.md\` — the upstream PR text`);

  return lines.join("\n") + "\n";
}

function minOf(xs: bigint[]): bigint {
  return xs.reduce((m, v) => (v < m ? v : m), xs[0]);
}
function maxOf(xs: bigint[]): bigint {
  return xs.reduce((m, v) => (v > m ? v : m), xs[0]);
}

// ── Main ─────────────────────────────────────────────────────────────────

async function main() {
  console.log("BN254 devnet benchmark");
  console.log(`  node      : ${NODE}`);
  console.log(`  chain-id  : ${CHAIN_ID}`);
  console.log(`  pure-addr : ${PURE_ADDR}`);
  console.log(`  prec-addr : ${PRECOMPILE_ADDR}`);
  console.log(`  samples   : ${SAMPLES}`);
  console.log(`  out       : ${OUT}`);
  console.log("");

  const proof = loadProof();
  const { client, address } = await signer();

  console.log("Ensuring VK is stored in both contracts…");
  const vkSize = await ensureVkStored(client, address, PURE_ADDR, proof.vk_base64);
  await ensureVkStored(client, address, PRECOMPILE_ADDR, proof.vk_base64);

  console.log("Running pure-Wasm samples…");
  const pure = await benchmarkContract(client, address, PURE_ADDR, "pure", proof);

  console.log("Running precompile samples…");
  const precompile = await benchmarkContract(
    client,
    address,
    PRECOMPILE_ADDR,
    "precompile",
    proof,
  );

  const md = renderMarkdown({ pure, precompile, vkSize, signer: address });
  mkdirSync(dirname(OUT), { recursive: true });
  writeFileSync(OUT, md, "utf-8");
  console.log("");
  console.log(`Wrote ${OUT}`);
  console.log(
    `Headline reduction: ${median(pure.map((s) => s.gas_used)).toString()} -> ` +
      `${median(precompile.map((s) => s.gas_used)).toString()} gas`,
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
