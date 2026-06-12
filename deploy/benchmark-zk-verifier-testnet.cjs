const { DirectSecp256k1HdWallet } = require('@cosmjs/proto-signing');
const { SigningCosmWasmClient, CosmWasmClient } = require('@cosmjs/cosmwasm-stargate');
const { GasPrice, calculateFee } = require('@cosmjs/stargate');
const { execSync } = require('child_process');
const { readFileSync, writeFileSync, existsSync } = require('fs');
const { join } = require('path');

const REPO_ROOT = join(__dirname, '..');

// ── Load .env if present ──
const envPath = join(REPO_ROOT, '.env');
if (existsSync(envPath)) {
  readFileSync(envPath, 'utf8')
    .split('\n')
    .forEach(line => {
      const m = line.match(/^([^#=\s]+)=(.*)$/);
      if (m && !process.env[m[1]]) process.env[m[1]] = m[2].trim();
    });
}

// ── Config ──
const RPC = 'https://juno.rpc.t.stavr.tech';
const CHAIN_ID = 'uni-7';
const GAS_PRICE = '0.075ujunox';

const MNEMONIC = process.env.JUNO_MNEMONIC;
if (!MNEMONIC) {
  console.error('Error: Create junoclaw/.env with JUNO_MNEMONIC=your words');
  process.exit(1);
}

const PURE_ADDR = 'juno19jk0dnvcjm8hm4kjxmgwy6f8phd4yumfvgjsjn5exu805j5ye6mqgvrfr2';

async function main() {

// ── Generate proof bundle if not cached ──
const PROOF_PATH = join(REPO_ROOT, 'tmpdir', 'groth16_proof.json');
if (!existsSync(PROOF_PATH)) {
  console.log('Generating Groth16 proof bundle...');
  execSync(
    'cargo run -p zk-verifier --example generate_proof --quiet',
    {
      cwd: REPO_ROOT,
      env: { ...process.env, PROOF_OUTPUT: PROOF_PATH },
      stdio: 'inherit'
    }
  );
}

const proofBundle = JSON.parse(readFileSync(PROOF_PATH, 'utf8'));
const { vk_base64, proof_base64, public_inputs_base64 } = proofBundle;

console.log(`VK: ${vk_base64.length} chars`);
console.log(`Proof: ${proof_base64.length} chars`);
console.log(`Public inputs: ${public_inputs_base64.length} chars`);

// ── Wallet + Client ──
const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' });
const [account] = await wallet.getAccounts();
const address = account.address;

console.log(`\n=== ZK-Verifier Benchmark (Testnet) ===`);
console.log(`Chain: ${CHAIN_ID}`);
console.log(`RPC: ${RPC}`);
console.log(`Contract: ${PURE_ADDR}`);
console.log(`Admin: ${address}`);

const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet, {
  gasPrice: GasPrice.fromString(GAS_PRICE),
});

// ── Query VK status ──
const queryClient = await CosmWasmClient.connect(RPC);
const vkStatus = await queryClient.queryContractSmart(PURE_ADDR, { vk_status: {} });
console.log(`VK stored: ${vkStatus.has_vk}`);

// ── Store VK if not already stored ──
if (!vkStatus.has_vk) {
  console.log('\n[1/3] Storing VK...');
  const storeFee = calculateFee(2_000_000, GasPrice.fromString(GAS_PRICE));
  const storeResult = await client.execute(
    address,
    PURE_ADDR,
    { store_vk: { vk_base64 } },
    storeFee
  );
  console.log(`  TX: ${storeResult.transactionHash}`);
  console.log(`  Gas used: ${storeResult.gasUsed}`);
} else {
  console.log('\n[1/3] VK already stored, skipping');
}

// ── Verify proof (multiple samples) ──
const SAMPLES = parseInt(process.env.SAMPLES || '3', 10);
const results = [];

console.log(`\n[2/3] Verifying proof ${SAMPLES} times...`);
for (let i = 0; i < SAMPLES; i++) {
  const verifyFee = calculateFee(1_000_000, GasPrice.fromString(GAS_PRICE));
  const start = Date.now();
  const result = await client.execute(
    address,
    PURE_ADDR,
    {
      verify_proof: {
        proof_base64,
        public_inputs_base64,
      },
    },
    verifyFee
  );
  const elapsed = Date.now() - start;
  results.push({
    sample: i + 1,
    tx: result.transactionHash,
    gasUsed: result.gasUsed,
    elapsedMs: elapsed,
  });
  console.log(`  Sample ${i + 1}: gas=${result.gasUsed}, time=${elapsed}ms, tx=${result.transactionHash.slice(0, 16)}...`);
}

// ── Query last verify ──
const lastVerify = await queryClient.queryContractSmart(PURE_ADDR, { last_verify: {} });
console.log(`\n[3/3] Last verify query: verified=${lastVerify.verified}, height=${lastVerify.block_height}`);

// ── Summary ──
const avgGas = Math.round(results.reduce((s, r) => s + parseInt(r.gasUsed), 0) / results.length);
const avgTime = Math.round(results.reduce((s, r) => s + r.elapsedMs, 0) / results.length);

console.log(`\n=== Benchmark Summary ===`);
console.log(`Samples: ${SAMPLES}`);
console.log(`Average gas: ${avgGas}`);
console.log(`Average time: ${avgTime}ms`);
console.log(`Gas price: ${GAS_PRICE}`);
console.log(`Est. cost per verify: ~${(avgGas * 0.075 / 1_000_000).toFixed(4)} ujunox`);

// Write results
const outPath = join(REPO_ROOT, 'docs', 'BN254_BENCHMARK_TESTNET.md');
const report = `# ZK-Verifier Benchmark Results (Testnet)

> Chain: ${CHAIN_ID} | Contract: ${PURE_ADDR} | Date: ${new Date().toISOString()}

## Configuration

| Parameter | Value |
|-----------|-------|
| Gas price | ${GAS_PRICE} |
| Samples | ${SAMPLES} |
| VK size | ${vk_base64.length} chars (base64) |
| Proof size | ${proof_base64.length} chars (base64) |

## Results

| Sample | Gas Used | Time (ms) | TX Hash |
|--------|----------|-----------|---------|
${results.map(r => `| ${r.sample} | ${r.gasUsed} | ${r.elapsedMs} | ${r.tx} |`).join('\n')}

## Summary

- **Average gas**: ${avgGas}
- **Average time**: ${avgTime}ms
- **Est. cost per verify**: ~${(avgGas * 0.075 / 1_000_000).toFixed(4)} ujunox

## Notes

- Pure wasm verifier (no BN254 precompile on uni-7)
- Compare with devnet precompile numbers when available
`;

writeFileSync(outPath, report);
console.log(`\nReport written to: ${outPath}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
