import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'
import 'dotenv/config'

const __dir = dirname(fileURLToPath(import.meta.url))

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

async function main() {
  console.log('\n  Deploy: emergency-compute-escrow to uni-7')
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

  // ── emergency-compute-escrow ──────────────────────────────────────────────

  if (!deployed['emergency-compute-escrow']?.code_id) {
    const wasmPath = join(ARTIFACTS_DIR, 'emergency_compute_escrow.wasm')
    if (!existsSync(wasmPath)) {
      console.error(`  emergency_compute_escrow.wasm not found at ${wasmPath}`)
      process.exit(1)
    }
    const wasm = readFileSync(wasmPath)
    console.log(`  Storing emergency-compute-escrow (${(wasm.length / 1024).toFixed(1)} KB)...`)
    const result = await client.upload(address, wasm, 'auto', 'JunoClaw Emergency Compute Escrow')
    console.log(`  code_id: ${result.codeId}  tx: ${result.transactionHash}`)
    deployed['emergency-compute-escrow'] = {
      code_id: result.codeId,
      store_tx: result.transactionHash,
      wasm_file: 'emergency_compute_escrow.wasm',
    }
    saveDeployed(deployed)
  } else {
    console.log(`  emergency-compute-escrow already stored (code_id ${deployed['emergency-compute-escrow'].code_id})`)
  }

  if (!deployed['emergency-compute-escrow']?.address && deployed['emergency-compute-escrow']?.code_id) {
    console.log(`  Instantiating emergency-compute-escrow...`)
    const msg = {
      admin: null,
      denom: DENOM,
      max_cost_per_lease: '50000000',
      min_timeout_secs: 30,
      max_timeout_secs: 3600,
      moultbook: deployed['moultbook-v0']?.address || deployed['moultbook']?.address || null,
      task_ledger: deployed['task-ledger']?.address || null,
    }
    const res = await client.instantiate(
      address, deployed['emergency-compute-escrow'].code_id, msg,
      'JunoClaw Emergency Compute Escrow', 'auto', { admin: address }
    )
    console.log(`  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
    deployed['emergency-compute-escrow'].address = res.contractAddress
    deployed['emergency-compute-escrow'].instantiate_tx = res.transactionHash
    saveDeployed(deployed)
  } else if (deployed['emergency-compute-escrow']?.address) {
    console.log(`  emergency-compute-escrow already instantiated: ${deployed['emergency-compute-escrow'].address}`)
  }

  // ── Summary ───────────────────────────────────────────────────────────────

  console.log('\n  --- Deployment Summary ---\n')
  const info = deployed['emergency-compute-escrow']
  if (info) {
    console.log(`  emergency-compute-escrow`)
    if (info.code_id) console.log(`    code_id:  ${info.code_id}`)
    if (info.address) console.log(`    address:  ${info.address}`)
  }
  console.log('')

  process.exit(0)
}

main().catch((err) => {
  console.error('\n  FAILED:', err.message || err)
  process.exit(1)
})
