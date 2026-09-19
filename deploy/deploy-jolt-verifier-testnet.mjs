import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

// ── Config ──────────────────────────────────────────────────────────────────

const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://juno-testnet-rpc.polkachu.com'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'

const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')
const WASM_PATH = process.env.WASM_PATH
  || 'C:\\Temp\\jolt\\target\\wasm32-unknown-unknown\\release\\jolt_cw_verifier_opt.wasm'

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
  console.error('Set MNEMONIC or PARLIAMENT_ROLE (e.g. "The Builder").')
  process.exit(1)
}

const MNEMONIC = loadMnemonic()
const DEPLOYED_FILE = join(__dir, 'deployed-testnet.json')

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
  console.log('\n  === Deploy: jolt-cw-verifier to uni-7 ===')
  console.log(`  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}`)
  console.log(`  WASM:     ${WASM_PATH}\n`)

  if (!existsSync(WASM_PATH)) {
    console.error(`  WASM file not found: ${WASM_PATH}`)
    process.exit(1)
  }

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
  const [{ address }] = await wallet.getAccounts()
  console.log(`  Deployer: ${address}`)

  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })

  const balance = await client.getBalance(address, 'ujunox')
  console.log(`  Balance:  ${(BigInt(balance.amount) / 1_000_000n).toString()} JUNOX (${balance.amount} ujunox)\n`)

  if (BigInt(balance.amount) < 500_000n) {
    console.error('  ERROR: Insufficient balance. Need at least 0.5 JUNOX for deployment.')
    process.exit(1)
  }

  const deployed = loadDeployed()

  // ── 1. Store code ──────────────────────────────────────────────────────────

  if (!deployed['jolt-cw-verifier']?.codeId) {
    const wasm = readFileSync(WASM_PATH)
    console.log(`  Storing jolt-cw-verifier (${(wasm.length / 1024).toFixed(1)} KB)...`)

    const storeResult = await client.upload(
      address,
      wasm,
      'auto',
      'JunoClaw Lattice Jolt CosmWasm Verifier'
    )
    console.log(`  ✅ Stored!  codeId: ${storeResult.codeId}`)
    console.log(`     tx: ${storeResult.transactionHash}`)

    deployed['jolt-cw-verifier'] = {
      codeId: storeResult.codeId,
      storeTx: storeResult.transactionHash,
      wasmFile: 'jolt_cw_verifier_opt.wasm',
      wasmSizeBytes: wasm.length,
    }
    saveDeployed(deployed)
  } else {
    console.log(`  jolt-cw-verifier already stored (codeId ${deployed['jolt-cw-verifier'].codeId})`)
  }

  // ── 2. Instantiate ─────────────────────────────────────────────────────────

  if (!deployed['jolt-cw-verifier']?.address) {
    console.log(`  Instantiating jolt-cw-verifier...`)

    const initMsg = {
      admin: address,
    }

    const instResult = await client.instantiate(
      address,
      deployed['jolt-cw-verifier'].codeId,
      initMsg,
      'JunoClaw Lattice Jolt Verifier',
      'auto'
    )
    console.log(`  ✅ Instantiated!  address: ${instResult.contractAddress}`)
    console.log(`     tx: ${instResult.transactionHash}`)

    deployed['jolt-cw-verifier'].address = instResult.contractAddress
    deployed['jolt-cw-verifier'].instantiateTx = instResult.transactionHash
    saveDeployed(deployed)
  } else {
    console.log(`  jolt-cw-verifier already instantiated at ${deployed['jolt-cw-verifier'].address}`)
  }

  // ── 3. Verify deployment ────────────────────────────────────────────────────

  console.log(`\n  Verifying deployment...`)
  const contractAddr = deployed['jolt-cw-verifier'].address

  // Query admin
  const adminResp = await client.queryContractSmart(contractAddr, { admin: {} })
  console.log(`  Admin: ${adminResp.admin}`)

  // Query proof status (should be empty)
  const statusResp = await client.queryContractSmart(contractAddr, { proof_status: {} })
  console.log(`  Proof stored: ${statusResp.has_proof}`)
  console.log(`  Last verified: ${statusResp.last_verify_verified}`)

  console.log(`\n  ═══════════════════════════════════════════════════════`)
  console.log(`  ✅ jolt-cw-verifier deployed to uni-7!`)
  console.log(`     codeId:  ${deployed['jolt-cw-verifier'].codeId}`)
  console.log(`     address: ${contractAddr}`)
  console.log(`     wasm:    ${(deployed['jolt-cw-verifier'].wasmSizeBytes / 1024).toFixed(1)} KB`)
  console.log(`  ═══════════════════════════════════════════════════════\n`)
}

main().catch((err) => {
  console.error('\n  ❌ Deployment failed:', err.message || err)
  if (err.stack) console.error(err.stack)
  process.exit(1)
})
