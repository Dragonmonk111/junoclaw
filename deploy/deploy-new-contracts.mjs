import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'
import 'dotenv/config'

const __dir = dirname(fileURLToPath(import.meta.url))

// ── Config ──────────────────────────────────────────────────────────────────

const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://juno.rpc.t.stavr.tech'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'

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
  console.error('Set MNEMONIC or PARLIAMENT_ROLE (e.g. "The Builder"). See .env.example.')
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
  console.log('\n  Deploy: moultbook-v0 + ibc-task-host to uni-7')
  console.log(`  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}\n`)

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
  const [{ address }] = await wallet.getAccounts()
  console.log(`  Deployer: ${address}`)

  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })

  const balance = await client.getBalance(address, 'ujunox')
  console.log(`  Balance:  ${(BigInt(balance.amount) / 1_000_000n).toString()} JUNOX\n`)

  const deployed = loadDeployed()
  const existing = deployed

  // ── moultbook-v0 ──────────────────────────────────────────────────────────

  if (!existing['moultbook-v0']?.code_id) {
    const wasmPath = join(ARTIFACTS_DIR, 'moultbook_v0.wasm')
    if (!existsSync(wasmPath)) {
      console.error(`  moultbook_v0.wasm not found at ${wasmPath}`)
      process.exit(1)
    }
    const wasm = readFileSync(wasmPath)
    console.log(`  Storing moultbook-v0 (${(wasm.length / 1024).toFixed(1)} KB)...`)
    const result = await client.upload(address, wasm, 'auto', 'JunoClaw Moultbook v0')
    console.log(`  code_id: ${result.codeId}  tx: ${result.transactionHash}`)
    deployed['moultbook-v0'] = {
      code_id: result.codeId,
      store_tx: result.transactionHash,
      wasm_file: 'moultbook_v0.wasm',
    }
    saveDeployed(deployed)
  } else {
    console.log(`  moultbook-v0 already stored (code_id ${existing['moultbook-v0'].code_id})`)
  }

  if (!existing['moultbook-v0']?.address && deployed['moultbook-v0']?.code_id) {
    const agentRegistryAddr = existing['agent-registry']?.address
    console.log(`  Instantiating moultbook-v0...`)
    const msg = {
      admin: address,
      whoami_contract: null,
      max_size_bytes: 65536,
      max_refs: 8,
      max_content_type_len: 64,
      max_group_size: 32,
      zk_verifier: null,
      agent_registry: agentRegistryAddr || null,
      membership_vk_hash: null,
      entries_per_key_per_epoch: 10,
      epoch_blocks: 100,
    }
    const res = await client.instantiate(
      address, deployed['moultbook-v0'].code_id, msg,
      'JunoClaw Moultbook v0', 'auto', { admin: address }
    )
    console.log(`  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
    deployed['moultbook-v0'].address = res.contractAddress
    deployed['moultbook-v0'].instantiate_tx = res.transactionHash
    saveDeployed(deployed)
  } else if (existing['moultbook-v0']?.address) {
    console.log(`  moultbook-v0 already instantiated: ${existing['moultbook-v0'].address}`)
  }

  // ── ibc-task-host ─────────────────────────────────────────────────────────

  if (!existing['ibc-task-host']?.code_id) {
    const wasmPath = join(ARTIFACTS_DIR, 'ibc_task_host.wasm')
    if (!existsSync(wasmPath)) {
      console.error(`  ibc_task_host.wasm not found at ${wasmPath}`)
      process.exit(1)
    }
    const wasm = readFileSync(wasmPath)
    console.log(`\n  Storing ibc-task-host (${(wasm.length / 1024).toFixed(1)} KB)...`)
    const result = await client.upload(address, wasm, 'auto', 'JunoClaw IBC Task Host')
    console.log(`  code_id: ${result.codeId}  tx: ${result.transactionHash}`)
    deployed['ibc-task-host'] = {
      code_id: result.codeId,
      store_tx: result.transactionHash,
      wasm_file: 'ibc_task_host.wasm',
    }
    saveDeployed(deployed)
  } else {
    console.log(`\n  ibc-task-host already stored (code_id ${existing['ibc-task-host'].code_id})`)
  }

  if (!existing['ibc-task-host']?.address && deployed['ibc-task-host']?.code_id) {
    const taskLedgerAddr = existing['task-ledger']?.address
    const escrowAddr = existing['escrow']?.address
    const zkVerifierAddr = existing['zk-verifier']?.address || null
    console.log(`  Instantiating ibc-task-host...`)
    const msg = {
      admin: address,
      task_ledger: taskLedgerAddr || null,
      escrow: escrowAddr || null,
      zk_verifier: null,
      allowed_pairs: [],
    }
    const res = await client.instantiate(
      address, deployed['ibc-task-host'].code_id, msg,
      'JunoClaw IBC Task Host', 'auto', { admin: address }
    )
    console.log(`  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
    deployed['ibc-task-host'].address = res.contractAddress
    deployed['ibc-task-host'].instantiate_tx = res.transactionHash
    saveDeployed(deployed)
  } else if (existing['ibc-task-host']?.address) {
    console.log(`  ibc-task-host already instantiated: ${existing['ibc-task-host'].address}`)
  }

  // ── Summary ───────────────────────────────────────────────────────────────

  console.log('\n  --- Deployment Summary ---\n')
  for (const name of ['moultbook-v0', 'ibc-task-host']) {
    const info = deployed[name]
    if (info) {
      console.log(`  ${name}`)
      if (info.code_id)  console.log(`    code_id:  ${info.code_id}`)
      if (info.address)  console.log(`    address:  ${info.address}`)
    }
  }
  console.log('')

  process.exit(0)
}

main().catch((err) => {
  console.error('\n  Deploy failed:', err.message)
  process.exit(1)
})
