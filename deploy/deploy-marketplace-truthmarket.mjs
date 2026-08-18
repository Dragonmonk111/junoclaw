import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'
import 'dotenv/config'

const __dir = dirname(fileURLToPath(import.meta.url))

// ── Config ──────────────────────────────────────────────────────────────────
// Mirrors deploy-new-contracts.mjs — stores + instantiates truth-market and
// marketplace on uni-7, wiring marketplace's truth_market/task_ledger fields
// to the freshly-deployed truth-market address and the already-deployed
// task-ledger address in deployed.json.

const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://juno.rpc.t.stavr.tech'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'
const DENOM     = process.env.DENOM     || 'ujunox'

const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')

function loadMnemonic() {
  if (process.env.MNEMONIC) return process.env.MNEMONIC
  if (process.env.PARLIAMENT_ROLE) {
    if (!existsSync(PARLIAMENT_STATE)) {
      console.error(`PARLIAMENT_ROLE set but ${PARLIAMENT_STATE} not found`)
      process.exit(1)
    }
    const state = JSON.parse(readFileSync(PARLIAMENT_STATE, 'utf8'))
    const role = process.env.PARLIAMENT_ROLE
    const mp = (state.mps || []).find((m) => m.name === role)
    if (!mp) {
      console.error(`No MP with name "${role}" in parliament-state.json`)
      process.exit(1)
    }
    console.log(`  Wallet:   ${role} (${mp.address})`)
    return mp.mnemonic
  }
  console.error('Set MNEMONIC or PARLIAMENT_ROLE in deploy/.env (copy from .env.example).')
  process.exit(1)
}

const MNEMONIC = loadMnemonic()

const ARTIFACTS_DIR = process.env.ARTIFACTS_DIR
  || 'C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release'

const DEPLOYED_FILE = join(__dir, 'deployed.json')

function loadDeployed() {
  if (existsSync(DEPLOYED_FILE)) {
    return JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))
  }
  return {}
}

function saveDeployed(data) {
  writeFileSync(DEPLOYED_FILE, JSON.stringify(data, null, 2))
  console.log(`  Saved to ${DEPLOYED_FILE}`)
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log('\n  Deploy: truth-market + marketplace to uni-7')
  console.log(`  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}\n`)

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
  const [{ address }] = await wallet.getAccounts()
  console.log(`  Deployer: ${address}`)

  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })

  const balance = await client.getBalance(address, DENOM)
  console.log(`  Balance:  ${(BigInt(balance.amount) / 1_000_000n).toString()} JUNOX\n`)

  const deployed = loadDeployed()

  const taskLedgerAddr = deployed['task-ledger']?.address
  if (!taskLedgerAddr) {
    console.error('  task-ledger not found in deployed.json — deploy it first (see deploy.mjs).')
    process.exit(1)
  }
  console.log(`  Using task-ledger: ${taskLedgerAddr}`)

  // ── truth-market ────────────────────────────────────────────────────────

  if (!deployed['truth-market']?.code_id) {
    const wasmPath = join(ARTIFACTS_DIR, 'truth_market.wasm')
    if (!existsSync(wasmPath)) {
      console.error(`  truth_market.wasm not found at ${wasmPath}`)
      process.exit(1)
    }
    const wasm = readFileSync(wasmPath)
    console.log(`  Storing truth-market (${(wasm.length / 1024).toFixed(1)} KB)...`)
    const result = await client.upload(address, wasm, 'auto', 'JunoClaw Truth Market')
    console.log(`  code_id: ${result.codeId}  tx: ${result.transactionHash}`)
    deployed['truth-market'] = {
      code_id: result.codeId,
      store_tx: result.transactionHash,
      wasm_file: 'truth_market.wasm',
    }
    saveDeployed(deployed)
  } else {
    console.log(`  truth-market already stored (code_id ${deployed['truth-market'].code_id})`)
  }

  if (!deployed['truth-market']?.address && deployed['truth-market']?.code_id) {
    console.log(`  Instantiating truth-market...`)
    const msg = {
      min_stake: '1000000',
      slash_percent: 10,
      reward_percent: 5,
      denom: DENOM,
      unstake_cooldown_secs: 86400,
    }
    const res = await client.instantiate(
      address, deployed['truth-market'].code_id, msg,
      'JunoClaw Truth Market', 'auto', { admin: address }
    )
    console.log(`  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
    deployed['truth-market'].address = res.contractAddress
    deployed['truth-market'].instantiate_tx = res.transactionHash
    saveDeployed(deployed)
  } else if (deployed['truth-market']?.address) {
    console.log(`  truth-market already instantiated: ${deployed['truth-market'].address}`)
  }

  const truthMarketAddr = deployed['truth-market']?.address

  // ── marketplace ───────────────────────────────────────────────────────────

  if (!deployed['marketplace']?.code_id) {
    const wasmPath = join(ARTIFACTS_DIR, 'marketplace.wasm')
    if (!existsSync(wasmPath)) {
      console.error(`  marketplace.wasm not found at ${wasmPath}`)
      process.exit(1)
    }
    const wasm = readFileSync(wasmPath)
    console.log(`\n  Storing marketplace (${(wasm.length / 1024).toFixed(1)} KB)...`)
    const result = await client.upload(address, wasm, 'auto', 'JunoClaw Marketplace')
    console.log(`  code_id: ${result.codeId}  tx: ${result.transactionHash}`)
    deployed['marketplace'] = {
      code_id: result.codeId,
      store_tx: result.transactionHash,
      wasm_file: 'marketplace.wasm',
    }
    saveDeployed(deployed)
  } else {
    console.log(`\n  marketplace already stored (code_id ${deployed['marketplace'].code_id})`)
  }

  if (!deployed['marketplace']?.address && deployed['marketplace']?.code_id) {
    if (!truthMarketAddr) {
      console.error('  Cannot instantiate marketplace: truth-market has no address yet.')
      process.exit(1)
    }
    console.log(`  Instantiating marketplace...`)
    const msg = {
      admin: null,
      truth_market: truthMarketAddr,
      task_ledger: taskLedgerAddr,
      skill_registry: null,
      denom: DENOM,
      cancel_window_secs: 3600,
    }
    const res = await client.instantiate(
      address, deployed['marketplace'].code_id, msg,
      'JunoClaw Marketplace', 'auto', { admin: address }
    )
    console.log(`  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
    deployed['marketplace'].address = res.contractAddress
    deployed['marketplace'].instantiate_tx = res.transactionHash
    saveDeployed(deployed)
  } else if (deployed['marketplace']?.address) {
    console.log(`  marketplace already instantiated: ${deployed['marketplace'].address}`)
  }

  // ── Summary ───────────────────────────────────────────────────────────────

  console.log('\n  --- Deployment Summary ---\n')
  for (const name of ['truth-market', 'marketplace']) {
    const info = deployed[name]
    if (info) {
      console.log(`  ${name}`)
      if (info.code_id) console.log(`    code_id:  ${info.code_id}`)
      if (info.address) console.log(`    address:  ${info.address}`)
    }
  }
  console.log('')

  process.exit(0)
}

main().catch((err) => {
  console.error('\n  FAILED:', err.message || err)
  process.exit(1)
})
