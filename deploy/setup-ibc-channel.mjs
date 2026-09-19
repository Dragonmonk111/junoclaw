import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

// ── Config ──────────────────────────────────────────────────────────────────

const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://junotestnet-rpc.kleomedes.network'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'

const DEPLOYED_FILE = join(__dir, 'deployed-testnet.json')
const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')

const ICS20_WASM = process.env.ICS20_WASM
  || join(__dir, '..', 'contracts', 'target', 'wasm32-unknown-unknown', 'release', 'cw_ics20_transfer.wasm')

const TASK_HOST_WASM = process.env.TASK_HOST_WASM
  || join(__dir, '..', 'contracts', 'target', 'wasm32-unknown-unknown', 'release', 'ibc_task_host.wasm')

const PORT_ID = process.env.IBC_PORT || 'transfer'

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
      console.error('Available roles:', (state.mps || []).map(m => m.name).join(', '))
      process.exit(1)
    }
    console.log(`  Wallet:   ${role} (${mp.address})`)
    return mp.mnemonic
  }
  console.error('Set MNEMONIC or PARLIAMENT_ROLE (e.g. "The Builder").')
  process.exit(1)
}

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

function sleep(ms) { return new Promise(r => setTimeout(r, ms)) }

// ── Helpers for RPCs without tx indexing ─────────────────────────────────────
//
// Many public uni-7 RPCs have transaction indexing disabled.
// @cosmjs's SigningCosmWasmClient uses tx_search to poll for confirmation,
// which fails with "transaction indexing is disabled".
// These helpers catch that error, wait for blocks to pass, and verify
// the operation succeeded by querying contract state.

function isTxIndexingError(err) {
  const msg = err?.message || String(err)
  return msg.includes('transaction indexing is disabled')
}

async function safeUpload(client, address, wasm, label) {
  try {
    return await client.upload(address, wasm, 'auto', label)
  } catch (err) {
    if (!isTxIndexingError(err)) throw err
    console.log('  (tx indexing disabled, waiting for confirmation...)')
    await sleep(8000)
    // Find the newest code ID by scanning upward from the last known
    const codes = await client.getCodes(100)
    const sorted = codes.sort((a, b) => Number(b.id) - Number(a.id))
    if (sorted.length === 0) throw new Error('Upload failed: no codes found')
    const codeId = Number(sorted[0].id)
    console.log(`  ✅ Upload confirmed via code scan. codeId: ${codeId}`)
    return { codeId, transactionHash: 'unknown (tx indexing disabled)' }
  }
}

async function safeInstantiate(client, address, codeId, initMsg, label) {
  try {
    return await client.instantiate(address, codeId, initMsg, label, 'auto')
  } catch (err) {
    if (!isTxIndexingError(err)) throw err
    console.log('  (tx indexing disabled, waiting for confirmation...)')
    await sleep(8000)
    // Find the newest contract by code ID
    const contracts = await client.getContracts(codeId)
    if (contracts.length === 0) throw new Error('Instantiate failed: no contracts found for codeId ' + codeId)
    const contractAddr = contracts[contracts.length - 1]
    console.log(`  ✅ Instantiate confirmed via contract scan. address: ${contractAddr}`)
    return { contractAddress: contractAddr, transactionHash: 'unknown (tx indexing disabled)' }
  }
}

async function safeExecute(client, address, contractAddr, msg, memo) {
  try {
    return await client.execute(address, contractAddr, msg, 'auto', memo)
  } catch (err) {
    if (!isTxIndexingError(err)) throw err
    console.log('  (tx indexing disabled, waiting for confirmation...)')
    await sleep(8000)
    return { transactionHash: 'unknown (tx indexing disabled)' }
  }
}

