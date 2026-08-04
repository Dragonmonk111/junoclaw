// Deploy zk-verifier (pure), jclaw-credential, moultbook-v0 to Juno mainnet
// (juno-1). Pure-Wasm builds only — no BN254/MAYO/ML-DSA precompile feature
// flags — so these load on stock Juno today, independent of the v30 upgrade
// (prop #377, voting ends 2026-07-28, upgrade height 40,420,069).
//
// Mirrors the encrypted-WalletStore pattern from deploy-skill-registry.mjs.
//
// Usage:
//   cd deploy
//   $env:WALLET_ID="builder"; $env:CHAIN_ID="juno-1"; node deploy-mainnet-core.mjs

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
const IS_MAINNET = CHAIN_ID === 'juno-1'
const RPC_URL   = process.env.RPC_URL   || (IS_MAINNET ? 'https://juno-rpc.polkachu.com' : 'https://juno.rpc.t.stavr.tech')
const DENOM     = process.env.DENOM     || (IS_MAINNET ? 'ujuno' : 'ujunox')
const GAS_PRICE = process.env.GAS_PRICE || `0.075${DENOM}`

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
  console.error('Set MNEMONIC, PARLIAMENT_ROLE, or WALLET_ID. See .env.example.')
  process.exit(1)
}

const WALLET_ID = process.env.WALLET_ID
const MNEMONIC = WALLET_ID ? null : loadMnemonic()

const ARTIFACTS_DIR = process.env.ARTIFACTS_DIR
  || 'C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release'

