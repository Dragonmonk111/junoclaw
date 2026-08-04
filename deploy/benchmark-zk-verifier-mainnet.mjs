// Benchmark VerifyProof gas on Juno mainnet (juno-1) against the
// already-deployed pure-Wasm zk-verifier (code ID 5146).
//
// Usage:
//   cd deploy
//   $env:WALLET_ID="builder"; $env:CHAIN_ID="juno-1"; node benchmark-zk-verifier-mainnet.mjs
//
// Optionally set SAMPLES=5 (default 3) to run more verify iterations.

import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { execSync } from 'child_process'
import { CosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import 'dotenv/config'

const __dir = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = join(__dir, '..')

// ── Config ──────────────────────────────────────────────────────────────────

const CHAIN_ID  = process.env.CHAIN_ID  || 'juno-1'
const IS_MAINNET = CHAIN_ID === 'juno-1'
const RPC_URL   = process.env.RPC_URL   || (IS_MAINNET ? 'https://juno-rpc.polkachu.com' : 'https://juno.rpc.t.stavr.tech')
const DENOM     = process.env.DENOM     || (IS_MAINNET ? 'ujuno' : 'ujunox')
const GAS_PRICE = process.env.GAS_PRICE || `0.075${DENOM}`

const ZK_VERIFIER_ADDR = IS_MAINNET
  ? 'juno1qd9qaggnw350kt7wjpw37h0c7666wuwulhz0makrve9tenkx0ymqvfkh7p'
  : 'juno19jk0dnvcjm8hm4kjxmgwy6f8phd4yumfvgjsjn5exu805j5ye6mqgvrfr2'

const SAMPLES = parseInt(process.env.SAMPLES || '3', 10)

// ── Main ────────────────────────────────────────────────────────────────────

async function main() {
  console.log(`\n  ZK-Verifier Benchmark — ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}`)
  console.log(`  Contract: ${ZK_VERIFIER_ADDR}`)
  console.log(`  Samples:  ${SAMPLES}\n`)

  if (IS_MAINNET) {
    console.log('  \u26A0  MAINNET — this spends real JUNO on VerifyProof txs. Ctrl+C now to abort.\n')
  }

  // ── Generate or load proof bundle ──
  const PROOF_PATH = join(REPO_ROOT, 'tmpdir', 'groth16_proof.json')
  if (!existsSync(PROOF_PATH)) {
    console.log('  Generating Groth16 proof bundle (cargo run --example generate_proof)...')
    execSync(
      'cargo run -p zk-verifier --example generate_proof --quiet',
      { cwd: REPO_ROOT, env: { ...process.env, PROOF_OUTPUT: PROOF_PATH }, stdio: 'inherit' }
    )
  }
  const proofBundle = JSON.parse(readFileSync(PROOF_PATH, 'utf8'))
  const { vk_base64, proof_base64, public_inputs_base64 } = proofBundle
  console.log(`  VK: ${vk_base64.length} chars, Proof: ${proof_base64.length} chars, Inputs: ${public_inputs_base64.length} chars\n`)

  // ── Connect read-only client for queries ──
  const queryClient = await CosmWasmClient.connect(RPC_URL)

  // ── Check VK status ──
  const vkStatus = await queryClient.queryContractSmart(ZK_VERIFIER_ADDR, { vk_status: {} })
  console.log(`  VK stored: ${vkStatus.has_vk}`)

  // ── Connect signing client ──
  let client, address
  const WALLET_ID = process.env.WALLET_ID

  if (WALLET_ID) {
    console.log(`  Wallet: encrypted store (id: "${WALLET_ID}")`)
    const { WalletStore } = await import('../mcp/dist/wallet/store.js')
    const store = WalletStore.defaultStore()
    const chainConfig = {
      chainId: CHAIN_ID,
      chainName: IS_MAINNET ? 'Juno Mainnet' : 'Juno Testnet',
      rpcEndpoint: RPC_URL,
      restEndpoint: IS_MAINNET ? 'https://juno-api.polkachu.com' : 'https://juno-testnet-api.polkachu.com',
      denom: DENOM,
      bech32Prefix: 'juno',
      gasPrice: GAS_PRICE,
      slip44: 118,
      explorerTx: IS_MAINNET ? 'https://mintscan.io/juno/tx' : 'https://testnet.mintscan.io/juno-testnet/tx',
      isTestnet: !IS_MAINNET,
    }
    const ctx = await store.signFor(WALLET_ID, chainConfig)
    client = ctx.client
    address = ctx.address
  } else {
    const { DirectSecp256k1HdWallet } = await import('@cosmjs/proto-signing')
    const { SigningCosmWasmClient } = await import('@cosmjs/cosmwasm-stargate')
    const { GasPrice } = await import('@cosmjs/stargate')
    const MNEMONIC = process.env.JUNO_MNEMONIC
    if (!MNEMONIC) {
      console.error('  Set WALLET_ID or JUNO_MNEMONIC')
      process.exit(1)
    }
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
    const [account] = await wallet.getAccounts()
    address = account.address
    client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
      gasPrice: GasPrice.fromString(GAS_PRICE),
    })
  }

  console.log(`  Deployer: ${address}`)
  const balance = await client.getBalance(address, DENOM)
  console.log(`  Balance:  ${(BigInt(balance.amount) / 1_000_000n).toString()} ${DENOM.replace('u', '').toUpperCase()}\n`)

  if (BigInt(balance.amount) === 0n) {
    console.error(`  Wallet ${address} has 0 ${DENOM} on ${CHAIN_ID}. Fund it before benchmarking.`)
    process.exit(1)
  }

  // ── Store VK if not already stored ──
  if (!vkStatus.has_vk) {
    console.log('  [1/3] Storing VK...')
    const storeResult = await client.execute(address, ZK_VERIFIER_ADDR, { store_vk: { vk_base64 } }, 'auto')
    console.log(`    TX: ${storeResult.transactionHash}`)
    console.log(`    Gas used: ${storeResult.gasUsed}`)
  } else {
    console.log('  [1/3] VK already stored, skipping')
  }

  // ── Verify proof (N samples) ──
  const results = []
  console.log(`\n  [2/3] Verifying proof ${SAMPLES} times...`)
  for (let i = 0; i < SAMPLES; i++) {
    const start = Date.now()
    const result = await client.execute(
      address,
      ZK_VERIFIER_ADDR,
      { verify_proof: { proof_base64, public_inputs_base64 } },
      'auto'
    )
    const elapsed = Date.now() - start
    results.push({
      sample: i + 1,
      tx: result.transactionHash,
      gasUsed: result.gasUsed,
      elapsedMs: elapsed,
    })
    console.log(`    Sample ${i + 1}: gas=${result.gasUsed}, time=${elapsed}ms, tx=${result.transactionHash.slice(0, 16)}...`)
  }

  // ── Query last verify ──
  const lastVerify = await queryClient.queryContractSmart(ZK_VERIFIER_ADDR, { last_verify: {} })
  console.log(`\n  [3/3] Last verify: verified=${lastVerify.verified}, height=${lastVerify.block_height}`)

  // ── Summary ──
  const avgGas = Math.round(results.reduce((s, r) => s + parseInt(r.gasUsed), 0) / results.length)
  const avgTime = Math.round(results.reduce((s, r) => s + r.elapsedMs, 0) / results.length)
  const minGas = Math.min(...results.map(r => parseInt(r.gasUsed)))
  const maxGas = Math.max(...results.map(r => parseInt(r.gasUsed)))

  console.log('\n  --- Benchmark Summary ---\n')
  console.log(`  Chain:      ${CHAIN_ID}`)
  console.log(`  Contract:   ${ZK_VERIFIER_ADDR}`)
  console.log(`  Variant:    pure-Wasm (no BN254 precompile)`)
  console.log(`  Samples:    ${SAMPLES}`)
  console.log(`  Avg gas:    ${avgGas}`)
  console.log(`  Min gas:    ${minGas}`)
  console.log(`  Max gas:    ${maxGas}`)
  console.log(`  Avg time:   ${avgTime}ms`)
  console.log(`  Gas price:  ${GAS_PRICE}`)
  console.log(`  Est cost:   ~${(avgGas * 0.075 / 1_000_000).toFixed(4)} ${DENOM}\n`)

  // ── Write report ──
  const outPath = join(REPO_ROOT, 'docs', IS_MAINNET ? 'BN254_BENCHMARK_MAINNET.md' : 'BN254_BENCHMARK_TESTNET.md')
  const report = `# ZK-Verifier Benchmark Results (${IS_MAINNET ? 'Mainnet' : 'Testnet'})

> Chain: ${CHAIN_ID} | Contract: ${ZK_VERIFIER_ADDR} | Date: ${new Date().toISOString()}

## Configuration

| Parameter | Value |
|-----------|-------|
| Gas price | ${GAS_PRICE} |
| Samples | ${SAMPLES} |
| Variant | pure-Wasm (no BN254 precompile) |
| VK size | ${vk_base64.length} chars (base64) |
| Proof size | ${proof_base64.length} chars (base64) |

## Results

| Sample | Gas Used | Time (ms) | TX Hash |
|--------|----------|-----------|---------|
${results.map(r => `| ${r.sample} | ${r.gasUsed} | ${r.elapsedMs} | ${r.tx} |`).join('\n')}

## Summary

- **Average gas**: ${avgGas}
- **Min gas**: ${minGas}
- **Max gas**: ${maxGas}
- **Average time**: ${avgTime}ms
- **Est. cost per verify**: ~${(avgGas * 0.075 / 1_000_000).toFixed(4)} ${DENOM}

## Context

- Pure-Wasm Groth16 verifier (no BN254 host-function dependency)
- Baseline for comparison against BN254 precompile variant (Track B, post v30.1 upgrade)
- Expected precompile gas: ~203,000 (1.82x reduction from devnet benchmarks)
`

  writeFileSync(outPath, report)
  console.log(`  Report: ${outPath}\n`)

  process.exit(0)
}

main().catch((err) => {
  console.error('\n  Benchmark failed:', err.message)
  process.exit(1)
})
