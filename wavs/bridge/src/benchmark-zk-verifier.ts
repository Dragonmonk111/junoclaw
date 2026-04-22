#!/usr/bin/env tsx
/**
 * Gas benchmark for the `zk-verifier` contract on uni-7.
 *
 * Runs `VerifyProof` N times against the already-deployed contract,
 * collects `gasUsed` from each tx receipt, and writes a markdown +
 * JSON artefact summarising the measurements. The contract's
 * `VerifyProof` handler is deterministic — every call with the same
 * inputs should produce an identical gas number. Running N > 1
 * therefore confirms stability, not averaging.
 *
 * The point of this benchmark is to produce a single, auditable,
 * re-runnable gas-cost reference for the "pure-Wasm Groth16 verify
 * vs. BN254 precompile" case. The original one-shot number
 * (371,486 gas, TX F6D5774E…, captured by `deploy-zk-verifier.ts`)
 * lives in `ZK_PRECOMPILE_ARTICLE.md`; this harness gives the
 * v7-hardening PR a fresh matching number to cite, and the script
 * itself is shippable for anyone else to reproduce.
 *
 * Usage:
 *   npx tsx src/benchmark-zk-verifier.ts            # 3 runs, default artefact paths
 *   npx tsx src/benchmark-zk-verifier.ts --dry-run  # offline validation only
 *   BENCH_RUNS=5 npx tsx src/benchmark-zk-verifier.ts
 *   ZK_VERIFIER_ADDR=juno1… npx tsx src/benchmark-zk-verifier.ts
 *
 * Prerequisites:
 *   - `WAVS_OPERATOR_MNEMONIC` in wavs/.env (signer; pays gas).
 *   - Proof data JSON at `$ZK_PROOF_PATH` (default: tmpdir/groth16_proof.json).
 *     Generate with: `cargo +stable run -p zk-verifier --example generate_proof`.
 *   - The zk-verifier contract must already be deployed and have a VK
 *     stored. Deploy via `src/deploy-zk-verifier.ts` if it isn't.
 *
 * Environment variables:
 *   WAVS_OPERATOR_MNEMONIC   signer mnemonic (required unless --dry-run)
 *   ZK_VERIFIER_ADDR         contract address override (defaults to
 *                            the uni-7 deployment captured in the
 *                            zk precompile article)
 *   ZK_PROOF_PATH            path to groth16_proof.json
 *   BENCH_RUNS               sample count (default 3)
 *   BENCHMARK_OUT_MD         markdown artefact path
 *                            (default docs/ZK_VERIFIER_BENCHMARK.md)
 *   BENCHMARK_OUT_JSON       json artefact path
 *                            (default docs/zk-verifier-benchmark-results.json)
 */