const DEPLOYED_FILE = join(__dir, IS_MAINNET ? 'deployed-mainnet.json' : 'deployed-testnet.json')

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
  console.log(`\n  Deploy: zk-verifier (pure) + jclaw-credential + moultbook-v0 to ${CHAIN_ID}`)
  console.log(`  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}\n`)

  if (IS_MAINNET) {
    console.log('  \u26A0  MAINNET deploy \u2014 this spends real JUNO. Ctrl+C now to abort.\n')
  }

  let client, address

  if (WALLET_ID) {
    console.log(`  Wallet:   encrypted store (id: "${WALLET_ID}")`)
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
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
    const [{ address: addr }] = await wallet.getAccounts()
    address = addr
    client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
      gasPrice: GasPrice.fromString(GAS_PRICE),
    })
  }

  console.log(`  Deployer: ${address}`)

  const balance = await client.getBalance(address, DENOM)
  console.log(`  Balance:  ${(BigInt(balance.amount) / 1_000_000n).toString()} ${DENOM.replace('u', '').toUpperCase()}\n`)

  if (BigInt(balance.amount) === 0n) {
    console.error(`  Deployer ${address} has 0 ${DENOM} on ${CHAIN_ID}. Fund it before deploying.`)
    process.exit(1)
  }

  const deployed = loadDeployed()

  // ── zk-verifier (pure) ──────────────────────────────────────────────────

  if (!deployed['zk-verifier']?.codeId) {
    const wasmPath = join(ARTIFACTS_DIR, 'zk_verifier.wasm')
    if (!existsSync(wasmPath)) {
      console.error(`  zk_verifier.wasm not found at ${wasmPath}`)
      process.exit(1)
    }
    const wasm = readFileSync(wasmPath)
    console.log(`  Storing zk-verifier (${(wasm.length / 1024).toFixed(1)} KB)...`)
    const result = await client.upload(address, wasm, 'auto', 'JunoClaw ZK Verifier (pure BN254 Groth16)')
    console.log(`  codeId: ${result.codeId}  tx: ${result.transactionHash}`)
    deployed['zk-verifier'] = { codeId: result.codeId, tx: result.transactionHash }
    saveDeployed(deployed)
  } else {
    console.log(`  zk-verifier already stored (codeId ${deployed['zk-verifier'].codeId})`)
  }

  if (!deployed['zk-verifier']?.address) {
    console.log(`  Instantiating zk-verifier...`)
    const msg = { admin: address }
    const res = await client.instantiate(
      address, deployed['zk-verifier'].codeId, msg,
      'JunoClaw ZK Verifier', 'auto', { admin: address }
    )
    console.log(`  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
    deployed['zk-verifier'].address = res.contractAddress
    deployed['zk-verifier'].instantiateTx = res.transactionHash
    saveDeployed(deployed)
  } else {
    console.log(`  zk-verifier already instantiated: ${deployed['zk-verifier'].address}`)
  }

  // ── jclaw-credential ─────────────────────────────────────────────────────

  if (!deployed['jclaw-credential']?.codeId) {
    const wasmPath = join(ARTIFACTS_DIR, 'jclaw_credential.wasm')
    if (!existsSync(wasmPath)) {
      console.error(`  jclaw_credential.wasm not found at ${wasmPath}`)
      process.exit(1)
    }
    const wasm = readFileSync(wasmPath)
    console.log(`\n  Storing jclaw-credential (${(wasm.length / 1024).toFixed(1)} KB)...`)
    const result = await client.upload(address, wasm, 'auto', 'JunoClaw Credential (soulbound governance)')
    console.log(`  codeId: ${result.codeId}  tx: ${result.transactionHash}`)
    deployed['jclaw-credential'] = { codeId: result.codeId, tx: result.transactionHash }
    saveDeployed(deployed)
  } else {
    console.log(`\n  jclaw-credential already stored (codeId ${deployed['jclaw-credential'].codeId})`)
  }

  if (!deployed['jclaw-credential']?.address) {
    console.log(`  Instantiating jclaw-credential...`)
    const msg = { admin: address, genesis: address, sunset_grace_seconds: 86400 }
    const res = await client.instantiate(
      address, deployed['jclaw-credential'].codeId, msg,
      'JunoClaw Credential', 'auto', { admin: address }
    )
    console.log(`  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
    deployed['jclaw-credential'].address = res.contractAddress
    deployed['jclaw-credential'].instantiateTx = res.transactionHash
    saveDeployed(deployed)
  } else {
    console.log(`  jclaw-credential already instantiated: ${deployed['jclaw-credential'].address}`)
  }

  // ── moultbook-v0 ─────────────────────────────────────────────────────────

  if (!deployed['moultbook']?.codeId) {
    const wasmPath = join(ARTIFACTS_DIR, 'moultbook_v0.wasm')
    if (!existsSync(wasmPath)) {
      console.error(`  moultbook_v0.wasm not found at ${wasmPath}`)
      process.exit(1)
    }
    const wasm = readFileSync(wasmPath)
    console.log(`\n  Storing moultbook-v0 (${(wasm.length / 1024).toFixed(1)} KB)...`)
    const result = await client.upload(address, wasm, 'auto', 'JunoClaw Moultbook v0')
    console.log(`  codeId: ${result.codeId}  tx: ${result.transactionHash}`)
    deployed['moultbook'] = { codeId: result.codeId, tx: result.transactionHash }
    saveDeployed(deployed)
  } else {
    console.log(`\n  moultbook already stored (codeId ${deployed['moultbook'].codeId})`)
  }

  if (!deployed['moultbook']?.address) {
    console.log(`  Instantiating moultbook-v0...`)
    const msg = {
      admin: address,
      whoami_contract: null,
      max_size_bytes: 65536,
      max_refs: 8,
      max_content_type_len: 64,
      max_group_size: 32,
      zk_verifier: deployed['zk-verifier']?.address || null,
      agent_registry: null,
      membership_vk_hash: null,
      entries_per_key_per_epoch: 10,
      epoch_blocks: 14400,
    }
    const res = await client.instantiate(
      address, deployed['moultbook'].codeId, msg,
      'JunoClaw Moultbook v0', 'auto', { admin: address }
    )
    console.log(`  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
    deployed['moultbook'].address = res.contractAddress
    deployed['moultbook'].instantiateTx = res.transactionHash
    saveDeployed(deployed)
  } else {
    console.log(`  moultbook already instantiated: ${deployed['moultbook'].address}`)
  }

  // ── Summary ───────────────────────────────────────────────────────────────

  console.log('\n  --- Deployment Summary ---\n')
  for (const name of ['zk-verifier', 'jclaw-credential', 'moultbook']) {
    const info = deployed[name]
    if (info) {
      console.log(`  ${name}`)
      if (info.codeId)  console.log(`    codeId:   ${info.codeId}`)
      if (info.address) console.log(`    address:  ${info.address}`)
    }
  }
  console.log('')

  process.exit(0)
}

main().catch((err) => {
  console.error('\n  Deploy failed:', err.message)
  process.exit(1)
})
