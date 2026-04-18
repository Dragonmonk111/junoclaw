import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice, coins } from '@cosmjs/stargate'
import 'dotenv/config'

const __dir = dirname(fileURLToPath(import.meta.url))

// ── Config ──────────────────────────────────────────────────────────────────

const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://juno-testnet-rpc.polkachu.com'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'
const MNEMONIC  = process.env.MNEMONIC
const AUTO      = process.env.AUTO_CONFIRM === 'true'

if (!MNEMONIC) {
  console.error('❌  MNEMONIC not set. Copy .env.example → .env and fill it in.')
  process.exit(1)
}

// Wasm file locations — copy built artifacts here, or set ARTIFACTS_DIR env var
const ARTIFACTS_DIR = process.env.ARTIFACTS_DIR
  || 'C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release'

const CONTRACTS = [
  { name: 'agent-registry', wasm: 'agent_registry_opt.wasm' },
  { name: 'task-ledger',    wasm: 'task_ledger_opt.wasm' },
  { name: 'escrow',         wasm: 'escrow_opt.wasm' },
  { name: 'agent-company',  wasm: 'agent_company_opt.wasm' },
]

const DEPLOYED_FILE = join(__dir, 'deployed.json')

// ── Helpers ──────────────────────────────────────────────────────────────────

function loadDeployed() {
  if (existsSync(DEPLOYED_FILE)) {
    return JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))
  }
  return {}
}

function saveDeployed(data) {
  writeFileSync(DEPLOYED_FILE, JSON.stringify(data, null, 2))
  console.log(`\n💾  Saved to ${DEPLOYED_FILE}`)
}

