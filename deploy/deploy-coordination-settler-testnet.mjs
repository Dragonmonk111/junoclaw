// Deploy coordination-settler to uni-7 testnet
//
// Usage:
//   $env:WALLET_ID="builder"; $env:CHAIN_ID="uni-7"; node deploy-coordination-settler-testnet.mjs
//
// Or with mnemonic:
//   $env:JUNO_MNEMONIC="word1 word2 ..."; node deploy-coordination-settler-testnet.mjs

import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))

const CHAIN_ID = process.env.CHAIN_ID || 'uni-7'
const RPC_URL = process.env.RPC_URL || 'https://juno.rpc.t.stavr.tech'
const DENOM = process.env.DENOM || 'ujunox'
const GAS_PRICE = process.env.GAS_PRICE || `0.075${DENOM}`
const WALLET_ID = process.env.WALLET_ID
const MNEMONIC = process.env.JUNO_MNEMONIC || null

const DEPLOYED_FILE = join(__dirname, 'deployed-testnet.json')
const WASM_PATH = process.env.WASM_PATH
  || 'C:\\Temp\\junoclaw-wasm-target\\coordination_settler.wasm'

async function main() {
  console.log('=== Deploy coordination-settler to testnet ===')
  console.log(`  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}`)
  console.log(`  Gas:      ${GAS_PRICE}`)
  console.log(`  WASM:     ${WASM_PATH}`)
  console.log()

  if (!existsSync(WASM_PATH)) {
    console.error(`ERROR: WASM not found at ${WASM_PATH}`)
    console.error('Build first: cargo build --release --target wasm32-unknown-unknown -p coordination-settler')
    console.error('Then: wasm-opt -Oz ... -o C:\\Temp\\junoclaw-wasm-target\\coordination_settler.wasm')
    process.exit(1)
  }

  let client, address

  if (WALLET_ID) {
    console.log(`  Wallet:   encrypted store (id: "${WALLET_ID}")`)
    const { WalletStore } = await import('../mcp/dist/wallet/store.js')
    const store = WalletStore.defaultStore()
    const chainConfig = {
      chainId: CHAIN_ID,
      chainName: 'Juno Testnet',
      rpcEndpoint: RPC_URL,
      restEndpoint: 'https://juno.api.t.stavr.tech',
      denom: DENOM,
      bech32Prefix: 'juno',
      gasPrice: GAS_PRICE,
      slip44: 118,
      explorerTx: 'https://testnet.mintscan.io/juno-testnet/tx',
      isTestnet: true,
    }
    const ctx = await store.signFor(WALLET_ID, chainConfig)
    client = ctx.client
    address = ctx.address
  } else if (MNEMONIC) {
    console.log('  Wallet:   mnemonic from env')
    const { DirectSecp256k1HdWallet } = await import('@cosmjs/proto-signing')
    const { SigningCosmWasmClient } = await import('@cosmjs/cosmwasm-stargate')
    const { GasPrice } = await import('@cosmjs/stargate')
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
    const [{ addr }] = await wallet.getAccounts()
    address = addr
    client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
      gasPrice: GasPrice.fromString(GAS_PRICE),
    })
  } else {
    console.error('Set WALLET_ID or JUNO_MNEMONIC env var')
    process.exit(1)
  }

  console.log(`  Address:  ${address}`)
  const balance = await client.getBalance(address, DENOM)
  console.log(`  Balance:  ${balance.amount} ${balance.denom}`)
  console.log()

  if (BigInt(balance.amount) < 5000000n) {
    console.error(`ERROR: Insufficient balance. Need at least 5 ${DENOM.replace('u', '')} for deployment.`)
    console.error('Get testnet tokens from the Juno testnet faucet.')
    process.exit(1)
  }

  let deployed = {}
  if (existsSync(DEPLOYED_FILE)) {
    deployed = JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))
  }

  // --- 1. Store code ---
  if (!deployed['coordination-settler']?.codeId) {
    const wasm = readFileSync(WASM_PATH)
    console.log(`Storing coordination-settler (${wasm.length} bytes)...`)
    const result = await client.upload(address, wasm, 'auto', 'coordination-settler')
    console.log(`  codeId: ${result.codeId}`)
    console.log(`  tx:     ${result.transactionHash}`)
    deployed['coordination-settler'] = { codeId: result.codeId, tx: result.transactionHash }
    writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2))
  } else {
    console.log(`coordination-settler already stored: codeId ${deployed['coordination-settler'].codeId}`)
  }

  // --- 2. Instantiate ---
  if (!deployed['coordination-settler']?.address) {
    // Initial validator set: 4 dummy BLS keys for testnet
    // In real deployment, these would be real Commonware validator public keys
    const validators = [
      Array.from({ length: 48 }, () => 0x11),
      Array.from({ length: 48 }, () => 0x22),
      Array.from({ length: 48 }, () => 0x33),
      Array.from({ length: 48 }, () => 0x44),
    ]

    const instantiateMsg = {
      admin: address,
      validators: validators.map(v => Buffer.from(v).toString('base64')),
      threshold: 3,
    }

    console.log('Instantiating coordination-settler...')
    const res = await client.instantiate(
      address,
      deployed['coordination-settler'].codeId,
      instantiateMsg,
      'coordination-settler',
      'auto',
      { admin: address }
    )
    console.log(`  address: ${res.contractAddress}`)
    console.log(`  tx:      ${res.transactionHash}`)
    deployed['coordination-settler'].address = res.contractAddress
    deployed['coordination-settler'].instantiateTx = res.transactionHash
    writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2))
  } else {
    console.log(`coordination-settler already instantiated: ${deployed['coordination-settler'].address}`)
  }

  // --- 3. Verify ---
  console.log()
  console.log('Verifying deployment...')
  const config = await client.queryContractSmart(
    deployed['coordination-settler'].address,
    { config: {} }
  )
  console.log(`  admin:           ${config.admin}`)
  console.log(`  threshold:       ${config.threshold}`)
  console.log(`  validator_count: ${config.validator_count}`)
  console.log(`  relayer_count:   ${config.relayer_count}`)
  console.log(`  latest_height:   ${config.latest_height}`)

  console.log()
  console.log('=== Deployment complete ===')
  console.log(`Contract: ${deployed['coordination-settler'].address}`)
  console.log(`Code ID:  ${deployed['coordination-settler'].codeId}`)
}

main().catch((err) => {
  console.error('FATAL:', err)
  process.exit(1)
})
