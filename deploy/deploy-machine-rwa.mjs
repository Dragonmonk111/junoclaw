/**
 * deploy-machine-rwa.mjs — Deploy machine-rwa contract to uni-7, instantiate
 * with Moultbook address, mint the first machine NFT bound to the DAO operator,
 * and query GetWorkIntegrityScore.
 *
 * Usage:
 *   CONFIRM=yes node deploy/deploy-machine-rwa.mjs
 */

import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath, pathToFileURL } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const MCP_DIST = join(__dirname, '..', 'mcp', 'dist')

function distImport(...segments) {
  return import(pathToFileURL(join(MCP_DIST, ...segments)).href)
}
function cosmImport(pkg) {
  return import(pathToFileURL(join(MCP_DIST, '..', 'node_modules', '@cosmjs', pkg, 'build', 'index.js')).href)
}

const RPC = 'https://juno.rpc.t.stavr.tech'
const MOULTBOOK_ADDR = 'juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4'
const DAO_OPERATOR_ADDR = 'juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd'
const ARTIFACTS_DIR = 'C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release'
const DEPLOYED_FILE = join(__dirname, 'deployed.json')
const CONFIRMED = process.env.CONFIRM === 'yes'

const { DirectSecp256k1HdWallet } = await cosmImport('proto-signing')
const { SigningCosmWasmClient } = await cosmImport('cosmwasm-stargate')
const { GasPrice } = await cosmImport('stargate')

const store = await distImport('wallet', 'store.js')
const ws = store.getDefaultWalletStore()

async function getSigner(walletId) {
  const mnemonic = await ws.exportMnemonicForExternalSigner(walletId)
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [acc] = await wallet.getAccounts()
  const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet, {
    gasPrice: GasPrice.fromString('0.075ujunox'),
  })
  return { client, address: acc.address }
}

const FEE = (gas, amount) => ({ amount: [{ denom: 'ujunox', amount: String(amount) }], gas: String(gas) })

function loadDeployed() {
  if (existsSync(DEPLOYED_FILE)) {
    return JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))
  }
  return {}
}

function saveDeployed(data) {
  writeFileSync(DEPLOYED_FILE, JSON.stringify(data, null, 2))
}

console.log('╔══════════════════════════════════════════════════════╗')
console.log('║  Deploy machine-rwa to uni-7 + Mint First Machine NFT ║')
console.log('╚══════════════════════════════════════════════════════╝')
console.log('')
console.log('Mode:', CONFIRMED ? 'LIVE' : 'DRY RUN')
console.log('RPC:', RPC)
console.log('Moultbook:', MOULTBOOK_ADDR)
console.log('DAO Operator:', DAO_OPERATOR_ADDR)
console.log('')

if (!CONFIRMED) {
  console.log('Dry run — nothing broadcast.')
  console.log('To execute: CONFIRM=yes node deploy/deploy-machine-rwa.mjs')
  process.exit(0)
}

const { client: builderClient, address: builderAddr } = await getSigner('builder')
console.log('Builder:', builderAddr)

const balance = await builderClient.getBalance(builderAddr, 'ujunox')
console.log('Balance:', (BigInt(balance.amount) / 1_000_000n).toString(), 'JUNOX')
console.log('')

const deployed = loadDeployed()

// ─── Step 1: Upload wasm ─────────────────────────────────────────────────────
let codeId = deployed['machine-rwa']?.code_id
if (!codeId) {
  const wasmPath = join(ARTIFACTS_DIR, 'machine_rwa.wasm')
  if (!existsSync(wasmPath)) {
    console.error(`machine_rwa.wasm not found at ${wasmPath}`)
    process.exit(1)
  }
  const wasm = readFileSync(wasmPath)
  console.log(`Step 1: Uploading machine_rwa.wasm (${(wasm.length / 1024).toFixed(1)} KB)...`)
  const result = await builderClient.upload(builderAddr, wasm, FEE(3_000_000, 225_000), 'JunoClaw machine-rwa')
  codeId = result.codeId
  console.log('  ✓ code_id:', codeId)
  console.log('  ✓ tx:', result.transactionHash)
  deployed['machine-rwa'] = {
    code_id: codeId,
    store_tx: result.transactionHash,
    wasm_file: 'machine_rwa.wasm',
  }
  saveDeployed(deployed)
} else {
  console.log(`Step 1: machine-rwa already uploaded (code_id ${codeId})`)
}