async function confirm(prompt) {
  if (AUTO) return true
  process.stdout.write(`\n${prompt} [y/N] `)
  return new Promise((resolve) => {
    process.stdin.resume()
    process.stdin.setEncoding('utf8')
    process.stdin.once('data', (data) => {
      process.stdin.pause()
      resolve(data.trim().toLowerCase() === 'y')
    })
  })
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log('\n╔══════════════════════════════════════════╗')
  console.log('║       JunoClaw Contract Deployer         ║')
  console.log('╚══════════════════════════════════════════╝')
  console.log(`\n  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}`)
  console.log(`  Gas:      ${GAS_PRICE}`)
  console.log(`  Artifacts: ${ARTIFACTS_DIR}\n`)

  // Connect
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
  const [{ address }] = await wallet.getAccounts()
  console.log(`  Deployer: ${address}`)

  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })

  const balance = await client.getBalance(address, 'ujunox')
  console.log(`  Balance:  ${balance.amount} ${balance.denom}\n`)

  if (BigInt(balance.amount) < 5_000_000n) {
    console.warn('⚠  Low balance — get testnet tokens from https://faucet.reece.sh/?chain=uni-7')
  }

  const deployed = loadDeployed()

  // ── STEP 1: Store all contracts ──────────────────────────────────────────

  console.log('━━━  Step 1: Store WASM artifacts  ━━━\n')

  for (const contract of CONTRACTS) {
    const wasmPath = join(ARTIFACTS_DIR, contract.wasm)
    if (!existsSync(wasmPath)) {
      console.error(`❌  Not found: ${wasmPath}`)
      console.error(`    Build WASMs first: cd contracts && cargo build --release --target wasm32-unknown-unknown`)
      process.exit(1)
    }

    if (deployed[contract.name]?.code_id) {
      console.log(`  ⏭  ${contract.name} already stored (code_id ${deployed[contract.name].code_id})`)
      continue
    }

    const ok = await confirm(`  Store ${contract.name}?`)
    if (!ok) { console.log('  Skipped.'); continue }

    const wasm = readFileSync(wasmPath)
    console.log(`  📦 Storing ${contract.name} (${(wasm.length / 1024).toFixed(1)} KB)…`)

    const result = await client.upload(address, wasm, 'auto', `JunoClaw ${contract.name}`)
    console.log(`  ✅  code_id: ${result.codeId}  tx: ${result.transactionHash}`)

    deployed[contract.name] = { code_id: result.codeId, store_tx: result.transactionHash }
    saveDeployed(deployed)
  }

  // ── STEP 2: Instantiate agent-registry ──────────────────────────────────

  console.log('\n━━━  Step 2: Instantiate agent-registry  ━━━\n')

  if (!deployed['agent-registry']?.address) {
    const codeId = deployed['agent-registry']?.code_id
    if (!codeId) { console.log('  ⏭  Skipping — not stored yet'); }
    else {
      const ok = await confirm('  Instantiate agent-registry?')
      if (ok) {
        const msg = {
          admin: address,
          max_agents: 1000,
          registration_fee_ujuno: '0',
          denom: 'ujunox',
        }
        const res = await client.instantiate(address, codeId, msg, 'JunoClaw Agent Registry', 'auto')
        console.log(`  ✅  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
        deployed['agent-registry'].address = res.contractAddress
        deployed['agent-registry'].instantiate_tx = res.transactionHash
        saveDeployed(deployed)
      }
    }
  } else {
    console.log(`  ⏭  Already instantiated: ${deployed['agent-registry'].address}`)
  }

  // ── STEP 3: Instantiate escrow ───────────────────────────────────────────

  console.log('\n━━━  Step 3: Instantiate escrow  ━━━\n')

  if (!deployed['escrow']?.address) {
    const codeId = deployed['escrow']?.code_id
    const taskLedgerAddr = deployed['task-ledger']?.address || address  // placeholder if not yet deployed
    if (!codeId) { console.log('  ⏭  Skipping — not stored yet') }
    else {
      const ok = await confirm('  Instantiate escrow?')
      if (ok) {
        const msg = {
          admin: address,
          task_ledger: taskLedgerAddr,
          timeout_blocks: 1000,
          denom: 'ujunox',
        }
        const res = await client.instantiate(address, codeId, msg, 'JunoClaw Escrow', 'auto')
        console.log(`  ✅  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
        deployed['escrow'].address = res.contractAddress
        deployed['escrow'].instantiate_tx = res.transactionHash
        saveDeployed(deployed)
      }
    }
  } else {
    console.log(`  ⏭  Already instantiated: ${deployed['escrow'].address}`)
  }

  // ── STEP 4: Instantiate task-ledger ─────────────────────────────────────

  console.log('\n━━━  Step 4: Instantiate task-ledger  ━━━\n')

  if (!deployed['task-ledger']?.address) {
    const codeId = deployed['task-ledger']?.code_id
    const registryAddr = deployed['agent-registry']?.address
    if (!codeId || !registryAddr) {
      console.log('  ⏭  Skipping — needs agent-registry address first')
    } else {
      const ok = await confirm('  Instantiate task-ledger?')
      if (ok) {
        const msg = {
          admin: address,
          agent_registry: registryAddr,
          operators: [],
        }
        const res = await client.instantiate(address, codeId, msg, 'JunoClaw Task Ledger', 'auto')
        console.log(`  ✅  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
        deployed['task-ledger'].address = res.contractAddress
        deployed['task-ledger'].instantiate_tx = res.transactionHash
        saveDeployed(deployed)
      }
    }
  } else {
    console.log(`  ⏭  Already instantiated: ${deployed['task-ledger'].address}`)
  }

  // ── STEP 5: Instantiate agent-company ───────────────────────────────────

  console.log('\n━━━  Step 5: Instantiate agent-company (example)  ━━━\n')

  if (!deployed['agent-company']?.address) {
    const codeId = deployed['agent-company']?.code_id
    const escrowAddr   = deployed['escrow']?.address
    const registryAddr = deployed['agent-registry']?.address
    if (!codeId || !escrowAddr || !registryAddr) {
      console.log('  ⏭  Skipping — needs escrow + registry addresses first')
    } else {
      const taskLedgerAddr = deployed['task-ledger']?.address || null
      const ok = await confirm('  Instantiate agent-company (demo "JunoClaw Core Team")?')
      if (ok) {
        const msg = {
          name: 'JunoClaw Core Team',
          admin: address,
          governance: null,
          escrow_contract: escrowAddr,
          agent_registry: registryAddr,
          task_ledger: taskLedgerAddr,
          nois_proxy: null,
          members: [
            { addr: address, weight: 10000, role: 'human' },
          ],
          denom: 'ujunox',
          voting_period_blocks: 100,
          quorum_percent: 51,
          adaptive_threshold_blocks: 10,
          adaptive_min_blocks: 13,
          verification: {
            model: 'witness_and_wavs',
            required_attestations: 2,
            total_witnesses: 3,
            attestation_timeout_blocks: 200,
            auto_release_on_verify: true,
          },
        }
        const res = await client.instantiate(address, codeId, msg, 'JunoClaw Agent Company', 'auto')
        console.log(`  ✅  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
        deployed['agent-company'].address = res.contractAddress
        deployed['agent-company'].instantiate_tx = res.transactionHash
        saveDeployed(deployed)
      }
    }
  } else {
    console.log(`  ⏭  Already instantiated: ${deployed['agent-company'].address}`)
  }

  // ── Summary ──────────────────────────────────────────────────────────────

  console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n')
  console.log('  Deployment summary:\n')
  for (const [name, info] of Object.entries(deployed)) {
    console.log(`  ${name}`)
    if (info.code_id)  console.log(`    code_id:  ${info.code_id}`)
    if (info.address)  console.log(`    address:  ${info.address}`)
  }
  console.log(`\n  Full details: ${DEPLOYED_FILE}`)
  console.log('\n  View on Mintscan: https://testnet.mintscan.io/juno-testnet\n')

  process.exit(0)
}

main().catch((err) => {
  console.error('\n❌  Deploy failed:', err.message)
  process.exit(1)
})