import { readFileSync, writeFileSync, mkdirSync, existsSync } from "fs";
import { tmpdir } from "os";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { SigningCosmWasmClient, CosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { GasPrice } from "@cosmjs/stargate";
import { config } from "./config.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "../../..");

const DRY_RUN = process.argv.includes("--dry-run");

// Fallbacks sourced from ZK_PRECOMPILE_ARTICLE.md and contracts/zk-verifier/README.md.
// These are the uni-7 Code ID 64 deployment captured during the ZK PoC run.
const DEFAULT_ZK_VERIFIER_ADDR =
  "juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem";

// Published baselines for the comparison table. See:
//   - docs/BN254_PRECOMPILE_CASE.md
//   - ZK_PRECOMPILE_ARTICLE.md
//   - Ethereum EIP-196 / EIP-197 gas schedules
const PRECOMPILE_BASELINE_GAS = 187_000;
const HASH_STORAGE_BASELINE_GAS = 200_000;

type ProofBundle = {
  vk_base64: string;
  proof_base64: string;
  public_inputs_base64: string;
};

type VerifyRun = {
  run: number;
  tx_hash: string;
  gas_used: string;
  gas_wanted: string;
  height: number;
};

type BenchmarkResult = {
  chain_id: string;
  contract: string;
  code_id?: number;
  vk_size_bytes: number;
  runs: VerifyRun[];
  summary: {
    count: number;
    min: string;
    max: string;
    median: string;
    mean: string;
    stable: boolean; // min === max
  };
  baselines: {
    precompile_bn254: number;
    hash_storage: number;
  };
  ratios: {
    vs_precompile: string;
    vs_hash_storage: string;
  };
  timestamp: string;
  signer: string;
};

function median(values: bigint[]): bigint {
  if (values.length === 0) return 0n;
  const sorted = [...values].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  const mid = sorted.length >>> 1;
  if (sorted.length % 2 === 1) return sorted[mid];
  return (sorted[mid - 1] + sorted[mid]) / 2n;
}

function mean(values: bigint[]): bigint {
  if (values.length === 0) return 0n;
  const sum = values.reduce((a, v) => a + v, 0n);
  return sum / BigInt(values.length);
}

function loadProof(): ProofBundle {
  const proofJsonPath =
    process.env.ZK_PROOF_PATH || resolve(tmpdir(), "groth16_proof.json");
  try {
    const raw = readFileSync(proofJsonPath, "utf-8");
    const parsed = JSON.parse(raw) as Partial<ProofBundle>;
    if (
      !parsed.vk_base64 ||
      !parsed.proof_base64 ||
      !parsed.public_inputs_base64
    ) {
      throw new Error(
        `proof bundle at ${proofJsonPath} is missing fields (need vk_base64, proof_base64, public_inputs_base64)`,
      );
    }
    console.log(`Proof bundle: ${proofJsonPath}`);
    console.log(
      `  VK=${parsed.vk_base64.length} chars, Proof=${parsed.proof_base64.length} chars, Inputs=${parsed.public_inputs_base64.length} chars`,
    );
    return parsed as ProofBundle;
  } catch (e) {
    console.error(`Could not read proof bundle: ${(e as Error).message}`);
    console.error(
      `Generate one with: cargo +stable run -p zk-verifier --example generate_proof`,
    );
    process.exit(1);
  }
}

function resolveContractAddr(): string {
  if (process.env.ZK_VERIFIER_ADDR) return process.env.ZK_VERIFIER_ADDR;
  for (const arg of process.argv) {
    if (arg.startsWith("--address=")) return arg.slice("--address=".length);
  }
  console.log(
    `No ZK_VERIFIER_ADDR supplied — falling back to the uni-7 Code ID 64 contract: ${DEFAULT_ZK_VERIFIER_ADDR}`,
  );
  return DEFAULT_ZK_VERIFIER_ADDR;
}

function parseRuns(): number {
  const fromArg = process.argv.find((a) => a.startsWith("--runs="));
  if (fromArg) {
    const n = Number(fromArg.slice("--runs=".length));
    if (Number.isFinite(n) && n > 0) return Math.floor(n);
  }
  const n = Number(process.env.BENCH_RUNS || "3");
  if (!Number.isFinite(n) || n <= 0) {
    throw new Error(`BENCH_RUNS must be a positive integer, got: ${process.env.BENCH_RUNS}`);
  }
  return Math.floor(n);
}

function outPaths(): { md: string; json: string } {
  const md =
    process.env.BENCHMARK_OUT_MD ||
    resolve(REPO_ROOT, "docs", "ZK_VERIFIER_BENCHMARK.md");
  const json =
    process.env.BENCHMARK_OUT_JSON ||
    resolve(REPO_ROOT, "docs", "zk-verifier-benchmark-results.json");
  return { md, json };
}

function renderMarkdown(result: BenchmarkResult): string {
  const lines: string[] = [];
  lines.push(`# zk-verifier Gas Benchmark`);
  lines.push("");
  lines.push(
    `> Auto-generated by \`wavs/bridge/src/benchmark-zk-verifier.ts\`. ` +
      `Do not hand-edit — re-run the script instead.`,
  );
  lines.push("");
  lines.push(`- **Chain:** \`${result.chain_id}\``);
  lines.push(`- **Contract:** \`${result.contract}\``);
  if (result.code_id !== undefined) {
    lines.push(`- **Code ID:** \`${result.code_id}\``);
  }
  lines.push(`- **VK size:** ${result.vk_size_bytes} bytes`);
  lines.push(`- **Signer:** \`${result.signer}\``);
  lines.push(`- **Timestamp:** ${result.timestamp}`);
  lines.push("");
  lines.push(`## Runs`);
  lines.push("");
  lines.push(`| # | gas_used | gas_wanted | block height | tx hash |`);
  lines.push(`|---|---:|---:|---:|---|`);
  for (const r of result.runs) {
    lines.push(
      `| ${r.run} | ${r.gas_used} | ${r.gas_wanted} | ${r.height} | \`${r.tx_hash}\` |`,
    );
  }
  lines.push("");
  lines.push(`## Summary`);
  lines.push("");
  lines.push(`| stat | gas |`);
  lines.push(`|------|----:|`);
  lines.push(`| count | ${result.summary.count} |`);
  lines.push(`| min | ${result.summary.min} |`);
  lines.push(`| max | ${result.summary.max} |`);
  lines.push(`| median | ${result.summary.median} |`);
  lines.push(`| mean | ${result.summary.mean} |`);
  lines.push(
    `| stable (min == max) | ${result.summary.stable ? "✅ yes" : "❌ no"} |`,
  );
  lines.push("");
  lines.push(`## Comparison`);
  lines.push("");
  lines.push(`| approach | gas | ratio vs measured |`);
  lines.push(`|----------|----:|------------------:|`);
  lines.push(
    `| SHA-256 hash storage (JunoClaw pre-v3) | ~${result.baselines.hash_storage.toLocaleString()} | ${result.ratios.vs_hash_storage}× |`,
  );
  lines.push(
    `| BN254 precompile (Ethereum EIP-196/197) | ~${result.baselines.precompile_bn254.toLocaleString()} | ${result.ratios.vs_precompile}× |`,
  );
  lines.push(
    `| **Pure CosmWasm Groth16 (this contract)** | **${result.summary.median}** | **1.0×** |`,
  );
  lines.push("");
  lines.push(`## Interpretation`);
  lines.push("");
  lines.push(
    `The pure-CosmWasm verification is **${result.ratios.vs_precompile}×** ` +
      `the cost a BN254 precompile would give us, as estimated from Ethereum's ` +
      `EIP-196 / EIP-197 gas schedules. The gap is the actual cost of paying ` +
      `for the BN254 pairing computation inside the Wasm VM rather than calling ` +
      `a native host function. It is tractable today — a single verify fits ` +
      `comfortably under Juno's per-transaction gas limit — but the delta is ` +
      `why the three-host-function proposal ` +
      `(\`bn254_add\`, \`bn254_scalar_mul\`, \`bn254_pairing_check\`) exists in ` +
      `\`docs/BN254_PRECOMPILE_CASE.md\`.`,
  );
  lines.push("");
  lines.push(
    `The hash-storage baseline is the \`agent-company::SubmitAttestation\` ` +
      `flow pre-v3, which simply stored a SHA-256 hash on-chain. The ` +
      `zk-verifier path costs ~${result.ratios.vs_hash_storage}× that — the ` +
      `price of turning "trust the operator" into a cryptographic check.`,
  );
  lines.push("");
  lines.push(`## Reproduction`);
  lines.push("");
  lines.push("```bash");
  lines.push("# 1. Generate a fresh proof (deterministic; same seed → same VK)");
  lines.push(
    "cargo +stable run -p zk-verifier --example generate_proof",
  );
  lines.push("");
  lines.push(
    "# 2. (First time only) deploy the contract and store the VK",
  );
  lines.push("npx tsx wavs/bridge/src/deploy-zk-verifier.ts");
  lines.push("");
  lines.push(
    "# 3. Benchmark — N runs (default 3) against the stored VK",
  );
  lines.push("npx tsx wavs/bridge/src/benchmark-zk-verifier.ts");
  lines.push("```");
  lines.push("");
  return lines.join("\n");
}

async function main() {
  console.log("═══ zk-verifier Gas Benchmark ═══\n");

  const contractAddr = resolveContractAddr();
  const runCount = parseRuns();
  const proof = loadProof();

  if (DRY_RUN) {
    console.log("── DRY RUN — no chain interaction ──");
    console.log(`Would run ${runCount} VerifyProof calls against ${contractAddr}`);
    console.log(`Would write results to: ${outPaths().md}`);
    return;
  }

  if (!config.mnemonic) {
    throw new Error(
      "WAVS_OPERATOR_MNEMONIC not set in wavs/.env (use --dry-run to skip chain)",
    );
  }

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();
  console.log(`Signer:   ${account.address}`);
  console.log(`Chain:    ${config.chainId} (${config.rpcEndpoint})`);
  console.log(`Contract: ${contractAddr}`);
  console.log(`Runs:     ${runCount}\n`);

  const signing = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString("0.075ujunox") },
  );
  const querying = await CosmWasmClient.connect(config.rpcEndpoint);

  const vkStatus = (await querying.queryContractSmart(contractAddr, {
    vk_status: {},
  })) as { has_vk: boolean; vk_size_bytes: number };
  if (!vkStatus.has_vk) {
    throw new Error(
      `Contract ${contractAddr} has no VK stored. Run deploy-zk-verifier.ts or call StoreVk first.`,
    );
  }
  console.log(`✓ VK is stored (${vkStatus.vk_size_bytes} bytes)\n`);

  const balance = await signing.getBalance(account.address, config.denom);
  console.log(`Signer balance: ${balance.amount} ${balance.denom}`);
  if (BigInt(balance.amount) === 0n) {
    throw new Error(
      `Signer has 0 ${config.denom}. Fund ${account.address} before running the benchmark.`,
    );
  }

  const runs: VerifyRun[] = [];
  for (let i = 1; i <= runCount; i++) {
    console.log(`── Run ${i}/${runCount} ──`);
    const result = await signing.execute(
      account.address,
      contractAddr,
      {
        verify_proof: {
          proof_base64: proof.proof_base64,
          public_inputs_base64: proof.public_inputs_base64,
        },
      },
      "auto",
    );
    // Look up gas_wanted + height from the indexer — signing.execute only
    // returns gas_used directly. This second round-trip is cheap.
    const tx = await querying.getTx(result.transactionHash);
    const gasWanted = tx?.gasWanted !== undefined ? String(tx.gasWanted) : "0";
    const height = tx?.height ?? 0;
    console.log(
      `  tx: ${result.transactionHash}  gas_used=${result.gasUsed}  gas_wanted=${gasWanted}  height=${height}`,
    );
    runs.push({
      run: i,
      tx_hash: result.transactionHash,
      gas_used: String(result.gasUsed),
      gas_wanted: gasWanted,
      height,
    });
  }

  const gasValues = runs.map((r) => BigInt(r.gas_used));
  const minV = gasValues.reduce((a, v) => (v < a ? v : a), gasValues[0]);
  const maxV = gasValues.reduce((a, v) => (v > a ? v : a), gasValues[0]);
  const medV = median(gasValues);
  const meanV = mean(gasValues);
  const stable = minV === maxV;

  const vsPrecompile = (Number(medV) / PRECOMPILE_BASELINE_GAS).toFixed(2);
  const vsHashStorage = (Number(medV) / HASH_STORAGE_BASELINE_GAS).toFixed(2);

  const benchmark: BenchmarkResult = {
    chain_id: config.chainId,
    contract: contractAddr,
    code_id: undefined, // could be filled via queryClient.getContract(addr).codeId
    vk_size_bytes: vkStatus.vk_size_bytes,
    runs,
    summary: {
      count: runs.length,
      min: minV.toString(),
      max: maxV.toString(),
      median: medV.toString(),
      mean: meanV.toString(),
      stable,
    },
    baselines: {
      precompile_bn254: PRECOMPILE_BASELINE_GAS,
      hash_storage: HASH_STORAGE_BASELINE_GAS,
    },
    ratios: {
      vs_precompile: vsPrecompile,
      vs_hash_storage: vsHashStorage,
    },
    timestamp: new Date().toISOString(),
    signer: account.address,
  };

  // Fetch code_id for the header (non-fatal if it fails)
  try {
    const info = await querying.getContract(contractAddr);
    benchmark.code_id = info.codeId;
  } catch {
    // leave undefined
  }

  const { md, json } = outPaths();
  mkdirSync(dirname(md), { recursive: true });
  mkdirSync(dirname(json), { recursive: true });
  writeFileSync(md, renderMarkdown(benchmark));
  writeFileSync(json, JSON.stringify(benchmark, null, 2));

  console.log("\n════════════════════════════════════════════════════");
  console.log("  ZK-VERIFIER GAS BENCHMARK — SUMMARY");
  console.log("════════════════════════════════════════════════════");
  console.log(`  runs:             ${benchmark.summary.count}`);
  console.log(`  min gas_used:     ${benchmark.summary.min}`);
  console.log(`  max gas_used:     ${benchmark.summary.max}`);
  console.log(`  median:           ${benchmark.summary.median}`);
  console.log(`  mean:             ${benchmark.summary.mean}`);
  console.log(`  stable:           ${benchmark.summary.stable ? "yes" : "no"}`);
  console.log(`  vs BN254 precompile (~${PRECOMPILE_BASELINE_GAS}): ${vsPrecompile}×`);
  console.log(`  vs hash storage     (~${HASH_STORAGE_BASELINE_GAS}): ${vsHashStorage}×`);
  console.log(`  markdown:         ${md}`);
  console.log(`  json:             ${json}`);
  console.log("════════════════════════════════════════════════════");
}

main().catch((err) => {
  console.error("\nBenchmark failed:", err?.message || err);
  if (err?.stack) console.error(err.stack);
  process.exit(1);
});