// ─── Step 2: Instantiate ────────────────────────────────────────────────────
let contractAddr = deployed['machine-rwa']?.address
if (!contractAddr) {
  console.log('\nStep 2: Instantiating machine-rwa...')
  const instantiateMsg = {
    admin: builderAddr,
    moultbook_contract: MOULTBOOK_ADDR,
  }
  const res = await builderClient.instantiate(
    builderAddr,
    codeId,
    instantiateMsg,
    'JunoClaw machine-rwa',
    FEE(500_000, 50_000),
    { admin: builderAddr },
  )
  contractAddr = res.contractAddress
  console.log('  ✓ address:', contractAddr)
  console.log('  ✓ tx:', res.transactionHash)
  deployed['machine-rwa'].address = contractAddr
  deployed['machine-rwa'].instantiate_tx = res.transactionHash
  saveDeployed(deployed)
} else {
  console.log(`Step 2: machine-rwa already instantiated: ${contractAddr}`)
}

// ─── Step 3: Verify config ───────────────────────────────────────────────────
console.log('\nStep 3: Verifying config...')
const config = await builderClient.queryContractSmart(contractAddr, { get_config: {} })
console.log('  Admin:', config.admin)
console.log('  Moultbook:', config.moultbook_contract)

// ─── Step 4: Mint first machine NFT ──────────────────────────────────────────
console.log('\nStep 4: Minting first machine NFT...')
const mintMsg = {
  mint: {
    model: 'Unitree Go2',
    serial_number: 'ROSIE-UNIT-001',
    sensor_suite: 'LiDAR+IMU+stereo+thermal',
    ipfs_metadata: 'ipfs://QmRosieUnit001Metadata',
    moultbook_author: DAO_OPERATOR_ADDR,
  },
}

const mintTx = await builderClient.execute(
  builderAddr,
  contractAddr,
  mintMsg,
  FEE(300_000, 30_000),
  'A052: Mint first machine NFT — bound to DAO operator',
)
console.log('  ✓ Mint tx:', mintTx.transactionHash)

// Extract token_id from events
let tokenId = null
const events = (mintTx.logs || []).flatMap((l) => l.events || []).concat(mintTx.events || [])
for (const ev of events) {
  if (ev.type === 'wasm') {
    const tokenAttr = ev.attributes.find((a) => a.key === 'token_id')
    if (tokenAttr?.value) {
      tokenId = tokenAttr.value
      console.log('  ✓ token_id:', tokenId)
    }
  }
}

if (!tokenId) {
  // Query list to find it
  const machines = await builderClient.queryContractSmart(contractAddr, {
    list_machines: { start_after: null, limit: 10 },
  })
  console.log('  Machines:', JSON.stringify(machines, null, 2))
  tokenId = machines.machines?.[0]?.token_id || 'machine-0'
  console.log('  token_id (from query):', tokenId)
}

// ─── Step 5: Query machine details ───────────────────────────────────────────
console.log('\nStep 5: Querying machine details...')
const machine = await builderClient.queryContractSmart(contractAddr, {
  get_machine: { token_id: tokenId },
})
console.log('  token_id:', machine.token_id)
console.log('  model:', machine.model)
console.log('  serial:', machine.serial_number)
console.log('  sensor_suite:', machine.sensor_suite)
console.log('  moultbook_author:', machine.moultbook_author)
console.log('  minter:', machine.minter)
console.log('  minted_at (block):', machine.minted_at)
console.log('  burned:', machine.burned)

// ─── Step 6: Query ownership ─────────────────────────────────────────────────
console.log('\nStep 6: Querying ownership...')
const ownership = await builderClient.queryContractSmart(contractAddr, {
  get_ownership: { token_id: tokenId },
})
console.log('  Owners:', ownership.owners.length)
for (const o of ownership.owners) {
  console.log(`    ${o.owner}: ${o.basis_points} BP (${(o.basis_points / 100).toFixed(0)}%)`)
}

// ─── Step 7: Query work integrity score ──────────────────────────────────────
console.log('\nStep 7: Querying work integrity score...')
try {
  const score = await builderClient.queryContractSmart(contractAddr, {
    get_work_integrity_score: { token_id: tokenId },
  })
  console.log('  Score:', JSON.stringify(score, null, 2))
} catch (e) {
  console.log('  Work integrity score query failed:', e.message.substring(0, 200))
  console.log('  (This is expected if moultbook does not have a credit_score query for this author)')
}

// ─── Summary ─────────────────────────────────────────────────────────────────
console.log('\n═══════════════════════════════════════════════════════')
console.log('  machine-rwa deployed to uni-7')
console.log('  code_id:', codeId)
console.log('  address:', contractAddr)
console.log('  first NFT:', tokenId)
console.log('  model:', machine.model)
console.log('  moultbook_author:', machine.moultbook_author)
console.log('  owner:', builderAddr, '(100%)')
console.log('═══════════════════════════════════════════════════════')
