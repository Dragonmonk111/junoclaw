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

// The manual this registry entry points at. Computed via:
//   Get-FileHash -Algorithm SHA256 mcp/SKILL.md
const SKILL_URI  = process.env.SKILL_URI
  || 'https://raw.githubusercontent.com/Dragonmonk111/junoclaw/main/mcp/SKILL.md'
const SKILL_HASH = process.env.SKILL_HASH
  || 'd76f8665409085d553fc8382221ab88902cc3df0edd48ff8527802a2e740954a'
const DAPP_NAME   = 'junoclaw-cosmos-mcp'
const DAPP_CHAIN  = CHAIN_ID // this first entry documents the registry's own testnet home

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log('\n  Deploy: skill-registry to uni-7 + self-register junoclaw-cosmos-mcp')
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

  // ── store ────────────────────────────────────────────────────────────────

  if (!deployed['skill-registry']?.codeId) {
    const wasmPath = join(ARTIFACTS_DIR, 'skill_registry.wasm')
    if (!existsSync(wasmPath)) {
      console.error(`  skill_registry.wasm not found at ${wasmPath}`)
      console.error(`  Build it first:`)
      console.error(`    cargo build --release --target wasm32-unknown-unknown --lib -p skill-registry`)
      console.error(`    wasm-opt --enable-sign-ext --signext-lowering --strip-target-features --strip-debug -Oz \\`)
      console.error(`      -o ${wasmPath} contracts/target/wasm32-unknown-unknown/release/skill_registry.wasm`)
      process.exit(1)
    }
    const wasm = readFileSync(wasmPath)
    console.log(`  Storing skill-registry (${(wasm.length / 1024).toFixed(1)} KB)...`)
    const result = await client.upload(address, wasm, 'auto', 'JunoClaw Skill Registry')
    console.log(`  codeId: ${result.codeId}  tx: ${result.transactionHash}`)
    deployed['skill-registry'] = {
      codeId: result.codeId,
      tx: result.transactionHash,
    }
    saveDeployed(deployed)
  } else {
    console.log(`  skill-registry already stored (codeId ${deployed['skill-registry'].codeId})`)
  }

  // ── instantiate ──────────────────────────────────────────────────────────

  if (!deployed['skill-registry']?.address) {
    console.log(`  Instantiating skill-registry...`)
    const msg = {
      admin: address,
      denom: 'ujuno',
      registration_fee: '0', // free on testnet; consider a small anti-spam fee on mainnet
    }
    const res = await client.instantiate(
      address, deployed['skill-registry'].codeId, msg,
      'JunoClaw Skill Registry', 'auto', { admin: address }
    )
    console.log(`  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
    deployed['skill-registry'].address = res.contractAddress
    deployed['skill-registry'].instantiateTx = res.transactionHash
    saveDeployed(deployed)
  } else {
    console.log(`  skill-registry already instantiated: ${deployed['skill-registry'].address}`)
  }

  // ── self-register junoclaw-cosmos-mcp ───────────────────────────────────

  if (!deployed['skill-registry'].registeredSelf) {
    console.log(`\n  Publishing '${DAPP_NAME}' skill entry...`)
    const res = await client.execute(
      address,
      deployed['skill-registry'].address,
      {
        publish_skill: {
          dapp_name: DAPP_NAME,
          chain_id: DAPP_CHAIN,
          skill_uri: SKILL_URI,
          skill_hash: SKILL_HASH,
        },
      },
      'auto',
      'self-register junoclaw-cosmos-mcp',
    )
    console.log(`  tx: ${res.transactionHash}`)
    deployed['skill-registry'].registeredSelf = true
    deployed['skill-registry'].selfEntryTx = res.transactionHash
    saveDeployed(deployed)
  } else {
    console.log(`\n  '${DAPP_NAME}' already registered (tx ${deployed['skill-registry'].selfEntryTx})`)
  }

  // ── Summary ───────────────────────────────────────────────────────────────

  console.log('\n  --- Deployment Summary ---\n')
  console.log(`  skill-registry`)
  console.log(`    codeId:   ${deployed['skill-registry'].codeId}`)
  console.log(`    address:  ${deployed['skill-registry'].address}`)
  console.log(`    entry:    ${DAPP_NAME} -> ${SKILL_URI}`)
  console.log('')

  process.exit(0)
}

main().catch((err) => {
  console.error('\n  Deploy failed:', err.message)
  process.exit(1)
})