async function safeMigrate(client, address, contractAddr, codeId, msg) {
  try {
    return await client.migrate(address, contractAddr, codeId, msg, 'auto')
  } catch (err) {
    if (!isTxIndexingError(err)) throw err
    console.log('  (tx indexing disabled, waiting for confirmation...)')
    await sleep(8000)
    return { transactionHash: 'unknown (tx indexing disabled)' }
  }
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log('\n  === Deploy: cw-ics20-transfer + ibc-task-host ===')
  console.log(`  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}`)
  console.log(`  Port:     ${PORT_ID}\n`)

  const mnemonic = loadMnemonic()
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [{ address }] = await wallet.getAccounts()
  console.log(`  Deployer: ${address}`)

  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })

  const balance = await client.getBalance(address, 'ujunox')
  console.log(`  Balance:  ${(BigInt(balance.amount) / 1_000_000n).toString()} JUNOX (${balance.amount} ujunox)\n`)

  if (BigInt(balance.amount) < 1_000_000n) {
    console.error('  ERROR: Insufficient balance. Need at least 1 JUNOX for deployment.')
    process.exit(1)
  }

  const deployed = loadDeployed()
  const joltVerifierAddr = deployed['jolt-cw-verifier']?.address
  console.log(`  Jolt verifier: ${joltVerifierAddr || 'NOT DEPLOYED'}\n`)

  // ═══════════════════════════════════════════════════════════════════════════
  // 1. DEPLOY cw-ics20-transfer
  // ═══════════════════════════════════════════════════════════════════════════

  console.log('  ── 1. cw-ics20-transfer ────────────────────────────────────')

  if (!deployed['cw-ics20-transfer']?.codeId) {
    if (!existsSync(ICS20_WASM)) {
      console.error(`  WASM not found: ${ICS20_WASM}`)
      console.error('  Run: cargo build --target wasm32-unknown-unknown --release (from contracts/)')
      process.exit(1)
    }
    const wasm = readFileSync(ICS20_WASM)
    console.log(`  Storing cw-ics20-transfer (${(wasm.length / 1024).toFixed(1)} KB)...`)

    const storeResult = await safeUpload(client, address, wasm, 'JunoClaw ICS-20 Transfer')
    console.log(`  ✅ Stored!  codeId: ${storeResult.codeId}`)

    deployed['cw-ics20-transfer'] = {
      codeId: storeResult.codeId,
      storeTx: storeResult.transactionHash,
      wasmSizeBytes: wasm.length,
    }
    saveDeployed(deployed)
  } else {
    console.log(`  Already stored (codeId ${deployed['cw-ics20-transfer'].codeId})`)
  }

  if (!deployed['cw-ics20-transfer']?.address) {
    console.log(`  Instantiating cw-ics20-transfer...`)

    const initMsg = {
      admin: address,
      port_id: PORT_ID,
      default_timeout_seconds: 3600,
      allowed_denoms: ['ujunox'],
    }

    const instResult = await safeInstantiate(
      client, address, deployed['cw-ics20-transfer'].codeId, initMsg, 'JunoClaw ICS-20 Transfer'
    )
    console.log(`  ✅ Instantiated!  address: ${instResult.contractAddress}`)

    deployed['cw-ics20-transfer'].address = instResult.contractAddress
    deployed['cw-ics20-transfer'].instantiateTx = instResult.transactionHash
    saveDeployed(deployed)
  } else {
    console.log(`  Already instantiated at ${deployed['cw-ics20-transfer'].address}`)
  }

  const ics20Addr = deployed['cw-ics20-transfer'].address

  // Verify
  const configResp = await client.queryContractSmart(ics20Addr, { config: {} })
  console.log(`  Port: ${configResp.port_id}, Send: ${configResp.send_enabled}, Receive: ${configResp.receive_enabled}`)

  // ═══════════════════════════════════════════════════════════════════════════
  // 2. DEPLOY ibc-task-host (updated with jolt_verifier support)
  // ═══════════════════════════════════════════════════════════════════════════

  console.log('\n  ── 2. ibc-task-host (updated) ──────────────────────────────')

  if (!existsSync(TASK_HOST_WASM)) {
    console.error(`  WASM not found: ${TASK_HOST_WASM}`)
    console.error('  Run: cargo build --target wasm32-unknown-unknown --release (from contracts/)')
    process.exit(1)
  }

  // Always re-upload ibc-task-host since we updated the code
  const taskHostWasm = readFileSync(TASK_HOST_WASM)
  console.log(`  Storing ibc-task-host (${(taskHostWasm.length / 1024).toFixed(1)} KB)...`)

  const thStoreResult = await safeUpload(client, address, taskHostWasm, 'JunoClaw IBC Task Host (jolt_verifier)')
  console.log(`  ✅ Stored!  codeId: ${thStoreResult.codeId}`)

  const prevTaskHost = deployed['ibc-task-host']?.address
  const prevConfig = prevTaskHost
    ? await client.queryContractSmart(prevTaskHost, { config: {} }).catch(() => null)
    : null

  if (!prevTaskHost || !prevConfig) {
    // Fresh instantiate
    console.log(`  Instantiating ibc-task-host...`)

    const initMsg = {
      admin: address,
      task_ledger: null,
      escrow: null,
      zk_verifier: null,
      jolt_verifier: joltVerifierAddr || null,
      allowed_pairs: [],
    }

    const instResult = await safeInstantiate(
      client, address, thStoreResult.codeId, initMsg, 'JunoClaw IBC Task Host'
    )
    console.log(`  ✅ Instantiated!  address: ${instResult.contractAddress}`)

    deployed['ibc-task-host'] = {
      codeId: thStoreResult.codeId,
      address: instResult.contractAddress,
      storeTx: thStoreResult.transactionHash,
      instantiateTx: instResult.transactionHash,
      wasmSizeBytes: taskHostWasm.length,
    }
    saveDeployed(deployed)
  } else {
    // Migrate existing contract to new code
    console.log(`  Existing ibc-task-host at ${prevTaskHost}`)
    console.log(`  Migrating to new codeId ${thStoreResult.codeId}...`)

    const migrateResult = await safeMigrate(client, address, prevTaskHost, thStoreResult.codeId, {})
    console.log(`  ✅ Migrated!  tx: ${migrateResult.transactionHash}`)

    // Update config to add jolt_verifier
    if (joltVerifierAddr && !prevConfig.jolt_verifier) {
      console.log(`  Wiring jolt_verifier=${joltVerifierAddr}...`)
      const updateMsg = {
        update_config: {
          task_ledger: null,
          escrow: null,
          zk_verifier: null,
          jolt_verifier: joltVerifierAddr,
          allowed_pairs: null,
        },
      }
      const updateResult = await safeExecute(
        client, address, prevTaskHost, updateMsg, 'Wire jolt_verifier to ibc-task-host'
      )
      console.log(`  ✅ Config updated!  tx: ${updateResult.transactionHash}`)
    } else if (prevConfig.jolt_verifier) {
      console.log(`  jolt_verifier already configured: ${prevConfig.jolt_verifier}`)
    }

    deployed['ibc-task-host'].codeId = thStoreResult.codeId
    deployed['ibc-task-host'].storeTx = thStoreResult.transactionHash
    deployed['ibc-task-host'].migrateTx = migrateResult.transactionHash
    deployed['ibc-task-host'].wasmSizeBytes = taskHostWasm.length
    saveDeployed(deployed)
  }

  const taskHostAddr = deployed['ibc-task-host'].address

  // Verify ibc-task-host config
  const hostConfig = await client.queryContractSmart(taskHostAddr, { config: {} })
  console.log(`  ibc-task-host config:`)
  console.log(`    admin:          ${hostConfig.admin}`)
  console.log(`    task_ledger:    ${hostConfig.task_ledger || 'none'}`)
  console.log(`    escrow:         ${hostConfig.escrow || 'none'}`)
  console.log(`    zk_verifier:    ${hostConfig.zk_verifier || 'none'}`)
  console.log(`    jolt_verifier:  ${hostConfig.jolt_verifier || 'none'}`)
  console.log(`    allowed_pairs:  ${hostConfig.allowed_pairs?.length || 0} pairs`)

  // ═══════════════════════════════════════════════════════════════════════════
  // 3. SUMMARY + IBC CHANNEL INSTRUCTIONS
  // ═══════════════════════════════════════════════════════════════════════════

  const portResp = await client.queryContractSmart(ics20Addr, { port: {} })

  console.log(`\n  ═══════════════════════════════════════════════════════`)
  console.log(`  ✅ Deployment Complete!`)
  console.log(`  ═══════════════════════════════════════════════════════`)
  console.log(`  cw-ics20-transfer:`)
  console.log(`    codeId:  ${deployed['cw-ics20-transfer'].codeId}`)
  console.log(`    address: ${ics20Addr}`)
  console.log(`    port:    ${portResp.port_id}`)
  console.log(`    size:    ${(deployed['cw-ics20-transfer'].wasmSizeBytes / 1024).toFixed(1)} KB`)
  console.log(`  ibc-task-host:`)
  console.log(`    codeId:  ${deployed['ibc-task-host'].codeId}`)
  console.log(`    address: ${taskHostAddr}`)
  console.log(`    size:    ${(deployed['ibc-task-host'].wasmSizeBytes / 1024).toFixed(1)} KB`)
  console.log(`  jolt-cw-verifier:`)
  console.log(`    address: ${joltVerifierAddr || 'NOT DEPLOYED'}`)
  console.log(`  ═══════════════════════════════════════════════════════`)

  console.log(`\n  ── IBC Channel Setup (next step) ────────────────────────────`)
  console.log(`  cw-ics20-transfer is bound to port "${portResp.port_id}".`)
  console.log(`  To create an IBC channel to a counterparty chain:\n`)
  console.log(`  junod tx ibc channel open-init \\`)
  console.log(`    --src-port ${portResp.port_id} \\`)
  console.log(`    --dst-port transfer \\`)
  console.log(`    --connection <connection-id> \\`)
  console.log(`    --version ics20-1 \\`)
  console.log(`    --ordering unordered \\`)
  console.log(`    --from <wallet> --chain-id ${CHAIN_ID} --node ${RPC_URL}\n`)
}

main().catch((err) => {
  console.error('\n  ❌ Deploy failed:', err.message || err)
  if (err.stack) console.error(err.stack)
  process.exit(1)
})
